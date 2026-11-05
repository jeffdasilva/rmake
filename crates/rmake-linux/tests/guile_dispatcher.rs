// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva
#![allow(unsafe_code)]
use rmake_core::diagnostic::{ExpansionContext, Location, Output};
use rmake_core::native::{NativeHost, NativeRuntime};
use rmake_core::variables::{Assignment, ExpandError, Origin, VariableStore};
use rmake_linux::guile::{Error, Runtime, run_host};
use std::ffi::c_int;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

unsafe extern "C" {
    fn gettid() -> c_int;
    fn signal(number: c_int, handler: usize) -> usize;
    fn kill(pid: c_int, number: c_int) -> c_int;
}
struct Host<'a> {
    runtime: &'a Runtime,
    tid: c_int,
    events: Vec<Vec<u8>>,
    handled: bool,
    worker_fatal: bool,
}
impl NativeHost for Host<'_> {
    fn expand(&mut self, input: &[u8]) -> Result<Vec<u8>, ExpandError> {
        assert_eq!(unsafe { gettid() }, self.tid);
        assert_eq!(self.tid as u32, std::process::id());
        assert!(stacker::remaining_stack().unwrap() > 1024 * 1024);
        self.events.push(input.to_vec());
        if input == b"worker-identity" {
            return self
                .runtime
                .evaluate(b"(eq? (current-thread) worker-id)", self)
                .map_err(|_| ExpandError::Unsupported(b"worker identity".to_vec()));
        }
        if input == b"worker-module" {
            return self
                .runtime
                .evaluate(b"(module-name (current-module))", self)
                .map_err(|_| ExpandError::Unsupported(b"worker module".to_vec()));
        }
        if input == b"panic" {
            panic!("component host panic");
        }
        if input == b"handled?" {
            return Ok(if self.handled {
                b"yes".as_slice()
            } else {
                b"no".as_slice()
            }
            .to_vec());
        }
        if input == b"signal-wait" {
            return stacker::grow(4 * 1024 * 1024, || self.runtime.evaluate(SIGNAL_LOOP, self))
                .map_err(|_| ExpandError::Cancelled);
        }
        if input == b"swallow" {
            assert!(
                self.runtime
                    .evaluate(b"(gmk-eval \"fatal\")", self)
                    .is_err()
            );
            return Ok(b"must-not-resume".to_vec());
        }
        if input == b"nested" || input == b"native-nested" {
            return stacker::grow(4 * 1024 * 1024, || {
                self.runtime
                    .evaluate(b"(begin (gc) (gmk-expand \"value\"))", self)
            })
            .map_err(|_| ExpandError::Unsupported(b"nested component failure".to_vec()));
        }
        if input == b"finalizer" {
            if self.worker_fatal {
                assert_eq!(
                    self.runtime
                        .evaluate(b"(eq? (current-thread) worker-id)", self)
                        .unwrap(),
                    b"#t"
                );
            }
            return self
                .runtime
                .evaluate(b"(begin (gc) \"host\")", self)
                .map_err(|_| ExpandError::Unsupported(b"finalizer component failure".to_vec()));
        }
        Ok(if input == b"value" {
            b"host".to_vec()
        } else {
            input.to_vec()
        })
    }
    fn evaluate(&mut self, input: &[u8], location: Option<&Location>) -> Result<(), ExpandError> {
        assert_eq!(unsafe { gettid() }, self.tid);
        assert!(location.is_none());
        self.events.push(input.to_vec());
        if input == b"fatal" {
            return Err(ExpandError::Unsupported(b"component fatal".to_vec()));
        }
        if input == b"handled" {
            self.handled = true;
        }
        Ok(())
    }
    fn emit(&mut self, _: Output<'_>) -> bool {
        true
    }
}

const SIGNAL_LOOP:&[u8]=b"(begin (call-with-output-file \"ready\" (lambda (p) (display \"ready\\n\" p))) (let loop () (loop)))";
fn has_native(case: &str) -> bool {
    case.starts_with("native-")
        || matches!(
            case,
            "signal-native" | "signal-scheme-native" | "signal-constructor" | "signal-worker-init"
        )
}

fn child(case: &str) {
    std::panic::set_hook(Box::new(|information| {
        if std::env::var_os("RMAKE_GUILE_PROTO_DIAGNOSTICS").is_some() {
            eprintln!("prototype assertion: {information}");
        }
        rmake_linux::guile::abort_host();
    }));
    let runtime = Runtime::new().unwrap();
    let native = runtime.native();
    let mut host = Host {
        runtime: &runtime,
        tid: unsafe { gettid() },
        events: Vec::new(),
        handled: false,
        worker_fatal: case == "native-worker-fatal",
    };
    assert_eq!(host.tid as u32, std::process::id());
    assert!(!runtime.initialized());
    if case == "empty" {
        assert!(runtime.evaluate(b"", &mut host).unwrap().is_empty());
        assert!(runtime.initialized());
    }
    if case == "registry" {
        native.register_guile().unwrap();
        let declaration = native.lookup(b"guile").unwrap();
        assert_eq!(declaration.minimum, 0);
        assert_eq!(declaration.maximum, Some(1));
        assert!(declaration.expand_arguments);
        assert!(!runtime.initialized());
        let mut variables = VariableStore::new();
        variables
            .define(b"guile", b"shadow", Assignment::Simple, Origin::File)
            .unwrap();
        let mut events = Vec::new();
        let mut emit = |output: Output<'_>| {
            if let Output::Info(value) = output {
                events.push(value.to_vec())
            }
        };
        let actual=variables.expand_with(b"$(guile)|$(call guile)|$(guile \"cedar,elm\")|$(guile '(north (#f) south))|$(call guile,631,$(info eager))|$(guile (gmk-expand 631))|$(foreach VALUE,loop,$(guile (gmk-expand \"$$(VALUE)\")))",ExpansionContext::new(None,&mut emit).with_native_runtime(native.clone())).unwrap();
        assert_eq!(actual, b"shadow||cedar,elm|north south|631|631|loop");
        let mut database = rmake_core::parser::Database::default();
        database.variables = variables;
        database
            .variables
            .define(b"VALUE", b"outer", Assignment::Simple, Origin::File)
            .unwrap();
        assert_eq!(database.expand_with(b"$(guile (begin (use-modules (ice-9 threads)) (join-thread (call-with-new-thread (lambda () (gmk-eval \"VALUE := worker\") (string-append (gmk-expand \"$$(VALUE)\") \"|\" (gmk-expand \"$$(guile (+ 17 19))\")))))))", ExpansionContext::new(None, &mut emit).with_native_runtime(native.clone())).unwrap(), b"worker|36");
        assert_eq!(events, [b"eager".to_vec()]);
    }
    if case.starts_with("signal") {
        assert_ne!(
            unsafe {
                signal(
                    15,
                    rmake_linux::guile::component_signal_handler() as *const () as usize,
                )
            },
            usize::MAX
        );
    }
    if has_native(case) {
        let module = std::env::current_dir().unwrap().join("native_guile.so");
        let location = Location {
            file: b"component.mk".as_slice().into(),
            line: 1,
        };
        let loaded = NativeRuntime::load(
            &*native,
            module.as_os_str().as_bytes(),
            false,
            &location,
            &mut host,
        );
        if matches!(case, "signal-constructor" | "signal-worker-init") {
            assert!(matches!(loaded, Err(ExpandError::Cancelled)));
            assert!(!runtime.initialized());
            assert_eq!(rmake_linux::guile::pending_signal(), Some(15));
            std::fs::write("cleaned", b"cleaned\n").unwrap();
            println!("CASE={case}:ok");
            native.terminate_signal(15, &mut host).unwrap();
            unreachable!();
        }
        loaded.unwrap();
        assert!(native.has_modules());
        let function = native.lookup(b"native_guile").unwrap();
        assert_eq!(
            function
                .callback
                .invoke(b"native_guile", &[], &mut host)
                .unwrap(),
            b"hostkept"
        );
        if case == "native-unload" {
            assert!(
                NativeRuntime::unload(&*native, module.as_os_str().as_bytes(), &mut host).unwrap()
            );
        }
        if case == "native-worker-fatal" {
            assert!(runtime.evaluate(b"(begin (use-modules (ice-9 threads)) (define worker-id #f) (join-thread (call-with-new-thread (lambda () (set! worker-id (current-thread)) (gmk-eval \"fatal\") (gmk-eval \"forbidden\")))))", &mut host).is_err());
        }
        if case == "native-fatal" {
            assert!(
                runtime
                    .evaluate(
                        b"(begin (gmk-eval \"fatal\") (gmk-eval \"forbidden\") \"bad\")",
                        &mut host
                    )
                    .is_err()
            );
        }
        if case == "native-panic" {
            runtime
                .evaluate(b"(gmk-expand \"panic\")", &mut host)
                .unwrap();
            unreachable!();
        }
    } else if case != "empty" && case != "signal-preinit" {
        assert_eq!(
            runtime.evaluate(b"\"initial\"", &mut host).unwrap(),
            b"initial"
        );
        assert!(runtime.initialized());
        assert!(native.has_modules());
        match case {
            "registry" => {}
            "workers" => {
                assert_eq!(runtime.evaluate(b"(begin (use-modules (ice-9 threads)) (join-thread (call-with-new-thread (lambda () (gmk-expand \"value\")))))", &mut host).unwrap(),b"host");
                assert!(runtime.evaluate(b"(join-thread (call-with-new-thread (lambda () (gmk-eval \"handled\"))))", &mut host).unwrap().is_empty());
                assert!(host.handled);
                assert_eq!(runtime.evaluate(b"(begin (define worker-id #f) (join-thread (call-with-new-thread (lambda () (set! worker-id (current-thread)) (string-append (gmk-expand \"worker-identity\") \"/\" (gmk-expand \"nested\"))))))", &mut host).unwrap(),b"#t/host");
                assert_eq!(runtime.evaluate(b"(join-thread (call-with-new-thread (lambda () (set-current-module (resolve-module '(gnu make))) (gmk-expand \"worker-module\"))))", &mut host).unwrap(),b"gnu make");
            }
            "namespace" => {
                assert_eq!(
                    runtime
                        .evaluate(b"(module-name (current-module))", &mut host)
                        .unwrap(),
                    b"guile-user"
                );
                assert!(
                    runtime
                        .evaluate(b"(begin (define retained \"cedar\") #f)", &mut host)
                        .unwrap()
                        .is_empty()
                );
                assert_eq!(runtime.evaluate(b"retained", &mut host).unwrap(), b"cedar");
                assert!(
                    runtime
                        .evaluate(
                            b"(begin (set-current-module (resolve-module '(gnu make))) #f)",
                            &mut host
                        )
                        .unwrap()
                        .is_empty()
                );
                assert_eq!(
                    runtime
                        .evaluate(b"(module-name (current-module))", &mut host)
                        .unwrap(),
                    b"guile-user"
                );
            }
            "conversions" => {
                for (source, expected) in [
                    ("#f", ""),
                    ("#t", "#t"),
                    ("631", "631"),
                    (
                        "123456789012345678901234567890123456789",
                        "123456789012345678901234567890123456789",
                    ),
                    ("7/3", "7/3"),
                    ("4+3i", "4.0+3.0i"),
                    ("'north", "north"),
                    ("#\\Q", "Q"),
                    ("#\\space", " "),
                    ("#\\newline", "\n"),
                    ("\"left\\nright\\tend\"", "left\nright\tend"),
                    ("'(north (west east) south)", "north west east south"),
                    ("'(north () (#f) south)", "north south"),
                    ("'(#f north #t #f)", "north #t"),
                    ("(list \"north\" \"\" \"south\")", "north south"),
                    ("(list \"north\" \" \" \"south\")", "north   south"),
                    ("(cons 'north 'south)", "north south"),
                    ("(if #f \"unused\")", ""),
                    ("(gmk-expand 631)", "631"),
                    ("(gmk-expand '(north (#f) south))", "north south"),
                ] {
                    assert_eq!(
                        runtime.evaluate(source.as_bytes(), &mut host).unwrap(),
                        expected.as_bytes(),
                        "{source}"
                    );
                }
                assert!(
                    runtime
                        .evaluate(b"(gmk-eval '(VALUE := changed))", &mut host)
                        .unwrap()
                        .is_empty()
                );
                assert_eq!(host.events.last().unwrap(), b"VALUE := changed");
                assert_eq!(runtime.evaluate(b"(catch #t (lambda () (gmk-expand #(7 19))) (lambda args \"vector-rejected\"))",&mut host).unwrap(),b"vector-rejected");
            }
            "signal" | "signal-nested" => {}
            "nested" => {
                assert_eq!(runtime.evaluate(b"(let ((held (make-vector 10000 \"kept\"))) (string-append (gmk-expand \"nested\") (vector-ref held 5000)))",&mut host).unwrap(),b"hostkept");
                assert_eq!(host.events, [b"nested".to_vec(), b"value".to_vec()]);
                assert_eq!(
                    runtime
                        .evaluate(
                            b"(string-append \"a\" (gmk-eval \"define\") \"b\")",
                            &mut host
                        )
                        .unwrap(),
                    b"ab"
                );
            }
            "exceptions" => {
                assert!(matches!(
                    runtime.evaluate(b"(error \"component-stop\")", &mut host),
                    Err(Error::Scheme(_))
                ));
                assert_eq!(runtime.evaluate(b"(begin (define saved #f) (call/cc (lambda (k) (set! saved k) \"captured\")))",&mut host).unwrap(),b"captured");
                assert!(matches!(
                    runtime.evaluate(b"(saved \"forbidden\")", &mut host),
                    Err(Error::Scheme(_))
                ));
                assert_eq!(runtime.evaluate(b"\"after\"", &mut host).unwrap(), b"after");
            }
            "gc" => {
                assert_eq!(runtime.evaluate(b"(begin (define retained (make-vector 10000 \"kept\")) (gc) (vector-ref retained 9999))",&mut host).unwrap(),b"kept");
                assert_eq!(
                    runtime
                        .evaluate(b"(begin (gc) (vector-ref retained 0))", &mut host)
                        .unwrap(),
                    b"kept"
                );
            }
            "threads" => {
                assert_eq!(runtime.evaluate(b"(begin (use-modules (ice-9 threads)) (number->string (length (all-threads))))",&mut host).unwrap(),b"2");
            }
            "fatal" => {
                assert!(matches!(
                    runtime.evaluate(
                        b"(begin (gmk-eval \"fatal\") (gmk-eval \"forbidden\") \"bad\")",
                        &mut host
                    ),
                    Err(Error::Host(_))
                ));
                assert_eq!(host.events, [b"fatal".to_vec()]);
                assert!(runtime.evaluate(b"\"forbidden\"", &mut host).is_err());
            }
            "nested-fatal" => {
                assert!(
                    runtime
                        .evaluate(
                            b"(begin (gmk-expand \"swallow\") (gmk-eval \"forbidden\") \"bad\")",
                            &mut host
                        )
                        .is_err()
                );
                assert_eq!(host.events, [b"swallow".to_vec(), b"fatal".to_vec()]);
            }
            _ => panic!("unknown component case"),
        }
    }
    if case.starts_with("signal") {
        if case == "signal-preinit" {
            std::fs::write("ready", b"ready\n").unwrap();
            let deadline = Instant::now() + Duration::from_secs(5);
            while rmake_linux::guile::pending_signal().is_none() {
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(1));
            }
            assert!(matches!(
                runtime.evaluate(b"(gmk-eval \"forbidden\")", &mut host),
                Err(Error::Host(ExpandError::Cancelled))
            ));
            assert!(!runtime.initialized());
        } else if case == "signal-scheme-native" {
            let source=b"(begin (sigaction SIGTERM (lambda (s) (gmk-eval \"handled\"))) (call-with-output-file \"ready\" (lambda (p) (display \"ready\\n\" p))) (let loop () (if (string=? (gmk-expand \"handled?\") \"yes\") \"done\" (loop))))";
            assert_eq!(runtime.evaluate(source, &mut host).unwrap(), b"done");
            assert!(host.handled);
            assert!(rmake_linux::guile::pending_signal().is_none());
        } else {
            let source = if case == "signal-nested" {
                b"(begin (gmk-expand \"signal-wait\") (gmk-eval \"forbidden\") \"bad\")".as_slice()
            } else {
                SIGNAL_LOOP
            };
            assert!(matches!(
                runtime.evaluate(source, &mut host),
                Err(Error::Host(ExpandError::Cancelled))
            ));
            assert_eq!(rmake_linux::guile::pending_signal(), Some(15));
            assert!(
                runtime
                    .evaluate(b"(gmk-eval \"forbidden\")", &mut host)
                    .is_err()
            );
        }
        std::fs::write("cleaned", b"cleaned\n").unwrap();
    }
    assert!(!host.events.iter().any(|event| event == b"forbidden"));
    println!("CASE={case}:ok");
    if case.starts_with("signal") && case != "signal-scheme-native" {
        native.terminate_signal(15, &mut host).unwrap();
        unreachable!();
    }
    // Remain inside the grown host closure through C exit and every finalizer.
    native.exit(0, &mut host).unwrap();
    unreachable!();
}

fn wait(mut process: std::process::Child) -> std::process::Output {
    let deadline = Instant::now() + Duration::from_secs(15);
    while process.try_wait().unwrap().is_none() {
        if Instant::now() > deadline {
            process.kill().unwrap();
            panic!("component bound: {:?}", process.wait_with_output().unwrap());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    process.wait_with_output().unwrap()
}
fn compile(directory: &Path, case: &str) {
    let prefix = PathBuf::from(std::env::var_os("RMAKE_GUILE_PREFIX").unwrap());
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/guile/native.c");
    let library = prefix.join("usr/lib/x86_64-linux-gnu");
    let mut command = Command::new("cc");
    if case == "signal-constructor" {
        command.arg("-DCONSTRUCTOR_GUILE_SIGNAL");
    }
    if case == "signal-worker-init" {
        command.arg("-DWORKER_GUILE_FIRST");
    }
    let output = wait(
        command
            .args([
                "-std=c11", "-Wall", "-Wextra", "-Werror", "-fPIC", "-shared",
            ])
            .arg(source)
            .arg("-I")
            .arg(prefix.join("usr/include"))
            .arg("-I")
            .arg(prefix.join("usr/include/x86_64-linux-gnu"))
            .arg("-I")
            .arg(prefix.join("usr/include/guile/3.0"))
            .arg("-L")
            .arg(&library)
            .arg("-lguile-3.0")
            .arg(format!("-Wl,-rpath,{}", library.display()))
            .arg("-o")
            .arg(directory.join("native_guile.so"))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    assert!(output.status.success(), "{output:?}");
}
fn main() {
    if let Ok(case) = std::env::var("RMAKE_GUILE_COMPONENT_CASE") {
        if case == "outside" {
            struct RejectedHost;
            impl NativeHost for RejectedHost {
                fn expand(&mut self, _: &[u8]) -> Result<Vec<u8>, ExpandError> {
                    panic!("unexpected host request")
                }
                fn evaluate(&mut self, _: &[u8], _: Option<&Location>) -> Result<(), ExpandError> {
                    panic!("unexpected host request")
                }
                fn emit(&mut self, _: Output<'_>) -> bool {
                    panic!("unexpected output")
                }
            }
            let native = rmake_linux::native::Runtime::new();
            native.register_guile().unwrap();
            assert!(!native.has_modules());
            let declaration = native.lookup(b"guile").unwrap();
            assert_eq!(declaration.minimum, 0);
            assert_eq!(declaration.maximum, Some(1));
            assert!(declaration.expand_arguments);
            assert!(
                matches!(declaration.callback.invoke(b"guile", &[b"42".to_vec()], &mut RejectedHost),Err(ExpandError::FileError(message)) if message==b"rmake: Guile-enabled native operations require the leading-C dispatcher")
            );
            let location = Location {
                file: b"outside.mk".as_slice().into(),
                line: 1,
            };
            assert!(
                matches!(NativeRuntime::load(&native,b"must-not-open.so",false,&location,&mut RejectedHost),Err(ExpandError::FileError(message)) if message==b"rmake: Guile-enabled native operations require the leading-C dispatcher")
            );
            println!("CASE=outside:ok");
            return;
        }
        // SAFETY: this process-leading main enters before any foreign call;
        // child retains its host/runtime and exits through native C dispatch.
        unsafe { run_host(|| child(&case)) }.unwrap();
        return;
    }
    assert!(Runtime::new().is_err());
    for case in [
        "outside",
        "empty",
        "registry",
        "namespace",
        "conversions",
        "workers",
        "nested",
        "exceptions",
        "gc",
        "threads",
        "fatal",
        "nested-fatal",
        "native-exit",
        "native-unload",
        "native-fatal",
        "native-worker-fatal",
        "native-panic",
        "signal",
        "signal-native",
        "signal-nested",
        "signal-scheme-native",
        "signal-preinit",
        "signal-constructor",
        "signal-worker-init",
    ] {
        let root = std::env::temp_dir().join(format!(
            "rmake-guile-component-{}-{case}",
            std::process::id()
        ));
        std::fs::create_dir(&root).unwrap();
        if has_native(case) {
            compile(&root, case);
        }
        let mut process = Command::new(std::env::current_exe().unwrap())
            .env("RMAKE_GUILE_COMPONENT_CASE", case)
            .current_dir(&root)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        if case.starts_with("signal") {
            let deadline = Instant::now() + Duration::from_secs(5);
            while std::fs::read(root.join("ready")).ok().as_deref() != Some(b"ready\n") {
                if process.try_wait().unwrap().is_some() || Instant::now() > deadline {
                    let _ = process.kill();
                    panic!(
                        "signal readiness {case}: {:?}",
                        process.wait_with_output().unwrap()
                    );
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            assert_eq!(std::fs::read(root.join("ready")).unwrap(), b"ready\n");
            assert_eq!(unsafe { kill(process.id() as c_int, 15) }, 0);
        }
        let output = wait(process);
        if case.starts_with("signal") {
            assert_eq!(std::fs::read(root.join("cleaned")).unwrap(), b"cleaned\n");
        }
        std::fs::remove_dir_all(&root).unwrap();
        if case == "native-panic" {
            assert_eq!(output.status.code(), Some(101), "{output:?}");
            assert_eq!(output.stdout, b"");
            assert_eq!(output.stderr, b"rmake: panic in Guile host\n");
            continue;
        }
        if case.starts_with("signal") && case != "signal-scheme-native" {
            assert_eq!(output.status.signal(), Some(15), "{case}: {output:?}");
        } else {
            assert!(output.status.success(), "{case}: {output:?}");
        }
        assert_eq!(
            output.stdout,
            format!("CASE={case}:ok\n").as_bytes(),
            "{case}"
        );
        let stderr = if case.starts_with("native-") || case == "signal-scheme-native" {
            b"native-finalizer:hostkept\n".as_slice()
        } else {
            b""
        };
        assert_eq!(output.stderr, stderr, "{case}");
    }
}
