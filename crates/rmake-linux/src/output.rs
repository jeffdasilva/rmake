// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Owned output spools and GNU-compatible POSIX byte-range coordination.
//! Only this adapter deals with temporary descriptors and fcntl lock records.
use std::ffi::{CString, c_char, c_int, c_short};
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};

unsafe extern "C" {
    fn mkostemp(template: *mut c_char, flags: c_int) -> c_int;
    fn fcntl(descriptor: c_int, operation: c_int, ...) -> c_int;
    fn access(path: *const c_char, mode: c_int) -> c_int;
    fn ttyname(descriptor: c_int) -> *mut c_char;
    fn isatty(descriptor: c_int) -> c_int;
}

fn directory() -> PathBuf {
    let mut choices = Vec::new();
    if let Some(value) = std::env::var_os("TMPDIR") {
        choices.push(PathBuf::from(value));
    }
    choices.push(PathBuf::from("/tmp"));
    choices
        .into_iter()
        .find(|path| {
            let Ok(name) = CString::new(path.as_os_str().as_bytes()) else {
                return false;
            };
            path.is_dir() && unsafe { access(name.as_ptr(), 3) } == 0
        })
        .unwrap_or_else(|| PathBuf::from("/tmp"))
}

struct Temporary {
    file: File,
    named: Option<(PathBuf, u64, u64)>,
}

impl Temporary {
    fn new(named: bool) -> io::Result<Self> {
        let directory = directory();
        let directory = if directory.is_absolute() {
            directory
        } else {
            std::env::current_dir()?.join(directory)
        };
        let mut template = CString::new(directory.join("GmXXXXXX").as_os_str().as_bytes())
            .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?
            .into_bytes_with_nul();
        // mkostemp initializes a fresh 0600 file and replaces only the six X's
        // in our uniquely owned, terminated template. CLOEXEC/APPEND are atomic.
        let descriptor = unsafe { mkostemp(template.as_mut_ptr().cast(), 0x80000 | 0x400) };
        if descriptor < 0 {
            return Err(io::Error::last_os_error());
        }
        let file = crate::process::private_descriptor(unsafe { File::from_raw_fd(descriptor) })?;
        template.pop();
        let path = PathBuf::from(std::ffi::OsString::from_vec(template));
        let identity = file.metadata()?;
        let mut temp = Self {
            file,
            named: Some((path, identity.dev(), identity.ino())),
        };
        if !named {
            temp.unlink()?;
        }
        Ok(temp)
    }

    fn unlink(&mut self) -> io::Result<()> {
        if let Some((path, device, inode)) = &self.named {
            if fs::symlink_metadata(path)
                .is_ok_and(|m| m.is_file() && m.dev() == *device && m.ino() == *inode)
            {
                fs::remove_file(path)?;
            }
            self.named = None;
        }
        Ok(())
    }
}

impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = self.unlink();
    }
}

#[repr(C)]
struct LockRecord {
    kind: c_short,
    whence: c_short,
    start: i64,
    length: i64,
    pid: c_int,
}

pub struct Coordination {
    file: Option<File>,
    owner: Option<Temporary>,
    authentication: Vec<u8>,
}

impl Coordination {
    pub fn create() -> io::Result<Self> {
        let temp = Temporary::new(true)?;
        let path = &temp.named.as_ref().expect("named mutex").0;
        let authentication = [b"fnm:".as_slice(), path.as_os_str().as_bytes()].concat();
        Ok(Self {
            file: None,
            owner: Some(temp),
            authentication,
        })
    }

    pub fn join(authentication: &[u8]) -> io::Result<Option<Self>> {
        let Some(path) = authentication.strip_prefix(b"fnm:") else {
            return Ok(None);
        };
        let file = OpenOptions::new()
            .write(true)
            .open(std::ffi::OsStr::from_bytes(path))?;
        let file = crate::process::private_descriptor(file)?;
        Ok(Some(Self {
            file: Some(file),
            owner: None,
            authentication: authentication.to_vec(),
        }))
    }

    pub fn authentication(&self) -> &[u8] {
        &self.authentication
    }

    pub fn clear(&mut self) {
        if let Some(owner) = &mut self.owner {
            let _ = owner.unlink();
        }
    }

    fn lock(&self, acquire: bool) -> io::Result<()> {
        // Initialized integer-only POSIX flock record; the whole record lives
        // through fcntl. The lock covers byte zero, matching GNU's fnm protocol.
        let mut lock: LockRecord = unsafe { std::mem::zeroed() };
        lock.kind = if acquire { 1 } else { 2 };
        lock.length = 1;
        // Linux's large-file lock command differs on 32-bit ABIs.
        let command = if std::mem::size_of::<std::ffi::c_long>() == 8 {
            7
        } else {
            14
        };
        if unsafe {
            fcntl(
                self.file
                    .as_ref()
                    .unwrap_or_else(|| &self.owner.as_ref().expect("owned mutex").file)
                    .as_raw_fd(),
                command,
                &lock,
            )
        } < 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}

fn stream_metadata(descriptor: c_int) -> Option<Metadata> {
    // Borrow standard streams via an owned CLOEXEC duplicate; never construct
    // a File owning the caller's descriptor or depend on procfs for metadata.
    let duplicate = unsafe { fcntl(descriptor, 1030, 3) };
    if duplicate < 0 {
        return None;
    }
    let file = unsafe { File::from_raw_fd(duplicate) };
    file.metadata().ok()
}

pub struct Spool {
    out: Option<Temporary>,
    err: Option<Temporary>,
    combined: bool,
}

impl Spool {
    pub fn new() -> io::Result<Self> {
        let out = stream_metadata(1);
        let err = stream_metadata(2);
        let combined = out
            .as_ref()
            .zip(err.as_ref())
            .is_some_and(|(a, b)| a.dev() == b.dev() && a.ino() == b.ino());
        let stdout = if out.is_some() {
            Some(Temporary::new(false)?)
        } else {
            None
        };
        let stderr = if err.is_some() && !combined {
            Some(Temporary::new(false)?)
        } else {
            None
        };
        if stdout.is_none() && stderr.is_none() {
            return Err(io::Error::from_raw_os_error(9));
        }
        Ok(Self {
            out: stdout,
            err: stderr,
            combined,
        })
    }

    fn stream(&self, error: bool) -> Option<&File> {
        if error && !self.combined {
            self.err.as_ref()
        } else {
            self.out.as_ref()
        }
        .map(|temp| &temp.file)
    }

    pub fn write(&self, error: bool, bytes: &[u8]) -> io::Result<()> {
        if let Some(mut file) = self.stream(error) {
            file.write_all(bytes)?;
        } else if error {
            io::stderr().lock().write_all(bytes)?;
        } else {
            io::stdout().lock().write_all(bytes)?;
        }
        Ok(())
    }

    pub fn configure_child(&self, child: &mut Command) -> io::Result<()> {
        if let Some(file) = self.stream(false) {
            child.stdout(Stdio::from(file.try_clone()?));
        }
        if let Some(file) = self.stream(true) {
            child.stderr(Stdio::from(file.try_clone()?));
        }
        Ok(())
    }

    pub fn configure_stderr(&self, child: &mut Command) -> io::Result<()> {
        if let Some(file) = self.stream(true) {
            child.stderr(Stdio::from(file.try_clone()?));
        }
        Ok(())
    }

    pub fn dump(
        &self,
        coordination: Option<&Coordination>,
        report: &mut dyn FnMut(&str, &io::Error),
    ) -> io::Result<()> {
        let nonempty = [&self.out, &self.err]
            .into_iter()
            .flatten()
            .any(|temp| temp.file.metadata().is_ok_and(|m| m.len() != 0));
        if !nonempty {
            return Ok(());
        }
        if let Some(lock) = coordination {
            lock.lock(true)?;
        }
        for (error, temp) in [(false, &self.out), (true, &self.err)] {
            let Some(temp) = temp else {
                continue;
            };
            let mut file = &temp.file;
            if let Err(error) = file.seek(SeekFrom::Start(0)) {
                report("lseek()", &error);
                continue;
            }
            let mut bytes = [0; 8192];
            loop {
                let count = match file.read(&mut bytes) {
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    Err(error) => {
                        report("fread()", &error);
                        break;
                    }
                    Ok(count) => count,
                };
                if count == 0 {
                    break;
                }
                let result = if error {
                    let mut out = io::stderr().lock();
                    out.write_all(&bytes[..count]).and_then(|_| out.flush())
                } else {
                    let mut out = io::stdout().lock();
                    out.write_all(&bytes[..count]).and_then(|_| out.flush())
                };
                if let Err(error) = result {
                    report("fwrite()", &error);
                    break;
                }
            }
            let _ = file.seek(SeekFrom::Start(0));
            while file
                .set_len(0)
                .is_err_and(|error| error.kind() == io::ErrorKind::Interrupted)
            {}
        }
        if let Some(lock) = coordination
            && let Err(error) = lock.lock(false)
        {
            report("fcntl()", &error);
        }
        Ok(())
    }
}

/// Snapshot tty spellings before any output diversion. An unavailable ttyname
/// still exposes GNU's true marker for an actual terminal.
pub fn terminal(descriptor: c_int) -> Option<Vec<u8>> {
    if unsafe { isatty(descriptor) } == 0 {
        return None;
    }
    let name = unsafe { ttyname(descriptor) };
    if name.is_null() {
        Some(b"true".to_vec())
    } else {
        Some(
            unsafe { std::ffi::CStr::from_ptr(name) }
                .to_bytes()
                .to_vec(),
        )
    }
}

/// Make appends to inherited regular output files, including explicit recipe
/// seeks. Preserve all other descriptor flags, and tolerate closed streams.
pub fn initialize() {
    for descriptor in [1, 2] {
        let flags = unsafe { fcntl(descriptor, 3) };
        if flags >= 0 {
            unsafe { fcntl(descriptor, 4, flags | 0x400) };
        }
    }
}
