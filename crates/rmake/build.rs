// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

fn main() {
    // Existing GNU plugins resolve these public C entry points from the host.
    for name in [
        "gmk_alloc",
        "gmk_free",
        "gmk_add_function",
        "gmk_expand",
        "gmk_eval",
    ] {
        println!("cargo:rustc-link-arg=-Wl,--export-dynamic-symbol={name}");
    }
}
