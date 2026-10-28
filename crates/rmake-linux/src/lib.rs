// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Linux effects behind the interpreter's explicit callback boundary.

#[cfg(not(target_os = "linux"))]
compile_error!("rmake-linux requires Linux");

pub mod archive;
pub mod filesystem;
pub mod timestamp;
// The public surface owns descriptors/tokens; only this adapter performs the
// reviewed Linux FIFO, pipe, and fork/exec descriptor operations.
#[allow(unsafe_code)]
pub mod jobserver;

#[allow(unsafe_code)]
pub mod process;

#[allow(unsafe_code)]
pub mod child;
#[allow(unsafe_code)]
pub mod signals;
mod wildcard;

// Preserve incremental reads, dot entries, errno and descriptor ownership.
#[allow(unsafe_code)]
mod directory;

#[allow(unsafe_code)]
mod posix;

// libc owns buffering and reports deferred write failures at fclose. Keeping
// that behavior requires a small FFI boundary; no semantic code uses unsafe.
#[allow(unsafe_code)]
mod stdio;

#[allow(unsafe_code)]
pub mod output;

#[allow(unsafe_code)]
pub mod environment;
