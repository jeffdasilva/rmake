// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva
fn main() {
    build_guile();
    println!("cargo:rerun-if-changed=native_shim.c");
    println!("cargo:rerun-if-changed=stdin_source.c");
    println!("cargo:rerun-if-changed=restart.c");
    let mut native = cc::Build::new();
    if std::env::var_os("CARGO_FEATURE_GUILE").is_some() {
        native.define("RMAKE_GUILE_DISPATCH", None);
    }
    native
        .file("native_shim.c")
        .file("stdin_source.c")
        .file("restart.c")
        .flag_if_supported("-std=c11")
        .warnings(true)
        .warnings_into_errors(true)
        .compile("rmake_native_shim");
    println!("cargo:rustc-link-lib=dl");
    println!("cargo:rustc-link-lib=m");
    println!("cargo:rustc-link-lib=pthread");
}

fn build_guile() {
    println!("cargo:rerun-if-env-changed=RMAKE_GUILE_PREFIX");
    println!("cargo:rerun-if-changed=guile.c");
    println!("cargo:rerun-if-changed=guile_interrupt.c");
    if std::env::var_os("CARGO_FEATURE_GUILE").is_none() {
        return;
    }
    assert_eq!(
        std::env::var("CARGO_CFG_TARGET_ARCH").as_deref(),
        Ok("x86_64"),
        "Guile component is qualified only for x86-64 Linux"
    );
    let prefix = std::path::PathBuf::from(
        std::env::var_os("RMAKE_GUILE_PREFIX")
            .expect("explicit guile feature requires verified RMAKE_GUILE_PREFIX"),
    );
    let library = prefix.join("usr/lib/x86_64-linux-gnu");
    cc::Build::new()
        .file("guile.c")
        .file("guile_interrupt.c")
        .flag_if_supported("-std=c11")
        .include(prefix.join("usr/include"))
        .include(prefix.join("usr/include/x86_64-linux-gnu"))
        .include(prefix.join("usr/include/guile/3.0"))
        .warnings(true)
        .warnings_into_errors(true)
        .compile("rmake_guile");
    println!("cargo:rustc-link-search=native={}", library.display());
    println!("cargo:rustc-link-lib=guile-3.0");
    println!("cargo:rustc-link-lib=gc");
    println!("cargo:rustc-link-arg-tests=-Wl,--export-dynamic");
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", library.display());
}
