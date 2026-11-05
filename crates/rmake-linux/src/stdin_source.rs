// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Named stdin source publication is separate from the buffered copy lifetime.
use std::cell::Cell;
use std::ffi::{CString, OsString, c_char, c_int, c_void};
use std::fs::File;
use std::io;
use std::os::fd::{FromRawFd, IntoRawFd};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};

unsafe extern "C" {
    fn rm_stdin_create(name: *mut c_char) -> c_int;
    fn rm_stdin_buffer(descriptor: c_int) -> *mut c_void;
    fn rm_stdin_read(bytes: *mut u8, capacity: usize, finished: *mut c_int) -> usize;
    fn rm_stdin_write(output: *mut c_void, bytes: *const u8, length: usize) -> c_int;
    fn rm_stdin_close(output: *mut c_void);
}

struct Buffered(*mut c_void);
impl Buffered {
    fn new(file: File) -> io::Result<Self> {
        let descriptor = file.into_raw_fd();
        let buffer = unsafe { rm_stdin_buffer(descriptor) };
        if buffer.is_null() {
            let error = io::Error::last_os_error();
            drop(unsafe { File::from_raw_fd(descriptor) });
            Err(error)
        } else {
            Ok(Self(buffer))
        }
    }

    fn abandon(self) {
        // Before publication a fatal signal abandons libc's unflushed copy.
        // The process will re-raise that signal; closing it would flush bytes
        // and incorrectly turn the observed empty source into a partial one.
        std::mem::forget(self);
    }
}
impl Drop for Buffered {
    fn drop(&mut self) {
        unsafe { rm_stdin_close(self.0) };
    }
}

pub struct Source {
    path: PathBuf,
    published: Cell<bool>,
}
impl Source {
    pub fn adopt(path: PathBuf) -> Self {
        Self {
            path,
            published: Cell::new(true),
        }
    }

    pub fn copy_stdin() -> io::Result<Self> {
        let mut name = CString::new(
            crate::temporary::initialize()
                .directory()
                .join("GmXXXXXX")
                .as_os_str()
                .as_bytes(),
        )
        .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?
        .into_bytes_with_nul();
        let descriptor = unsafe { rm_stdin_create(name.as_mut_ptr().cast()) };
        if descriptor < 0 {
            return Err(io::Error::last_os_error());
        }
        name.pop();
        let source = Self {
            path: PathBuf::from(OsString::from_vec(name)),
            published: Cell::new(false),
        };
        let file = crate::process::private_descriptor(unsafe { File::from_raw_fd(descriptor) })?;
        let buffer = Buffered::new(file)?;
        let mut bytes = [0_u8; 4096];
        loop {
            if crate::signals::pending().is_some() {
                buffer.abandon();
                return Err(io::ErrorKind::Interrupted.into());
            }
            let mut finished = 0;
            let length = unsafe { rm_stdin_read(bytes.as_mut_ptr(), bytes.len(), &mut finished) };
            if crate::signals::pending().is_some() {
                buffer.abandon();
                return Err(io::ErrorKind::Interrupted.into());
            }
            if length > 0 && unsafe { rm_stdin_write(buffer.0, bytes.as_ptr(), length) } < 0 {
                return Err(io::Error::last_os_error());
            }
            if finished != 0 {
                drop(buffer);
                source.published.set(true);
                return Ok(source);
            }
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn disarm(&self) {
        self.published.set(false);
    }

    pub fn unlink(&self) -> io::Result<()> {
        if !self.published.replace(false) {
            return Ok(());
        }
        loop {
            match std::fs::remove_file(&self.path) {
                Ok(()) => return Ok(()),
                Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
            }
        }
    }
}
impl Drop for Source {
    fn drop(&mut self) {
        let _ = self.unlink();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);

    #[test]
    fn adopted_source_unlinks_once_without_losing_its_raw_identity() {
        let path = std::env::temp_dir().join(format!(
            "rmake-stdin-owner-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, b"owned").unwrap();
        let source = Source::adopt(path.clone());
        source.unlink().unwrap();
        assert_eq!(source.path(), path);
        std::fs::write(&path, b"new owner").unwrap();
        drop(source);
        assert_eq!(std::fs::read(&path).unwrap(), b"new owner");
        std::fs::remove_file(path).unwrap();
    }
}
