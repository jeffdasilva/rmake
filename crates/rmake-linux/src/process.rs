// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Owned Linux process effects. No descriptors or libc pointers escape unowned.

use std::ffi::{CStr, CString, c_char, c_int};
use std::fs::File;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::{AtomicU8, Ordering};

const STANDARD_STREAMS: [c_int; 3] = [0, 1, 2];
const STDIN_MASK: u8 = 0b001;
const OUTPUT_MASK: u8 = 0b110;
const ALL_STANDARD_STREAMS: u8 = STDIN_MASK | OUTPUT_MASK;
const F_GETFD: c_int = 1;
const F_SETFD: c_int = 2;
const FD_CLOEXEC: c_int = 1;
const EBADF: c_int = 9;
static STARTUP_CLOSED_STREAMS: AtomicU8 = AtomicU8::new(0);
static CLI_UNAVAILABLE_OUTPUTS: AtomicU8 = AtomicU8::new(0);

unsafe extern "C" {
    fn strsignal(number: c_int) -> *mut c_char;
    fn strtod(text: *const c_char, end: *mut *mut c_char) -> f64;
    fn snprintf(buffer: *mut c_char, length: usize, format: *const c_char, ...) -> c_int;
    fn __errno_location() -> *mut c_int;
    fn getloadavg(values: *mut f64, count: c_int) -> c_int;
    fn fcntl(descriptor: c_int, operation: c_int, ...) -> c_int;
    fn pipe2(descriptors: *mut c_int, flags: c_int) -> c_int;
    fn _exit(status: c_int) -> !;
    fn euidaccess(path: *const c_char, mode: c_int) -> c_int;
}

pub(crate) fn remember_inherited_streams() {
    // SAFETY: F_GETFD only observes the descriptor. Restore the startup
    // thread's errno, which also makes this constructor safe for library users.
    unsafe {
        let errno = __errno_location();
        let saved = *errno;
        let mut closed = 0;
        for descriptor in STANDARD_STREAMS {
            if fcntl(descriptor, F_GETFD) < 0 && *errno == EBADF {
                closed |= 1 << descriptor;
            }
        }
        STARTUP_CLOSED_STREAMS.store(closed, Ordering::Relaxed);
        *errno = saved;
    }
}

/// The CLI calls this before its first native operation after Rust startup.
/// Rust fills missing standard streams with placeholders. Keep those slots
/// occupied in the parent, and let exec close them unless a child redirects.
pub fn preserve_closed_startup_streams() {
    // GNU checks again when none of the three streams was initially open.
    // That case keeps the existing output path and inherited stdin handling.
    let closed = match STARTUP_CLOSED_STREAMS.load(Ordering::Relaxed) {
        ALL_STANDARD_STREAMS => STDIN_MASK,
        streams => streams,
    };
    if closed == 0 {
        return;
    }
    // SAFETY: the entry observation and immediate CLI call bracket Rust's
    // repair. Only replacement descriptor flags are changed.
    let mut marked = 0;
    unsafe {
        let errno = __errno_location();
        let saved = *errno;
        for descriptor in STANDARD_STREAMS {
            let mask = 1 << descriptor;
            if closed & mask != 0 {
                let flags = fcntl(descriptor, F_GETFD);
                if flags >= 0 && fcntl(descriptor, F_SETFD, flags | FD_CLOEXEC) >= 0 {
                    marked |= mask;
                }
            }
        }
        *errno = saved;
    }
    CLI_UNAVAILABLE_OUTPUTS.store(marked & OUTPUT_MASK, Ordering::Relaxed);
}

pub(crate) fn startup_output_was_closed(descriptor: c_int) -> bool {
    CLI_UNAVAILABLE_OUTPUTS.load(Ordering::Relaxed) & (1 << descriptor) != 0
}

pub fn startup_outputs_were_closed() -> bool {
    CLI_UNAVAILABLE_OUTPUTS.load(Ordering::Relaxed) == OUTPUT_MASK
}

/// Probe an executable candidate using the caller's effective credentials.
pub fn executable(path: &Path) -> io::Result<()> {
    const X_OK: c_int = 1;
    let name = CString::new(path.as_os_str().as_bytes())?;
    // SAFETY: the C string lives through the read-only access check.
    if unsafe { euidaccess(name.as_ptr(), X_OK) } < 0 {
        return Err(io::Error::last_os_error());
    }
    if path.metadata()?.is_dir() {
        return Err(io::Error::from_raw_os_error(13));
    }
    Ok(())
}

/// Recheck the launched path after a child exits with status 127.
pub fn recheck_executable(path: &Path) -> io::Result<()> {
    const OWNER_EXECUTE: u32 = 0o100;
    const EACCES: c_int = 13;
    const ENOEXEC: c_int = 8;

    let metadata = crate::filesystem::retry_interrupted(|| path.metadata())?;
    if metadata.is_dir() || metadata.mode() & OWNER_EXECUTE == 0 {
        return Err(io::Error::from_raw_os_error(EACCES));
    }
    if metadata.len() == 0 {
        return Err(io::Error::from_raw_os_error(ENOEXEC));
    }
    Ok(())
}

/// Terminate after an unsuccessful self-exec, without running C/native
/// finalizers or Rust owners. The caller flushes streams and retires published
/// temporary-source ownership before selecting this process boundary.
pub fn exit_immediately(status: i32) -> ! {
    // SAFETY: _exit has no pointer arguments and never returns to Rust frames.
    unsafe { _exit(status) }
}

/// Parallel commands without the input lease receive a pipe with no writers.
/// Unlike /dev/null this preserves the observable FIFO descriptor type.
pub fn closed_stdin() -> io::Result<Stdio> {
    closed_stdin_file().map(Stdio::from)
}

pub fn closed_stdin_file() -> io::Result<File> {
    let mut descriptors = [-1; 2];
    // pipe2 initializes both descriptors on success. CLOEXEC is atomic even
    // when other recipe workers spawn concurrently with this call.
    if unsafe { pipe2(descriptors.as_mut_ptr(), 0x80000) } < 0 {
        return Err(io::Error::last_os_error());
    }
    let reader = unsafe { File::from_raw_fd(descriptors[0]) };
    let writer = unsafe { File::from_raw_fd(descriptors[1]) };
    drop(writer);
    private_descriptor(reader)
}

/// GNU's floating option accepts the C numeric prefix, including trailing text.
pub fn load_number(bytes: &[u8]) -> f64 {
    let text = CString::new(bytes).expect("argument cannot contain NUL");
    // C locale is unchanged by rmake; strtod reads a live terminated string.
    unsafe { strtod(text.as_ptr(), std::ptr::null_mut()) }
}

pub fn load_spelling(value: f64) -> String {
    let mut bytes = [0 as c_char; 64];
    // %g writes at most 24 bytes for an IEEE double with default precision.
    // snprintf additionally enforces the initialized buffer's actual capacity.
    unsafe { snprintf(bytes.as_mut_ptr(), bytes.len(), c"%g".as_ptr(), value) };
    unsafe { CStr::from_ptr(bytes.as_ptr()) }
        .to_string_lossy()
        .into_owned()
}

/// Format the fixed GNU load-debug decimal in the active numeric locale.
pub fn load_debug_spelling(value: f64) -> Vec<u8> {
    let mut bytes = vec![0; 64];
    loop {
        // SAFETY: the buffer is initialized and uniquely writable. The
        // compile-time format has exactly one matching double argument.
        let written = unsafe {
            snprintf(
                bytes.as_mut_ptr().cast(),
                bytes.len(),
                c"%f".as_ptr(),
                value,
            )
        };
        let Ok(written) = usize::try_from(written) else {
            return format!("{value:.6}").into_bytes();
        };
        if written < bytes.len() {
            bytes.truncate(written);
            return bytes;
        }
        bytes.resize(written + 1, 0);
    }
}

pub fn average_load() -> io::Result<f64> {
    let mut value = 0.0;
    // libc writes one initialized f64 and returns the number of values written.
    // This thread-local errno pointer is valid for the lifetime of the call.
    unsafe { *__errno_location() = 0 };
    if unsafe { getloadavg(&mut value, 1) } == 1 {
        Ok(value)
    } else {
        Err(io::Error::last_os_error())
    }
}

/// Return an owned diagnostic spelling. Only the interpreter renders statuses;
/// the libc string is copied before another signal-name lookup can overwrite it.
pub fn signal_name(number: i32) -> String {
    let text = unsafe { strsignal(number) };
    if text.is_null() {
        return format!("Signal {number}");
    }
    unsafe { CStr::from_ptr(text) }
        .to_string_lossy()
        .into_owned()
}

/// Keep private effects out of standard descriptor slots that a caller closed.
/// Duplicating above stderr also makes child dup2 inheritance unambiguous.
pub(crate) fn private_descriptor(file: File) -> io::Result<File> {
    if file.as_raw_fd() >= 3 {
        return Ok(file);
    }
    // Linux F_DUPFD_CLOEXEC atomically allocates a new descriptor at least 3.
    let descriptor = unsafe { fcntl(file.as_raw_fd(), 1030, 3) };
    if descriptor < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { File::from_raw_fd(descriptor) })
}
