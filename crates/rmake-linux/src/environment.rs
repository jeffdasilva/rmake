// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! An owned, ordered Linux environment image. Registration performs all
//! allocation in the parent; the final post-fork hook only calls execve.

use std::collections::BTreeMap;
use std::ffi::{CString, OsStr, OsString, c_char, c_int};
use std::io;
use std::os::fd::{AsRawFd, BorrowedFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::process::CommandExt;
use std::process::Command;
use std::sync::Arc;

unsafe extern "C" {
    fn execve(
        program: *const c_char,
        argv: *const *const c_char,
        envp: *const *mut c_char,
    ) -> std::ffi::c_int;
    fn rmake_exec_live_restart(
        arguments: *const *mut c_char,
        count: u32,
        level: u32,
        traced: c_int,
        pipe: *const c_int,
    ) -> c_int;
    fn rmake_spawn_inherited(
        program: *const c_char,
        argv: *const *const c_char,
        envp: *const *mut c_char,
        preserve_pipe_ignore: c_int,
        out: c_int,
        err: c_int,
        pid: *mut c_int,
    ) -> c_int;
}

/// Replace the interpreter using the raw live process environment. A real
/// exec preserves ignores and the signal mask while resetting caught handlers.
/// The serialized process owner calls this after flushing streams and finishing
/// makefile jobs. Native callbacks must respect POSIX's existing restriction on
/// concurrent environment mutation. No borrowed environment storage enters Rust.
pub fn exec_live_restart(
    program: &OsStr,
    arguments: &[OsString],
    count: u32,
    level: u32,
    traced: bool,
    jobserver: Option<&crate::jobserver::Jobserver>,
) -> io::Error {
    let arguments: Result<Vec<_>, _> = std::iter::once(program)
        .chain(arguments.iter().map(OsString::as_os_str))
        .map(|argument| CString::new(argument.as_bytes()))
        .collect();
    let Ok(arguments) = arguments else {
        return io::ErrorKind::InvalidInput.into();
    };
    let mut pointers: Vec<_> = arguments
        .iter()
        .map(|argument| argument.as_ptr().cast_mut())
        .collect();
    pointers.push(std::ptr::null_mut());
    let pair = jobserver.and_then(crate::jobserver::Jobserver::recursive_exec_pipe);
    let descriptors = pair.as_ref().map(|pipe| pipe.descriptors());
    // SAFETY: the terminated vector and every CString remain live for the
    // call. The optional descriptor array contains two incoming numbers and
    // two references kept live by the borrowed jobserver. C checks identity
    // before adjusting incoming flags, owns the environment and returns errno.
    let failure = unsafe {
        rmake_exec_live_restart(
            pointers.as_ptr(),
            count,
            level,
            traced.into(),
            descriptors
                .as_ref()
                .map_or(std::ptr::null(), |fds| fds.as_ptr()),
        )
    };
    io::Error::from_raw_os_error(failure)
}

struct Storage {
    _strings: Vec<CString>,
    pointers: Vec<*mut c_char>,
}

// All pointers address immutable CString allocations retained in this storage.
// The image is never mutated; each installed pointer is used only in a forked
// child, whose address space is independent of the parent and other children.
unsafe impl Send for Storage {}
unsafe impl Sync for Storage {}

#[derive(Clone)]
pub struct Image(Arc<Storage>);

impl Image {
    fn from_entries(entries: impl IntoIterator<Item = (Vec<u8>, Vec<u8>)>) -> io::Result<Self> {
        let mut strings = Vec::new();
        for (mut key, value) in entries {
            key.push(b'=');
            key.extend_from_slice(&value);
            strings
                .push(CString::new(key).map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?);
        }
        let mut pointers: Vec<_> = strings
            .iter()
            .map(|value| value.as_ptr().cast_mut())
            .collect();
        pointers.push(std::ptr::null_mut());
        Ok(Self(Arc::new(Storage {
            _strings: strings,
            pointers,
        })))
    }

    /// Resolve set/remove operations while retaining the interpreter's selected
    /// vector order. No implicit parent merge occurs; GNU's two synthesized
    /// trailing bindings are emitted after the ordinary vector.
    pub fn from_command(
        command: &Command,
        preferred: &[Vec<u8>],
        original_shell: Option<&[u8]>,
    ) -> io::Result<Self> {
        // The interpreter selects the complete vector. In particular, undefine
        // and plugin changes must not reintroduce unselected process entries.
        let mut values: BTreeMap<OsString, OsString> = BTreeMap::new();
        let original_shell = original_shell.map(|value| OsStr::from_bytes(value).to_owned());
        for (name, value) in command.get_envs() {
            if let Some(value) = value {
                values.insert(name.to_owned(), value.to_owned());
            } else {
                values.remove(name);
            }
        }
        if !preferred.iter().any(|name| name == b"SHELL")
            && let Some(value) = original_shell
        {
            // GNU forwards the original shell even after unexport SHELL. An
            // actually exported selected binding instead owns its table slot.
            values.insert(OsString::from("SHELL"), value);
        }
        let mut entries = Vec::new();
        for name in preferred {
            if let Some(value) = values.remove(OsStr::from_bytes(name)) {
                entries.push((name.clone(), value.as_bytes().to_vec()));
            }
        }
        let shell = values.remove(OsStr::new("SHELL"));
        let makelevel = values.remove(OsStr::new("MAKELEVEL"));
        entries.extend(
            values
                .into_iter()
                .map(|(key, value)| (key.as_bytes().to_vec(), value.as_bytes().to_vec())),
        );
        if let Some(value) = shell {
            entries.push((b"SHELL".to_vec(), value.as_bytes().to_vec()));
        }
        if let Some(value) = makelevel {
            entries.push((b"MAKELEVEL".to_vec(), value.as_bytes().to_vec()));
        }
        Self::from_entries(entries)
    }

    /// Register the final pre-exec hook after descriptor/signal setup. All
    /// strings/pointers are prepared in the parent. A successful exec replaces
    /// the child; a failed exec returns errno through std's owned error pipe.
    pub fn configure_exec(&self, command: &mut Command, arg0: &OsStr) -> io::Result<()> {
        let frame = self.exec_frame(command, arg0)?;
        unsafe {
            command.pre_exec(move || Err(frame.execute()));
        }
        Ok(())
    }

    fn exec_frame(&self, command: &Command, arg0: &OsStr) -> io::Result<ExecFrame> {
        let program = CString::new(command.get_program().as_bytes())
            .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
        let mut arguments = Vec::new();
        arguments.push(
            CString::new(arg0.as_bytes())
                .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?,
        );
        for argument in command.get_args() {
            arguments.push(
                CString::new(argument.as_bytes())
                    .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?,
            );
        }
        let mut pointers: Vec<_> = arguments.iter().map(|argument| argument.as_ptr()).collect();
        pointers.push(std::ptr::null());
        Ok(ExecFrame {
            program,
            _arguments: arguments,
            pointers,
            environment: self.clone(),
        })
    }

    pub(crate) fn spawn_inherited(
        &self,
        command: &Command,
        arg0: &OsStr,
        output: [Option<BorrowedFd<'_>>; 2],
    ) -> Option<io::Result<c_int>> {
        let frame = match self.exec_frame(command, arg0) {
            Ok(frame) => frame,
            Err(error) => return Some(Err(error)),
        };
        let mut pid = 0;
        // SAFETY: the frame keeps both terminated vectors and their CStrings
        // live for the full synchronous libc call. Both optional spool
        // sources remain borrowed for the same call; pid is a writable scalar.
        let error = unsafe {
            rmake_spawn_inherited(
                frame.program.as_ptr(),
                frame.pointers.as_ptr(),
                frame.environment.0.pointers.as_ptr(),
                crate::signals::preserve_child_pipe_ignore().into(),
                output[0].map_or(-1, |fd| fd.as_raw_fd()),
                output[1].map_or(-1, |fd| fd.as_raw_fd()),
                &mut pid,
            )
        };
        match error {
            -1 => None,
            0 => Some(Ok(pid)),
            error => Some(Err(io::Error::from_raw_os_error(error))),
        }
    }
}

// Pointers refer only to retained CString allocations. The closure reads the
// immutable frame after fork; no pointer refers to another thread's stack.
struct ExecFrame {
    program: CString,
    _arguments: Vec<CString>,
    pointers: Vec<*const c_char>,
    environment: Image,
}
impl ExecFrame {
    fn execute(&self) -> io::Error {
        if let Err(error) = crate::signals::clear_exec_mask() {
            return error;
        }
        // The whole frame stays captured, including every CString owner, until
        // exec succeeds or std reports this errno through its child-error pipe.
        unsafe {
            execve(
                self.program.as_ptr(),
                self.pointers.as_ptr(),
                self.environment.0.pointers.as_ptr(),
            );
        }
        io::Error::last_os_error()
    }
}
unsafe impl Send for ExecFrame {}
unsafe impl Sync for ExecFrame {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actual_exec_preserves_vector_order_and_binary_values() {
        let image = Image::from_entries([
            (b"B".to_vec(), vec![0xff]),
            (b"A".to_vec(), b"one=two".to_vec()),
        ])
        .unwrap();
        let mut command = Command::new("/usr/bin/env");
        image
            .configure_exec(&mut command, OsStr::new("/usr/bin/env"))
            .unwrap();
        let output = command.output().unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, b"B=\xff\nA=one=two\n");
    }

    #[test]
    fn a_spawn_does_not_change_the_parent_environment() {
        let before = std::env::var_os("RMAKE_ORDERED_IMAGE_TEST");
        let image =
            Image::from_entries([(b"RMAKE_ORDERED_IMAGE_TEST".to_vec(), b"child".to_vec())])
                .unwrap();
        let mut command = Command::new("/usr/bin/env");
        image
            .configure_exec(&mut command, OsStr::new("/usr/bin/env"))
            .unwrap();
        assert_eq!(
            command.output().unwrap().stdout,
            b"RMAKE_ORDERED_IMAGE_TEST=child\n"
        );
        assert_eq!(std::env::var_os("RMAKE_ORDERED_IMAGE_TEST"), before);
    }

    #[test]
    fn invalid_bytes_fail_before_a_child_hook_is_registered() {
        assert!(Image::from_entries([(b"A".to_vec(), b"bad\0value".to_vec())]).is_err());
    }
}
