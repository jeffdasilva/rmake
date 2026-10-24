// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Bounded Linux archive scans shared by globbing and build metadata.
//! File contents are read afresh; no member index survives an effect.

use crate::posix;
use rmake_core::diagnostic::FilesystemError;
use rmake_core::variables::MAX_OUTPUT;
use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::ffi::OsStrExt;
use std::time::{Duration, SystemTime};

const LIMIT: &[u8] = b"variable expansion resource limit exceeded";

pub(crate) struct Member {
    pub name: Vec<u8>,
    pub date: u64,
    pub offset: u64,
    truncated: bool,
}

pub(crate) enum Scan<T> {
    Found(T),
    Absent,
    Invalid,
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

pub(crate) fn scan<T>(
    file: &mut File,
    path: &[u8],
    mut visit: impl FnMut(Member) -> Result<Option<T>, FilesystemError>,
) -> Result<Scan<T>, FilesystemError> {
    let mut signature = [0; 8];
    if file.read_exact(&mut signature).is_err() || &signature != b"!<arch>\n" {
        return Ok(Scan::Invalid);
    }
    let mut offset = 8u64;
    let mut table = Vec::new();
    let mut headers = 0;
    loop {
        headers += 1;
        if headers > 1_000_000 {
            return Err(LIMIT.to_vec().into());
        }
        let mut header = [0; 60];
        if file.seek(SeekFrom::Start(offset)).is_err() {
            return Ok(Scan::Invalid);
        }
        if file.read_exact(&mut header).is_err() {
            return Ok(if file.stream_position().ok() == Some(offset) {
                Scan::Absent
            } else {
                Scan::Invalid
            });
        }
        if &header[58..] != b"`\n" {
            return Ok(Scan::Invalid);
        }
        let mut raw = &header[..16];
        while raw.last() == Some(&b' ') {
            raw = &raw[..raw.len() - 1];
        }
        let mut truncated = true;
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
            truncated = false;
        } else if let Some(inline) = name.strip_prefix(b"#1/") {
            let Some(length) = decimal(inline).filter(|&n| n > 0 && n < 4096) else {
                break;
            };
            truncated = false;
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
        let date = archive_field(&header[16..28], 10, i64::MAX as u64, b"date", path, &name)?;
        archive_field(&header[28..34], 10, i32::MAX as u64, b"uid", path, &name)?;
        archive_field(&header[34..40], 10, i32::MAX as u64, b"gid", path, &name)?;
        if let Some(result) = visit(Member {
            name,
            date,
            offset,
            truncated,
        })? {
            return Ok(Scan::Found(result));
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
    Ok(Scan::Invalid)
}

fn matches(member: &Member, requested: &[u8]) -> bool {
    let requested = requested
        .rsplit(|&byte| byte == b'/')
        .next()
        .unwrap_or(requested);
    if member.truncated {
        requested.get(..15).unwrap_or(requested) == member.name.get(..15).unwrap_or(&member.name)
    } else {
        requested == member.name
    }
}

/// GNU treats an archive member as absent when its date is zero, or when a
/// standalone member file has a strictly later integer-second timestamp.
pub fn date(archive: &[u8], member: &[u8]) -> Result<Option<u64>, FilesystemError> {
    let Ok(mut file) = posix::open_archive(archive) else {
        return Ok(None);
    };
    let Scan::Found(date) = scan(&mut file, archive, |entry| {
        Ok((entry.date > 0 && matches(&entry, member)).then_some(entry.date))
    })?
    else {
        return Ok(None);
    };
    Ok(Some(date))
}

pub fn modified(archive: &[u8], member: &[u8]) -> Result<Option<SystemTime>, FilesystemError> {
    let Some(date) = date(archive, member)? else {
        return Ok(None);
    };
    if fs::metadata(OsStr::from_bytes(member))
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|time| time.duration_since(SystemTime::UNIX_EPOCH).ok())
        .is_some_and(|duration| duration.as_secs() > date)
    {
        return Ok(None);
    }
    Ok(SystemTime::UNIX_EPOCH.checked_add(Duration::from_secs(date)))
}

/// Update only the selected date field through the descriptor that was scanned.
/// Keep every other header/payload byte, including long-name tables, untouched.
pub fn touch(archive: &[u8], member: &[u8]) -> Result<(), Vec<u8>> {
    let mut file = match OpenOptions::new()
        .read(true)
        .write(true)
        .open(OsStr::from_bytes(archive))
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err([b"touch: Archive '".as_slice(), archive, b"' does not exist"].concat());
        }
        Err(error) => return Err(io_message(archive, error)),
    };
    let found = scan(&mut file, archive, |entry| {
        Ok(matches(&entry, member).then_some(entry.offset))
    })
    .map_err(|error| error.into_message())?;
    let offset = match found {
        Scan::Found(offset) => offset,
        Scan::Invalid => {
            return Err([b"touch: '".as_slice(), archive, b"' is not a valid archive"].concat());
        }
        Scan::Absent => {
            return Err([
                b"touch: Member '".as_slice(),
                member,
                b"' does not exist in '",
                archive,
                b"'",
            ]
            .concat());
        }
    };
    let time = file
        .metadata()
        .and_then(|metadata| metadata.modified())
        .map_err(|error| io_message(archive, error))?;
    let seconds = time
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_err(|_| b"touch: archive timestamp out of range".to_vec())?
        .as_secs()
        .to_string();
    if seconds.len() >= 12 {
        return Err(b"touch: archive timestamp out of range".to_vec());
    }
    let mut date = [b' '; 12];
    date[..seconds.len()].copy_from_slice(seconds.as_bytes());
    file.seek(SeekFrom::Start(offset + 16))
        .and_then(|_| file.write_all(&date))
        .map_err(|error| io_message(archive, error))
}

fn io_message(archive: &[u8], error: std::io::Error) -> Vec<u8> {
    let message = error.to_string();
    let suffix = error
        .raw_os_error()
        .map(|code| format!(" (os error {code})"))
        .unwrap_or_default();
    [
        b"touch: ".as_slice(),
        archive,
        b": ",
        message.strip_suffix(&suffix).unwrap_or(&message).as_bytes(),
    ]
    .concat()
}
