// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

#[cfg(not(target_os = "linux"))]
compile_error!("rmake currently supports Linux only");

use std::io::{self, Write};
use std::process::ExitCode;

mod cli;
mod command_variables;
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

fn main() -> ExitCode {
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
    let output = std::env::var_os("MAKE_RESTARTS")
        .filter(|value| std::os::unix::ffi::OsStrExt::as_bytes(value.as_os_str()).starts_with(b"-"))
        .map(|_| output::Controller::new(output::Mode::None, &program));
    let process_output = output.as_ref().map(output::Controller::process_lifetime);
    let launch = match engine::Engine::invocation_with_output(
        args,
        &program,
        &invocation,
        std::env::var_os("MAKEFLAGS"),
        std::env::var_os("GNUMAKEFLAGS"),
        output,
    ) {
        Ok(launch) => launch,
        Err(error) => {
            engine::emit_invocation_error(&program, error);
            if let Some(process_output) = &process_output {
                process_output.finish();
            }
            drop(signals);
            return ExitCode::from(rmake_linux::signals::exit_status(2));
        }
    };
    let options = &launch.options;
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
        return ExitCode::SUCCESS;
    }
    if options.version {
        print_version();
        return ExitCode::SUCCESS;
    }
    let mut retained_engine = None;
    let status = match engine::Engine::from_invocation(launch, program.clone(), &invocation) {
        Ok(mut engine) => {
            let status = engine.run_retained();
            if crate::native::has_modules() {
                engine.prepare_native_exit();
                retained_engine = Some(engine);
            }
            status
        }
        Err(message) => {
            if rmake_linux::signals::pending().is_none() {
                let mut stderr = io::stderr().lock();
                let _ = stderr.write_all(&message.prefix(&program));
                let _ = stderr.write_all(b"*** ");
                let _ = stderr.write_all(message.message.as_bytes());
                let _ = stderr.write_all(b".  Stop.\n");
            }
            2
        }
    };
    if let Some(process_output) = &process_output {
        process_output.finish();
    }
    drop(signals);
    let status = rmake_linux::signals::exit_status(status);
    if crate::native::has_modules()
        && let Some(engine) = &mut retained_engine
    {
        return ExitCode::from(engine.native_exit(status));
    }
    ExitCode::from(status)
}

/// Branding is deliberately distinct; exit-phase ordering is still semantic.
pub(crate) fn print_version() {
    println!("rmake {}", env!("CARGO_PKG_VERSION"));
    println!(
        "GNU Make {} compatibility target (implementation incomplete)",
        rmake_core::COMPATIBILITY_VERSION
    );
    println!("License GPLv3+: GNU GPL version 3 or later.");
    println!("This is free software; there is no warranty.");
}
