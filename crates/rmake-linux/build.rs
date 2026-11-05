// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva
fn main() {
    println!("cargo:rerun-if-changed=native_shim.c");
    println!("cargo:rerun-if-changed=stdin_source.c");
    cc::Build::new()
        .file("native_shim.c")
        .file("stdin_source.c")
        .flag_if_supported("-std=c11")
        .warnings(true)
        .warnings_into_errors(true)
        .compile("rmake_native_shim");
    println!("cargo:rustc-link-lib=dl");
    println!("cargo:rustc-link-lib=pthread");
}
