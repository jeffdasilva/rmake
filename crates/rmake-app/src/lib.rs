// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

#[cfg(not(target_os = "linux"))]
compile_error!("rmake currently supports Linux only");

use std::process::ExitCode;

mod cli;
mod command_variables;
mod debug;
mod engine;
mod file_aliases;
mod intermediates;
mod load;
mod makefiles;
mod native;
mod oneshell;
mod output;
mod process;
mod recipe_flow;
mod shell;
mod shuffle;

/// Concrete semantic entry used by the reviewed process bootstrap.
pub fn run() -> ExitCode {
    rmake_linux::output::initialize();
    let signals = match rmake_linux::signals::Guard::install() {
        Ok(guard) => guard,
        Err(error) => {
            eprintln!("rmake: signal setup: {error}");
            return ExitCode::from(2);
        }
    };
    let mut args = std::env::args_os();
    let invocation = args.next().unwrap_or_else(|| "rmake".into());
    let program = cli::invocation_name(&invocation);
    let output = output::Controller::new(output::Mode::None, &program);
    output.bind_debug_signal(signals.debug_handle());
    let process_output = output.process_lifetime();
    let (status, mut native_exit) = match engine::Engine::invocation_with_output(
        args,
        &program,
        &invocation,
        std::env::var_os("MAKEFLAGS"),
        std::env::var_os("GNUMAKEFLAGS"),
        Some(output.clone()),
    ) {
        Err(failure) => {
            let (error, exit) = failure.into_parts();
            engine::emit_invocation_error(&output, &program, error);
            (2, exit)
        }
        Ok(launch) if print_requested_exit(&launch.options) => {
            (0, launch.into_native_exit(program.clone()))
        }
        Ok(launch) => match engine::Engine::from_invocation(launch, program.clone(), &invocation) {
            Ok(mut engine) => {
                let status = engine.run_retained();
                let exit = if crate::native::has_modules() {
                    Some(Box::new(engine::NativeExit::Engine(Box::new(engine))))
                } else {
                    None
                };
                (status, exit)
            }
            Err(failure) => {
                let (message, exit) = failure.into_parts();
                if rmake_linux::signals::pending().is_none() {
                    output.write_parts(
                        true,
                        &[
                            &message.prefix(&program),
                            b"*** ",
                            message.message.as_bytes(),
                            b".  Stop.\n",
                        ],
                    );
                }
                (2, exit)
            }
        },
    };
    if let Some(exit) = &mut native_exit {
        exit.prepare();
    }
    process_output.finish();
    #[cfg(feature = "guile")]
    if crate::native::has_modules() {
        let Some(exit) = native_exit.as_mut() else {
            rmake_linux::guile::abort_host();
        };
        // The signal guard and the actual phase host remain alive while C
        // finalizers or the platform's fatal signal termination run.
        match rmake_linux::signals::exit_action(status) {
            rmake_linux::signals::ExitAction::Code(status) => {
                exit.finish(status);
            }
            rmake_linux::signals::ExitAction::Signal(number) => {
                exit.finish_signal(number);
            }
        }
        // Foreign lifetime may not escape the private host stack on return.
        rmake_linux::guile::abort_host();
    }
    let status = rmake_linux::signals::exit_status(status);
    let status = native_exit
        .as_mut()
        .map_or(status, |exit| exit.finish(status));
    drop(signals);
    ExitCode::from(status)
}

pub(crate) fn print_requested_exit(options: &cli::Options) -> bool {
    if (options.help || options.version) && crate::native::has_modules() {
        let _ = rmake_linux::native::flush_stream(false);
    }
    if options.help {
        if options.version {
            print_version();
            println!();
        }
        println!("Usage: rmake [OPTIONS] [TARGETS]");
        println!("  --help     Show this help");
        println!("  --version  Show rmake version and compatibility target");
        println!("  -f FILE    Read the specified Makefile (may be repeated)");
        println!("  -E STRING  Evaluate Makefile text before reading files (--eval)");
        println!("  --trace    Print rebuild reasons and recipe commands");
        println!("  --warn-undefined-variables  Diagnose missing variable references");
        println!("  -C DIR     Change directory before reading Makefiles");
        println!("  -w / --no-print-directory  Enable / suppress directory reports");
        println!("  -I DIR     Search for included Makefiles; -I- resets the search path");
        println!("  -L         Include symlink timestamps (--check-symlink-times)");
        println!("  -o FILE    Ignore this file and its rules (--old-file)");
        println!("  -W FILE    Assume this file is newest (--what-if)");
        println!("  -n / -s    Print commands without running / suppress command echo");
        println!("  -B / -q    Always rebuild / query whether recipes are needed");
        println!("  -i / -k    Ignore recipe errors / continue independent targets");
        println!("  -t / -e    Touch targets / let environment override file variables");
        println!("  -rR / -jN  Disable built-ins / set shared local job limit");
        println!("  -O[TYPE]   Synchronize output: line, target (default), recurse, none");
        println!("  -l[LOAD]   Limit parallel launches by Linux runnable load");
        println!("  --jobserver-style=fifo|pipe  Select Linux jobserver transport");
        println!("\nDevelopment build: GNU Make compatibility is incomplete.");
        println!("Complete runtime compatibility remains unfinished.");
        return true;
    }
    if options.version {
        print_version();
        return true;
    }
    false
}

/// Branding is deliberately distinct; exit-phase ordering is still semantic.
pub(crate) fn print_version() {
    if crate::native::has_modules() {
        let _ = rmake_linux::native::flush_stream(false);
    }
    println!("rmake {}", env!("CARGO_PKG_VERSION"));
    println!(
        "GNU Make {} compatibility target (implementation incomplete)",
        rmake_core::COMPATIBILITY_VERSION
    );
    println!("License GPLv3+: GNU GPL version 3 or later.");
    println!("This is free software; there is no warranty.");
}

/// Internal leading-thread test entry; never used by the product CLI.
#[cfg(feature = "test-support")]
#[doc(hidden)]
pub fn run_native_test_case(name: &str) -> bool {
    if name == "independent-runtimes" {
        native::run_native_safety_check();
        true
    } else {
        engine::run_native_ownership_check(name)
    }
}
