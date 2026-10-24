// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Linux effects behind the interpreter's explicit callback boundary.

#[cfg(not(target_os = "linux"))]
compile_error!("rmake-linux requires Linux");

pub mod archive;
pub mod filesystem;
// The public surface owns descriptors/tokens; only this adapter performs the
// reviewed Linux FIFO, pipe, and fork/exec descriptor operations.
#[allow(unsafe_code)]
pub mod jobserver;
mod wildcard;

#[allow(unsafe_code)]
mod posix;

// libc owns buffering and reports deferred write failures at fclose. Keeping
// that behavior requires a small FFI boundary; no semantic code uses unsafe.
#[allow(unsafe_code)]
mod stdio;
