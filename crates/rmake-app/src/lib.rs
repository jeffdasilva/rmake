// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

#[cfg(not(target_os = "linux"))]
compile_error!("rmake currently supports Linux only");

use std::process::ExitCode;

mod cli;
mod command_variables;
mod debug;
#[allow(dead_code)] // Samples are used by the catalog build script.
mod debug_messages;
mod engine;
mod file_aliases;
mod intermediates;
mod load;
mod makefiles;
mod messages;
use rmake_core::messages as system_messages;
mod native;
mod oneshell;
mod output;
mod process;
mod recipe_flow;
mod shell;
mod shuffle;
mod stdin_source;
mod usage;

/// Concrete semantic entry used by the reviewed process bootstrap.
pub fn run() -> ExitCode {
    if let Err(error) =
        rmake_linux::messages::initialize(std::path::Path::new(env!("RMAKE_LOCALEDIR")))
    {
        eprintln!("rmake: {error}");
        return ExitCode::from(2);
    }
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
    stdin_source::initialize(&output, &program);
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
        Ok(launch) => {
            rmake_linux::native::initialize_stdio();
            match engine::Engine::from_invocation(launch, program.clone(), &invocation) {
                Ok(mut engine) => {
                    let status = engine.run_retained();
                    let exit = if engine.requires_retained_exit() {
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
                                &messages::diagnostic(&message.message),
                                &messages::stop_for(&message.message),
                            ],
                        );
                    }
                    (2, exit)
                }
            }
        }
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
            let _ = rmake_linux::output::write_standard(false, b"\n");
        }
        let _ = rmake_linux::output::write_standard(
            false,
            concat!(
                "Usage: rmake [OPTIONS] [TARGETS]\n",
                "  --help     Show this help\n",
                "  --version  Show rmake version and compatibility target\n",
                "  -f FILE    Read the specified Makefile (may be repeated)\n",
                "  -E STRING  Evaluate Makefile text before reading files (--eval)\n",
                "  --trace    Print rebuild reasons and recipe commands\n",
                "  --warn-undefined-variables  Diagnose missing variable references\n",
                "  -C DIR     Change directory before reading Makefiles\n",
                "  -w / --no-print-directory  Enable / suppress directory reports\n",
                "  -I DIR     Search for included Makefiles; -I- resets the search path\n",
                "  -L         Include symlink timestamps (--check-symlink-times)\n",
                "  -o FILE    Ignore this file and its rules (--old-file)\n",
                "  -W FILE    Assume this file is newest (--what-if)\n",
                "  -n / -s    Print commands without running / suppress command echo\n",
                "  -B / -q    Always rebuild / query whether recipes are needed\n",
                "  -i / -k    Ignore recipe errors / continue independent targets\n",
                "  -t / -e    Touch targets / let environment override file variables\n",
                "  -rR / -jN  Disable built-ins / set shared local job limit\n",
                "  -O[TYPE]   Synchronize output: line, target (default), recurse, none\n",
                "  -l[LOAD]   Limit parallel launches by Linux runnable load\n",
                "  --jobserver-style=fifo|pipe  Select Linux jobserver transport\n",
                "\nDevelopment build: GNU Make compatibility is incomplete.\n",
                "Complete runtime compatibility remains unfinished.\n",
            )
            .as_bytes(),
        );
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
    let text = format!(
        concat!(
            "rmake {}\n",
            "GNU Make {} compatibility target (implementation incomplete)\n",
            "License GPLv3+: GNU GPL version 3 or later.\n",
            "This is free software; there is no warranty.\n",
        ),
        env!("CARGO_PKG_VERSION"),
        rmake_core::COMPATIBILITY_VERSION
    );
    let _ = rmake_linux::output::write_standard(false, text.as_bytes());
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
