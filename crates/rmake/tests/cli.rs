// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

use std::process::Command;

#[test]
fn version_identifies_product_and_incomplete_compatibility() {
    let output = Command::new(env!("CARGO_BIN_EXE_rmake"))
        .arg("--version")
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.starts_with("rmake "));
    assert!(text.contains("4.4.1"));
    assert!(text.contains("implementation incomplete"));
    assert!(text.contains("GPLv3+"));
    assert!(output.stderr.is_empty());
}

#[test]
fn help_describes_actual_capabilities() {
    let output = Command::new(env!("CARGO_BIN_EXE_rmake"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("not implemented")
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn unsupported_builds_do_not_report_success() {
    for args in [
        vec![],
        vec!["all"],
        vec!["--unknown"],
        vec!["--help", "all"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_rmake"))
            .args(args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }
}
