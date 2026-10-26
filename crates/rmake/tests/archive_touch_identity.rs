// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Candidate safety: a controlled inode replacement must never modify the new
//! archive. GNU's unsafe race is not counted as a compatibility pass.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Root(PathBuf);
impl Drop for Root {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn archive(date: &str, payload: &[u8]) -> Vec<u8> {
    let mut result = b"!<arch>\n".to_vec();
    result.extend_from_slice(
        format!(
            "{:<16}{:<12}{:<6}{:<6}{:<8}{:<10}`\n",
            "member/",
            date,
            "0",
            "0",
            "100644",
            payload.len()
        )
        .as_bytes(),
    );
    result.extend_from_slice(payload);
    result
}

#[test]
fn touching_archive_rejects_a_replacement_before_acquiring_write_access() {
    let root = Root(std::env::temp_dir().join(format!(
        "rmake-archive-identity-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )));
    fs::create_dir(&root.0).unwrap();
    let project = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let library = root.0.join("faults.so");
    let compile = Command::new("cc")
        .env("PATH", "/usr/bin:/bin")
        .args([
            "-shared", "-fPIC", "-O2", "-Wall", "-Wextra", "-Werror", "-o",
        ])
        .arg(&library)
        .arg(project.join("tools/stdio_faults.c"))
        .arg("-ldl")
        .output()
        .unwrap();
    assert!(
        compile.status.success(),
        "{}",
        String::from_utf8_lossy(&compile.stderr)
    );
    let replacement = archive("1600000040", b"changed\n");
    fs::write(
        root.0.join("effect-probe"),
        archive("1600000020", b"payload\n"),
    )
    .unwrap();
    fs::write(root.0.join("replacement.a"), &replacement).unwrap();
    fs::write(
        root.0.join("Makefile"),
        b"effect-probe(member):\n\t+@printf armed > arm.marker\n\t@printf forbidden\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rmake"))
        .current_dir(&root.0)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("LC_ALL", "C")
        .env("LD_PRELOAD", &library)
        .env("RMAKE_STDIO_FAULT", "archive-touch-replace")
        .args(["-rR", "--no-print-directory", "-Bt"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(output.stdout, b"touch effect-probe(member)\n");
    assert!(
        output
            .stderr
            .windows(b"archive changed while opening 'effect-probe'".len())
            .any(|part| part == b"archive changed while opening 'effect-probe'"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read(root.0.join("effect-probe")).unwrap(), replacement);
    assert!(!root.0.join("replacement.a").exists());
    assert_eq!(
        fs::read(root.0.join("fault.log")).unwrap(),
        b"archive-replaced\n"
    );
    assert_eq!(fs::read(root.0.join("arm.marker")).unwrap(), b"armed");
}
