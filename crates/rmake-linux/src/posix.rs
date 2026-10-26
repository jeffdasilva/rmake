// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Small Linux libc boundary. Callers receive owned results, never libc pointers.

use std::ffi::{CStr, CString, c_char, c_int};
use std::fs::File;
use std::io;
use std::os::fd::FromRawFd;

#[repr(C)]
struct Passwd {
    name: *mut c_char,
    password: *mut c_char,
    uid: u32,
    gid: u32,
    gecos: *mut c_char,
    directory: *mut c_char,
    shell: *mut c_char,
}

unsafe extern "C" {
    fn open(path: *const c_char, flags: c_int, ...) -> c_int;
    fn fnmatch(pattern: *const c_char, name: *const c_char, flags: c_int) -> c_int;
    fn getpwnam_r(
        name: *const c_char,
        entry: *mut Passwd,
        buffer: *mut c_char,
        length: usize,
        result: *mut *mut Passwd,
    ) -> c_int;
    fn getlogin_r(buffer: *mut c_char, length: usize) -> c_int;
}

/// GNU's archive scanner makes one open attempt. Rust's standard File::open
/// retries EINTR, which changes the observable archive fallback on this profile.
pub fn open_archive(path: &[u8]) -> io::Result<File> {
    let path = CString::new(path).map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
    // O_RDONLY is zero. Linux O_CLOEXEC atomically protects scans that overlap
    // another worker's exec, without adding a retry or an fcntl race window.
    let descriptor = unsafe { open(path.as_ptr(), 0x80000) };
    if descriptor < 0 {
        return Err(io::Error::last_os_error());
    }
    // The successful open created a uniquely owned descriptor. File closes it
    // exactly once on all success, error and unwinding paths.
    Ok(unsafe { File::from_raw_fd(descriptor) })
}

pub struct Matcher {
    pattern: CString,
    simple_star: Option<usize>,
}

impl Matcher {
    pub fn new(pattern: &[u8]) -> Option<Self> {
        let simple_star = pattern.iter().position(|&b| b == b'*').filter(|&star| {
            !pattern[star + 1..].contains(&b'*')
                && !pattern
                    .iter()
                    .any(|b| matches!(b, b'?' | b'[' | b'\\' | b'/'))
        });
        Some(Self {
            pattern: CString::new(pattern).ok()?,
            simple_star,
        })
    }

    #[inline(always)]
    fn simple_match(&self, name: &[u8]) -> Option<bool> {
        let star = self.simple_star?;
        if name.contains(&b'/') {
            return None;
        }
        let pattern = self.pattern.as_bytes();
        Some(
            (name.first() != Some(&b'.') || pattern.first() == Some(&b'.'))
                && name.len() >= pattern.len() - 1
                && name.starts_with(&pattern[..star])
                && name.ends_with(&pattern[star + 1..]),
        )
    }

    pub fn matches(&self, name: &CStr) -> bool {
        if let Some(result) = self.simple_match(name.to_bytes()) {
            return result;
        }
        // Both immutable terminated strings live throughout the call. Linux's
        // PATHNAME and PERIOD flags prevent implicit slash/dot matching.
        unsafe { fnmatch(self.pattern.as_ptr(), name.as_ptr(), 1 | 4) == 0 }
    }

    pub fn matches_bytes(&self, name: &[u8]) -> bool {
        if name.contains(&0) {
            return false;
        }
        if let Some(result) = self.simple_match(name) {
            return result;
        }
        let Ok(name) = CString::new(name) else {
            return false;
        };
        self.matches(&name)
    }
}

pub fn home(user: Option<&[u8]>) -> Option<Vec<u8>> {
    let login;
    let user = match user {
        Some(user) => user,
        None => {
            let mut buffer = vec![0_u8; 16384];
            // getlogin_r writes only within the supplied initialized buffer.
            if unsafe { getlogin_r(buffer.as_mut_ptr().cast(), buffer.len()) } != 0 {
                return None;
            }
            let end = buffer.iter().position(|&b| b == 0)?;
            login = buffer[..end].to_vec();
            &login
        }
    };
    let name = CString::new(user).ok()?;
    let mut size = 16384;
    while size <= 1024 * 1024 {
        let mut buffer = vec![0_u8; size];
        let mut entry = Passwd {
            name: std::ptr::null_mut(),
            password: std::ptr::null_mut(),
            uid: 0,
            gid: 0,
            gecos: std::ptr::null_mut(),
            directory: std::ptr::null_mut(),
            shell: std::ptr::null_mut(),
        };
        let mut result = std::ptr::null_mut();
        // Reentrant lookup owns all returned string storage in buffer. No
        // pointer escapes this call or survives the buffer's lifetime.
        let status = unsafe {
            getpwnam_r(
                name.as_ptr(),
                &mut entry,
                buffer.as_mut_ptr().cast(),
                size,
                &mut result,
            )
        };
        if status == 34 {
            // Linux ERANGE: retry with a bounded larger buffer.
            size *= 2;
            continue;
        }
        if status != 0 || result.is_null() || entry.directory.is_null() {
            return None;
        }
        // A successful libc lookup supplies a terminated pw_dir in buffer.
        return Some(
            unsafe { CStr::from_ptr(entry.directory) }
                .to_bytes()
                .to_vec(),
        );
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_matcher_paths_agree_with_libc_for_raw_names_and_dot_boundaries() {
        for bytes in [
            b"*".as_slice(),
            b"a*",
            b"*a",
            b"a*b",
            b".*",
            b"*\xff",
            b"\xff*",
            b"a?b",
            b"[a-c]*.o",
            b"a\\*b",
            b"dir/*",
        ] {
            let matcher = Matcher::new(bytes).unwrap();
            for first in 1..=255u8 {
                for suffix in [b"".as_slice(), b"ab", b".a", b"/a", b"\xff"] {
                    let name = CString::new([&[first], suffix].concat()).unwrap();
                    // Independent libc path validates the optimized result.
                    let native =
                        unsafe { fnmatch(matcher.pattern.as_ptr(), name.as_ptr(), 1 | 4) } == 0;
                    assert_eq!(
                        matcher.matches(&name),
                        native,
                        "pattern={bytes:?}, name={name:?}"
                    );
                    assert_eq!(matcher.matches_bytes(name.as_bytes()), native);
                }
            }
        }
    }

    #[test]
    fn byte_matcher_rejects_nul_in_names_and_patterns() {
        assert!(Matcher::new(b"*\0tail").is_none());
        for pattern in [b"*".as_slice(), b"*.o", b"[ab]*"] {
            let matcher = Matcher::new(pattern).unwrap();
            assert!(!matcher.matches_bytes(b"a\0.o"));
            assert!(!matcher.matches_bytes(b"a.o\0"));
        }
    }
}

#[cfg(test)]
mod archive_descriptor_tests {
    use super::open_archive;
    use std::os::fd::AsRawFd;
    use std::os::unix::ffi::OsStrExt;

    #[test]
    fn archive_descriptor_is_not_inherited_by_recipe_children() {
        let name = format!(
            "rmake-archive-descriptor-{}-{}.a",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let path = std::env::temp_dir().join(name);
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        struct Remove(std::path::PathBuf);
        impl Drop for Remove {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }
        let _remove = Remove(path.clone());
        let file = open_archive(path.as_os_str().as_bytes()).unwrap();
        // A recipe worker may spawn while the interpreter is scanning another
        // member. Observe inheritance through a real exec, while the scan lives.
        let descriptor = format!("/proc/self/fd/{}", file.as_raw_fd());
        let result = std::process::Command::new("/usr/bin/readlink")
            .arg(descriptor)
            .output()
            .unwrap();
        let mut leaked_path = std::fs::canonicalize(&path)
            .unwrap()
            .as_os_str()
            .as_bytes()
            .to_vec();
        leaked_path.push(b'\n');
        assert_ne!(
            result.stdout, leaked_path,
            "archive descriptor reached the child"
        );
    }
}
