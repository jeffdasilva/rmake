// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Process-entry locale selection, before interpreter callbacks or workers.

use std::ffi::{CStr, c_char, c_int, c_void};

unsafe extern "C" {
    fn setlocale(category: c_int, locale: *const c_char) -> *mut c_char;
    fn uselocale(locale: *mut c_void) -> *mut c_void;
    fn strcoll(left: *const c_char, right: *const c_char) -> c_int;
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

/// A query-time snapshot of the calling thread's collation policy. Native
/// callbacks can change either global or per-thread selection between globs.
pub(crate) struct Collation {
    bytewise: bool,
}

impl Collation {
    pub(crate) fn current() -> Self {
        // POSIX LC_GLOBAL_LOCALE is (locale_t)-1. A thread-local override can
        // disagree with setlocale's global name, so it always uses strcoll.
        let global = unsafe { uselocale(std::ptr::null_mut()) } as isize == -1;
        let bytewise = global && {
            // LC_COLLATE is 3 on Linux. This query changes no locale and its
            // libc-owned result is consumed before any foreign callback.
            let pointer = unsafe { setlocale(3, std::ptr::null()) };
            !pointer.is_null()
                && matches!(
                    unsafe { CStr::from_ptr(pointer) }.to_bytes(),
                    b"C" | b"POSIX"
                )
        };
        Self { bytewise }
    }

    pub(crate) fn is_bytewise(&self) -> bool {
        self.bytewise
    }

    pub(crate) fn sort(&self, names: &mut [Vec<u8>]) {
        if self.bytewise {
            names.sort_unstable();
            return;
        }
        // One terminator per owned result, reused for every comparison. The
        // comparator allocates nothing and keeps raw Linux filename bytes.
        for name in names.iter_mut() {
            name.push(0);
        }
        names.sort_by(|left, right| {
            // Both owned vectors remain terminated and immovable for this
            // call. Equal collation keys preserve the directory traversal.
            let value = unsafe { strcoll(left.as_ptr().cast(), right.as_ptr().cast()) };
            value.cmp(&0)
        });
        for name in names {
            name.pop();
        }
    }
}
