// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Linux include search policy. Only the directory list is retained; each read
//! opens the file afresh, including reads after eval, recipes and restarts.

use rmake_core::diagnostic::{
    ExpansionContext, FilesystemEffect, IncludeError, IncludeFailure, IncludedFile,
};
use rmake_core::variables::{ExpandError, VariableStore};
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::PathBuf;

/// Resolve a preload's tilde at read time while retaining dot prefixes.
pub fn expand_preload_name(
    name: &[u8],
    variables: &mut VariableStore,
    mut context: ExpansionContext<'_>,
) -> Result<Vec<u8>, ExpandError> {
    expand_tilde_with(name, || {
        let warnings = context.replace_undefined_warnings(false);
        let result = variables.expand_mut_with(b"$(HOME)", context.reborrow());
        context.replace_undefined_warnings(warnings);
        result
    })
}

/// Observe the bindings at this argument's position, before later command-line
/// assignments. Deduplication follows expansion, and precedes path selection.
pub fn prepare_argument_with(
    name: &[u8],
    home: impl FnOnce() -> Result<Vec<u8>, ExpandError>,
) -> Result<PathBuf, ExpandError> {
    let mut expanded = expand_tilde_with(name, home)?;
    let normalized = rmake_core::filenames::without_dot_prefix(&expanded);
    if let Some(prefix) = expanded.len().checked_sub(normalized.len())
        && std::ptr::eq(normalized.as_ptr(), expanded[prefix..].as_ptr())
    {
        expanded.drain(..prefix);
    } else {
        // Only the exhausted path uses the helper's static fallback rather
        // than a suffix of the original buffer.
        debug_assert_eq!(normalized, b"./");
        expanded.clear();
        expanded.extend_from_slice(b"./");
    }
    Ok(OsString::from_vec(expanded).into())
}

/// Resolve tilde bytes without argument-only dot-prefix normalization. The
/// caller supplies HOME at the current evaluation phase and warning policy.
pub fn expand_tilde_with(
    name: &[u8],
    home: impl FnOnce() -> Result<Vec<u8>, ExpandError>,
) -> Result<Vec<u8>, ExpandError> {
    let mut expanded = Vec::new();
    if name.starts_with(b"~") {
        if name.len() == 1 || name[1] == b'/' {
            let home = home()?;
            if !home.is_empty() {
                expanded = [home.as_slice(), &name[1..]].concat();
            }
        }
        if expanded.is_empty() {
            expanded =
                rmake_linux::filesystem::execute(FilesystemEffect::Tilde(name)).unwrap_or_default();
        }
    }
    Ok(if expanded.is_empty() {
        name.to_vec()
    } else {
        expanded
    })
}

#[derive(Default)]
pub struct IncludeSearch {
    directories: Vec<Vec<u8>>,
}

impl IncludeSearch {
    pub fn new(arguments: &[PathBuf]) -> Self {
        let mut builder = IncludeSearchBuilder::default();
        for argument in arguments {
            builder.push(argument.as_os_str().as_bytes());
        }
        builder.finish()
    }

    fn add_directory(&mut self, name: &[u8]) {
        let metadata = loop {
            // A fatal signal must not begin another filesystem observation.
            if rmake_linux::signals::pending().is_some() {
                return;
            }
            match fs::metadata(OsStr::from_bytes(name)) {
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                result => break result,
            }
        };
        if metadata.is_ok_and(|metadata| metadata.is_dir()) {
            let mut end = name.len();
            while end > 1 && name[end - 1] == b'/' {
                end -= 1;
            }
            self.directories.push(name[..end].to_vec());
        }
    }

    pub fn directories(&self) -> impl Iterator<Item = &[u8]> {
        self.directories.iter().map(Vec::as_slice)
    }

    pub fn read(&self, name: &[u8]) -> Result<IncludedFile, IncludeFailure> {
        let original = match Self::read_path(name) {
            Ok(file) => return Ok(file),
            Err(error) if error.kind() == io::ErrorKind::NotFound && !name.starts_with(b"/") => {
                error
            }
            Err(error) => return Err(open_failure(None, error)),
        };
        // Joining bytes preserves ./, repeated slashes, escapes and non-UTF-8.
        // A non-ENOENT error stops search; a later directory must not mask it.
        for directory in &self.directories {
            let path = [directory.as_slice(), b"/", name].concat();
            match Self::read_path(&path) {
                Ok(file) => return Ok(file),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(open_failure(Some(path), error)),
            }
        }
        Err(open_failure(None, original))
    }

    fn read_path(name: &[u8]) -> io::Result<IncludedFile> {
        rmake_linux::filesystem::open_source(name).map(|input| IncludedFile {
            name: name.to_vec(),
            input,
        })
    }
}

pub(crate) fn source_open_exhausted(error: &io::Error) -> bool {
    matches!(error.raw_os_error(), Some(12 | 23 | 24)) // ENOMEM, ENFILE, EMFILE
}

fn open_failure(resolved_name: Option<Vec<u8>>, error: io::Error) -> IncludeFailure {
    if resolved_name.is_none() && source_open_exhausted(&error) {
        return IncludeFailure::Fatal(rmake_linux::filesystem::source_error_detail(&error));
    }
    let detail = match error.raw_os_error() {
        Some(code) => IncludeError::Errno(code),
        None => IncludeError::Bytes(rmake_linux::filesystem::source_error_detail(&error)),
    };
    IncludeFailure::Deferred {
        resolved_name,
        detail,
    }
}

/// Incremental search owns only selected directories, independently of argv.
pub struct IncludeSearchBuilder {
    search: IncludeSearch,
    defaults: bool,
}

impl Default for IncludeSearchBuilder {
    fn default() -> Self {
        Self {
            search: IncludeSearch::default(),
            defaults: true,
        }
    }
}

impl IncludeSearchBuilder {
    pub fn push(&mut self, name: &[u8]) {
        if name == b"-" {
            self.defaults = false;
            self.search.directories.clear();
        } else {
            self.push_resolved(name);
        }
    }

    /// Expanded names are directories even when HOME produces a literal '-'.
    pub fn push_resolved(&mut self, name: &[u8]) {
        self.search.add_directory(name);
    }

    pub fn finish(mut self) -> IncludeSearch {
        if self.defaults {
            for name in [
                option_env!("RMAKE_INCLUDEDIR").unwrap_or("/usr/local/include"),
                "/usr/gnu/include",
                "/usr/local/include",
                "/usr/include",
            ] {
                self.search.add_directory(name.as_bytes());
            }
        }
        self.search
    }
}

#[cfg(test)]
mod spelling_tests {
    use super::*;

    #[test]
    fn argument_and_search_expansion_keep_their_own_spelling_boundaries() {
        let raw = b"~/inc///";
        let selected = expand_tilde_with(raw, || Ok(b"./hdir\xff".to_vec())).unwrap();
        assert_eq!(selected, b"./hdir\xff/inc///");
        let argument = prepare_argument_with(raw, || Ok(b"./hdir\xff".to_vec())).unwrap();
        assert_eq!(argument.as_os_str().as_bytes(), b"hdir\xff/inc///");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_initial_source_open_exhaustion_is_fatal() {
        for errno in [12, 23, 24] {
            assert!(matches!(
                open_failure(None, io::Error::from_raw_os_error(errno)),
                IncludeFailure::Fatal(detail) if !detail.is_empty()
            ));
            assert!(matches!(
                open_failure(Some(b"search/part.mk".to_vec()), io::Error::from_raw_os_error(errno)),
                IncludeFailure::Deferred {
                    resolved_name: Some(path),
                    detail: IncludeError::Errno(code),
                } if path.as_slice() == b"search/part.mk" && code == errno
            ));
        }
        assert!(matches!(
            open_failure(None, io::Error::from_raw_os_error(2)),
            IncludeFailure::Deferred {
                resolved_name: None,
                detail: IncludeError::Errno(2)
            }
        ));
    }
}
