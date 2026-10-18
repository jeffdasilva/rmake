// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

#[cfg(not(target_os = "linux"))]
compile_error!("rmake currently supports Linux only");

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() == 1 && args[0] == "--version" {
        println!("rmake {}", env!("CARGO_PKG_VERSION"));
        println!(
            "GNU Make {} compatibility target (implementation incomplete)",
            rmake_core::COMPATIBILITY_VERSION
        );
        println!("License GPLv3+: GNU GPL version 3 or later.");
        println!("This is free software; there is no warranty.");
        return ExitCode::SUCCESS;
    }
    if args.len() == 1 && args[0] == "--help" {
        println!("Usage: rmake [OPTIONS] [TARGETS]");
        println!("  --help     Show this help");
        println!("  --version  Show rmake version and compatibility target");
        println!("\nDevelopment build: Makefile evaluation is not implemented yet.");
        return ExitCode::SUCCESS;
    }
    eprintln!("rmake: Makefile evaluation is not implemented yet.");
    ExitCode::from(2)
}
