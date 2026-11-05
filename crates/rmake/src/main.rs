// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

#[cfg(not(target_os = "linux"))]
compile_error!("rmake currently supports Linux only");

use std::process::ExitCode;

#[cfg(feature = "guile")]
mod panic_guard;

fn main() -> ExitCode {
    // Select libc locale while still on the ordinary process-leading stack.
    rmake_linux::locale::initialize();
    #[cfg(feature = "guile")]
    {
        guile_entry()
    }
    #[cfg(not(feature = "guile"))]
    {
        rmake_app::run()
    }
}

#[cfg(feature = "guile")]
#[allow(unsafe_code)]
fn guile_entry() -> ExitCode {
    panic_guard::install();
    // SAFETY: this is the concrete reviewed process entry, on the leading
    // thread before foreign initialization. The semantic entry retains its
    // NativeExit and SignalsGuard through C termination after foreign use.
    // The panic hook prevents unwinding those live borrowed host frames.
    match unsafe { rmake_linux::guile::run_host(rmake_app::run) } {
        Ok(status) => status,
        Err(error) => {
            eprintln!("rmake: Guile entry: {error}");
            ExitCode::from(2)
        }
    }
}
