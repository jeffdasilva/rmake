// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Owned libc file streams behind the filesystem effect boundary.
//! No stream pointer or borrowed C buffer escapes the safe interface.

use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::io;
use std::ptr::NonNull;

#[repr(C)]
struct CFile {
    _opaque: [u8; 0],
}

unsafe extern "C" {
    fn fopen(path: *const c_char, mode: *const c_char) -> *mut CFile;
    fn fread(buffer: *mut c_void, size: usize, count: usize, stream: *mut CFile) -> usize;
    fn ferror(stream: *mut CFile) -> c_int;
    fn clearerr(stream: *mut CFile);
    fn fputs(text: *const c_char, stream: *mut CFile) -> c_int;
    fn fputc(byte: c_int, stream: *mut CFile) -> c_int;
    fn fclose(stream: *mut CFile) -> c_int;
}

/// Neither Clone nor Send/Sync: the pointer has exactly one Rust owner.
pub(super) struct Stream(Option<NonNull<CFile>>);

impl Stream {
    pub(super) fn open(path: &[u8], mode: &CStr) -> io::Result<Self> {
        let path = CString::new(path)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "NUL byte in filename"))?;
        loop {
            // SAFETY: both strings are terminated and live for this call. A
            // successful fopen returns a fresh stream exclusively owned here.
            let pointer = unsafe { fopen(path.as_ptr(), mode.as_ptr()) };
            if let Some(pointer) = NonNull::new(pointer) {
                return Ok(Self(Some(pointer)));
            }
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::Interrupted {
                return Err(error);
            }
        }
    }

    pub(super) fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let pointer = self.0.expect("only consuming close removes a live stream");
        loop {
            // SAFETY: the stream is live and exclusively borrowed. The buffer
            // is initialized writable storage of exactly the passed capacity.
            let count = unsafe {
                fread(
                    buffer.as_mut_ptr().cast(),
                    1,
                    buffer.len(),
                    pointer.as_ptr(),
                )
            };
            // SAFETY: checking stream state neither transfers nor frees it.
            if unsafe { ferror(pointer.as_ptr()) } == 0 {
                return Ok(count);
            }
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::Interrupted {
                return Err(error);
            }
            // SAFETY: clear only the live stream's sticky interruption flag.
            // Retain any bytes read before EINTR and retry on the next call.
            unsafe { clearerr(pointer.as_ptr()) };
            if count != 0 {
                return Ok(count);
            }
        }
    }

    pub(super) fn write_text(&mut self, text: &[u8], newline: bool) -> io::Result<()> {
        let text = CString::new(text)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "NUL byte in file text"))?;
        let pointer = self.0.expect("only consuming close removes a live stream");
        // SAFETY: a valid terminated string and an exclusively borrowed live
        // stream. libc reads the string synchronously and does not retain it.
        if unsafe { fputs(text.as_ptr(), pointer.as_ptr()) } < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: a byte value and the same live stream. Do not flush here:
        // deferred errors belong to close, as with the reference runtime.
        if newline && unsafe { fputc(c_int::from(b'\n'), pointer.as_ptr()) } < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    pub(super) fn close(mut self) -> io::Result<()> {
        let pointer = self.0.take().expect("live stream consumed once");
        // SAFETY: ownership was removed before closing. fclose invalidates the
        // stream even on failure: never retry it or let Drop close it again.
        if unsafe { fclose(pointer.as_ptr()) } == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        if let Some(pointer) = self.0.take() {
            // SAFETY: best-effort cleanup of the single still-owned stream.
            // Earlier failures have already captured their errno in io::Error.
            unsafe { fclose(pointer.as_ptr()) };
        }
    }
}
