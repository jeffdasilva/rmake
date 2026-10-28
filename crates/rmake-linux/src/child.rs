// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Interpreter-owned child processes. Every live child has one reaper; pidfds
//! wake the scheduler without a waiter thread per command. Older/restricted
//! kernels use bounded polling while the unreaped Child retains PID ownership.
use std::ffi::{c_int, c_long};
use std::fs::File;
use std::io;
use std::io::Read;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd};
use std::process::{Command, ExitStatus};

unsafe extern "C" {
    fn syscall(number: c_long, ...) -> c_long;
    fn fcntl(descriptor: c_int, operation: c_int, ...) -> c_int;
    fn read(descriptor: c_int, bytes: *mut std::ffi::c_void, size: usize) -> isize;
    fn kill(pid: c_int, signal: c_int) -> c_int;
    fn poll(descriptors: *mut PollFd, count: usize, timeout: c_int) -> c_int;
}

#[repr(C)]
struct PollFd {
    descriptor: c_int,
    events: i16,
    returned: i16,
}

pub struct Child {
    child: std::process::Child,
    pidfd: Option<File>,
}

impl Child {
    pub fn spawn(command: &mut Command) -> io::Result<Self> {
        Ok(Self::from_std(command.spawn()?))
    }

    pub fn spawn_ordered(
        command: &mut Command,
        image: &crate::environment::Image,
        arg0: &std::ffi::OsStr,
    ) -> io::Result<Self> {
        image.configure_exec(command, arg0)?;
        Self::spawn(command)
    }

    pub fn spawn_capture_ordered(
        command: &mut Command,
        image: &crate::environment::Image,
        arg0: &std::ffi::OsStr,
    ) -> io::Result<(Self, File)> {
        image.configure_exec(command, arg0)?;
        Self::spawn_capture(command)
    }

    fn from_std(child: std::process::Child) -> Self {
        // pidfd_open and pidfd_send_signal use these numbers on Linux x86-64
        // and asm-generic architectures. pidfd_open creates a CLOEXEC handle.
        let descriptor = unsafe { syscall(434, child.id() as c_int, 0_u32) };
        let pidfd = if descriptor >= 0 {
            crate::process::private_descriptor(unsafe { File::from_raw_fd(descriptor as c_int) })
                .ok()
        } else {
            None
        };
        Self { child, pidfd }
    }

    pub fn spawn_capture(command: &mut Command) -> io::Result<(Self, File)> {
        command.stdout(std::process::Stdio::piped());
        let mut child = Self::from_std(command.spawn()?);
        let descriptor: OwnedFd = child.child.stdout.take().expect("piped stdout").into();
        let stdout = crate::process::private_descriptor(File::from(descriptor))?;
        let flags = unsafe { fcntl(stdout.as_raw_fd(), 3) };
        if flags < 0 || unsafe { fcntl(stdout.as_raw_fd(), 4, flags | 0x800) } < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok((child, stdout))
    }

    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        self.child.try_wait()
    }

    pub fn send_signal(&mut self, number: i32) -> io::Result<()> {
        if self.try_wait()?.is_some() {
            return Ok(());
        }
        // A pidfd cannot address a reused PID. Without one, this uniquely
        // owned, unreaped Child reserves its PID until this interpreter reaps.
        let result = if let Some(fd) = &self.pidfd {
            unsafe { syscall(424, fd.as_raw_fd(), number, std::ptr::null::<u8>(), 0_u32) as c_int }
        } else {
            unsafe { kill(self.child.id() as c_int, number) }
        };
        if result < 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() != Some(3) {
                return Err(error);
            }
        }
        Ok(())
    }
}

impl Drop for Child {
    fn drop(&mut self) {
        // Normal completion has already been reaped. On error/unwinding,
        // terminate only this still-owned child and reap before releasing it.
        if matches!(self.try_wait(), Ok(None)) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

/// Return ready identities in creation order. A signal wake carries no fake
/// child completion: the interpreter checks the pending signal independently.
pub fn wait(
    children: &[(u64, &Child)],
    block: bool,
    interruptible: bool,
    additional: Option<BorrowedFd<'_>>,
    maximum_wait: Option<std::time::Duration>,
) -> io::Result<Vec<u64>> {
    let mut descriptors: Vec<_> = children
        .iter()
        .map(|(_, child)| PollFd {
            descriptor: child.pidfd.as_ref().map_or(-1, AsRawFd::as_raw_fd),
            events: 1,
            returned: 0,
        })
        .collect();
    let fallback = descriptors.iter().any(|fd| fd.descriptor < 0);
    if interruptible {
        descriptors.push(PollFd {
            descriptor: crate::signals::wake_descriptor(),
            events: 1,
            returned: 0,
        });
    }
    if let Some(fd) = additional {
        descriptors.push(PollFd {
            descriptor: fd.as_raw_fd(),
            events: 1,
            returned: 0,
        });
    }
    let mut timeout = if !block {
        0
    } else if fallback {
        5
    } else {
        -1
    };
    if block && let Some(maximum) = maximum_wait {
        let maximum = maximum.as_millis().min(c_int::MAX as u128) as c_int;
        timeout = if timeout < 0 {
            maximum
        } else {
            timeout.min(maximum)
        };
    }
    // The vector owns a correctly sized array of initialized Linux pollfd
    // records. EINTR is a scheduler wake, allowing safe signal processing.
    if unsafe { poll(descriptors.as_mut_ptr(), descriptors.len(), timeout) } < 0 {
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error);
        }
    }
    let mut ready: Vec<_> = children
        .iter()
        .zip(&descriptors)
        .filter_map(|((id, child), fd)| (child.pidfd.is_none() || fd.returned != 0).then_some(*id))
        .collect();
    ready.sort_unstable();
    Ok(ready)
}

/// Capture stays responsive both while reading a pipe and after its writer
/// closes stdout but keeps running. Dropping the uniquely owned child on any
/// error kills and reaps it; a recycled PID can never be signalled here.
pub fn capture(
    mut child: Child,
    mut stdout: File,
    maximum: usize,
) -> io::Result<(Vec<u8>, ExitStatus)> {
    let mut bytes = Vec::new();
    let mut eof = false;
    let mut status = None;
    loop {
        if crate::signals::pending().is_some() {
            return Err(io::ErrorKind::Interrupted.into());
        }
        if !eof {
            let mut block = [0; 8192];
            loop {
                match stdout.read(&mut block) {
                    Ok(0) => {
                        eof = true;
                        break;
                    }
                    Ok(count) => {
                        if bytes.len().saturating_add(count) > maximum {
                            return Err(io::ErrorKind::FileTooLarge.into());
                        }
                        bytes.extend_from_slice(&block[..count]);
                        if crate::signals::pending().is_some() {
                            return Err(io::ErrorKind::Interrupted.into());
                        }
                    }
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => {
                        if crate::signals::pending().is_some() {
                            return Err(error);
                        }
                    }
                    Err(error) => return Err(error),
                }
            }
        }
        if status.is_none() {
            status = child.try_wait()?;
        }
        if eof && let Some(status) = status {
            return Ok((bytes, status));
        }
        let children = [(0, &child)];
        wait(
            if status.is_none() { &children } else { &[] },
            true,
            true,
            (!eof).then(|| stdout.as_fd()),
            None,
        )?;
    }
}

/// Stdin Makefiles may be pipes. Avoid read_to_end's unconditional EINTR retry
/// so an empty or nonterminating stream can be cancelled without a waiter thread.
pub fn read_stdin() -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut block = [0; 8192];
    loop {
        if crate::signals::pending().is_some() {
            return Err(io::ErrorKind::Interrupted.into());
        }
        let count = unsafe { read(0, block.as_mut_ptr().cast(), block.len()) };
        if count == 0 {
            return Ok(bytes);
        }
        if count > 0 {
            bytes.extend_from_slice(&block[..count as usize]);
        } else {
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::Interrupted {
                return Err(error);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::process::ExitStatusExt;
    use std::time::{Duration, Instant};

    fn terminated(mut child: Child) {
        child.send_signal(15).unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert_eq!(status.signal(), Some(15));
                // A second delivery to a reaped handle is a no-op, even if the
                // numeric PID could subsequently be reused by another process.
                child.send_signal(15).unwrap();
                return;
            }
            assert!(Instant::now() < deadline, "owned child did not terminate");
            wait(&[(0, &child)], true, false, None, None).unwrap();
        }
    }

    #[test]
    fn capture_waits_for_exit_after_stdout_closes_and_preserves_status() {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "exec 1>&-; sleep .01; exit 7"]);
        let (child, stdout) = Child::spawn_capture(&mut command).unwrap();
        let (bytes, status) = capture(child, stdout, 16).unwrap();
        assert!(bytes.is_empty());
        assert_eq!(status.code(), Some(7));
    }

    #[test]
    fn capture_enforces_the_byte_bound_and_reaps_the_owned_writer() {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "printf abc"]);
        let (child, stdout) = Child::spawn_capture(&mut command).unwrap();
        assert_eq!(
            capture(child, stdout, 2).unwrap_err().kind(),
            io::ErrorKind::FileTooLarge
        );
    }

    #[test]
    fn owned_child_wakes_and_signals_through_a_pidfd() {
        terminated(Child::spawn(Command::new("/bin/sleep").arg("30")).unwrap());
    }

    #[test]
    fn unavailable_pidfd_preserves_unique_reaping_and_signal_delivery() {
        let mut child = Child::spawn(Command::new("/bin/sleep").arg("30")).unwrap();
        child.pidfd = None;
        terminated(child);
    }

    #[test]
    fn dropping_a_live_child_reaps_it_before_releasing_ownership() {
        unsafe extern "C" {
            fn waitpid(pid: c_int, status: *mut c_int, flags: c_int) -> c_int;
        }
        let child = Child::spawn(Command::new("/bin/sleep").arg("30")).unwrap();
        let pid = child.child.id() as c_int;
        drop(child);
        let mut status = 0;
        // A PID-specific wait cannot reap another test's child. ECHILD proves
        // that Drop already consumed this child's status rather than leaking it.
        assert_eq!(unsafe { waitpid(pid, &mut status, 1) }, -1);
        assert_eq!(io::Error::last_os_error().raw_os_error(), Some(10));
    }
}
