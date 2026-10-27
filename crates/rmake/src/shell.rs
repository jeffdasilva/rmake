// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Process capture for make's shell function and shell assignments. Expansion
//! and newline folding live in core; this module only launches and captures.

use rmake_core::diagnostic::{ShellRequest, ShellResult};
use rmake_core::variables::MAX_OUTPUT;
use std::ffi::OsStr;
use std::io::Read;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::process::ExitStatusExt;
use std::process::{Command, Stdio};

pub fn run(
    request: ShellRequest<'_>,
    program_name: &[u8],
    makelevel: u32,
    output: &crate::output::Controller,
) -> Result<ShellResult, Vec<u8>> {
    let invocation =
        crate::process::prepare(request.command, request.shell, request.flags, request.ifs);
    let (program, args) = match invocation {
        crate::process::Invocation::Direct { program, args }
        | crate::process::Invocation::Shell { program, args } => (program, args),
    };
    let mut command = Command::new(OsStr::from_bytes(&program));
    command.args(args.iter().map(|arg| OsStr::from_bytes(arg)));
    for (name, value) in request.environment {
        if let Some(value) = value {
            command.env(OsStr::from_bytes(name), OsStr::from_bytes(value));
        } else {
            command.env_remove(OsStr::from_bytes(name));
        }
    }
    command.env("MAKELEVEL", makelevel.wrapping_add(1).to_string());
    let buffer = output.current();
    let mut child = match crate::engine::launch_child(&program, &command, |mut command| {
        command.stdout(Stdio::piped());
        if let Some(buffer) = &buffer {
            buffer.configure_stderr(&mut command)?;
        }
        command.spawn()
    }) {
        Ok(child) => child,
        Err(error) => {
            output.write_parts(
                true,
                &[
                    program_name,
                    b": ",
                    &program,
                    b": ",
                    crate::engine::os_error(&error).as_bytes(),
                    b"\n",
                ],
            );
            return Ok(ShellResult {
                stdout: Vec::new(),
                status: 127,
            });
        }
    };
    let mut stdout = Vec::new();
    let result = child
        .stdout
        .take()
        .expect("piped capture stdout")
        .take((MAX_OUTPUT + 1) as u64)
        .read_to_end(&mut stdout);
    if result.is_err() || stdout.len() > MAX_OUTPUT {
        let _ = child.kill();
        let _ = child.wait();
        return Err(b"rmake: shell output limit exceeded or capture failed".to_vec());
    }
    let status = child
        .wait()
        .map_err(|error| crate::engine::os_error(&error).into_bytes())?;
    Ok(ShellResult {
        stdout,
        status: status
            .code()
            .unwrap_or_else(|| 128 + status.signal().unwrap_or(0)),
    })
}
