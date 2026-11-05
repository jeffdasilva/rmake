// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Process-entry locale selection, before interpreter callbacks or workers.

use std::ffi::{c_char, c_int};

unsafe extern "C" {
    fn setlocale(category: c_int, locale: *const c_char) -> *mut c_char;
}

/// Let libc select every category using LC_ALL, category overrides and LANG.
/// The executable calls this once at entry, before any foreign code or threads.
/// An unavailable selection leaves the initial C locale in place; it does not
/// add a diagnostic or rewrite the environment inherited by recipes.
pub fn initialize() {
    // LC_ALL is 6 on supported Linux libcs. The static empty C string asks
    // libc to apply its environment precedence. No libc-owned pointer escapes.
    unsafe {
        setlocale(6, c"".as_ptr());
    }
}
