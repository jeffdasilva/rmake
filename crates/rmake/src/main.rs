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
mod oneshell;
mod output;
mod process;
mod recipe_flow;
mod shell;

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
    let options = match cli::Options::with_environment(
        args,
        std::env::var_os("MAKEFLAGS"),
        std::env::var_os("GNUMAKEFLAGS"),
    ) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{}: {message}", String::from_utf8_lossy(&program));
            return ExitCode::from(2);
        }
    };
    if options.version {
        println!("rmake {}", env!("CARGO_PKG_VERSION"));
        println!(
            "GNU Make {} compatibility target (implementation incomplete)",
            rmake_core::COMPATIBILITY_VERSION
        );
        println!("License GPLv3+: GNU GPL version 3 or later.");
        println!("This is free software; there is no warranty.");
        return ExitCode::SUCCESS;
    }
    if options.help {
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
    let status = match engine::Engine::new(options, program.clone(), &invocation) {
        Ok(engine) => engine.run(),
        Err(message) => {
            if rmake_linux::signals::pending().is_none() {
                let mut stderr = io::stderr().lock();
                let _ = stderr.write_all(&program);
                let _ = stderr.write_all(b": *** ");
                let _ = stderr.write_all(message.as_bytes());
                let _ = stderr.write_all(b".  Stop.\n");
            }
            2
        }
    };
    drop(signals);
    ExitCode::from(rmake_linux::signals::exit_status(status))
}
