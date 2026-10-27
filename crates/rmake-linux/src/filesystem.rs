// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Byte-preserving filesystem effects, with GNU-compatible stdio boundaries.

use std::ffi::OsStr;
use std::fs;
use std::io;
use std::os::unix::ffi::OsStrExt;

use rmake_core::diagnostic::{FilesystemEffect, FilesystemError};
use rmake_core::variables::MAX_OUTPUT;

use crate::stdio::Stream;
use crate::{posix, wildcard::Wildcard};

#[derive(Default)]
pub struct Runtime {
    wildcard: Wildcard,
}

impl Runtime {
    /// The same boundary expires both directory data and failed rule searches.
    pub fn generation(&self) -> u64 {
        self.wildcard.generation()
    }

    /// Rule searches share directory membership with wildcard queries. This
    /// does not perform a fresh stat or interpret archive-member syntax.
    pub fn file_exists(&mut self, path: &[u8]) -> Result<bool, FilesystemError> {
        self.wildcard.file_exists(path)
    }

    pub fn execute(&mut self, effect: FilesystemEffect<'_>) -> Result<Vec<u8>, FilesystemError> {
        execute_with(effect, &mut self.wildcard)
    }

    /// Completed make children invalidate directory observations, including
    /// failed commands. Reads and skipped commands retain the observations.
    pub fn child_completed(&mut self) {
        self.wildcard.invalidate();
    }
}

/// One-shot adapter for callers without an invocation runtime. The engine owns
/// a Runtime across evaluations; reads and realpath observations remain fresh.
pub fn execute(effect: FilesystemEffect<'_>) -> Result<Vec<u8>, Vec<u8>> {
    Runtime::default()
        .execute(effect)
        .map_err(FilesystemError::into_message)
}

fn execute_with(
    effect: FilesystemEffect<'_>,
    wildcard: &mut Wildcard,
) -> Result<Vec<u8>, FilesystemError> {
    match effect {
        FilesystemEffect::Wildcard(pattern) => wildcard.expand(pattern),
        FilesystemEffect::ArchiveGlob(pattern) => wildcard.archive_rule(pattern),
        FilesystemEffect::RuleGlob(pattern) => wildcard.rule(pattern),
        FilesystemEffect::Tilde(name) => {
            let slash = name.iter().position(|&b| b == b'/').unwrap_or(name.len());
            let home = if slash == 1 {
                std::env::var_os("HOME")
                    .filter(|home| !home.is_empty())
                    .map(|home| home.as_bytes().to_vec())
                    .or_else(|| posix::home(None))
            } else {
                posix::home(Some(&name[1..slash]))
            };
            Ok(home.map_or_else(Vec::new, |mut home| {
                home.extend_from_slice(&name[slash..]);
                home
            }))
        }
        FilesystemEffect::Realpath(path) => {
            // GNU's Linux profile uses PATH_MAX-sized input/output buffers and
            // verifies the resolved path still exists. Failures omit this word.
            if path.len() >= 4096 {
                return Ok(Vec::new());
            }
            let Ok(path) = retry_interrupted(|| fs::canonicalize(OsStr::from_bytes(path))) else {
                return Ok(Vec::new());
            };
            if path.as_os_str().as_bytes().len() >= 4096
                || retry_interrupted(|| fs::metadata(&path)).is_err()
            {
                return Ok(Vec::new());
            }
            Ok(path.into_os_string().as_bytes().to_vec())
        }
        FilesystemEffect::ReadFile(path) => read(path).map_err(Into::into),
        FilesystemEffect::WriteFile { path, text, append } => {
            let mut stream = Stream::open(path, if append { c"a" } else { c"w" })
                .map_err(|error| diagnostic(b"open", path, error))?;
            wildcard.invalidate();
            if let Some(text) = text {
                stream
                    .write_text(text, !text.ends_with(b"\n"))
                    .map_err(|error| diagnostic(b"write", path, error))?;
            }
            stream
                .close()
                .map_err(|error| diagnostic(b"close", path, error))?;
            Ok(Vec::new())
        }
    }
}

pub(crate) fn retry_interrupted<T>(mut operation: impl FnMut() -> io::Result<T>) -> io::Result<T> {
    loop {
        match operation() {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            result => return result,
        }
    }
}

fn read(path: &[u8]) -> Result<Vec<u8>, Vec<u8>> {
    let mut stream = match Stream::open(path, c"r") {
        Ok(stream) => stream,
        Err(error) if error.raw_os_error() == Some(2) => return Ok(Vec::new()),
        Err(error) => return Err(diagnostic(b"open", path, error)),
    };
    let mut output = Vec::new();
    let mut buffer = [0; 1024];
    loop {
        let count = stream
            .read(&mut buffer)
            .map_err(|error| diagnostic(b"read", path, error))?;
        if count == 0 {
            break;
        }
        // Allow the removable CR/LF before applying the interpreter's existing
        // output bound. This also bounds memory for streams without an EOF.
        if output.len().saturating_add(count) > MAX_OUTPUT + 2 {
            return Err(b"variable expansion resource limit exceeded".to_vec());
        }
        output.extend_from_slice(&buffer[..count]);
    }
    stream
        .close()
        .map_err(|error| diagnostic(b"close", path, error))?;
    if output.ends_with(b"\n") {
        output.pop();
        if output.ends_with(b"\r") {
            output.pop();
        }
    }
    if output.len() > MAX_OUTPUT {
        return Err(b"variable expansion resource limit exceeded".to_vec());
    }
    Ok(output)
}

fn diagnostic(stage: &[u8], path: &[u8], error: io::Error) -> Vec<u8> {
    let message = error.to_string();
    let suffix = error
        .raw_os_error()
        .map(|code| format!(" (os error {code})"))
        .unwrap_or_default();
    let message = message.strip_suffix(&suffix).unwrap_or(&message);
    [stage, b": ", path, b": ", message.as_bytes()].concat()
}

/// Whole-source reads retain the interpreter's existing source representation,
/// while honoring cancellation during fopen/fread instead of retrying forever.
pub fn read_source(path: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut stream = Stream::open(path, c"r")?;
    let mut bytes = Vec::new();
    let mut block = [0; 16384];
    loop {
        let count = stream.read(&mut block)?;
        if count == 0 {
            break;
        }
        bytes.extend_from_slice(&block[..count]);
    }
    stream.close()?;
    Ok(bytes)
}
