// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Invocation-scoped directory observations and byte-preserving filename globbing.

use std::collections::HashMap;
use std::ffi::{CString, OsStr};
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::sync::Arc;

use crate::filesystem::retry_interrupted;
use crate::posix;
use rmake_core::diagnostic::FilesystemError;
use rmake_core::variables::MAX_OUTPUT;

const LIMIT: &[u8] = b"variable expansion resource limit exceeded";

#[derive(Clone)]
struct Entry {
    name: CString,
    directory: Option<bool>,
}

#[derive(Default)]
pub struct Wildcard {
    directories: HashMap<(u64, u64), Arc<Vec<Entry>>>,
    paths: HashMap<Vec<u8>, Arc<Vec<Entry>>>,
    bytes: usize,
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
    pub fn invalidate(&mut self) {
        self.directories.clear();
        self.paths.clear();
        self.bytes = 0;
    }

    fn entries(&mut self, path: &[u8]) -> Result<Arc<Vec<Entry>>, Vec<u8>> {
        let key_path = if path.is_empty() {
            b".".as_slice()
        } else {
            path
        };
        if let Some(entries) = self.paths.get(key_path) {
            return Ok(Arc::clone(entries));
        }
        self.bytes = self.bytes.saturating_add(key_path.len() + 64);
        if self.bytes > MAX_OUTPUT * 8 {
            return Err(LIMIT.to_vec());
        }
        let path = OsStr::from_bytes(key_path);
        let Ok(metadata) = retry_interrupted(|| fs::metadata(path)) else {
            let entries = Arc::default();
            self.paths.insert(key_path.to_vec(), Arc::clone(&entries));
            return Ok(entries);
        };
        let key = (metadata.dev(), metadata.ino());
        if let Some(entries) = self.directories.get(&key) {
            self.paths.insert(key_path.to_vec(), Arc::clone(entries));
            return Ok(Arc::clone(entries));
        }
        let mut names = Vec::new();
        if let Ok(entries) = retry_interrupted(|| fs::read_dir(path)) {
            names.extend([c".".to_owned(), c"..".to_owned()].map(|name| Entry {
                name,
                directory: Some(true),
            }));
            for entry in entries {
                let Ok(entry) = entry else {
                    break;
                };
                let Ok(name) = CString::new(entry.file_name().as_bytes()) else {
                    continue;
                };
                self.bytes = self.bytes.saturating_add(name.as_bytes().len() + 32);
                if self.bytes > MAX_OUTPUT * 8 {
                    return Err(LIMIT.to_vec());
                }
                let directory = entry.file_type().ok().and_then(|kind| {
                    if kind.is_symlink() {
                        None
                    } else {
                        Some(kind.is_dir())
                    }
                });
                names.push(Entry { name, directory });
            }
        }
        names.sort_unstable_by(|left, right| left.name.as_bytes().cmp(right.name.as_bytes()));
        let names = Arc::new(names);
        self.directories.insert(key, Arc::clone(&names));
        self.paths.insert(key_path.to_vec(), Arc::clone(&names));
        Ok(names)
    }

    fn paths(&mut self, pattern: &[u8]) -> Result<Vec<Vec<u8>>, Vec<u8>> {
        if let Some(result) = literal(pattern) {
            return result.map(|path| {
                if path.is_empty() {
                    Vec::new()
                } else {
                    vec![path]
                }
            });
        }
        self.query(pattern, false, false, 0)
    }

    fn query(
        &mut self,
        text: &[u8],
        directories_only: bool,
        mark: bool,
        depth: usize,
    ) -> Result<Vec<Vec<u8>>, Vec<u8>> {
        if depth > 512 {
            return Err(LIMIT.to_vec());
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
            let mut emit = |name: &[u8]| -> Result<(), Vec<u8>> {
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
                    return Err(LIMIT.to_vec());
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

fn decimal(bytes: &[u8]) -> Option<u64> {
    let bytes = bytes.trim_ascii();
    if bytes.is_empty() {
        return None;
    }
    bytes.iter().try_fold(0u64, |n, &b| {
        if b.is_ascii_digit() {
            n.checked_mul(10)?.checked_add(u64::from(b - b'0'))
        } else {
            None
        }
    })
}

fn archive_field(
    bytes: &[u8],
    base: u64,
    max: u64,
    label: &[u8],
    archive: &[u8],
    name: &[u8],
) -> Result<u64, FilesystemError> {
    let mut field = bytes;
    while field.first() == Some(&b' ') {
        field = &field[1..];
    }
    let mut number = 0u64;
    for &byte in field.iter().take_while(|&&b| b != b' ') {
        let value = byte.checked_sub(b'0').filter(|&n| u64::from(n) < base);
        let next = value
            .and_then(|n| number.checked_mul(base)?.checked_add(u64::from(n)))
            .filter(|&n| n <= max);
        let Some(next) = next else {
            return Err(FilesystemError::Unlocated(
                [
                    b"Invalid ".as_slice(),
                    label,
                    b" for archive ",
                    archive,
                    b" member ",
                    name,
                ]
                .concat(),
            ));
        };
        number = next;
    }
    Ok(number)
}

fn archive_matches(path: &[u8], pattern: &[u8]) -> Result<usize, FilesystemError> {
    let Ok(mut file) = posix::open_archive(path) else {
        return Ok(0);
    };
    let mut signature = [0; 8];
    if file.read_exact(&mut signature).is_err() || &signature != b"!<arch>\n" {
        return Ok(0);
    }
    let mut offset = 8u64;
    let mut table = Vec::new();
    let mut count = 0;
    let mut headers = 0;
    loop {
        headers += 1;
        if headers > 1_000_000 {
            return Err(LIMIT.to_vec().into());
        }
        let mut header = [0; 60];
        if file.seek(SeekFrom::Start(offset)).is_err()
            || file.read_exact(&mut header).is_err()
            || &header[58..] != b"`\n"
        {
            break;
        }
        let mut raw = &header[..16];
        while raw.last() == Some(&b' ') {
            raw = &raw[..raw.len() - 1];
        }
        let mapping = matches!(raw, b"//" | b"ARFILENAMES/");
        let mut name = raw.strip_suffix(b"/").unwrap_or(raw).to_vec();
        if let Some(nul) = name.iter().position(|&b| b == 0) {
            name.truncate(nul);
        }
        if !mapping && !table.is_empty() && matches!(name.first(), Some(b' ' | b'/')) {
            let Some(reference) = decimal(&name[1..]) else {
                break;
            };
            let Some(entry) = table
                .get(usize::try_from(reference).unwrap_or(usize::MAX)..)
                .filter(|entry| entry.len() > 1)
            else {
                break;
            };
            let end = entry.iter().position(|&b| b == 0).unwrap_or(entry.len());
            if end == 0 {
                break;
            }
            name = entry[..end].to_vec();
        } else if let Some(inline) = name.strip_prefix(b"#1/") {
            let Some(length) = decimal(inline).filter(|&n| n > 0 && n < 4096) else {
                break;
            };
            name = vec![0; length as usize];
            if file.read_exact(&mut name).is_err() {
                break;
            }
            if let Some(nul) = name.iter().position(|&b| b == 0) {
                name.truncate(nul);
            }
        }
        archive_field(&header[40..48], 8, u32::MAX as u64, b"mode", path, &name)?;
        let size = archive_field(&header[48..58], 10, i64::MAX as u64, b"size", path, &name)?;
        archive_field(&header[16..28], 10, i64::MAX as u64, b"date", path, &name)?;
        archive_field(&header[28..34], 10, i32::MAX as u64, b"uid", path, &name)?;
        archive_field(&header[34..40], 10, i32::MAX as u64, b"gid", path, &name)?;
        if posix::matches(pattern, &name) {
            count += 1;
            if count > MAX_OUTPUT / (path.len() + 1) {
                return Err(LIMIT.to_vec().into());
            }
        }
        let Some(end) = offset.checked_add(60).and_then(|p| p.checked_add(size)) else {
            break;
        };
        if mapping {
            if size > MAX_OUTPUT as u64 {
                return Err(LIMIT.to_vec().into());
            }
            table.resize(size as usize + 1, 0);
            if file.read_exact(&mut table[..size as usize]).is_err() {
                break;
            }
            for pos in 0..size as usize {
                if table[pos] == b'\n' {
                    table[pos] = 0;
                    if pos > 0 && table[pos - 1] == b'/' {
                        table[pos - 1] = 0;
                    }
                }
            }
            table[size as usize] = 0;
        }
        let Some(next) = end.checked_add(size % 2) else {
            break;
        };
        offset = next;
    }
    Ok(count)
}
