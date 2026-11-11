// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Process capture for make's shell function and shell assignments. Expansion
//! and newline folding live in core; this module only launches and captures.

use rmake_core::diagnostic::{EnvironmentValue, Output, ShellRequest, ShellResult};
use rmake_core::variables::{ExpandError, MAX_OUTPUT};
use rmake_linux::child::{CaptureError, CapturePipe, CaptureSpawnError, Child};
use std::ffi::OsStr;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::process::ExitStatusExt;
use std::process::Command;

pub(crate) fn original_environment(name: &[u8]) -> Vec<u8> {
    std::env::var_os(OsStr::from_bytes(name))
        .unwrap_or_default()
        .into_vec()
}

pub fn run(
    request: ShellRequest<'_>,
    program_name: &[u8],
    makelevel: u32,
    output: &crate::output::Controller,
    jobserver: Option<&rmake_linux::jobserver::Jobserver>,
    child_completed: impl FnOnce(),
) -> Result<ShellResult, ExpandError> {
    let invocation = crate::process::prepare_shell_function(
        request.command,
        request.shell,
        request.flags,
        request.ifs,
        request.one_shell,
    );
    let (program, args) = match invocation {
        crate::process::Invocation::Direct { program, args }
        | crate::process::Invocation::Shell { program, args } => (program, args),
    };
    let mut command = Command::new(OsStr::from_bytes(&program));
    command.args(args.iter().map(|arg| OsStr::from_bytes(arg)));
    let mut order = Vec::new();
    let mut inherited_mflags = false;
    for (name, value) in request.environment {
        match value {
            EnvironmentValue::Set { value, origin } => {
                if name == b"MFLAGS" {
                    inherited_mflags = *origin == rmake_core::variables::Origin::Environment;
                }
                command.env(OsStr::from_bytes(name), OsStr::from_bytes(value));
                order.push(name.clone());
            }
            EnvironmentValue::Remove => {
                command.env_remove(OsStr::from_bytes(name));
            }
            EnvironmentValue::Original => {
                let value = original_environment(name);
                command.env(OsStr::from_bytes(name), OsStr::from_bytes(&value));
                order.push(name.clone());
            }
        }
    }
    command.env("MAKELEVEL", makelevel.wrapping_add(1).to_string());
    if jobserver.is_some_and(|server| !server.auth().starts_with(b"fifo:")) {
        crate::process::disable_pipe_jobserver(&mut command, inherited_mflags);
    }
    let buffer = output.current();
    output.start();
    let environment = match rmake_linux::environment::Image::from_command(
        &command,
        &order,
        request.original_shell,
    ) {
        Ok(environment) => environment,
        Err(error) => return Ok(launch_failure(output, program_name, &program, &error)),
    };
    let spool = buffer.as_ref().and_then(|buffer| buffer.child_spool());
    let stderr = spool
        .as_deref()
        .and_then(rmake_linux::output::Spool::child_stderr);
    // GNU creates the capture pair before looking up the executable. A failed
    // lookup keeps the pair just like a failed launch of a selected program.
    let mut pipe = match CapturePipe::new() {
        Ok(pipe) => Some(pipe),
        Err(error) => {
            drop(spool);
            let detail = crate::engine::os_error(&error);
            let message = [b"pipe: ", detail.as_bytes()].concat();
            crate::engine::emit_output(
                output,
                program_name,
                Output::Warning {
                    location: request.reading_location,
                    message: &message,
                },
            );
            return Ok(ShellResult {
                stdout: Vec::new(),
                status: None,
            });
        }
    };
    let mut retain_failed_pipe = true;
    let launch = crate::engine::launch_child_with_environment(
        &program,
        &command,
        &environment,
        |command, image, arg0| {
            retain_failed_pipe = false;
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
    );
    drop(spool);
    let (child, stdout) = match launch {
        Ok(child) => child,
        Err(error) => {
            if retain_failed_pipe {
                pipe.expect("failed launch owns a capture pipe")
                    .retain_failed();
            }
            return Ok(launch_failure(output, program_name, &program, &error));
        }
    };
    let captured = rmake_linux::child::capture(child, stdout, MAX_OUTPUT, request.service_workers);
    child_completed();
    let (stdout, status) = captured.map_err(|error| match error {
        CaptureError::Io(_) => ExpandError::UnlocatedError(
            b"rmake: shell output limit exceeded or capture failed".to_vec(),
        ),
        CaptureError::Worker(error) => error,
    })?;
    Ok(ShellResult {
        stdout,
        status: Some(
            status
                .code()
                .unwrap_or_else(|| 128 + status.signal().unwrap_or(0)),
        ),
    })
}

fn launch_failure(
    output: &crate::output::Controller,
    program_name: &[u8],
    program: &[u8],
    error: &std::io::Error,
) -> ShellResult {
    output.write_parts(
        true,
        &[
            program_name,
            b": ",
            program,
            b": ",
            crate::engine::os_error(error).as_bytes(),
            b"\n",
        ],
    );
    ShellResult {
        stdout: Vec::new(),
        status: Some(127),
    }
}
