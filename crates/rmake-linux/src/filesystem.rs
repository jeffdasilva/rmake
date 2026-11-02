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

/// Open now and retain the source descriptor through interpretation. The
/// interpreter requests each C-string fragment at its actual read boundary.
pub fn open_source(path: &[u8]) -> io::Result<rmake_core::reader::Input> {
    let mut stream = Stream::open(path, c"r")?;
    stream.prevent_inheritance()?;
    Ok(rmake_core::reader::Input::Stream(Box::new(SourceFile {
        stream,
        scratch: Vec::new(),
        scratch_clean: false,
        error: None,
    })))
}
struct SourceFile {
    stream: Stream,
    scratch: Vec<u8>,
    scratch_clean: bool,
    error: Option<io::Error>,
}
impl rmake_core::reader::Source for SourceFile {
    fn fragment(&mut self, count: usize) -> Option<Vec<u8>> {
        let (value, error) =
            self.stream
                .source_fragment(count, &mut self.scratch, &mut self.scratch_clean);
        if error.is_some() {
            self.error = error;
        }
        value
    }
    fn finish(&mut self) {
        self.stream.finish_source();
    }
    fn retained_bytes(&self) -> usize {
        self.scratch.capacity()
    }
    fn error(&self) -> Option<Vec<u8>> {
        self.error.as_ref().map(|e| {
            let message = e.to_string();
            let suffix = e
                .raw_os_error()
                .map(|n| format!(" (os error {n})"))
                .unwrap_or_default();
            message
                .strip_suffix(&suffix)
                .unwrap_or(&message)
                .as_bytes()
                .to_vec()
        })
    }
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

#[cfg(test)]
mod source_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    fn path() -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "rmake-live-source-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ))
    }
    #[test]
    fn source_observes_a_tail_appended_after_its_first_line() {
        use std::io::Write;
        let path = path();
        fs::write(&path, b"first\nsecond\n").unwrap();
        let rmake_core::reader::Input::Stream(mut source) =
            open_source(path.as_os_str().as_bytes()).unwrap()
        else {
            panic!("stream expected")
        };
        assert_eq!(source.fragment(200), Some(b"first\n".to_vec()));
        fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(b"tail\n")
            .unwrap();
        assert_eq!(source.fragment(200), Some(b"second\n".to_vec()));
        assert_eq!(source.fragment(200), Some(b"tail\n".to_vec()));
        assert_eq!(source.fragment(200), None);
        assert_eq!(source.error(), None);
        drop(source);
        fs::remove_file(path).unwrap();
    }
    #[test]
    fn retained_large_buffer_keeps_short_and_hidden_nul_boundaries() {
        let path = path();
        let long = vec![b'x'; 100_000];
        let bytes = [b"small\n".as_slice(), &long, b"\nvisible\0hidden\nlast\n"].concat();
        fs::write(&path, bytes).unwrap();
        let rmake_core::reader::Input::Stream(mut source) =
            open_source(path.as_os_str().as_bytes()).unwrap()
        else {
            panic!("stream expected")
        };
        assert_eq!(source.fragment(200), Some(b"small\n".to_vec()));
        assert_eq!(
            source.fragment(200_000),
            Some([long, b"\n".to_vec()].concat())
        );
        assert_eq!(source.fragment(200_000), Some(b"visible".to_vec()));
        assert_eq!(source.fragment(200_000), Some(b"last\n".to_vec()));
        assert_eq!(source.fragment(200_000), None);
        assert_eq!(source.error(), None);
        drop(source);
        fs::remove_file(path).unwrap();
    }
    #[test]
    fn source_descriptor_is_not_inherited_by_a_child() {
        let path = path();
        fs::write(&path, b"source\n").unwrap();
        let input = open_source(path.as_os_str().as_bytes()).unwrap();
        let result = std::process::Command::new("/bin/sh")
            .args([
                "-c",
                "for f in /proc/$$/fd/*; do readlink \"$f\" || :; done",
            ])
            .output()
            .unwrap();
        assert!(result.status.success());
        assert!(
            !result
                .stdout
                .windows(path.as_os_str().as_bytes().len())
                .any(|p| p == path.as_os_str().as_bytes())
        );
        drop(input);
        fs::remove_file(path).unwrap();
    }
}
