// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Linux build timestamps, with optional final-component symlink traversal.

use std::ffi::OsStr;
use std::io;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::time::SystemTime;

const PATH_LIMIT: usize = 4096;

pub struct Issue {
    pub operation: &'static [u8],
    pub path: Vec<u8>,
    pub error: io::Error,
}

#[derive(Default)]
pub struct Observation {
    pub time: Option<SystemTime>,
    pub issues: Vec<Issue>,
}

fn retry<T>(mut operation: impl FnMut() -> io::Result<T>) -> io::Result<T> {
    loop {
        match operation() {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            result => return result,
        }
    }
}

fn missing(error: &io::Error) -> bool {
    matches!(error.raw_os_error(), Some(2 | 20)) // Linux ENOENT / ENOTDIR.
}

impl Observation {
    fn issue(&mut self, operation: &'static [u8], path: &[u8], error: io::Error) {
        self.issues.push(Issue {
            operation,
            path: path.to_vec(),
            error,
        });
    }

    fn link_metadata(&mut self, path: &[u8]) -> Option<std::fs::Metadata> {
        match retry(|| std::fs::symlink_metadata(OsStr::from_bytes(path))) {
            Ok(metadata) => Some(metadata),
            Err(error) => {
                if !missing(&error) {
                    self.issue(b"lstat: ", path, error);
                }
                None
            }
        }
    }
}

pub fn observe(name: &[u8], symlinks: bool) -> Observation {
    let mut result = Observation::default();
    match retry(|| std::fs::metadata(OsStr::from_bytes(name))) {
        Ok(metadata) => result.time = metadata.modified().ok(),
        Err(error) if missing(&error) => {}
        Err(error) => {
            result.issue(b"stat: ", name, error);
            return result;
        }
    }
    if !symlinks || name.len() > PATH_LIMIT {
        return result;
    }
    let Some(mut link) = result.link_metadata(name) else {
        return result;
    };
    if !link.file_type().is_symlink() {
        return result;
    }
    // Ordinary files returned before allocating a walk pathname. A real link
    // walk keeps one owned spelling, reused for relative targets.
    let mut path = name.to_vec();
    // A stable Linux lookup follows at most 40 links. Bound a walk whose
    // topology changes concurrently too, retaining its latest observation.
    for _ in 0..128 {
        result.time = result.time.max(link.modified().ok());
        let target = match retry(|| std::fs::read_link(OsStr::from_bytes(&path))) {
            Ok(target) => target.into_os_string().into_vec(),
            Err(error) => {
                result.issue(b"readlink: ", &path, error);
                break;
            }
        };
        if target.starts_with(b"/") || !path.contains(&b'/') {
            path = target;
        } else {
            let slash = path.iter().rposition(|&byte| byte == b'/').unwrap();
            if slash + target.len() + 2 > PATH_LIMIT {
                break;
            }
            path.truncate(slash + 1);
            path.extend_from_slice(&target);
        }
        let Some(next) = result.link_metadata(&path) else {
            break;
        };
        if !next.file_type().is_symlink() {
            break;
        }
        link = next;
    }
    result
}
