// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Linux effects behind the interpreter's explicit callback boundary.

#[cfg(not(target_os = "linux"))]
compile_error!("rmake-linux requires Linux");

pub mod filesystem;
mod wildcard;

#[allow(unsafe_code)]
mod posix;

// libc owns buffering and reports deferred write failures at fclose. Keeping
// that behavior requires a small FFI boundary; no semantic code uses unsafe.
#[allow(unsafe_code)]
mod stdio;
