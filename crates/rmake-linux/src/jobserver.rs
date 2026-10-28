// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Linux FIFO/pipe jobserver effects. The interpreter retains each acquired
//! byte for one complete recipe. Dropping its permit returns that same byte.

use std::ffi::{CString, c_char, c_int, c_uint};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{FileTypeExt, MetadataExt, OpenOptionsExt};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

const NONBLOCK: c_int = 0x800;
const CLOEXEC: c_int = 0x80000;
const NOFOLLOW: c_int = 0x20000;
static NEXT_FIFO: AtomicU64 = AtomicU64::new(0);

unsafe extern "C" {
    fn mkfifo(path: *const c_char, mode: c_uint) -> c_int;
    fn pipe2(descriptors: *mut c_int, flags: c_int) -> c_int;
    fn fcntl(descriptor: c_int, operation: c_int, ...) -> c_int;
}

struct Inner {
    reader: File,
    writer: File,
    protocol_reader: Option<File>,
    auth: Vec<u8>,
    owned_slots: Option<usize>,
    created: Option<(PathBuf, u64, u64)>,
    error: Mutex<Option<io::Error>>,
}

impl Drop for Inner {
    fn drop(&mut self) {
        if let Some((path, device, inode)) = &self.created
            && fs::symlink_metadata(path).is_ok_and(|metadata| {
                metadata.file_type().is_fifo()
                    && metadata.dev() == *device
                    && metadata.ino() == *inode
            })
        {
            let _ = fs::remove_file(path);
        }
    }
}

#[derive(Clone)]
pub struct Jobserver(Arc<Inner>);

/// The implicit permit is the slot inherited from the parent command. External
/// permits own exactly one token and cannot be cloned.
pub struct Permit {
    server: Option<Jobserver>,
    byte: u8,
}

impl Permit {
    pub fn implicit() -> Self {
        Self {
            server: None,
            byte: 0,
        }
    }
    pub fn is_implicit(&self) -> bool {
        self.server.is_none()
    }
}

impl Drop for Permit {
    fn drop(&mut self) {
        if let Some(server) = &self.server {
            let mut writer = &server.0.writer;
            if let Err(error) = writer.write_all(&[self.byte]) {
                *server.0.error.lock().expect("jobserver error lock") = Some(error);
            }
        }
    }
}

fn reopen(descriptor: c_int, read: bool, nonblocking: bool) -> io::Result<File> {
    let file = OpenOptions::new()
        .read(read)
        .write(!read)
        .custom_flags(if nonblocking { NONBLOCK } else { 0 })
        .open(format!("/proc/self/fd/{descriptor}"))?;
    if !file.metadata()?.file_type().is_fifo() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "jobserver descriptor is not a pipe",
        ));
    }
    Ok(file)
}

impl Jobserver {
    pub fn create(jobs: usize, pipe: bool) -> io::Result<Self> {
        if jobs < 2 {
            return Err(io::Error::from(io::ErrorKind::InvalidInput));
        }
        let mut server = if pipe {
            let mut descriptors = [-1; 2];
            // A successful pipe2 returns two new, uniquely owned descriptors.
            if unsafe { pipe2(descriptors.as_mut_ptr(), CLOEXEC) } < 0 {
                return Err(io::Error::last_os_error());
            }
            let reader = unsafe { File::from_raw_fd(descriptors[0]) };
            let writer = unsafe { File::from_raw_fd(descriptors[1]) };
            Self::from_pipe(reader, writer)?
        } else {
            Self::create_fifo()?
        };
        // A separate nonblocking description avoids changing flags shared with
        // recursive children, and cannot hang if the requested pool is too big.
        let mut writer = reopen(server.0.writer.as_raw_fd(), false, true)?;
        let tokens = vec![b'+'; (jobs - 1).min(1_048_576)];
        if tokens.len() != jobs - 1 {
            return Err(io::Error::from(io::ErrorKind::InvalidInput));
        }
        writer.write_all(&tokens)?;
        Arc::get_mut(&mut server.0)
            .expect("unshared new jobserver")
            .owned_slots = Some(jobs);
        Ok(server)
    }

    fn create_fifo() -> io::Result<Self> {
        let directory = std::env::temp_dir();
        let directory = if directory.is_absolute() {
            directory
        } else {
            std::env::current_dir()?.join(directory)
        };
        for _ in 0..100 {
            let generation = NEXT_FIFO.fetch_add(1, Ordering::Relaxed);
            let name = if generation == 0 {
                format!("GMfifo{}", std::process::id())
            } else {
                format!("GMfifo{}-{generation}", std::process::id())
            };
            let path = directory.join(name);
            let name = CString::new(path.as_os_str().as_bytes())
                .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
            // mkfifo never replaces an existing file. The name is NUL-terminated.
            if unsafe { mkfifo(name.as_ptr(), 0o600) } < 0 {
                let error = io::Error::last_os_error();
                if error.kind() == io::ErrorKind::AlreadyExists {
                    continue;
                }
                return Err(error);
            }
            let identity = fs::symlink_metadata(&path)?;
            let result = Self::open_fifo(&path, true);
            if result.is_err()
                && fs::symlink_metadata(&path).is_ok_and(|metadata| {
                    metadata.file_type().is_fifo()
                        && metadata.dev() == identity.dev()
                        && metadata.ino() == identity.ino()
                })
            {
                let _ = fs::remove_file(&path);
            }
            return result;
        }
        Err(io::Error::from(io::ErrorKind::AlreadyExists))
    }

    fn open_fifo(path: &Path, owned: bool) -> io::Result<Self> {
        let reader = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(NONBLOCK | NOFOLLOW)
            .open(path)?;
        let metadata = reader.metadata()?;
        if !metadata.file_type().is_fifo() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "jobserver is not a FIFO",
            ));
        }
        let writer = reader.try_clone()?;
        let auth = [b"fifo:".as_slice(), path.as_os_str().as_bytes()].concat();
        Ok(Self(Arc::new(Inner {
            reader,
            writer,
            protocol_reader: None,
            auth,
            owned_slots: None,
            created: owned.then(|| (path.to_owned(), metadata.dev(), metadata.ino())),
            error: Mutex::new(None),
        })))
    }

    fn from_pipe(protocol_reader: File, writer: File) -> io::Result<Self> {
        let read_metadata = protocol_reader.metadata()?;
        let write_metadata = writer.metadata()?;
        if !read_metadata.file_type().is_fifo()
            || !write_metadata.file_type().is_fifo()
            || read_metadata.dev() != write_metadata.dev()
            || read_metadata.ino() != write_metadata.ino()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "jobserver descriptors refer to different pipes",
            ));
        }
        let reader = reopen(protocol_reader.as_raw_fd(), true, true)?;
        let auth = format!("{},{}", protocol_reader.as_raw_fd(), writer.as_raw_fd()).into_bytes();
        Ok(Self(Arc::new(Inner {
            reader,
            writer,
            protocol_reader: Some(protocol_reader),
            auth,
            owned_slots: None,
            created: None,
            error: Mutex::new(None),
        })))
    }

    /// The pool size is present only for the process that created this server.
    pub fn owned_slots(&self) -> Option<usize> {
        self.0.owned_slots
    }

    pub fn join(auth: &[u8]) -> io::Result<Self> {
        if let Some(path) = auth.strip_prefix(b"fifo:") {
            return Self::open_fifo(Path::new(std::ffi::OsStr::from_bytes(path)), false);
        }
        let text =
            std::str::from_utf8(auth).map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
        let (read, write) = text.split_once(',').ok_or(io::ErrorKind::InvalidInput)?;
        let read: c_int = read
            .parse()
            .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
        let write: c_int = write
            .parse()
            .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
        if read < 3 || write < 3 || read == write {
            return Err(io::Error::from(io::ErrorKind::InvalidInput));
        }
        let reader = reopen(read, true, false)?;
        let writer = reopen(write, false, false)?;
        let server = Self::from_pipe(reader, writer)?;
        // FD flags belong to this process's descriptors, not the parent's open
        // descriptions. Ordinary commands must not inherit the incoming pair.
        for descriptor in [read, write] {
            if unsafe { fcntl(descriptor, 2, 1) } < 0 {
                return Err(io::Error::last_os_error());
            }
        }
        Ok(server)
    }

    pub fn readiness(&self) -> BorrowedFd<'_> {
        self.0.reader.as_fd()
    }

    /// Release the owned pathname during fatal shutdown; open descriptors and
    /// permits remain valid so outstanding borrowed token bytes can be returned.
    pub fn unlink_owned(&self) {
        if let Some((path, device, inode)) = &self.0.created
            && fs::symlink_metadata(path).is_ok_and(|metadata| {
                metadata.file_type().is_fifo()
                    && metadata.dev() == *device
                    && metadata.ino() == *inode
            })
        {
            let _ = fs::remove_file(path);
        }
    }

    pub fn auth(&self) -> &[u8] {
        &self.0.auth
    }

    pub fn try_acquire(&self) -> io::Result<Option<Permit>> {
        let mut byte = [0];
        let mut reader = &self.0.reader;
        loop {
            match reader.read(&mut byte) {
                Ok(1) => {
                    return Ok(Some(Permit {
                        server: Some(self.clone()),
                        byte: byte[0],
                    }));
                }
                Ok(_) => return Err(io::Error::from(io::ErrorKind::UnexpectedEof)),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(None),
                Err(error) => return Err(error),
            }
        }
    }

    pub fn take_error(&self) -> Option<io::Error> {
        self.0.error.lock().expect("jobserver error lock").take()
    }

    pub fn configure_child(&self, command: &mut Command, recursive: bool) {
        if recursive && let Some(reader) = &self.0.protocol_reader {
            let read = reader.as_raw_fd();
            let write = self.0.writer.as_raw_fd();
            let owner = self.clone();
            // Only async-signal-safe fcntl calls run between fork and exec. Arc
            // owners in the caller keep both descriptors live through spawn.
            unsafe {
                command.pre_exec(move || {
                    // Retain descriptor ownership for the Command's lifetime,
                    // including callers that drop their original server first.
                    let _owner = &owner;
                    if fcntl(read, 2, 0) < 0 || fcntl(write, 2, 0) < 0 {
                        return Err(io::Error::last_os_error());
                    }
                    Ok(())
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joining_a_pool_does_not_acquire_creator_metadata_or_consume_tokens() {
        for pipe in [false, true] {
            let owner = Jobserver::create(3, pipe).unwrap();
            let joined = Jobserver::join(owner.auth()).unwrap();
            assert_eq!(owner.owned_slots(), Some(3));
            assert_eq!(owner.clone().owned_slots(), Some(3));
            assert_eq!(joined.owned_slots(), None);
            let first = joined.try_acquire().unwrap().unwrap();
            let second = owner.try_acquire().unwrap().unwrap();
            assert!(joined.try_acquire().unwrap().is_none());
            drop(first);
            drop(second);
            let first = owner.try_acquire().unwrap().unwrap();
            let second = joined.try_acquire().unwrap().unwrap();
            assert!(owner.try_acquire().unwrap().is_none());
            drop(first);
            drop(second);
        }
    }

    #[test]
    fn fifo_tokens_are_exhaustible_and_each_permit_returns_its_original_byte() {
        let server = Jobserver::create(3, false).unwrap();
        let first = server.try_acquire().unwrap().unwrap();
        let second = server.try_acquire().unwrap().unwrap();
        assert!(server.try_acquire().unwrap().is_none());
        drop(first);
        let returned = server.try_acquire().unwrap().unwrap();
        assert!(server.try_acquire().unwrap().is_none());
        drop(returned);
        drop(second);
        // Replace one drained token with a distinctive byte supplied externally.
        let one = server.try_acquire().unwrap().unwrap();
        let two = server.try_acquire().unwrap().unwrap();
        let mut writer = &server.0.writer;
        writer.write_all(b"Q").unwrap();
        let special = server.try_acquire().unwrap().unwrap();
        assert_eq!(special.byte, b'Q');
        drop(special);
        assert_eq!(server.try_acquire().unwrap().unwrap().byte, b'Q');
        drop((one, two));
        assert!(server.take_error().is_none());
    }

    #[test]
    fn joined_fifo_does_not_unlink_the_owners_resource() {
        let server = Jobserver::create(2, false).unwrap();
        let path = PathBuf::from(std::ffi::OsStr::from_bytes(&server.auth()[5..]));
        let joined = Jobserver::join(server.auth()).unwrap();
        let permit = joined.try_acquire().unwrap().unwrap();
        assert!(server.try_acquire().unwrap().is_none());
        drop(permit);
        drop(joined);
        assert!(path.exists());
        drop(server);
        assert!(!path.exists());
    }

    #[test]
    fn fifo_cleanup_waits_for_the_last_owner_and_preserves_replacements() {
        let server = Jobserver::create(2, false).unwrap();
        let path = PathBuf::from(std::ffi::OsStr::from_bytes(&server.auth()[5..]));
        let owner = server.clone();
        drop(server);
        assert!(path.exists());
        fs::remove_file(&path).unwrap();
        fs::write(&path, b"replacement").unwrap();
        drop(owner);
        assert_eq!(fs::read(&path).unwrap(), b"replacement");
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn ordinary_files_and_symlinks_are_rejected_without_modification() {
        let server = Jobserver::create(2, false).unwrap();
        let path = PathBuf::from(std::ffi::OsStr::from_bytes(&server.auth()[5..]));
        let link = path.with_extension("link");
        std::os::unix::fs::symlink(&path, &link).unwrap();
        let auth = [b"fifo:".as_slice(), link.as_os_str().as_bytes()].concat();
        assert!(Jobserver::join(&auth).is_err());
        fs::remove_file(link).unwrap();
        fs::remove_file(&path).unwrap();
        fs::write(&path, b"preserve").unwrap();
        assert!(Jobserver::join(server.auth()).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"preserve");
        drop(server);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn pipe_protocol_descriptors_are_inherited_only_by_recursive_children() {
        let server = Jobserver::create(2, true).unwrap();
        let read = server
            .0
            .protocol_reader
            .as_ref()
            .unwrap()
            .as_raw_fd()
            .to_string();
        let mut recursive = Command::new("python3");
        recursive.args([
            "-c",
            "import os,sys; assert os.read(int(sys.argv[1]),1)==b'+'",
            &read,
        ]);
        server.configure_child(&mut recursive, true);
        assert!(recursive.status().unwrap().success());
        assert!(server.try_acquire().unwrap().is_none());
        let mut ordinary = Command::new("python3");
        ordinary.args(["-c", "import os,sys\ntry: os.fstat(int(sys.argv[1]))\nexcept OSError: sys.exit(0)\nsys.exit(1)", &read]);
        server.configure_child(&mut ordinary, false);
        assert!(ordinary.status().unwrap().success());
        let retained = Jobserver::create(2, true).unwrap();
        let descriptor = retained
            .0
            .protocol_reader
            .as_ref()
            .unwrap()
            .as_raw_fd()
            .to_string();
        let mut child = Command::new("python3");
        child.args([
            "-c",
            "import os,sys; assert os.read(int(sys.argv[1]),1)==b'+'",
            &descriptor,
        ]);
        retained.configure_child(&mut child, true);
        drop(retained);
        assert!(child.status().unwrap().success());
    }

    #[test]
    fn invalid_pipe_auth_and_serial_pool_requests_fail() {
        assert!(Jobserver::create(1, false).is_err());
        for auth in [
            b"garbage".as_slice(),
            b"-1,-2",
            b"0,1",
            b"999999,999998",
            b"3,3",
        ] {
            assert!(Jobserver::join(auth).is_err());
        }
        let first = Jobserver::create(2, true).unwrap();
        let second = Jobserver::create(2, true).unwrap();
        let mismatched = format!(
            "{},{}",
            first.0.protocol_reader.as_ref().unwrap().as_raw_fd(),
            second.0.writer.as_raw_fd()
        );
        assert!(Jobserver::join(mismatched.as_bytes()).is_err());
        let first_token = first.try_acquire().unwrap().unwrap();
        let second_token = second.try_acquire().unwrap().unwrap();
        assert_eq!(first_token.byte, b'+');
        assert_eq!(second_token.byte, b'+');
        assert!(first.try_acquire().unwrap().is_none());
        assert!(second.try_acquire().unwrap().is_none());
    }
}
