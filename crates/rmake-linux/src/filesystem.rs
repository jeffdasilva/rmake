// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Byte-preserving filesystem effects, with GNU-compatible stdio boundaries.

use std::ffi::OsStr;
use std::fs;
use std::io;
use std::os::unix::ffi::OsStrExt;

use rmake_core::diagnostic::FilesystemEffect;
use rmake_core::variables::MAX_OUTPUT;

use crate::stdio::Stream;

/// Execute exactly the requested effect, without consulting or caching make
/// variables. Reads and path observations must run again on every evaluation.
pub fn execute(effect: FilesystemEffect<'_>) -> Result<Vec<u8>, Vec<u8>> {
    match effect {
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
        FilesystemEffect::ReadFile(path) => read(path),
        FilesystemEffect::WriteFile { path, text, append } => {
            let mut stream = Stream::open(path, if append { c"a" } else { c"w" })
                .map_err(|error| diagnostic(b"open", path, error))?;
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

fn retry_interrupted<T>(mut operation: impl FnMut() -> io::Result<T>) -> io::Result<T> {
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
