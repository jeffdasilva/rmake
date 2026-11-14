// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Owned output spools and GNU-compatible POSIX byte-range coordination.
//! Only this adapter deals with temporary descriptors and fcntl lock records.
use std::ffi::{CStr, CString, c_char, c_int, c_short, c_uint};
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

unsafe extern "C" {
    fn mkostemp(template: *mut c_char, flags: c_int) -> c_int;
    fn umask(mask: c_uint) -> c_uint;
    fn fcntl(descriptor: c_int, operation: c_int, ...) -> c_int;
    fn ttyname(descriptor: c_int) -> *mut c_char;
    fn isatty(descriptor: c_int) -> c_int;
    fn rm_register_stdout_close();
    fn rm_set_stdout_level(level: u32);
    fn rm_write_standard(error: c_int, bytes: *const u8, length: usize) -> usize;
}

/// Render into the C finalizer's existing stack buffer. The callback neither
/// accesses an application host nor retains a pointer from the closing stream.
#[unsafe(no_mangle)]
unsafe extern "C" fn rm_format_stdout_error(
    detail: *const c_char,
    buffer: *mut c_char,
    capacity: usize,
) {
    if capacity == 0 {
        return;
    }
    // SAFETY: the C caller supplies either NULL or a live strerror string and
    // a writable stack buffer of the given size. Copy the strerror text before
    // calling gettext, which also uses the current libc locale.
    let detail = (!detail.is_null()).then(|| unsafe { CStr::from_ptr(detail) }.to_bytes().to_vec());
    let message = crate::messages::render_stdout_error(detail.as_deref());
    let length = message.len().min(capacity - 1);
    unsafe {
        std::ptr::copy_nonoverlapping(message.as_ptr(), buffer.cast(), length);
        *buffer.add(length) = 0;
    }
}

fn write_standard_count(error: bool, bytes: &[u8]) -> usize {
    // SAFETY: the readable byte span is valid for the synchronous write.
    unsafe { rm_write_standard(error.into(), bytes.as_ptr(), bytes.len()) }
}

/// Parent output shares libc's standard stream with native extensions.
pub fn write_standard(error: bool, bytes: &[u8]) -> io::Result<()> {
    if write_standard_count(error, bytes) == bytes.len() {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

pub fn set_stdout_level(level: u32) {
    unsafe { rm_set_stdout_level(level) };
}

/// Track actual stream failures separately from formatter errors.
#[derive(Default)]
pub struct Stdout {
    failed: bool,
}

impl Stdout {
    pub fn failed(&self) -> bool {
        self.failed
    }
}

impl Write for Stdout {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let count = write_standard_count(false, bytes);
        self.failed |= count != bytes.len();
        if count == 0 && !bytes.is_empty() {
            Err(io::Error::last_os_error())
        } else {
            Ok(count)
        }
    }

    fn write_all(&mut self, bytes: &[u8]) -> io::Result<()> {
        let result = write_standard(false, bytes);
        self.failed |= result.is_err();
        result
    }

    fn flush(&mut self) -> io::Result<()> {
        let result = crate::native::flush_stream(false);
        self.failed |= result.is_err();
        result
    }
}

fn directory() -> PathBuf {
    crate::temporary::initialize().directory().to_owned()
}

fn temporary_spelling(path: &Path) -> PathBuf {
    directory().join(path.file_name().expect("temporary file name"))
}

#[derive(Debug)]
pub struct TemporaryError {
    pub creation_path: Option<PathBuf>,
    pub error: io::Error,
}

impl From<io::Error> for TemporaryError {
    fn from(error: io::Error) -> Self {
        Self {
            creation_path: None,
            error,
        }
    }
}

struct Temporary {
    file: File,
    named: Option<(PathBuf, u64, u64)>,
}

impl Temporary {
    fn new(named: bool) -> Result<Self, TemporaryError> {
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
            let error = io::Error::last_os_error();
            // GNU leaves the temporary-file creation mask active on failure.
            // SAFETY: umask updates process permissions and takes no pointers.
            unsafe { umask(0o077) };
            template.pop();
            let path = Path::new(std::ffi::OsStr::from_bytes(&template));
            return Err(TemporaryError {
                creation_path: Some(temporary_spelling(path)),
                error,
            });
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
    pub fn create() -> Result<Self, TemporaryError> {
        let temp = Temporary::new(true)?;
        let path = &temp.named.as_ref().expect("named mutex").0;
        // The owner retains an absolute cleanup path. Descendants receive
        // the selected spelling and resolve it in their own directories.
        let spelling = temporary_spelling(path);
        let authentication = [b"fnm:".as_slice(), spelling.as_os_str().as_bytes()].concat();
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

    /// Keep responsibility for the original pathname when a valid decode
    /// reopens that same authentication. The replaced file is closed now;
    /// cleanup still checks the original pathname identity before unlinking.
    pub fn preserve_named_path_from(&mut self, previous: &mut Self) {
        if self.authentication == previous.authentication
            && let Some(owner) = previous.owner.as_mut()
            && let Some((_, device, inode)) = &owner.named
            && self
                .file
                .as_ref()
                .and_then(|file| file.metadata().ok())
                .is_some_and(|metadata| {
                    metadata.is_file() && metadata.dev() == *device && metadata.ino() == *inode
                })
            && let Some(file) = self.file.take()
        {
            owner.file = file;
            self.owner = previous.owner.take();
        }
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
    pub fn new() -> Result<Self, TemporaryError> {
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
            return Err(io::Error::from_raw_os_error(9).into());
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
            write_standard(false, bytes)?;
        }
        Ok(())
    }

    pub fn configure_child(&self, child: &mut Command) -> io::Result<()> {
        let stdout = self.stream(false).map(File::try_clone).transpose()?;
        let stderr = self.stream(true).map(File::try_clone).transpose()?;
        if let Some(file) = stdout {
            child.stdout(Stdio::from(file));
        }
        if let Some(file) = stderr {
            child.stderr(Stdio::from(file));
        }
        Ok(())
    }

    pub(crate) fn child_output(&self) -> [Option<BorrowedFd<'_>>; 2] {
        [
            self.stream(false).map(AsFd::as_fd),
            self.stream(true).map(AsFd::as_fd),
        ]
    }

    pub fn child_stderr(&self) -> Option<BorrowedFd<'_>> {
        self.stream(true).map(AsFd::as_fd)
    }

    pub fn dump(
        &self,
        coordination: Option<&Coordination>,
        frame: &mut dyn FnMut(bool),
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
        frame(true);
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
                if let Err(error) = write_standard(error, &bytes[..count]) {
                    report("fwrite()", &error);
                    break;
                }
                // A delayed flush failure is reported by the stdout owner
                // at exit. Only failure of the fwrite itself is reported here.
                let _ = crate::native::flush_stream(error);
            }
            let _ = file.seek(SeekFrom::Start(0));
            while file
                .set_len(0)
                .is_err_and(|error| error.kind() == io::ErrorKind::Interrupted)
            {}
        }
        frame(false);
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
    unsafe { rm_register_stdout_close() };
    for descriptor in [1, 2] {
        let flags = unsafe { fcntl(descriptor, 3) };
        if flags >= 0 {
            unsafe { fcntl(descriptor, 4, flags | 0x400) };
        }
    }
}

#[cfg(test)]
mod frame_tests {
    use super::*;

    fn locked_elsewhere(authentication: &[u8]) -> bool {
        let path = authentication.strip_prefix(b"fnm:").unwrap();
        let status = Command::new("python3")
            .arg("-c")
            .arg("import fcntl,sys\nf=open(sys.argv[1],'rb+')\ntry: fcntl.lockf(f,fcntl.LOCK_EX|fcntl.LOCK_NB,1)\nexcept BlockingIOError: sys.exit(0)\nsys.exit(1)")
            .arg(std::ffi::OsStr::from_bytes(path))
            .status().unwrap();
        status.success()
    }

    #[test]
    fn reopened_named_lock_is_available_to_peers_until_owner_exit() {
        let mut owner = Coordination::create().unwrap();
        let auth = owner.authentication().to_vec();
        let path = PathBuf::from(std::ffi::OsStr::from_bytes(
            auth.strip_prefix(b"fnm:").unwrap(),
        ));
        let mut reopened = Coordination::join(&auth).unwrap().unwrap();
        reopened.preserve_named_path_from(&mut owner);
        drop(owner);
        assert!(path.is_file());
        reopened.lock(true).unwrap();
        assert!(locked_elsewhere(&auth));
        reopened.lock(false).unwrap();
        drop(reopened);
        assert!(!path.exists());
    }

    #[test]
    fn reopening_an_externally_replaced_lock_does_not_claim_the_new_inode() {
        let mut owner = Coordination::create().unwrap();
        let auth = owner.authentication().to_vec();
        let path = PathBuf::from(std::ffi::OsStr::from_bytes(
            auth.strip_prefix(b"fnm:").unwrap(),
        ));
        let displaced = path.with_extension("prior");
        fs::rename(&path, &displaced).unwrap();
        fs::write(&path, b"replacement").unwrap();
        let mut reopened = Coordination::join(&auth).unwrap().unwrap();
        reopened.preserve_named_path_from(&mut owner);
        drop(owner);
        drop(reopened);
        assert_eq!(fs::read(&path).unwrap(), b"replacement");
        fs::remove_file(path).unwrap();
        fs::remove_file(displaced).unwrap();
    }

    #[test]
    fn nonempty_spool_frames_share_the_payload_lock_and_empty_dumps_stay_silent() {
        let coordination = Coordination::create().unwrap();
        let spool = Spool::new().unwrap();
        let mut frames = Vec::new();
        spool
            .dump(
                Some(&coordination),
                &mut |entering| frames.push(entering),
                &mut |op, error| panic!("{op}: {error}"),
            )
            .unwrap();
        assert!(frames.is_empty());
        spool.write(false, b"spool frame unit\n").unwrap();
        spool
            .dump(
                Some(&coordination),
                &mut |entering| {
                    assert!(locked_elsewhere(coordination.authentication()));
                    frames.push(entering);
                },
                &mut |op, error| panic!("{op}: {error}"),
            )
            .unwrap();
        assert_eq!(frames, [true, false]);
        assert!(!locked_elsewhere(coordination.authentication()));
        spool
            .dump(
                Some(&coordination),
                &mut |entering| frames.push(entering),
                &mut |op, error| panic!("{op}: {error}"),
            )
            .unwrap();
        assert_eq!(frames, [true, false]);
    }
}
