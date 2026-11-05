// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Linux FIFO/pipe jobserver effects. The interpreter retains each acquired
//! byte for one complete recipe. Dropping its permit returns that same byte.

use std::ffi::{CString, c_char, c_int, c_uint, c_void};
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
    fn rmake_restart_pipe_private(pipe: *const c_int);
    fn rmake_pipe_identity(descriptor: c_int, reader: c_int, identity: *mut u64) -> c_int;
    fn read(descriptor: c_int, bytes: *mut c_void, length: usize) -> isize;
    fn write(descriptor: c_int, bytes: *const c_void, length: usize) -> isize;
}

fn descriptor_control(fd: c_int, operation: c_int, flags: c_int) -> io::Result<c_int> {
    loop {
        let value = unsafe { fcntl(fd, operation, flags) };
        if value >= 0 {
            return Ok(value);
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error);
        }
    }
}

fn pipe_identity(fd: c_int, reader: bool) -> io::Result<[u64; 2]> {
    let mut identity = [0; 2];
    // SAFETY: the Linux helper initializes both words on success. Passing
    // a numeric descriptor does not assert Rust ownership of that slot.
    match unsafe { rmake_pipe_identity(fd, reader.into(), identity.as_mut_ptr()) } {
        0 => Ok(identity),
        error => Err(io::Error::from_raw_os_error(error)),
    }
}

enum Endpoint {
    Owned(File),
    Inherited {
        descriptor: c_int,
        reader: bool,
        identity: [u64; 2],
    },
}

impl Endpoint {
    fn checked_number(&self) -> io::Result<c_int> {
        match self {
            Self::Owned(file) => Ok(file.as_raw_fd()),
            Self::Inherited {
                descriptor,
                reader,
                identity,
            } => {
                // Incoming slots can be synchronously rebound by native code.
                // Validate the retained object and direction before operating.
                // Concurrent native descriptor rebinding is outside the same
                // process-owner contract as ordinary native descriptor access.
                if pipe_identity(*descriptor, *reader)? == *identity {
                    Ok(*descriptor)
                } else {
                    Err(io::Error::from_raw_os_error(9))
                }
            }
        }
    }
}

impl Read for &Endpoint {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        if let Endpoint::Owned(file) = *self {
            let mut file = file;
            return file.read(bytes);
        }
        let fd = self.checked_number()?;
        if descriptor_control(fd, 3, 0)? & NONBLOCK == 0 {
            return Err(io::Error::from_raw_os_error(22));
        }
        // SAFETY: bytes is writable for the advertised length. No File or
        // BorrowedFd is constructed for an unowned inherited slot.
        let size = unsafe { read(fd, bytes.as_mut_ptr().cast(), bytes.len()) };
        if size < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(size as usize)
        }
    }
}

impl Write for &Endpoint {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if let Endpoint::Owned(file) = *self {
            let mut file = file;
            return file.write(bytes);
        }
        let fd = self.checked_number()?;
        // SAFETY: bytes remains live and readable for the syscall.
        let size = unsafe { write(fd, bytes.as_ptr().cast(), bytes.len()) };
        if size < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(size as usize)
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct Inner {
    reader: Endpoint,
    writer: Endpoint,
    protocol_reader: Option<File>,
    inherited_pipe: Option<[c_int; 2]>,
    compact_pipe: bool,
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

#[derive(Debug)]
pub struct CreationError {
    pub operation: &'static str,
    pub error: io::Error,
}

impl From<io::Error> for CreationError {
    fn from(error: io::Error) -> Self {
        Self {
            operation: "jobserver",
            error,
        }
    }
}

/// The invocation's authentication names the incoming descriptors. The
/// privately owned endpoints keep their pipe identity live during self-exec.
pub(crate) struct RestartPipe<'a> {
    incoming: [c_int; 2],
    owned: [BorrowedFd<'a>; 2],
}

impl RestartPipe<'_> {
    pub(crate) fn descriptors(&self) -> [c_int; 4] {
        [
            self.incoming[0],
            self.incoming[1],
            self.owned[0].as_raw_fd(),
            self.owned[1].as_raw_fd(),
        ]
    }
}

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

    /// Ordinary scheduler completion returns the protocol byte. A dropped
    /// permit still recovers its original byte on an interrupted path.
    pub fn release_completed(mut self) {
        self.byte = b'+';
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
    pub fn create(jobs: usize, pipe: bool) -> Result<Self, CreationError> {
        if jobs < 2 {
            return Err(io::Error::from(io::ErrorKind::InvalidInput).into());
        }
        let mut server = if pipe {
            let mut descriptors = [-1; 2];
            // A successful pipe2 returns two new, uniquely owned descriptors.
            if unsafe { pipe2(descriptors.as_mut_ptr(), CLOEXEC) } < 0 {
                return Err(CreationError {
                    operation: "creating jobs pipe",
                    error: io::Error::last_os_error(),
                });
            }
            let reader = unsafe { File::from_raw_fd(descriptors[0]) };
            let writer = unsafe { File::from_raw_fd(descriptors[1]) };
            if crate::signals::compact_pipe_budget() {
                let read = reader.as_raw_fd();
                let write = writer.as_raw_fd();
                descriptor_control(read, 4, descriptor_control(read, 3, 0)? | NONBLOCK)?;
                Self::compact(
                    Endpoint::Owned(reader),
                    Endpoint::Owned(writer),
                    format!("{read},{write}").into_bytes(),
                )
            } else {
                Self::from_pipe(reader, writer)?
            }
        } else {
            Self::create_fifo()?
        };
        // A separate nonblocking description avoids changing flags shared with
        // recursive children, and cannot hang if the requested pool is too big.
        let tokens = vec![b'+'; (jobs - 1).min(1_048_576)];
        if tokens.len() != jobs - 1 {
            return Err(io::Error::from(io::ErrorKind::InvalidInput).into());
        }
        let writer = server.0.writer.checked_number()?;
        if server.0.compact_pipe {
            // This newly owned writer has not been published to any child.
            // Seed with nonblocking writes and restore the wire status before
            // publishing it. No transient descriptor is required.
            let flags = descriptor_control(writer, 3, 0)?;
            descriptor_control(writer, 4, flags | NONBLOCK)?;
            let seeded = (&server.0.writer).write_all(&tokens);
            let restored = descriptor_control(writer, 4, flags);
            seeded?;
            restored?;
        } else if pipe {
            reopen(writer, false, true)?.write_all(&tokens)?;
        } else {
            // FIFO endpoints already share a nonblocking description. Seed
            // the unpublished FIFO without spending a third descriptor.
            (&server.0.writer).write_all(&tokens)?;
        }
        Arc::get_mut(&mut server.0)
            .expect("unshared new jobserver")
            .owned_slots = Some(jobs);
        Ok(server)
    }

    fn compact(reader: Endpoint, writer: Endpoint, auth: Vec<u8>) -> Self {
        Self(Arc::new(Inner {
            reader,
            writer,
            auth,
            protocol_reader: None,
            inherited_pipe: None,
            compact_pipe: true,
            owned_slots: None,
            created: None,
            error: Mutex::new(None),
        }))
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
            reader: Endpoint::Owned(reader),
            writer: Endpoint::Owned(writer),
            protocol_reader: None,
            inherited_pipe: None,
            compact_pipe: false,
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
            reader: Endpoint::Owned(reader),
            writer: Endpoint::Owned(writer),
            protocol_reader: Some(protocol_reader),
            inherited_pipe: None,
            compact_pipe: false,
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
        if crate::signals::compact_pipe_budget() {
            return Self::join_compact(read, write);
        }
        let reader = reopen(read, true, false)?;
        let writer = reopen(write, false, false)?;
        let mut server = Self::from_pipe(reader, writer)?;
        // FD flags belong to this process's descriptors, not the parent's open
        // descriptions. Ordinary commands must not inherit the incoming pair.
        for descriptor in [read, write] {
            if unsafe { fcntl(descriptor, 2, 1) } < 0 {
                return Err(io::Error::last_os_error());
            }
        }
        Arc::get_mut(&mut server.0)
            .expect("unshared joined jobserver")
            .inherited_pipe = Some([read, write]);
        Ok(server)
    }

    fn join_compact(read: c_int, write: c_int) -> io::Result<Self> {
        let identity = pipe_identity(read, true)?;
        if pipe_identity(write, false)? != identity {
            return Err(io::Error::from_raw_os_error(9));
        }
        for descriptor in [read, write] {
            let flags = descriptor_control(descriptor, 1, 0)?;
            descriptor_control(descriptor, 2, flags | 1)?;
        }
        // F_SETFL affects the inherited open-file description, so the peer
        // observes the same nonblocking read status as a GNU join.
        descriptor_control(read, 4, descriptor_control(read, 3, 0)? | NONBLOCK)?;
        Ok(Self::compact(
            Endpoint::Inherited {
                descriptor: read,
                reader: true,
                identity,
            },
            Endpoint::Inherited {
                descriptor: write,
                reader: false,
                identity,
            },
            format!("{read},{write}").into_bytes(),
        ))
    }

    pub fn readiness(&self) -> io::Result<c_int> {
        self.0.reader.checked_number()
    }

    pub fn uses_compact_pipe(&self) -> bool {
        self.0.compact_pipe
    }

    pub(crate) fn compact_wire(&self) -> io::Result<Option<[c_int; 2]>> {
        if self.uses_compact_pipe() {
            Ok(Some([
                self.0.reader.checked_number()?,
                self.0.writer.checked_number()?,
            ]))
        } else {
            Ok(None)
        }
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

    pub fn configure_child(&self, command: &mut Command, recursive: bool) -> io::Result<()> {
        if !recursive {
            return Ok(());
        }
        let wire = if let Some(wire) = self.compact_wire()? {
            Some(wire)
        } else if let Some(reader) = &self.0.protocol_reader {
            Some([reader.as_raw_fd(), self.0.writer.checked_number()?])
        } else {
            None
        };
        if let Some([read, write]) = wire {
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
        Ok(())
    }

    pub(crate) fn recursive_exec_pipe(&self) -> Option<RestartPipe<'_>> {
        let Endpoint::Owned(writer) = &self.0.writer else {
            return None;
        };
        Some(RestartPipe {
            incoming: self.0.inherited_pipe?,
            owned: [self.0.protocol_reader.as_ref()?.as_fd(), writer.as_fd()],
        })
    }

    /// A compact join never owns the advertised numbers. Keep their captured
    /// pipe identity for a final C check immediately before process replacement.
    pub(crate) fn compact_restart_pipe(&self) -> io::Result<Option<([c_int; 4], [u64; 2])>> {
        let Endpoint::Inherited { identity, .. } = &self.0.reader else {
            return Ok(None);
        };
        Ok(self
            .compact_wire()?
            .map(|[read, write]| ([read, write, -1, -1], *identity)))
    }

    /// A failed recursive self-exec returns with its protocol pair exposed.
    /// Restore this process's private descriptor flags before source cleanup.
    pub fn finish_recursive_exec(&self) {
        if self.uses_compact_pipe() {
            for endpoint in [&self.0.reader, &self.0.writer] {
                if let Ok(descriptor) = endpoint.checked_number()
                    && let Ok(flags) = descriptor_control(descriptor, 1, 0)
                {
                    let _ = descriptor_control(descriptor, 2, flags | 1);
                }
            }
            return;
        }
        if let Some(pipe) = self.recursive_exec_pipe() {
            // SAFETY: the owned endpoints remain live and C validates the
            // incoming numbers before restoring their close-on-exec flags.
            unsafe { rmake_restart_pipe_private(pipe.descriptors().as_ptr()) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_join_preserves_the_owner_and_rejects_a_rebound_pipe() {
        let owner = Jobserver::create(2, true).unwrap();
        let read = owner.0.protocol_reader.as_ref().unwrap().as_raw_fd();
        let write = owner.0.writer.checked_number().unwrap();
        let joined = Jobserver::join_compact(read, write).unwrap();
        assert_eq!(descriptor_control(read, 3, 0).unwrap() & NONBLOCK, NONBLOCK);
        assert_eq!(descriptor_control(write, 3, 0).unwrap() & NONBLOCK, 0);
        let permit = joined.try_acquire().unwrap().unwrap();
        assert!(owner.try_acquire().unwrap().is_none());
        drop((permit, joined));
        assert_eq!(owner.try_acquire().unwrap().unwrap().byte, b'+');

        let foreign = Jobserver::create(2, true).unwrap();
        let foreign_read = foreign.0.protocol_reader.as_ref().unwrap().as_raw_fd();
        let identity = pipe_identity(read, true).unwrap();
        let replacement = Endpoint::Inherited {
            descriptor: foreign_read,
            reader: true,
            identity,
        };
        assert_eq!(
            (&replacement).read(&mut [0]).unwrap_err().raw_os_error(),
            Some(9)
        );
        assert_eq!(foreign.try_acquire().unwrap().unwrap().byte, b'+');
        assert_eq!(descriptor_control(foreign_read, 1, 0).unwrap(), 1);
    }

    #[test]
    fn transferring_a_paid_lease_to_the_implicit_slot_returns_exactly_one_original_byte() {
        for pipe in [false, true] {
            let server = Jobserver::create(2, pipe).unwrap();
            let mut slot = server.try_acquire().unwrap().unwrap();
            let original = slot.byte;
            assert!(server.try_acquire().unwrap().is_none());
            let paid = std::mem::replace(&mut slot, Permit::implicit());
            drop(paid);
            let borrowed = server.try_acquire().unwrap().unwrap();
            assert_eq!(borrowed.byte, original);
            drop(slot);
            assert!(server.try_acquire().unwrap().is_none());
            drop(borrowed);
            let returned = server.try_acquire().unwrap().unwrap();
            assert_eq!(returned.byte, original);
            assert!(server.try_acquire().unwrap().is_none());
        }
    }

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
        server.configure_child(&mut recursive, true).unwrap();
        assert!(recursive.status().unwrap().success());
        assert!(server.try_acquire().unwrap().is_none());
        let mut ordinary = Command::new("python3");
        ordinary.args(["-c", "import os,sys\ntry: os.fstat(int(sys.argv[1]))\nexcept OSError: sys.exit(0)\nsys.exit(1)", &read]);
        server.configure_child(&mut ordinary, false).unwrap();
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
        retained.configure_child(&mut child, true).unwrap();
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
            second.0.writer.checked_number().unwrap()
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
