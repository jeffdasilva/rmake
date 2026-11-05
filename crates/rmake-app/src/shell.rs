// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Process capture for make's shell function and shell assignments. Expansion
//! and newline folding live in core; this module only launches and captures.

use rmake_core::diagnostic::{EnvironmentValue, ShellRequest, ShellResult};
use rmake_core::variables::MAX_OUTPUT;
use rmake_linux::child::{CaptureSpawnError, Child};
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::process::ExitStatusExt;
use std::process::Command;

pub fn run(
    request: ShellRequest<'_>,
    program_name: &[u8],
    makelevel: u32,
    output: &crate::output::Controller,
    jobserver: Option<&rmake_linux::jobserver::Jobserver>,
) -> Result<ShellResult, Vec<u8>> {
    let invocation =
        crate::process::prepare(request.command, request.shell, request.flags, request.ifs);
    let (program, args) = match invocation {
        crate::process::Invocation::Direct { program, args }
        | crate::process::Invocation::Shell { program, args } => (program, args),
    };
    let mut command = Command::new(OsStr::from_bytes(&program));
    command.args(args.iter().map(|arg| OsStr::from_bytes(arg)));
    let mut order = Vec::new();
    for (name, value) in request.environment {
        match value {
            EnvironmentValue::Set(value) => {
                command.env(OsStr::from_bytes(name), OsStr::from_bytes(value));
                order.push(name.clone());
            }
            EnvironmentValue::Remove => {
                command.env_remove(OsStr::from_bytes(name));
            }
            EnvironmentValue::Original => {
                let value = std::env::var_os(OsStr::from_bytes(name)).unwrap_or_default();
                command.env(OsStr::from_bytes(name), value);
                order.push(name.clone());
            }
        }
    }
    command.env("MAKELEVEL", makelevel.wrapping_add(1).to_string());
    if jobserver.is_some_and(|server| !server.auth().starts_with(b"fifo:")) {
        crate::process::disable_pipe_jobserver(&mut command);
    }
    let buffer = output.current();
    output.start();
    let mut pipe = None;
    let mut retain_failed_pipe = false;
    let (child, stdout) = match crate::engine::launch_child(
        &program,
        &command,
        &order,
        request.original_shell,
        |command, image, arg0| {
            retain_failed_pipe = false;
            let spool = buffer.as_ref().and_then(|buffer| buffer.child_spool());
            let stderr = spool
                .as_deref()
                .and_then(rmake_linux::output::Spool::child_stderr);
            match Child::spawn_capture_ordered(
                command,
                image,
                OsStr::from_bytes(arg0),
                &mut pipe,
                stderr,
            ) {
                Ok(capture) => Ok(capture),
                Err(CaptureSpawnError::Prepare(error)) => Err(error),
                Err(CaptureSpawnError::Launch(error)) => {
                    retain_failed_pipe = true;
                    Err(error)
                }
            }
        },
    ) {
        Ok(child) => child,
        Err(error) => {
            if retain_failed_pipe {
                pipe.expect("failed launch owns a capture pipe")
                    .retain_failed();
            }
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
    let (stdout, status) = rmake_linux::child::capture(child, stdout, MAX_OUTPUT)
        .map_err(|_| b"rmake: shell output limit exceeded or capture failed".to_vec())?;
    Ok(ShellResult {
        stdout,
        status: status
            .code()
            .unwrap_or_else(|| 128 + status.signal().unwrap_or(0)),
    })
}
