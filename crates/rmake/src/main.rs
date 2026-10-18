// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

#[cfg(not(target_os = "linux"))]
compile_error!("rmake currently supports Linux only");

use std::process::ExitCode;

mod cli;
mod engine;

fn main() -> ExitCode {
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
        println!("  -C DIR     Change directory before reading Makefiles");
        println!("  -n / -s    Print commands without running / suppress command echo");
        println!("  -B / -q    Always rebuild / query whether recipes are needed");
        println!("  -i / -k    Ignore recipe errors / continue independent targets");
        println!("  -t / -e    Touch targets / let environment override file variables");
        println!("  -rR / -j1  Disable built-ins / execute serially");
        println!("\nDevelopment build: explicit serial rules and core variables only.");
        println!("Includes, implicit rules, effects and parallel jobs are not implemented.");
        return ExitCode::SUCCESS;
    }
    match engine::Engine::new(options, program.clone(), &invocation) {
        Ok(engine) => ExitCode::from(engine.run()),
        Err(message) => {
            eprintln!(
                "{}: *** {message}.  Stop.",
                String::from_utf8_lossy(&program)
            );
            ExitCode::from(2)
        }
    }
}
