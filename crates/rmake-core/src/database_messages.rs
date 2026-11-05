// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Typed schemas for formatted database catalog keys.
//! Runtime rendering and build-time catalog validation share these declarations.
use crate::messages::Argument;
use std::ffi::CStr;

#[derive(Clone, Copy)]
pub(crate) struct Key {
    pub msgid: &'static CStr,
    #[allow(dead_code)] // Used by the application build-time catalog validator.
    pub sample: &'static [Argument<'static>],
}

const B: Argument<'static> = Argument::Bytes(b"bytes");
const D: Argument<'static> = Argument::Signed(-1);
const U: Argument<'static> = Argument::Unsigned(3);
const F0: Argument<'static> = Argument::FixedDecimal {
    bytes: b"3",
    digits: 0,
};
const F1: Argument<'static> = Argument::FixedDecimal {
    bytes: b"3.5",
    digits: 1,
};

macro_rules! keys {
    ($($name:ident: $msgid:expr, [$($sample:expr),*];)+) => {
        $(pub(crate) const $name: Key = Key { msgid: $msgid, sample: &[$($sample),*] };)+
        #[allow(dead_code)] // Used by the application build-time catalog validator.
        pub(crate) const ALL: &[Key] = &[$($name),+];
    };
}

keys! {
    OPEN: c"\n# Make data base, printed on %s", [B];
    CLOSE: c"\n# Finished Make data base on %s\n", [B];
    LOAD: c"Load=%lu/%lu=%.0f%%, ", [U, U, F0];
    REHASH: c"Rehash=%u, ", [U];
    COLLISIONS: c"Collisions=%lu/%lu=%.0f%%", [U, U, F0];
    VARIABLE_DEFINITION: c" (from '%s', line %lu)", [B, U];
    PATTERN_COUNT: c"\n# %u pattern-specific variable values", [U];
    RECIPE_DEFINITION: c" (from '%s', line %lu):\n", [B, U];
    IMPLICIT_COUNT: c"\n# %u implicit rules, %u (%.1f%%) terminal.", [U, U, F1];
    VPATH_COUNT: c"\n# %u 'vpath' search paths.\n", [U];
    DIRECTORY_MISSING: c"# %s: could not be stat'd.\n", [B];
    DIRECTORY_UNOPENED: c"# %s (device %ld, inode %ld): could not be opened.\n", [B, D, D];
    DIRECTORY_ENUMERATED: c"# %s (device %ld, inode %ld): ", [B, D, D];
    DIRECTORY_TOTAL: c" impossibilities in %lu directories.\n", [U];
    STEM: c"#  Implicit/static pattern stem: '%s'\n", [B];
    MTIME: c"#  Last modified %s\n", [B];
    CACHE_EMPTY: c"\n%s No strcache buffers\n", [B];
    CACHE_BUFFERS: c"\n%s strcache buffers: %lu (%lu) / strings = %lu / storage = %lu B / avg = %lu B\n", [B, U, U, U, U, U];
    CACHE_CURRENT: c"%s current buf: size = %hu B / used = %hu B / count = %hu / avg = %u B\n", [B, U, U, U, U];
    CACHE_OTHER_USED: c"%s other used: total = %lu B / count = %lu / avg = %lu B\n", [B, U, U, U];
    CACHE_OTHER_FREE: c"%s other free: total = %lu B / max = %lu B / min = %lu B / avg = %hu B\n", [B, U, U, U, U];
    CACHE_PERFORMANCE: c"\n%s strcache performance: lookups = %lu / hit rate = %lu%%\n", [B, U, U];
}
