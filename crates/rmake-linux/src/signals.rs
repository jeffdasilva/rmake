// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! A handler records one signal and wakes the interpreter. Allocation, target
//! cleanup, token return, diagnostics and child reaping run outside handlers.
use std::ffi::{c_int, c_void};
use std::fs::File;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

const CAUGHT: [c_int; 7] = [1, 2, 3, 13, 15, 24, 25];
static CANCELLED: AtomicBool = AtomicBool::new(false);
static TERMINAL_STATUS: AtomicI32 = AtomicI32::new(-1);
static PENDING: AtomicI32 = AtomicI32::new(0);
static WAKE_READER: AtomicI32 = AtomicI32::new(-1);
static WAKE_WRITER: AtomicI32 = AtomicI32::new(-1);
static INITIAL_PIPE: AtomicI32 = AtomicI32::new(0);

unsafe extern "C" {
    fn signal(number: c_int, handler: usize) -> usize;
    fn siginterrupt(number: c_int, interrupt: c_int) -> c_int;
    fn pipe2(descriptors: *mut c_int, flags: c_int) -> c_int;
    fn fcntl(descriptor: c_int, operation: c_int, ...) -> c_int;
    fn read(descriptor: c_int, bytes: *mut c_void, length: usize) -> isize;
    fn write(descriptor: c_int, bytes: *const c_void, length: usize) -> isize;
    fn __errno_location() -> *mut c_int;
    fn getpid() -> c_int;
    fn kill(pid: c_int, number: c_int) -> c_int;
}

// Rust's startup changes SIGPIPE to SIG_IGN. Record its inherited disposition
// before lang_start so explicit inherited ignore can still be honored.
extern "C" fn remember_pipe() {
    let old = unsafe { signal(13, 1) };
    if old != usize::MAX {
        INITIAL_PIPE.store(i32::from(old == 1), Ordering::Relaxed);
        unsafe { signal(13, old) };
    }
}

#[used]
#[unsafe(link_section = ".init_array")]
static BEFORE_RUST: extern "C" fn() = remember_pipe;

extern "C" fn caught(number: c_int) {
    // AtomicI32 is lock-free on supported Linux targets. write, signal, getpid
    // and kill are async-signal-safe. Preserve this thread's interrupted errno.
    let saved = unsafe { *__errno_location() };
    unsafe { signal(number, 0) };
    CANCELLED.store(true, Ordering::Relaxed);
    PENDING.store(number, Ordering::SeqCst);
    let byte = number as u8;
    unsafe {
        let descriptor = WAKE_WRITER.load(Ordering::Relaxed);
        if descriptor >= 0 {
            write(descriptor, (&byte as *const u8).cast(), 1);
        }
    }
    unsafe { *__errno_location() = saved };
}

// Reserve the conventional first protocol pair before handlers or user code
// can retain a wake descriptor. A live wake pipe is never relocated.
fn private_wake_end(file: File) -> io::Result<File> {
    if file.as_raw_fd() >= 5 {
        return Ok(file);
    }
    let descriptor = unsafe { fcntl(file.as_raw_fd(), 1030, 5) };
    if descriptor < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { File::from_raw_fd(descriptor) })
}

fn wake_pipe() -> io::Result<(File, File)> {
    let mut descriptors = [-1; 2];
    if unsafe { pipe2(descriptors.as_mut_ptr(), 0x80000 | 0x800) } < 0 {
        return Err(io::Error::last_os_error());
    }
    let reader = unsafe { File::from_raw_fd(descriptors[0]) };
    let writer = unsafe { File::from_raw_fd(descriptors[1]) };
    Ok((private_wake_end(reader)?, private_wake_end(writer)?))
}

pub struct Guard {
    saved: Vec<(c_int, usize)>,
    _reader: Option<File>,
    _writer: Option<File>,
}

impl Guard {
    /// Install once, before parsing or launching children. Preserve dispositions
    /// explicitly ignored by the caller, and restore ordinary SIGCHLD reaping.
    pub fn install() -> io::Result<Self> {
        let (reader, writer) = match wake_pipe() {
            Ok((reader, writer)) => (Some(reader), Some(writer)),
            // A low descriptor limit can reject the pipe itself or the minimum
            // private descriptor number. Do not occupy protocol slots 3/4.
            // Interruptible scheduler waits use a bounded poll in this mode.
            Err(error) if matches!(error.raw_os_error(), Some(22..=24)) => (None, None),
            Err(error) => return Err(error),
        };
        let mut guard = Self {
            saved: Vec::new(),
            _reader: reader,
            _writer: writer,
        };
        WAKE_READER.store(
            guard._reader.as_ref().map_or(-1, AsRawFd::as_raw_fd),
            Ordering::Relaxed,
        );
        WAKE_WRITER.store(
            guard._writer.as_ref().map_or(-1, AsRawFd::as_raw_fd),
            Ordering::Relaxed,
        );
        PENDING.store(0, Ordering::SeqCst);
        TERMINAL_STATUS.store(-1, Ordering::Relaxed);
        CANCELLED.store(false, Ordering::Relaxed);
        for number in CAUGHT {
            let old = unsafe { signal(number, caught as *const () as usize) };
            if old == usize::MAX {
                return Err(io::Error::last_os_error());
            }
            let inherited_ignore = if number == 13 {
                INITIAL_PIPE.load(Ordering::Relaxed) != 0
            } else {
                old == 1
            };
            // Blocking output locks must wake for fatal shutdown. libc owns
            // the sigaction ABI; use its flag update rather than a Rust layout.
            if !inherited_ignore {
                unsafe { siginterrupt(number, 1) };
            }
            guard.saved.push((number, old));
            if inherited_ignore {
                unsafe { signal(number, 1) };
            }
        }
        let old = unsafe { signal(17, 0) };
        if old == usize::MAX {
            return Err(io::Error::last_os_error());
        }
        guard.saved.push((17, old));
        Ok(guard)
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        for &(number, old) in &self.saved {
            unsafe { signal(number, old) };
        }
        WAKE_READER.store(-1, Ordering::Relaxed);
        WAKE_WRITER.store(-1, Ordering::Relaxed);
    }
}

/// Rust resets SIGPIPE in spawned children. GNU preserves an explicitly
/// inherited ignore disposition; restore it after std's child setup. Only the
/// async-signal-safe disposition operation runs between fork and exec.
pub fn configure_child(command: &mut std::process::Command) {
    use std::os::unix::process::CommandExt;
    if !preserve_child_pipe_ignore() {
        return;
    }
    unsafe {
        command.pre_exec(|| {
            if signal(13, 1) == usize::MAX {
                Err(io::Error::last_os_error())
            } else {
                Ok(())
            }
        });
    }
}

pub(crate) fn preserve_child_pipe_ignore() -> bool {
    INITIAL_PIPE.load(Ordering::Relaxed) != 0
}

pub fn pending() -> Option<i32> {
    let value = PENDING.load(Ordering::SeqCst);
    (value != 0).then_some(value)
}

/// Drain only after a signal is pending, then resample pending(). Keeping the
/// pipe in every wait prevents a subsequent signal just before poll from being
/// lost; resampling after this drain cannot consume an unseen wake permanently.
pub fn drain() {
    let descriptor = WAKE_READER.load(Ordering::Relaxed);
    if descriptor < 0 {
        return;
    }
    let mut bytes = [0_u8; 64];
    loop {
        let count = unsafe { read(descriptor, bytes.as_mut_ptr().cast(), bytes.len()) };
        if count > 0 {
            continue;
        }
        if count < 0 && io::Error::last_os_error().kind() == io::ErrorKind::Interrupted {
            continue;
        }
        return;
    }
}

pub(crate) fn wake_descriptor() -> i32 {
    WAKE_READER.load(Ordering::Relaxed)
}

pub fn deletes_targets(number: i32) -> bool {
    matches!(number, 1 | 2 | 3 | 15)
}

/// Call after the interpreter has released owned resources. SIGQUIT has GNU's
/// explicit status 1 convention; other fatal signals retain wait-status identity.
pub fn exit_status(status: u8) -> u8 {
    let terminal = TERMINAL_STATUS.load(Ordering::Relaxed);
    if terminal >= 0 {
        return terminal as u8;
    }
    if let Some(number) = pending() {
        if number == 3 {
            return 1;
        }
        unsafe {
            signal(number, 0);
            kill(getpid(), number);
        }
        return (128 + number) as u8;
    }
    status
}

/// Core evaluation receives only a cooperative cancellation flag, with no
/// platform effects or callback dispatch on its hot expansion path.
pub fn cancellation() -> &'static AtomicBool {
    &CANCELLED
}

/// A normal fatal diagnostic can supersede signal exit while GNU's logical
/// completion owner reenters shutdown. Effects and cleanup still observe PENDING.
pub fn terminal_status(status: u8) {
    TERMINAL_STATUS.store(i32::from(status), Ordering::Relaxed);
}
