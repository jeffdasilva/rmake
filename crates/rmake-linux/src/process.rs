// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Owned Linux process effects. No descriptors or libc pointers escape unowned.

use std::ffi::{CStr, CString, c_char, c_int};
use std::fs::File;
use std::io;
use std::os::fd::FromRawFd;
use std::process::Stdio;

unsafe extern "C" {
    fn strtod(text: *const c_char, end: *mut *mut c_char) -> f64;
    fn snprintf(buffer: *mut c_char, length: usize, format: *const c_char, ...) -> c_int;
    fn __errno_location() -> *mut c_int;
    fn getloadavg(values: *mut f64, count: c_int) -> c_int;
    fn pipe2(descriptors: *mut c_int, flags: c_int) -> c_int;
}

/// Parallel commands without the input lease receive a pipe with no writers.
/// Unlike /dev/null this preserves the observable FIFO descriptor type.
pub fn closed_stdin() -> io::Result<Stdio> {
    let mut descriptors = [-1; 2];
    // pipe2 initializes both descriptors on success. CLOEXEC is atomic even
    // when other recipe workers spawn concurrently with this call.
    if unsafe { pipe2(descriptors.as_mut_ptr(), 0x80000) } < 0 {
        return Err(io::Error::last_os_error());
    }
    let reader = unsafe { File::from_raw_fd(descriptors[0]) };
    let writer = unsafe { File::from_raw_fd(descriptors[1]) };
    drop(writer);
    Ok(Stdio::from(reader))
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
