// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Owned Linux libc directory streams. Only owned names cross this boundary.

use std::ffi::{CString, c_char, c_int};
use std::io;
use std::ptr::NonNull;

#[repr(C)]
struct CDirectory {
    _opaque: [u8; 0],
}

// Linux/glibc dirent64 ABI, independent of the Rust File/ReadDir API.
// Read only the fixed fields and the bounded variable record's name bytes.
#[repr(C)]
struct Dirent {
    inode: u64,
    offset: i64,
    record_length: u16,
    kind: u8,
    name: [c_char; 256],
}

unsafe extern "C" {
    fn opendir(path: *const c_char) -> *mut CDirectory;
    fn readdir64(directory: *mut CDirectory) -> *mut Dirent;
    fn closedir(directory: *mut CDirectory) -> c_int;
    fn __errno_location() -> *mut c_int;
}

pub(super) struct Entry {
    pub name: CString,
    pub directory: Option<bool>,
}

/// Exclusive ownership; neither Clone nor Send/Sync. opendir sets CLOEXEC.
pub(super) struct Stream(NonNull<CDirectory>);

impl Stream {
    pub fn open(path: &[u8]) -> io::Result<Self> {
        let path = CString::new(path)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "NUL byte in filename"))?;
        loop {
            // SAFETY: a live terminated path; success transfers a new stream.
            let pointer = unsafe { opendir(path.as_ptr()) };
            if let Some(pointer) = NonNull::new(pointer) {
                return Ok(Self(pointer));
            }
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::Interrupted {
                return Err(error);
            }
        }
    }

    pub fn next_entry(&mut self) -> io::Result<Option<Entry>> {
        loop {
            // SAFETY: errno is thread-local, and this exclusively borrowed
            // stream is live. Clear errno to distinguish EOF from failure.
            let pointer = unsafe {
                *__errno_location() = 0;
                readdir64(self.0.as_ptr())
            };
            if pointer.is_null() {
                let error = io::Error::last_os_error();
                match error.raw_os_error() {
                    Some(0) => return Ok(None),
                    Some(4) => continue, // Linux EINTR; retain the stream.
                    _ => return Err(error),
                }
            }
            // SAFETY: libc supplies a native dirent record until the next
            // read. Raw field reads do not borrow an entire padded struct.
            let inode = unsafe { std::ptr::addr_of!((*pointer).inode).read_unaligned() };
            if inode == 0 {
                continue;
            }
            let length =
                unsafe { std::ptr::addr_of!((*pointer).record_length).read_unaligned() } as usize;
            let offset = std::mem::offset_of!(Dirent, name);
            let Some(capacity) = length.checked_sub(offset).filter(|length| *length > 0) else {
                return Err(io::Error::from(io::ErrorKind::InvalidData));
            };
            // SAFETY: the record contains record_length bytes. Bound the
            // borrowed region to its name field and copy before another read.
            let bytes = unsafe {
                std::slice::from_raw_parts(
                    std::ptr::addr_of!((*pointer).name).cast::<u8>(),
                    capacity.min(256),
                )
            };
            let Some(end) = bytes.iter().position(|byte| *byte == 0) else {
                return Err(io::Error::from(io::ErrorKind::InvalidData));
            };
            let name = CString::new(&bytes[..end]).expect("bounded prefix has no NUL");
            let kind = unsafe { std::ptr::addr_of!((*pointer).kind).read() };
            return Ok(Some(Entry {
                name,
                directory: match kind {
                    0 | 10 => None, // DT_UNKNOWN and DT_LNK need a fresh stat.
                    4 => Some(true),
                    _ => Some(false),
                },
            }));
        }
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        // SAFETY: this is the sole owner. closedir consumes it even on error;
        // never retry a close or retain a borrowed entry across this call.
        unsafe { closedir(self.0.as_ptr()) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::symlink;

    struct Temporary(std::path::PathBuf);
    impl Temporary {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "rmake-directory-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Temporary {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn native_stream_returns_owned_dot_raw_and_type_entries() {
        let temporary = Temporary::new();
        std::fs::write(temporary.0.join(OsStr::from_bytes(b"raw-\xff")), b"x").unwrap();
        std::fs::create_dir(temporary.0.join("sub")).unwrap();
        symlink("absent", temporary.0.join("link")).unwrap();
        let mut stream = Stream::open(temporary.0.as_os_str().as_bytes()).unwrap();
        let mut entries = Vec::new();
        while let Some(entry) = stream.next_entry().unwrap() {
            entries.push(entry);
        }
        drop(stream);
        entries.sort_by(|a, b| a.name.as_bytes().cmp(b.name.as_bytes()));
        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.name.as_bytes())
                .collect::<Vec<_>>(),
            [b".".as_slice(), b"..", b"link", b"raw-\xff", b"sub"]
        );
        assert_eq!(entries[0].directory, Some(true));
        assert_eq!(entries[2].directory, None);
        assert_eq!(entries[3].directory, Some(false));
        assert_eq!(entries[4].directory, Some(true));
    }

    #[test]
    fn directory_descriptors_have_close_on_exec() {
        unsafe extern "C" {
            fn dirfd(directory: *mut CDirectory) -> c_int;
            fn fcntl(descriptor: c_int, command: c_int, ...) -> c_int;
        }
        let temporary = Temporary::new();
        let stream = Stream::open(temporary.0.as_os_str().as_bytes()).unwrap();
        // SAFETY: inspect flags of the still-owned live directory descriptor.
        let descriptor = unsafe { dirfd(stream.0.as_ptr()) };
        assert!(descriptor >= 0);
        assert_eq!(unsafe { fcntl(descriptor, 1) } & 1, 1); // F_GETFD / FD_CLOEXEC.
    }
}
