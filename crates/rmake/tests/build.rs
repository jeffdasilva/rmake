// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

use std::ffi::{OsStr, OsString};
use std::fs::{self, File, FileTimes};
use std::os::unix::ffi::OsStringExt;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Project(PathBuf);

impl Project {
    fn new(makefile: &[u8]) -> Self {
        let path = std::env::temp_dir().join(format!(
            "rmake-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        fs::write(path.join("Makefile"), makefile).unwrap();
        Self(path)
    }

    fn run(&self, args: &[&OsStr]) -> Output {
        self.command().args(args).output().unwrap()
    }

    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_rmake"));
        command
            .current_dir(&self.0)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("LC_ALL", "C")
            .env("HOME", &self.0)
            .args(["-rR", "--no-print-directory"]);
        command
    }

    fn file(&self, name: &str, bytes: &[u8], seconds: u64, nanos: u32) {
        let path = self.0.join(name);
        fs::write(&path, bytes).unwrap();
        File::open(&path)
            .unwrap()
            .set_times(
                FileTimes::new()
                    .set_modified(SystemTime::UNIX_EPOCH + Duration::new(seconds, nanos)),
            )
            .unwrap();
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn rebuild_then_noop_preserves_artifact_timestamp() {
    let project = Project::new(b"output: input\n\t@cat input > output\n");
    project.file("input", b"new", 2000, 0);
    project.file("output", b"old", 1000, 0);
    let first = project.run(&[]);
    assert!(first.status.success(), "{:?}", first.stderr);
    assert!(first.stdout.is_empty());
    assert_eq!(fs::read(project.0.join("output")).unwrap(), b"new");
    let before = fs::metadata(project.0.join("output"))
        .unwrap()
        .modified()
        .unwrap();
    let second = project.run(&[]);
    assert!(second.status.success());
    assert_eq!(second.stdout, b"rmake: 'output' is up to date.\n");
    assert_eq!(
        fs::metadata(project.0.join("output"))
            .unwrap()
            .modified()
            .unwrap(),
        before
    );
}

#[test]
fn equal_nanoseconds_are_current_and_one_nanosecond_newer_rebuilds() {
    let project = Project::new(b"output: input\n\t@printf rebuilt > output\n");
    project.file("input", b"", 1000, 100);
    project.file("output", b"equal", 1000, 100);
    assert!(project.run(&[]).status.success());
    assert_eq!(fs::read(project.0.join("output")).unwrap(), b"equal");
    project.file("input", b"", 1000, 101);
    assert!(project.run(&[]).status.success());
    assert_eq!(fs::read(project.0.join("output")).unwrap(), b"rebuilt");
}

#[test]
fn order_only_prerequisite_does_not_invalidate_existing_target() {
    let project = Project::new(b"output: | directory\n\t@printf wrong > output\n");
    project.file("output", b"retained", 1000, 0);
    project.file("directory", b"", 2000, 0);
    assert!(project.run(&[]).status.success());
    assert_eq!(fs::read(project.0.join("output")).unwrap(), b"retained");
}

#[test]
fn dry_run_and_inherited_dry_run_do_not_write() {
    let project = Project::new(b"all:\n\t@printf touched > forbidden\n");
    for output in [
        project.run(&[OsStr::new("-n")]),
        project.command().env("MAKEFLAGS", "n").output().unwrap(),
    ] {
        assert!(output.status.success(), "{:?}", output.stderr);
        assert_eq!(output.stdout, b"printf touched > forbidden\n");
        assert!(!project.0.join("forbidden").exists());
    }
}

#[test]
fn question_mode_stops_before_a_later_forced_command() {
    let project =
        Project::new(b"all:\n\t@printf normal > forbidden\n\t+@printf forced > forbidden\n");
    let output = project.run(&[OsStr::new("-q")]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(!project.0.join("forbidden").exists());
}

#[test]
fn automatic_target_and_command_assignment_preserve_non_utf8_bytes() {
    let project = Project::new(b"\xff:\n\t@printf '%s|%s\\n' '$@' '$(VALUE)'\n");
    let target = OsString::from_vec(b"\xff".to_vec());
    let variable = OsString::from_vec(b"VALUE=\xfe".to_vec());
    let output = project.run(&[&target, &variable]);
    assert!(output.status.success(), "{:?}", output.stderr);
    assert_eq!(output.stdout, b"\xff|\xfe\n");
}

#[test]
fn delete_on_error_removes_changed_output_but_preserves_precious() {
    for precious in [false, true] {
        let makefile = format!(
            ".DELETE_ON_ERROR:\n{}output:\n\t@printf partial > output\n\t@exit 7\n",
            if precious { ".PRECIOUS: output\n" } else { "" }
        );
        let project = Project::new(makefile.as_bytes());
        let output = project.run(&[OsStr::new("output")]);
        assert_eq!(output.status.code(), Some(2));
        assert_eq!(project.0.join("output").exists(), precious);
    }
}
