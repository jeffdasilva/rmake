// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Interpreter-owned child processes. Every live child has one reaper; pidfds
//! wake the scheduler without a waiter thread per command. Older/restricted
//! kernels use bounded polling while the unreaped Child retains PID ownership.
use std::ffi::{c_int, c_long};
use std::fs::File;
use std::io;
use std::os::fd::{AsRawFd, BorrowedFd, FromRawFd};
use std::process::{Command, ExitStatus};

unsafe extern "C" {
    fn syscall(number: c_long, ...) -> c_long;
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
        let child = command.spawn()?;
        // pidfd_open and pidfd_send_signal use these numbers on Linux x86-64
        // and asm-generic architectures. pidfd_open creates a CLOEXEC handle.
        let descriptor = unsafe { syscall(434, child.id() as c_int, 0_u32) };
        let pidfd = if descriptor >= 0 {
            crate::process::private_descriptor(unsafe { File::from_raw_fd(descriptor as c_int) })
                .ok()
        } else {
            None
        };
        Ok(Self { child, pidfd })
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
