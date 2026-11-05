// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

fn main() {
    println!("cargo:rerun-if-env-changed=RMAKE_GUILE_PREFIX");
    if std::env::var_os("CARGO_FEATURE_GUILE").is_some() {
        let prefix = std::path::PathBuf::from(
            std::env::var_os("RMAKE_GUILE_PREFIX")
                .expect("guile requires an explicit verified library prefix"),
        );
        let library = prefix.join("usr/lib/x86_64-linux-gnu");
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", library.display());
    }
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

// Dependency link arguments do not propagate to this executable/test target.
// The enabled profile uses the same explicitly verified library prefix as the
// Linux adapter; default builds add no foreign library search path.
