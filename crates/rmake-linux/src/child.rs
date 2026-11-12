// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Interpreter-owned child processes. Every live child has one reaper; pidfds
//! wake the scheduler without a waiter thread per command. Older/restricted
//! kernels use bounded polling while the unreaped Child retains PID ownership.
use std::ffi::{c_int, c_long};
use std::fs::File;
use std::io;
use std::io::Read;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, OwnedFd};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::{Command, ExitStatus};
use std::sync::Mutex;

unsafe extern "C" {
    fn syscall(number: c_long, ...) -> c_long;
    fn dup2(source: c_int, destination: c_int) -> c_int;
    fn fcntl(descriptor: c_int, operation: c_int, ...) -> c_int;
    fn read(descriptor: c_int, bytes: *mut std::ffi::c_void, size: usize) -> isize;
    fn kill(pid: c_int, signal: c_int) -> c_int;
    fn poll(descriptors: *mut PollFd, count: usize, timeout: c_int) -> c_int;
    fn waitpid(pid: c_int, status: *mut c_int, options: c_int) -> c_int;
}

#[repr(C)]
struct PollFd {
    descriptor: c_int,
    events: i16,
    returned: i16,
}

pub struct Child {
    child: Process,
    pidfd: Option<File>,
}

enum Process {
    Standard(std::process::Child),
    Spawned {
        pid: c_int,
        status: Option<ExitStatus>,
    },
}

/// One logical shell capture owns the same pair across executable attempts.
pub struct CapturePipe {
    reader: File,
    writer: File,
}

pub enum CaptureSpawnError {
    Prepare(io::Error),
    Launch(io::Error),
}

impl CapturePipe {
    pub fn new() -> io::Result<Self> {
        let (reader, writer) = io::pipe()?;
        Ok(Self {
            // Keep any occupied standard slots until the spawn attempt has
            // finished. The child's fork-status socket must stay independent.
            reader: File::from(OwnedFd::from(reader)),
            writer: File::from(OwnedFd::from(writer)),
        })
    }

    pub fn retain_failed(self) {
        struct Retained {
            private_ends: Vec<File>,
            wake_accounted: bool,
        }

        static RETAINED: Mutex<Retained> = Mutex::new(Retained {
            private_ends: Vec::new(),
            wake_accounted: false,
        });
        let mut retained = RETAINED.lock().expect("failed capture ownership");
        // GNU leaves two private close-on-exec descriptors open after a
        // failed launch. Our interpreter-wide wake pipe already reserves
        // those two slots for the same lifetime. Count it once and leave
        // its live endpoints and the handler's access to them untouched.
        if !retained.wake_accounted
            && self.reader.as_raw_fd() > 2
            && self.writer.as_raw_fd() > 2
            && crate::signals::wake_descriptor() >= 0
        {
            retained.wake_accounted = true;
            return;
        }
        retained
            .private_ends
            .extend([self.reader, self.writer].into_iter().filter_map(|end| {
                if end.as_raw_fd() > 2 {
                    return Some(end);
                }
                // The public standard slot stays occupied until it is restored
                // by native code, or closed by exec/exit. Do not keep a Rust
                // owner that could outlive a native replacement of that slot.
                let _ = end.into_raw_fd();
                None
            }));
    }
}

fn set_capture_stream(source: c_int, target: c_int) -> io::Result<()> {
    if source == target {
        // An identity action leaves the original close-on-exec flag intact.
        return Ok(());
    }
    loop {
        // SAFETY: the ordered spawn retains the source through fork. The child
        // changes only its own standard descriptor table here.
        if unsafe { dup2(source, target) } >= 0 {
            return Ok(());
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error);
        }
    }
}

fn prepare_capture_reader(stdout: File) -> io::Result<File> {
    let stdout = crate::process::private_descriptor(stdout)?;
    let flags = unsafe { fcntl(stdout.as_raw_fd(), 3) };
    if flags < 0 || unsafe { fcntl(stdout.as_raw_fd(), 4, flags | 0x800) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(stdout)
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

    /// A recipe with inherited stdin, stdout and stderr, and no jobserver
    /// child hook, can avoid the std exec-error pipe under descriptor pressure.
    /// This path consumes only the command's program and arguments.
    pub fn try_spawn_inherited_ordered(
        command: &Command,
        image: &crate::environment::Image,
        arg0: &std::ffi::OsStr,
    ) -> Option<io::Result<Self>> {
        Self::try_spawn_ordered_fds(command, image, arg0, [None; 3], None, true)
    }

    pub fn try_spawn_ordered_spool(
        command: &Command,
        image: &crate::environment::Image,
        arg0: &std::ffi::OsStr,
        spool: &crate::output::Spool,
    ) -> Option<io::Result<Self>> {
        let [out, err] = spool.child_output();
        Self::try_spawn_ordered_fds(command, image, arg0, [None, out, err], None, true)
    }

    /// A compact pipe uses its original wire numbers for recursive children.
    /// A FIFO is reopened by name and inherits no pool descriptors. Leave the
    /// scarce spare descriptors available to the interpreter and its children.
    pub fn try_spawn_ordered_pool(
        command: &Command,
        image: &crate::environment::Image,
        arg0: &std::ffi::OsStr,
        stdin: Option<&File>,
        server: &crate::jobserver::Jobserver,
        recursive: bool,
    ) -> Option<io::Result<Self>> {
        if !server.uses_compact_pipe() && !Self::wake_pipe_unavailable() {
            return None;
        }
        let wire = if recursive {
            match server.compact_wire() {
                Ok(wire) => wire,
                Err(error) => return Some(Err(error)),
            }
        } else {
            None
        };
        // The compact owner can also be selected after an active wake
        // pipe leaves too few setup slots. Its wake endpoints stay private.
        image
            .spawn_inherited(command, arg0, [stdin.map(AsFd::as_fd), None, None], wire)
            .map(|result| result.and_then(|pid| Self::from_spawned_pid(pid, false)))
    }

    /// Retry an inherited-stdio launch under descriptor pressure. An owned
    /// normal pipe already uses the advertised wire numbers, so the C spawn
    /// action can expose them without remapping another process's handles.
    pub fn try_spawn_ordered_owned_pipe(
        command: &Command,
        image: &crate::environment::Image,
        arg0: &std::ffi::OsStr,
        server: &crate::jobserver::Jobserver,
        recursive: bool,
    ) -> Option<io::Result<Self>> {
        let wire = server.owned_normal_pipe_wire()?;
        image
            .spawn_inherited(command, arg0, [None; 3], recursive.then_some(wire))
            .map(|result| result.and_then(|pid| Self::from_spawned_pid(pid, true)))
    }

    pub fn wake_pipe_unavailable() -> bool {
        crate::signals::wake_descriptor() < 0
    }

    fn try_spawn_ordered_fds(
        command: &Command,
        image: &crate::environment::Image,
        arg0: &std::ffi::OsStr,
        stdio: [Option<BorrowedFd<'_>>; 3],
        wire: Option<[c_int; 2]>,
        retain_pidfd: bool,
    ) -> Option<io::Result<Self>> {
        if !Self::wake_pipe_unavailable() {
            return None;
        }
        image
            .spawn_inherited(command, arg0, stdio, wire)
            .map(|result| result.and_then(|pid| Self::from_spawned_pid(pid, retain_pidfd)))
    }

    pub fn spawn_capture_ordered(
        mut command: Command,
        image: &crate::environment::Image,
        arg0: &std::ffi::OsStr,
        capture: &mut Option<CapturePipe>,
        stderr: Option<BorrowedFd<'_>>,
    ) -> Result<(Self, File), CaptureSpawnError> {
        let pipe = match capture.as_ref() {
            Some(pipe) => pipe,
            None => capture.insert(CapturePipe::new().map_err(CaptureSpawnError::Prepare)?),
        };
        let writer = pipe.writer.as_raw_fd();
        let stderr_fd = stderr.map(|file| file.as_raw_fd());
        if writer != 1 || stderr_fd.is_some() {
            // SAFETY: capture and the borrowed spool own both sources while
            // this command spawns synchronously. Copy stdout first: when the
            // capture writer occupies fd 2, installing stderr would replace it.
            unsafe {
                command.pre_exec(move || {
                    set_capture_stream(writer, 1)?;
                    if let Some(file) = stderr_fd {
                        set_capture_stream(file, 2)?;
                    }
                    Ok(())
                });
            }
        }
        image
            .configure_exec(&mut command, arg0)
            .map_err(CaptureSpawnError::Prepare)?;
        let child = match command.spawn() {
            Ok(child) => Self::from_std(child),
            Err(error) if matches!(error.raw_os_error(), Some(23 | 24)) => {
                // The std fork-status socket can exhaust the remaining
                // descriptors even though the capture pair already exists.
                let out = (writer != 1).then(|| pipe.writer.as_fd());
                let err = stderr.filter(|fd| fd.as_raw_fd() != 2);
                image
                    .spawn_inherited(&command, arg0, [None, out, err], None)
                    .unwrap_or(Err(error))
                    .and_then(|pid| Self::from_spawned_pid(pid, true))
                    .map_err(CaptureSpawnError::Launch)?
            }
            Err(error) => return Err(CaptureSpawnError::Launch(error)),
        };
        let CapturePipe { reader, writer } = capture.take().expect("capture pipe exists");
        drop(writer);
        let stdout = prepare_capture_reader(reader).map_err(CaptureSpawnError::Prepare)?;
        Ok((child, stdout))
    }

    fn from_spawned_pid(pid: c_int, retain_pidfd: bool) -> io::Result<Self> {
        if pid <= 0 {
            return Err(io::Error::other("spawn did not return an owned child"));
        }
        Ok(Self::with_pid(
            Process::Spawned { pid, status: None },
            pid,
            retain_pidfd,
        ))
    }

    fn from_std(child: std::process::Child) -> Self {
        let pid = child.id() as c_int;
        Self::with_pid(Process::Standard(child), pid, true)
    }

    fn with_pid(child: Process, pid: c_int, retain_pidfd: bool) -> Self {
        // pidfd_open and pidfd_send_signal use these numbers on Linux x86-64
        // and asm-generic architectures. pidfd_open creates a CLOEXEC handle.
        let descriptor = if retain_pidfd {
            unsafe { syscall(434, pid, 0_u32) }
        } else {
            -1
        };
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
        let mut child = command.spawn()?;
        let descriptor: OwnedFd = child.stdout.take().expect("piped stdout").into();
        let child = Self::from_std(child);
        Ok((child, prepare_capture_reader(File::from(descriptor))?))
    }

    pub fn id(&self) -> u32 {
        match &self.child {
            Process::Standard(child) => child.id(),
            Process::Spawned { pid, .. } => *pid as u32,
        }
    }

    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        match &mut self.child {
            Process::Standard(child) => child.try_wait(),
            Process::Spawned { pid, status } => {
                if status.is_none() {
                    *status = wait_owned_pid(*pid, false)?;
                }
                Ok(*status)
            }
        }
    }

    pub fn send_signal(&mut self, number: i32) -> io::Result<()> {
        if self.try_wait()?.is_some() {
            return Ok(());
        }
        self.signal_owned(number)
    }

    fn signal_owned(&self, number: i32) -> io::Result<()> {
        // A pidfd cannot address a reused PID. Without one, this uniquely
        // owned, unreaped Child reserves its PID until this interpreter reaps.
        let result = if let Some(fd) = &self.pidfd {
            unsafe { syscall(424, fd.as_raw_fd(), number, std::ptr::null::<u8>(), 0_u32) as c_int }
        } else {
            unsafe { kill(self.id() as c_int, number) }
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
            match &mut self.child {
                Process::Standard(child) => {
                    let _ = child.kill();
                    let _ = child.wait();
                }
                Process::Spawned { pid, .. } => {
                    let pid = *pid;
                    let _ = self.signal_owned(9);
                    let _ = wait_owned_pid(pid, true);
                }
            }
        }
    }
}

fn wait_owned_pid(pid: c_int, block: bool) -> io::Result<Option<ExitStatus>> {
    let mut status = 0;
    loop {
        // SAFETY: this owner passes one positive child PID and one writable
        // status word; WNOHANG returns zero without writing a status.
        let result = unsafe { waitpid(pid, &mut status, i32::from(!block)) };
        if result >= 0 {
            return Ok((result != 0).then(|| ExitStatus::from_raw(status)));
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error);
        }
    }
}

/// Return ready identities in creation order. A signal wake carries no fake
/// child completion: the interpreter checks the pending signal independently.
pub fn wait(
    children: &[(u64, &Child)],
    block: bool,
    interruptible: bool,
    additional: Option<c_int>,
    maximum_wait: Option<std::time::Duration>,
) -> io::Result<Vec<u64>> {
    wait_with_notification(
        children,
        block,
        interruptible,
        additional,
        None,
        maximum_wait,
    )
    .map(|events| events.children)
}

pub struct WaitEvents {
    pub children: Vec<u64>,
    pub notification_only: bool,
    pub timed_out: bool,
}

/// Preserve the distinction between a private notification and any child,
/// signal or jobserver readiness. This wait never consumes their data.
pub fn wait_with_notification(
    children: &[(u64, &Child)],
    block: bool,
    interruptible: bool,
    additional: Option<c_int>,
    notification: Option<BorrowedFd<'_>>,
    maximum_wait: Option<std::time::Duration>,
) -> io::Result<WaitEvents> {
    let mut descriptors = Vec::with_capacity(children.len());
    for (_, child) in children {
        if let Some(pidfd) = &child.pidfd {
            descriptors.push(PollFd {
                descriptor: pidfd.as_raw_fd(),
                events: 1,
                returned: 0,
            });
        }
    }
    let child_descriptors = descriptors.len();
    let wake = interruptible
        .then(crate::signals::wake_descriptor)
        .filter(|fd| *fd >= 0);
    // Linux counts even ignored poll slots against RLIMIT_NOFILE. Compact
    // pools can have more children than descriptors; check their owned PIDs
    // after a bounded wait without adding placeholder descriptors.
    // With no wake pipe, this also bounds the pending-signal race.
    let fallback = child_descriptors < children.len() || (interruptible && wake.is_none());
    if let Some(descriptor) = wake {
        descriptors.push(PollFd {
            descriptor,
            events: 1,
            returned: 0,
        });
    }
    for fd in [additional, notification.map(|fd| fd.as_raw_fd())]
        .into_iter()
        .flatten()
    {
        descriptors.push(PollFd {
            descriptor: fd,
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
    let result = unsafe { poll(descriptors.as_mut_ptr(), descriptors.len(), timeout) };
    if result < 0 {
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error);
        }
    }
    let notification_only = notification.is_some()
        && result == 1
        && descriptors.last().is_some_and(|fd| fd.returned == 1);
    let mut polled_children = descriptors[..child_descriptors].iter();
    let mut ready: Vec<_> = children
        .iter()
        .filter_map(|(id, child)| {
            (child.pidfd.is_none()
                || polled_children.next().expect("one slot per pidfd").returned != 0)
                .then_some(*id)
        })
        .collect();
    ready.sort_unstable();
    Ok(WaitEvents {
        children: ready,
        notification_only,
        timed_out: result == 0,
    })
}

/// Capture stays responsive both while reading a pipe and after its writer
/// closes stdout but keeps running. Dropping the uniquely owned child on any
/// error kills and reaps it; a recycled PID can never be signalled here.
pub fn capture(
    mut child: Child,
    stdout: File,
    maximum: usize,
) -> io::Result<(Vec<u8>, ExitStatus)> {
    let mut bytes = Vec::new();
    let mut stdout = Some(stdout);
    let mut status = None;
    loop {
        if crate::signals::pending().is_some() {
            return Err(io::ErrorKind::Interrupted.into());
        }
        if let Some(reader) = &mut stdout {
            let mut block = [0; 8192];
            loop {
                match reader.read(&mut block) {
                    Ok(0) => {
                        // The writer can still be running after EOF.
                        // Release only the pipe before waiting for its status.
                        stdout = None;
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
        if stdout.is_none()
            && let Some(status) = status
        {
            return Ok((bytes, status));
        }
        let children = [(0, &child)];
        wait(
            if status.is_none() { &children } else { &[] },
            true,
            true,
            stdout.as_ref().map(AsRawFd::as_raw_fd),
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
    fn notification_only_keeps_simultaneous_reader_readiness_distinct() {
        use std::io::Write;
        use std::os::unix::net::UnixStream;

        let (mut token, mut token_sender) = UnixStream::pair().unwrap();
        let (mut notification, mut notification_sender) = UnixStream::pair().unwrap();
        token.set_nonblocking(true).unwrap();
        notification.set_nonblocking(true).unwrap();
        notification_sender.write_all(b"wake").unwrap();
        let wait = |token: &UnixStream, notification: &UnixStream| {
            wait_with_notification(
                &[],
                false,
                false,
                Some(token.as_raw_fd()),
                Some(notification.as_fd()),
                None,
            )
            .unwrap()
        };
        let events = wait(&token, &notification);
        assert!(events.notification_only && events.children.is_empty());
        token_sender.write_all(b"+").unwrap();
        let events = wait(&token, &notification);
        assert!(!events.notification_only && events.children.is_empty());
        let (mut token_bytes, mut notification_bytes) = ([0], [0; 4]);
        token.read_exact(&mut token_bytes).unwrap();
        notification.read_exact(&mut notification_bytes).unwrap();
        assert_eq!((&token_bytes, &notification_bytes), (b"+", b"wake"));
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
    fn child_wait_keeps_mixed_descriptor_and_pid_readiness_aligned() {
        let sleeping = Child::spawn(Command::new("/bin/sleep").arg("30")).unwrap();
        let mut fallback = Child::spawn(Command::new("/bin/sleep").arg("30")).unwrap();
        let mut exited = Child::spawn(&mut Command::new("/bin/true")).unwrap();
        if sleeping.pidfd.is_none() || exited.pidfd.is_none() {
            return;
        }
        fallback.pidfd = None;
        let deadline = Instant::now() + Duration::from_secs(3);
        while exited.try_wait().unwrap().is_none() {
            assert!(Instant::now() < deadline, "owned child did not exit");
            wait(
                &[(0, &exited)],
                true,
                false,
                None,
                Some(Duration::from_millis(10)),
            )
            .unwrap();
        }

        let ready = wait(
            &[(3, &sleeping), (2, &fallback), (1, &exited)],
            false,
            false,
            None,
            None,
        )
        .unwrap();
        assert_eq!(ready, [1, 2]);
    }

    fn dropping_reaps(child: Child) {
        let pid = child.id() as c_int;
        drop(child);
        let mut status = 0;
        // A PID-specific wait cannot reap another test's child. ECHILD proves
        // that Drop already consumed this child's status rather than leaking it.
        assert_eq!(unsafe { waitpid(pid, &mut status, 1) }, -1);
        assert_eq!(io::Error::last_os_error().raw_os_error(), Some(10));
    }

    #[test]
    fn dropping_a_live_child_reaps_it_before_releasing_ownership() {
        dropping_reaps(Child::spawn(Command::new("/bin/sleep").arg("30")).unwrap());
    }

    #[test]
    fn dropping_a_spawned_child_reaps_it_even_without_a_pidfd() {
        use crate::environment::Image;
        use std::ffi::OsStr;
        let mut command = Command::new("/bin/sleep");
        command.arg("30");
        let image = Image::from_command(&command, &[], None).unwrap();
        let Some(child) = Child::try_spawn_inherited_ordered(&command, &image, OsStr::new("sleep"))
        else {
            return;
        };
        let mut child = child.unwrap();
        assert!(matches!(child.child, Process::Spawned { .. }));
        child.pidfd = None;
        dropping_reaps(child);
    }
}
