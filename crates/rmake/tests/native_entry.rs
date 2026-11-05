// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Existing native assertions run on the leading thread in enabled builds.
use rmake_core::diagnostic::{Location, Output};
use rmake_core::native::{NativeHost, NativeRuntime};
use rmake_core::variables::ExpandError;
use std::process::{Command, ExitCode, Stdio};
use std::time::{Duration, Instant};

#[path = "../src/panic_guard.rs"]
mod panic_guard;

const CASES: &[&str] = &[
    "independent-runtimes",
    "decoder_failure_retains_effects_and_last_policy_without_defaults_or_replay",
    "prepared_help_pass_keeps_constructor_unstarted_and_actual_bindings",
    "failed_constructor_keeps_only_metadata_published_before_home_expansion",
    "failed_chdir_keeps_early_database_without_late_constructor_metadata",
    "restarted_help_owns_fresh_decoder_bindings_and_restart_count",
];

#[allow(unsafe_code)]
fn child(case: String) {
    panic_guard::install();
    // SAFETY: a fresh leading-thread subprocess executes only the selected
    // existing assertion. Startup cases retain their actual phase through C
    // exit. The pinning case unloads every image before ordinary return.
    unsafe {
        rmake_linux::guile::run_host(move || {
            if case.starts_with("panic-") {
                panic_case(&case);
            } else {
                assert!(rmake_app::run_native_test_case(&case), "unknown case");
            }
        })
    }
    .expect("leading test host");
}

struct UnwindMarker;
impl Drop for UnwindMarker {
    fn drop(&mut self) {
        std::fs::write("unwound", b"unexpected\n").unwrap();
    }
}
struct PanicHost;
impl NativeHost for PanicHost {
    fn expand(&mut self, _: &[u8]) -> Result<Vec<u8>, ExpandError> {
        std::fs::write("entered-expand", b"entered\n").unwrap();
        panic!("deliberate host panic before unwinding");
    }
    fn evaluate(&mut self, _: &[u8], _: Option<&Location>) -> Result<(), ExpandError> {
        panic!("unexpected evaluation");
    }
    fn emit(&mut self, _: Output<'_>) -> bool {
        true
    }
}
fn panic_case(case: &str) {
    let _marker = UnwindMarker;
    if case == "panic-before-foreign" {
        panic!("deliberate early panic");
    }
    let native = std::rc::Rc::new(rmake_linux::native::Runtime::new());
    let mut host = PanicHost;
    let module = std::env::current_dir().unwrap().join("panic_host.so");
    use std::os::unix::ffi::OsStrExt;
    native
        .load(
            module.as_os_str().as_bytes(),
            false,
            &Location {
                file: b"panic-check.mk".as_slice().into(),
                line: 1,
            },
            &mut host,
        )
        .unwrap();
    let runtime = rmake_linux::guile::Runtime::from_native(native).unwrap();
    let _ = runtime.evaluate(b"(gmk-expand \"panic\")", &mut host);
    std::fs::write("resumed", b"unexpected\n").unwrap();
    rmake_linux::guile::abort_host();
}

fn check_panics() {
    let root = std::env::temp_dir().join(format!("rmake-entry-panics-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let source = root.join("module.c");
    std::fs::write(
        &source,
        br#"/* Original finalizer witness, no GNU headers. */
#include <stdio.h>
int plugin_is_GPL_compatible = 1;
int panic_host_gmk_setup(const void *location) { (void)location; return 1; }
__attribute__((destructor)) static void finish(void) {
  FILE *file=fopen("finalizer", "w"); if(file) {fputs("unexpected\n",file); fclose(file);}
}
"#,
    )
    .unwrap();
    assert!(
        Command::new("cc")
            .args([
                "-std=c11", "-Wall", "-Wextra", "-Werror", "-fPIC", "-shared"
            ])
            .arg(&source)
            .arg("-o")
            .arg(root.join("panic_host.so"))
            .status()
            .unwrap()
            .success()
    );
    for case in ["panic-before-foreign", "panic-during-expand"] {
        let mut process = Command::new(std::env::current_exe().unwrap())
            .current_dir(&root)
            .env("RMAKE_NATIVE_ENTRY_CASE", case)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let started = Instant::now();
        while process.try_wait().unwrap().is_none() {
            if started.elapsed() > Duration::from_secs(10) {
                process.kill().unwrap();
                panic!("panic child exceeded bound");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let output = process.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(101), "{case}");
        assert!(output.stdout.is_empty(), "{case}");
        assert_eq!(output.stderr, b"rmake: panic in Guile host\n", "{case}");
        assert_eq!(
            root.join("entered-expand").exists(),
            case == "panic-during-expand",
            "{case}"
        );
        for name in ["unwound", "resumed", "finalizer"] {
            assert!(!root.join(name).exists(), "{case}: {name}");
        }
        println!("PASS native entry {case}");
    }
    std::fs::remove_dir_all(&root).unwrap();
}

fn main() -> ExitCode {
    if let Some(case) = std::env::var_os("RMAKE_NATIVE_ENTRY_CASE") {
        child(case.into_string().expect("literal test case"));
        return ExitCode::SUCCESS;
    }
    for case in CASES {
        let mut process = Command::new(std::env::current_exe().unwrap())
            .env("RMAKE_NATIVE_ENTRY_CASE", case)
            .env("RMAKE_STARTUP_OWNER_UNIT_CHILD", "1")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let started = Instant::now();
        loop {
            if process.try_wait().unwrap().is_some() {
                break;
            }
            if started.elapsed() > Duration::from_secs(10) {
                process.kill().unwrap();
                panic!("leading native case exceeded bound: {case}");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let output = process.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{case}: status {:?}\n{}\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        println!("PASS native entry {case}");
    }
    check_panics();
    ExitCode::SUCCESS
}
