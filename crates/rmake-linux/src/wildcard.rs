// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Invocation-scoped directory observations and byte-preserving filename globbing.

use std::borrow::Borrow;
use std::collections::HashMap;
use std::ffi::{CString, OsStr};
use std::fs;
use std::hash::{Hash, Hasher};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;

use crate::filesystem::retry_interrupted;
use crate::{directory, posix};
use rmake_core::diagnostic::FilesystemError;
use rmake_core::variables::MAX_OUTPUT;

const LIMIT: &[u8] = b"variable expansion resource limit exceeded";

#[derive(Clone)]
struct Entry {
    name: CString,
    directory: Option<bool>,
}

// Borrow raw filename bytes without another string allocation or hashing the
// C terminator. The stored CString also serves fnmatch directly.
#[derive(Eq, PartialEq)]
struct Name(CString);

impl Borrow<[u8]> for Name {
    fn borrow(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

impl Hash for Name {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.as_bytes().hash(state);
    }
}

#[derive(Default)]
struct Contents {
    generation: Option<u64>,
    opened: bool,
    stream: Option<directory::Stream>,
    pending: HashMap<Name, Option<bool>>,
    sorted: Option<Vec<Entry>>,
    bytes: usize,
}

#[derive(Clone, Copy)]
struct PathEntry {
    contents: Option<usize>,
    generation: u64,
}

#[derive(Default)]
pub struct Wildcard {
    // Stable indices share contents across inode aliases without copying names
    // or sharing a mutable libc stream between Rust owners.
    directories: HashMap<(u64, u64), usize>,
    paths: HashMap<Vec<u8>, PathEntry>,
    contents: Vec<Contents>,
    open_streams: usize,
    bytes: usize,
    generation: u64,
}

pub fn magic(pattern: &[u8]) -> bool {
    let mut escaped = false;
    let mut bracket = false;
    for &byte in pattern {
        if escaped {
            escaped = false;
            continue;
        }
        match byte {
            b'\\' => escaped = true,
            b'*' | b'?' => return true,
            b'[' => bracket = true,
            b']' if bracket => return true,
            _ => (),
        }
    }
    false
}

fn scanned(text: &[u8], escapes: bool) -> bool {
    let mut quoted = false;
    for &byte in text {
        if quoted {
            quoted = false;
            continue;
        }
        if byte == b'\\' {
            if escapes {
                return true;
            }
            quoted = true;
        } else if matches!(byte, b'*' | b'?' | b'[') {
            return true;
        }
    }
    false
}

fn join(directory: &[u8], name: &[u8]) -> Vec<u8> {
    let mut path = directory.to_vec();
    if directory != b"/" {
        path.push(b'/');
    }
    path.extend_from_slice(name);
    path
}

fn unquote(pattern: &[u8]) -> Vec<u8> {
    let mut output = Vec::new();
    let mut pos = 0;
    while pos < pattern.len() {
        if pattern[pos] == b'\\' {
            pos += 1;
            if pos == pattern.len() {
                break;
            }
        }
        output.push(pattern[pos]);
        pos += 1;
    }
    output
}

fn exists(path: &[u8], directory: bool) -> bool {
    if directory {
        fs::metadata(OsStr::from_bytes(path)).is_ok_and(|m| m.is_dir())
    } else {
        match fs::symlink_metadata(OsStr::from_bytes(path)) {
            Ok(_) => true,
            Err(error) => error.raw_os_error() == Some(75),
        }
    }
}

fn literal(pattern: &[u8]) -> Option<Result<Vec<u8>, Vec<u8>>> {
    if !pattern.is_empty() && !pattern.ends_with(b"/") && !scanned(pattern, true) {
        let name = if pattern.starts_with(b"//") && !pattern[2..].contains(&b'/') {
            &pattern[1..]
        } else {
            pattern
        };
        let local;
        let lookup = if name.contains(&b'/') {
            name
        } else {
            local = join(b".", name);
            &local
        };
        if !exists(lookup, false) {
            return Some(Ok(Vec::new()));
        }
        if name.len().saturating_add(32) > MAX_OUTPUT {
            return Some(Err(LIMIT.to_vec()));
        }
        return Some(Ok(name.to_vec()));
    }
    None
}

impl Wildcard {
    pub fn rule(&mut self, pattern: &[u8]) -> Result<Vec<u8>, FilesystemError> {
        if rmake_core::filenames::archive_parts(pattern).is_some() {
            return self.archive_rule(pattern);
        }
        let mut names = self.query(pattern, false, false, 0)?;
        if names.is_empty() {
            names.push(pattern.to_vec());
        }
        let mut output = Vec::new();
        for name in names {
            if output.len().saturating_add(name.len()).saturating_add(1) > MAX_OUTPUT {
                return Err(LIMIT.to_vec().into());
            }
            output.extend_from_slice(&name);
            output.push(0);
        }
        Ok(output)
    }

    /// Rule declarations keep complete archive-member names, unlike GNU's
    /// non-interned wildcard function. NUL delimiters preserve filename bytes.
    pub fn archive_rule(&mut self, pattern: &[u8]) -> Result<Vec<u8>, FilesystemError> {
        let Some((path, member)) = rmake_core::filenames::archive_parts(pattern) else {
            return Ok(pattern.to_vec());
        };
        let mut paths = if path.iter().any(|b| matches!(b, b'*' | b'?' | b'[')) {
            self.query(path, false, false, 0)?
        } else {
            vec![path.to_vec()]
        };
        if paths.is_empty() {
            paths.push(path.to_vec());
        }
        let mut output = Vec::new();
        for path in paths {
            let mut names = Vec::new();
            let mut bytes = 0usize;
            if magic(member)
                && let Ok(mut file) = posix::open_archive(&path)
            {
                crate::archive::scan(&mut file, &path, |entry| {
                    if posix::matches(member, &entry.name) {
                        bytes = bytes.saturating_add(path.len() + entry.name.len() + 3);
                        if bytes > MAX_OUTPUT {
                            return Err(LIMIT.to_vec().into());
                        }
                        names.push(entry.name);
                    }
                    Ok(None::<()>)
                })?;
            }
            if names.is_empty() {
                names.push(member.to_vec());
            } else {
                names.sort_unstable();
            }
            for member in names {
                if output
                    .len()
                    .saturating_add(path.len())
                    .saturating_add(member.len())
                    .saturating_add(3)
                    > MAX_OUTPUT
                {
                    return Err(LIMIT.to_vec().into());
                }
                output.extend_from_slice(&path);
                output.push(b'(');
                output.extend_from_slice(&member);
                output.extend_from_slice(b")\0");
            }
        }
        Ok(output)
    }

    pub fn invalidate(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        // GNU refreshes a stale directory on its next access, retaining other
        // open streams in the meantime. Only counter wrap needs a full reset.
        if self.generation == 0 {
            self.directories.clear();
            self.paths.clear();
            self.contents.clear();
            self.open_streams = 0;
            self.bytes = 0;
        }
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    fn charge(&mut self, bytes: usize) -> Result<(), FilesystemError> {
        self.bytes = self.bytes.saturating_add(bytes);
        if self.bytes > MAX_OUTPUT * 8 {
            Err(LIMIT.to_vec().into())
        } else {
            Ok(())
        }
    }

    fn close_stream(&mut self, index: usize) {
        if self.contents[index].stream.take().is_some() {
            self.open_streams -= 1;
        }
    }

    fn clear_contents(&mut self, index: usize) {
        self.close_stream(index);
        self.bytes = self.bytes.saturating_sub(self.contents[index].bytes);
        self.contents[index] = Contents::default();
    }

    fn directory(&mut self, path: &[u8]) -> Result<Option<usize>, FilesystemError> {
        let key_path = if path.is_empty() {
            b".".as_slice()
        } else {
            path
        };
        if let Some(previous) = self.paths.get(key_path).copied() {
            if let Some(index) = previous.contents {
                if self.contents[index].generation == Some(self.generation) {
                    return Ok(Some(index));
                }
                self.clear_contents(index);
            } else if previous.generation == self.generation {
                return Ok(None);
            }
        } else {
            self.charge(key_path.len() + 64)?;
        }
        let metadata = retry_interrupted(|| fs::metadata(OsStr::from_bytes(key_path)));
        let Ok(metadata) = metadata else {
            self.paths.insert(
                key_path.to_vec(),
                PathEntry {
                    contents: None,
                    generation: self.generation,
                },
            );
            return Ok(None);
        };
        let key = (metadata.dev(), metadata.ino());
        let index = if let Some(index) = self.directories.get(&key) {
            *index
        } else {
            self.charge(128)?;
            let index = self.contents.len();
            self.contents.push(Contents::default());
            self.directories.insert(key, index);
            index
        };
        self.paths.insert(
            key_path.to_vec(),
            PathEntry {
                contents: Some(index),
                generation: self.generation,
            },
        );
        if self.contents[index].generation != Some(self.generation) {
            self.clear_contents(index);
            let stream = directory::Stream::open(key_path).ok();
            let opened = stream.is_some();
            self.contents[index].generation = Some(self.generation);
            self.contents[index].opened = opened;
            self.contents[index].stream = stream;
            if opened {
                self.open_streams += 1;
                // GNU drains the newly opened tenth stream, keeping at most
                // nine partially read directories between queries.
                if self.open_streams == 10
                    && let Err(error) = self.read_directory(index, key_path, None)
                {
                    // Budget errors can occur before EOF. Release this newly
                    // opened stream just as for read failures, even on abort.
                    self.close_stream(index);
                    return Err(error);
                }
            }
        }
        Ok(Some(index))
    }

    fn next_entry(
        &mut self,
        index: usize,
        path: &[u8],
    ) -> Result<Option<directory::Entry>, FilesystemError> {
        let Some(stream) = &mut self.contents[index].stream else {
            return Ok(None);
        };
        match stream.next_entry() {
            Ok(Some(entry)) => Ok(Some(entry)),
            Ok(None) => {
                self.close_stream(index);
                Ok(None)
            }
            Err(error) => {
                self.close_stream(index);
                let message = error.to_string();
                let suffix = error
                    .raw_os_error()
                    .map(|code| format!(" (os error {code})"))
                    .unwrap_or_default();
                let message = message.strip_suffix(&suffix).unwrap_or(&message);
                Err(FilesystemError::Unlocated(
                    [b"readdir ".as_slice(), path, b": ", message.as_bytes()].concat(),
                ))
            }
        }
    }

    fn read_all(&mut self, index: usize, path: &[u8]) -> Result<(), FilesystemError> {
        if self.contents[index].sorted.is_some() {
            return Ok(());
        }
        let mut entries: Vec<_> = std::mem::take(&mut self.contents[index].pending)
            .into_iter()
            .map(|(name, directory)| Entry {
                name: name.0,
                directory,
            })
            .collect();
        // A full glob needs no incremental hash index. Append directly and
        // sort once, retaining the latest type if a mutating stream repeats a
        // name. Partial lookups keep their hash table until a glob needs it.
        while let Some(entry) = self.next_entry(index, path)? {
            let bytes = entry.name.as_bytes().len() + 48;
            self.charge(bytes)?;
            self.contents[index].bytes += bytes;
            entries.push(Entry {
                name: entry.name,
                directory: entry.directory,
            });
        }
        entries.sort_by(|left, right| left.name.as_bytes().cmp(right.name.as_bytes()));
        let mut discarded = 0;
        entries.dedup_by(|later, earlier| {
            if later.name == earlier.name {
                earlier.directory = later.directory;
                discarded += later.name.as_bytes().len() + 48;
                true
            } else {
                false
            }
        });
        self.bytes = self.bytes.saturating_sub(discarded);
        self.contents[index].bytes = self.contents[index].bytes.saturating_sub(discarded);
        self.contents[index].sorted = Some(entries);
        Ok(())
    }

    fn read_directory(
        &mut self,
        index: usize,
        path: &[u8],
        filename: Option<&[u8]>,
    ) -> Result<bool, FilesystemError> {
        let Some(filename) = filename else {
            self.read_all(index, path)?;
            return Ok(false);
        };
        if filename.is_empty() {
            return Ok(self.contents[index].opened);
        }
        if let Some(entries) = &self.contents[index].sorted {
            return Ok(entries
                .binary_search_by(|entry| entry.name.as_bytes().cmp(filename))
                .is_ok());
        }
        if self.contents[index].pending.contains_key(filename) {
            return Ok(true);
        }
        while let Some(entry) = self.next_entry(index, path)? {
            let matches = filename == entry.name.as_bytes();
            if !self.contents[index]
                .pending
                .contains_key(entry.name.as_bytes())
            {
                let bytes = entry.name.as_bytes().len() + 48;
                self.charge(bytes)?;
                self.contents[index].bytes += bytes;
            }
            self.contents[index]
                .pending
                .insert(Name(entry.name), entry.directory);
            if matches {
                return Ok(true);
            }
        }
        // A negative membership probe leaves the completed hash table ready
        // for later probes; sorting is needed only if a wildcard consumes it.
        Ok(false)
    }

    /// Literal directory membership for rule search, distinct from glob's
    /// fresh literal lstat. Archive interpretation belongs to the caller.
    pub fn file_exists(&mut self, path: &[u8]) -> Result<bool, FilesystemError> {
        let (parent, filename) = match path.iter().rposition(|byte| *byte == b'/') {
            None => (b".".as_slice(), path),
            Some(0) => (b"/".as_slice(), &path[1..]),
            Some(slash) => (&path[..slash], &path[slash + 1..]),
        };
        let Some(index) = self.directory(parent)? else {
            return Ok(false);
        };
        self.read_directory(index, parent, Some(filename))
    }

    fn entries(&mut self, path: &[u8]) -> Result<&[Entry], FilesystemError> {
        let Some(index) = self.directory(path)? else {
            return Ok(&[]);
        };
        if self.contents[index].sorted.is_none() {
            self.read_all(index, path)?;
        }
        Ok(self.contents[index].sorted.as_deref().unwrap_or_default())
    }

    fn paths(&mut self, pattern: &[u8]) -> Result<Vec<Vec<u8>>, FilesystemError> {
        if let Some(result) = literal(pattern) {
            return result
                .map(|path| {
                    if path.is_empty() {
                        Vec::new()
                    } else {
                        vec![path]
                    }
                })
                .map_err(Into::into);
        }
        self.query(pattern, false, false, 0)
    }

    fn query(
        &mut self,
        text: &[u8],
        directories_only: bool,
        mark: bool,
        depth: usize,
    ) -> Result<Vec<Vec<u8>>, FilesystemError> {
        if depth > 512 {
            return Err(LIMIT.to_vec().into());
        }
        if text.is_empty() {
            return Ok(Vec::new());
        }
        let slash = text.iter().rposition(|&b| b == b'/');
        // A directory suffix is a query for its parent with directory marking.
        // Its spelling follows the Linux glob boundary, including repeated '/'.
        if text.ends_with(b"/") && slash.is_some_and(|pos| pos > 1) {
            let mut parent = &text[..text.len() - 1];
            if parent.iter().rev().take_while(|&&b| b == b'\\').count() % 2 == 1 {
                parent = &parent[..parent.len() - 1];
            }
            return self.query(parent, true, true, depth + 1);
        }
        let (parent, leaf, prefix) = match slash {
            None => (b".".as_slice(), text, false),
            Some(pos) if pos == 0 || (pos == 1 && text[0] == b'\\') => {
                (b"/".as_slice(), &text[pos + 1..], true)
            }
            Some(pos) => (&text[..pos], &text[pos + 1..], true),
        };
        let parents = if scanned(parent, false) {
            let parent = if parent.iter().rev().take_while(|&&b| b == b'\\').count() % 2 == 1 {
                &parent[..parent.len() - 1]
            } else {
                parent
            };
            self.query(parent, true, false, depth + 1)?
        } else {
            vec![unquote(parent)]
        };
        let mut result = Vec::new();
        let mut bytes = 0usize;
        for parent in parents {
            let mut emit = |name: &[u8]| -> Result<(), FilesystemError> {
                let mut path = if prefix {
                    join(&parent, name)
                } else {
                    name.to_vec()
                };
                if mark && exists(&path, true) {
                    path.push(b'/');
                }
                bytes = bytes.saturating_add(path.len() + 32);
                if bytes > MAX_OUTPUT {
                    return Err(LIMIT.to_vec().into());
                }
                result.push(path);
                Ok(())
            };
            if scanned(leaf, true) {
                let Some(matcher) = posix::Matcher::new(leaf) else {
                    continue;
                };
                let entries = self.entries(&parent)?;
                for entry in entries.iter() {
                    if matcher.matches(&entry.name)
                        && (!directories_only
                            || entry.directory.unwrap_or_else(|| {
                                exists(&join(&parent, entry.name.as_bytes()), true)
                            }))
                    {
                        emit(entry.name.as_bytes())?;
                    }
                }
            } else if exists(&join(&parent, leaf), false) {
                emit(leaf)?;
            }
        }
        result.sort_unstable();
        Ok(result)
    }

    pub fn expand(&mut self, pattern: &[u8]) -> Result<Vec<u8>, FilesystemError> {
        let (path, member) = rmake_core::filenames::archive_parts(pattern)
            .map_or((pattern, None), |(path, member)| (path, Some(member)));
        if member.is_none()
            && let Some(result) = literal(path)
        {
            return result.map_err(Into::into);
        }
        let mut paths = if member.is_some() {
            self.paths(path)?
        } else {
            self.query(path, false, false, 0)?
        };
        if member.is_none() && paths.len() == 1 {
            return Ok(paths.pop().unwrap());
        }
        let mut output = Vec::new();
        for matched in paths {
            let repeats = if let Some(member) = member {
                if magic(member) {
                    archive_matches(&matched, member)?
                } else {
                    0
                }
            } else {
                0
            };
            if repeats > 0 {
                // GNU 4.4.1's non-interned filename sequence exposes the original
                // archive pattern once for each matching member, with no member.
                for _ in 0..repeats {
                    append(&mut output, path)?;
                }
            } else {
                let mut name = matched;
                if let Some(member) = member {
                    name.push(b'(');
                    name.extend_from_slice(member);
                    name.push(b')');
                }
                append(&mut output, &name)?;
            }
        }
        Ok(output)
    }
}

fn append(output: &mut Vec<u8>, value: &[u8]) -> Result<(), Vec<u8>> {
    if output.len().saturating_add(value.len()).saturating_add(1) > MAX_OUTPUT {
        return Err(LIMIT.to_vec());
    }
    if !output.is_empty() {
        output.push(b' ');
    }
    output.extend_from_slice(value);
    Ok(())
}

fn archive_matches(path: &[u8], pattern: &[u8]) -> Result<usize, FilesystemError> {
    let Ok(mut file) = posix::open_archive(path) else {
        return Ok(0);
    };
    let mut count = 0;
    crate::archive::scan(&mut file, path, |member| {
        if posix::matches(pattern, &member.name) {
            count += 1;
            if count > MAX_OUTPUT / (path.len() + 1) {
                return Err(LIMIT.to_vec().into());
            }
        }
        Ok(None::<()>)
    })?;
    Ok(count)
}

#[cfg(test)]
mod stream_budget_tests {
    use super::*;

    #[test]
    fn allocation_failure_while_draining_tenth_stream_releases_it() {
        let root = std::env::temp_dir().join(format!(
            "rmake-dir-budget-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        struct Remove(std::path::PathBuf);
        impl Drop for Remove {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        let remove = Remove(root);
        let mut cache = Wildcard::default();
        for index in 0..10 {
            let directory = remove.0.join(index.to_string());
            fs::create_dir(&directory).unwrap();
            fs::write(directory.join("input"), b"original").unwrap();
            if index < 9 {
                assert!(
                    cache
                        .file_exists(directory.join("input").as_os_str().as_bytes())
                        .unwrap()
                );
            }
        }
        let last = remove.0.join("9");
        // Inject a budget failure after opening the tenth directory, at its
        // first entry allocation. This must not retain a tenth live stream.
        cache.bytes = MAX_OUTPUT * 8 - (last.as_os_str().as_bytes().len() + 64 + 128);
        assert!(
            cache
                .file_exists(last.join("input").as_os_str().as_bytes())
                .is_err()
        );
        assert_eq!(cache.open_streams, 9);
        assert!(cache.contents.last().unwrap().stream.is_none());
    }
}
