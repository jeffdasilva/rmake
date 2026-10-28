// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Linux include search policy. Only the directory list is retained; each read
//! opens the file afresh, including reads after eval, recipes and restarts.

use rmake_core::diagnostic::{ExpansionContext, FilesystemEffect, IncludedFile};
use rmake_core::variables::{ExpandError, VariableStore};
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::PathBuf;

/// Observe the bindings at this argument's position, before later command-line
/// assignments. Deduplication follows expansion, and precedes path selection.
pub fn prepare_argument(
    name: &[u8],
    variables: &mut VariableStore,
    context: ExpansionContext<'_>,
) -> Result<PathBuf, ExpandError> {
    prepare_argument_with(name, || variables.expand_mut_with(b"$(HOME)", context))
}

pub fn prepare_argument_with(
    name: &[u8],
    home: impl FnOnce() -> Result<Vec<u8>, ExpandError>,
) -> Result<PathBuf, ExpandError> {
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
    let path = if expanded.is_empty() { name } else { &expanded };
    Ok(OsString::from_vec(rmake_core::filenames::without_dot_prefix(path).to_vec()).into())
}

#[derive(Default)]
pub struct IncludeSearch {
    directories: Vec<Vec<u8>>,
}

impl IncludeSearch {
    pub fn new(arguments: &[PathBuf]) -> Self {
        let mut result = Self::default();
        let mut defaults = true;
        for argument in arguments {
            let name = argument.as_os_str().as_bytes();
            if name == b"-" {
                defaults = false;
                result.directories.clear();
            } else {
                result.add_directory(name);
            }
        }
        if defaults {
            for name in [
                option_env!("RMAKE_INCLUDEDIR").unwrap_or("/usr/local/include"),
                "/usr/gnu/include",
                "/usr/local/include",
                "/usr/include",
            ] {
                result.add_directory(name.as_bytes());
            }
        }
        result
    }

    fn add_directory(&mut self, name: &[u8]) {
        if fs::metadata(OsStr::from_bytes(name)).is_ok_and(|metadata| metadata.is_dir()) {
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

    pub fn read(&self, name: &[u8]) -> Result<IncludedFile, Vec<u8>> {
        match Self::read_path(name) {
            Ok(file) => return Ok(file),
            Err(error) if error.kind() == io::ErrorKind::NotFound && !name.starts_with(b"/") => {}
            Err(error) => return Err(crate::engine::os_error(&error).into_bytes()),
        }
        // Joining bytes preserves ./, repeated slashes, escapes and non-UTF-8.
        // A non-ENOENT error stops search; a later directory must not mask it.
        for directory in &self.directories {
            let path = [directory.as_slice(), b"/", name].concat();
            match Self::read_path(&path) {
                Ok(file) => return Ok(file),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(crate::engine::os_error(&error).into_bytes()),
            }
        }
        Err(b"No such file or directory".to_vec())
    }

    fn read_path(name: &[u8]) -> io::Result<IncludedFile> {
        rmake_linux::filesystem::read_source(name).map(|bytes| IncludedFile {
            name: name.to_vec(),
            bytes,
        })
    }
}
