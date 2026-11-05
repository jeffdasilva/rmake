// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

mod catalog_validation;
#[allow(dead_code)]
#[path = "../rmake-core/src/messages.rs"]
mod system_messages;

fn main() {
    let source = std::path::PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("Cargo manifest directory"),
    )
    .join("../../catalogs");
    let inventory = source.join("languages.txt");
    println!("cargo:rerun-if-changed={}", inventory.display());
    println!("cargo:rerun-if-changed=../rmake-core/src/messages.rs");
    println!("cargo:rerun-if-env-changed=RMAKE_LOCALEDIR");
    let languages = std::fs::read_to_string(&inventory).expect("read owned catalog inventory");
    let mut previous = None;
    let mut count = 0;
    for language in languages.lines() {
        let bytes = language.as_bytes();
        let valid = matches!(bytes, [b'a'..=b'z', b'a'..=b'z'])
            || matches!(
                bytes,
                [b'a'..=b'z', b'a'..=b'z', b'_', b'A'..=b'Z', b'A'..=b'Z']
            );
        assert!(
            valid && previous.is_none_or(|last| last < language),
            "invalid catalog inventory"
        );
        previous = Some(language);
        count += 1;
        let catalog = source.join(language).join("LC_MESSAGES/make.mo");
        println!("cargo:rerun-if-changed={}", catalog.display());
        catalog_validation::validate(&catalog)
            .unwrap_or_else(|error| panic!("owned catalog {}: {error}", catalog.display()));
    }
    assert_eq!(count, 30, "incomplete GNU Make 4.4.1 catalog inventory");
    let installed = std::env::var_os("RMAKE_LOCALEDIR")
        .map(std::path::PathBuf::from)
        .unwrap_or(source);
    assert!(
        installed.is_absolute(),
        "RMAKE_LOCALEDIR must be an absolute catalog directory"
    );
    println!("cargo:rustc-env=RMAKE_LOCALEDIR={}", installed.display());
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
