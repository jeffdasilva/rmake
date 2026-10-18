// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

use std::ffi::{OsStr, OsString};
use std::fs::{self, File, FileTimes};
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
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

#[test]
fn fatal_expansion_stops_prerequisites_and_goals_even_when_keeping_or_ignoring_errors() {
    for function in [b"$(error fatal)".as_slice(), b"$(intcmp bad,0)"] {
        let source = [
            b".DELETE_ON_ERROR:\n.PHONY: all before bad after goal\nall: before bad after\nbefore:\n\t@printf retained > before\nbad:\n\t-@".as_slice(),
            function,
            b"\nafter:\n\t@touch forbidden\ngoal:\n\t@touch forbidden\n",
        ].concat();
        let project = Project::new(&source);
        project.file("bad", b"preserved", 1000, 123);
        let before = fs::metadata(project.0.join("bad"))
            .unwrap()
            .modified()
            .unwrap();
        let output = project.run(&[OsStr::new("-ki"), OsStr::new("all"), OsStr::new("goal")]);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(output.stderr.starts_with(b"Makefile:7: *** "));
        assert_eq!(
            output.stderr.iter().filter(|byte| **byte == b'\n').count(),
            1
        );
        assert_eq!(fs::read(project.0.join("before")).unwrap(), b"retained");
        assert_eq!(fs::read(project.0.join("bad")).unwrap(), b"preserved");
        assert_eq!(
            fs::metadata(project.0.join("bad"))
                .unwrap()
                .modified()
                .unwrap(),
            before
        );
        assert!(!project.0.join("forbidden").exists());
    }
}

#[test]
fn error_function_recipe_context_differs_from_export_binding_context() {
    for (source, line) in [
        (
            b"FAIL = $(error raw\xff)\nall:\n\t@$(call FAIL)\n".as_slice(),
            3,
        ),
        (
            b"FAIL = $(error raw\xff)\nexport SENT = $(FAIL)\nall:\n\t@touch forbidden\n",
            2,
        ),
    ] {
        let project = Project::new(source);
        let output = project.run(&[]);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert_eq!(
            output.stderr,
            [
                format!("Makefile:{line}: *** ").as_bytes(),
                b"raw\xff.  Stop.\n"
            ]
            .concat()
        );
        assert!(!project.0.join("forbidden").exists());
    }
}

#[test]
fn environment_exports_are_raw_but_references_to_them_still_expand() {
    let project = Project::new(b"unexport VALUE\nexport VALUE\nall:\n\t@printf '%s' \"$$VALUE\"\n");
    let output = project
        .command()
        .env("VALUE", "$(error fatal)")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout, b"$(error fatal)");
    let project = Project::new(b"export ALIAS = $(VALUE)\nall:\n\t@touch forbidden\n");
    let output = project
        .command()
        .env("VALUE", "$(error fatal)")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(output.stderr, b"Makefile:1: *** fatal.  Stop.\n");
    assert!(!project.0.join("forbidden").exists());
}

#[test]
fn automatic_export_names_do_not_expand_ineligible_values_before_they_are_used() {
    for name in [b"FAIL-NAME".as_slice(), b"1X", b"\xff"] {
        for policy in [b"".as_slice(), b"export ", b"inherited"] {
            let mut source = Vec::new();
            if policy == b"export " {
                source.extend_from_slice(&[policy, name, b"\n"].concat());
            }
            source.extend_from_slice(b"all: anchor\n\t@$(");
            source.extend_from_slice(name);
            source.extend_from_slice(b")\nanchor:\n\t@printf anchor > anchor\n");
            let project = Project::new(&source);
            let mut command = project.command();
            command.arg(OsString::from_vec([name, b"=$(error fatal)"].concat()));
            if policy == b"inherited" {
                command.env(OsString::from_vec(name.to_vec()), "old");
            }
            let output = command.output().unwrap();
            assert_eq!(output.status.code(), Some(2));
            assert_eq!(project.0.join("anchor").exists(), policy.is_empty());
            assert_eq!(
                output.stderr,
                if policy.is_empty() {
                    b"Makefile:2: *** fatal.  Stop.\n".as_slice()
                } else {
                    b"rmake: *** fatal.  Stop.\n".as_slice()
                }
            );
        }
    }
    let project = Project::new(b"export\n1X = $(error unused)\nall:\n\t@printf anchor\n");
    let output = project.run(&[]);
    assert!(output.status.success());
    assert_eq!(output.stdout, b"anchor");
    assert!(output.stderr.is_empty());
}

#[test]
fn empty_shell_and_flags_execute_a_literal_recipe_filename() {
    let project =
        Project::new(b"SHELL :=\n.SHELLFLAGS :=\nall:\n\t@./tool words;touch forbidden\n");
    let helper = project.0.join("tool words;touch forbidden");
    fs::write(&helper, b"#!/bin/sh\nprintf 'literal:%s' \"$#\"\n").unwrap();
    fs::set_permissions(&helper, fs::Permissions::from_mode(0o755)).unwrap();
    let output = project.run(&[]);
    assert!(output.status.success(), "{:?}", output.stderr);
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout, b"literal:0");
    assert!(!project.0.join("forbidden").exists());
}

#[test]
fn empty_shell_colon_noop_skips_exports_but_oneshell_keeps_empty_program() {
    let project =
        Project::new(b"SHELL :=\n.SHELLFLAGS :=\nexport BAD = $(error unused)\nall:\n\t@:\n");
    let output = project.run(&[]);
    assert!(output.status.success(), "{:?}", output.stderr);
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    let project =
        Project::new(b".ONESHELL:\nundefine SHELL\nall:\n\t@printf first\n\t@printf second\n");
    let output = project.run(&[]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(
        output.stderr,
        b"rmake: : Permission denied\nrmake: *** [Makefile:4: all] Error 127\n"
    );
    let output = project.run(&[OsStr::new("-n")]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout, b"printf first\n@printf second\n");
}

#[test]
fn empty_shell_assignment_words_do_not_evaluate_values_as_commands() {
    for (body, assignment) in [
        (b"A=B".as_slice(), true),
        (b"A=B;touch forbidden", true),
        (b"A=`touch forbidden`", true),
        (b"1=B", false),
        (b"./A=B", false),
        (b"A\\=B", false),
    ] {
        for export in [false, true] {
            let source = [
                b"SHELL :=\n.SHELLFLAGS :=\nexport PATH := .:/usr/bin:/bin\n".as_slice(),
                if export {
                    b"export BAD = $(error export-block)\n"
                } else {
                    b""
                },
                b"all:\n\t@",
                body,
                b"\n",
            ]
            .concat();
            let project = Project::new(&source);
            let name = body.strip_prefix(b"./").unwrap_or(body);
            let helper = project.0.join(OsString::from_vec(name.to_vec()));
            fs::write(&helper, b"#!/bin/sh\nprintf literal\n").unwrap();
            fs::set_permissions(&helper, fs::Permissions::from_mode(0o755)).unwrap();
            let output = project.run(&[]);
            assert_eq!(output.status.code(), Some(if export { 2 } else { 0 }));
            assert_eq!(
                output.stdout,
                if assignment || export {
                    b"".as_slice()
                } else {
                    b"literal"
                }
            );
            assert_eq!(
                output.stderr,
                if export {
                    b"Makefile:4: *** export-block.  Stop.\n".as_slice()
                } else {
                    b""
                }
            );
            assert!(!project.0.join("forbidden").exists());
        }
    }
    let project = Project::new(b"SHELL :=\n.SHELLFLAGS :=\nall:\n\t@./missing=name\n");
    let output = project.run(&[]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        output.stderr,
        b"/bin/sh: 1: ./missing=name: not found\nrmake: *** [Makefile:4: all] Error 127\n"
    );
}

#[test]
fn makelevel_startup_normalizes_imports_and_preserves_cli_flavor() {
    let source = b"SNAP := $(value MAKELEVEL)|$(origin MAKELEVEL)|$(flavor MAKELEVEL)\nall:\n\t@printf '%s|%s\\n' '$(SNAP)' \"$$MAKELEVEL\"\n";
    for (environment, assignment, expected) in [
        (None, None, b"0|environment|simple|1\n".as_slice()),
        (Some("007"), None, b"7|environment|simple|8\n"),
        (Some(" -1"), None, b"4294967295|environment|simple|0\n"),
        (None, Some("MAKELEVEL?=5"), b"5|command line|recursive|6\n"),
        (None, Some("MAKELEVEL+=2"), b"2|command line|recursive|3\n"),
        (
            None,
            Some("MAKELEVEL=$(intcmp 3,3)"),
            b"$(intcmp 3,3)|command line|recursive|1\n",
        ),
        (
            None,
            Some("MAKELEVEL:=$(intcmp 3,3)"),
            b"3|command line|simple|4\n",
        ),
    ] {
        let project = Project::new(source);
        let mut command = project.command();
        if let Some(value) = environment {
            command.env("MAKELEVEL", value);
        }
        if let Some(value) = assignment {
            command.arg(value);
        }
        let output = command.output().unwrap();
        assert!(output.status.success(), "{:?}", output.stderr);
        assert_eq!(output.stdout, expected);
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn makelevel_child_depth_ignores_file_override_and_unexport_but_keeps_export_effects() {
    let source = b"override MAKELEVEL = 19\nunexport MAKELEVEL\nall:\n\t@printf '%s|%s\\n' '$(MAKELEVEL)' \"$$MAKELEVEL\"\n";
    let project = Project::new(source);
    let output = project.command().env("MAKELEVEL", "7").output().unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"19|8\n");
    assert!(output.stderr.is_empty());
    for imported in [false, true] {
        let project =
            Project::new(b"MAKELEVEL = $(error exported)\nall:\n\t@printf '%s' \"$$MAKELEVEL\"\n");
        let mut command = project.command();
        if imported {
            command.env("MAKELEVEL", "7");
        }
        let output = command.output().unwrap();
        if imported {
            assert_eq!(output.status.code(), Some(2));
            assert!(output.stdout.is_empty());
            assert_eq!(output.stderr, b"Makefile:1: *** exported.  Stop.\n");
        } else {
            assert!(output.status.success());
            assert_eq!(output.stdout, b"1");
            assert!(output.stderr.is_empty());
        }
    }
}

#[test]
fn makelevel_runtime_prefix_uses_snapshot_but_cli_immediate_errors_are_unprefixed() {
    let project = Project::new(b"override MAKELEVEL = 99\nall:\n\t@false\n");
    let output = project.command().env("MAKELEVEL", "7").output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(output.stderr, b"rmake[7]: *** [Makefile:3: all] Error 1\n");
    let output = project
        .command()
        .env("MAKELEVEL", "7")
        .args(["MAKELEVEL=9", "X:=$(error cli)"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(output.stderr, b"rmake: *** cli.  Stop.\n");
    let project = Project::new(b"all: ;\n");
    let output = project.command().env("MAKELEVEL", "7").output().unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"rmake[7]: 'all' is up to date.\n");
}

#[test]
fn enoexec_fallback_preserves_the_path_entry_that_was_actually_executable() {
    let project = Project::new(b"export PATH := first:second:/usr/bin:/bin\nall:\n\t@plain\n");
    for (directory, contents, mode) in [
        ("first", b"printf first".as_slice(), 0o601),
        ("second", b"printf second".as_slice(), 0o755),
    ] {
        fs::create_dir(project.0.join(directory)).unwrap();
        let path = project.0.join(directory).join("plain");
        fs::write(&path, contents).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
    }
    let result = project.run(&[]);
    assert!(result.status.success(), "{:?}", result.stderr);
    // Root can execute when any execute bit is set. An ordinary owner cannot
    // use the 'other execute' bit in place of its missing owner-execute bit.
    let root = fs::metadata("/proc/self").unwrap().uid() == 0;
    assert_eq!(
        result.stdout,
        if root {
            b"first".as_slice()
        } else {
            b"second".as_slice()
        }
    );
}

#[test]
fn output_functions_survive_fatal_recipe_expansion_without_launching_any_recipe() {
    let project = Project::new(b"all:\n\t@touch forbidden\n\t@$(info retained\xff)$(warning before)$(error stop)\nlater:\n\t@touch later\n");
    let output = project.run(&[OsStr::new("-ki"), OsStr::new("all"), OsStr::new("later")]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(output.stdout, b"retained\xff\n");
    assert_eq!(
        output.stderr,
        b"Makefile:3: before\nMakefile:3: *** stop.  Stop.\n"
    );
    assert!(!project.0.join("forbidden").exists());
    assert!(!project.0.join("later").exists());
}

#[test]
fn runtime_output_uses_body_config_export_order_and_distinct_locations() {
    let project = Project::new(b"F = $(warning exported)\nexport SENT = $(F)\nSHELL = $(info shell)/bin/sh\n.SHELLFLAGS = $(info flags)-c\nIFS = $(info ifs) \nall:\n\t@$(info body)printf child\n");
    let output = project.run(&[]);
    assert!(output.status.success(), "{:?}", output.stderr);
    assert_eq!(output.stdout, b"body\nshell\nflags\nifs\nchild");
    assert_eq!(output.stderr, b"Makefile:2: exported\n");
    let output = project.run(&[OsStr::new("-q")]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(output.stdout, b"body\nshell\nflags\nifs\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn makefile_list_appends_raw_metadata_and_preserves_recursive_flavor() {
    let project = Project::new(b"MAKEFILE_LIST = $(warning untouched)old\n");
    fs::write(
        project.0.join("second.mk"),
        b"$(info [$(value MAKEFILE_LIST)]:[$(flavor MAKEFILE_LIST)])\nall:;@true\n",
    )
    .unwrap();
    let output = project.run(&[
        OsStr::new("-f"),
        OsStr::new("Makefile"),
        OsStr::new("-f"),
        OsStr::new("second.mk"),
    ]);
    assert!(output.status.success(), "{:?}", output.stderr);
    assert_eq!(
        output.stdout,
        b"[$(warning untouched)old second.mk]:[recursive]\n"
    );
    assert!(output.stderr.is_empty());
    let project =
        Project::new(b"$(info [$(value MAKEFILE_LIST)]:[$(flavor MAKEFILE_LIST)])\nall:;@true\n");
    let output = project
        .command()
        .env("MAKEFILE_LIST", "$(warning imported)old")
        .output()
        .unwrap();
    assert!(output.status.success(), "{:?}", output.stderr);
    assert_eq!(output.stdout, b"[Makefile]:[simple]\n");
    assert!(output.stderr.is_empty());
}
