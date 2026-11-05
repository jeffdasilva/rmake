// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Default-off ordinary-C-stack component, not an enabled make function/profile.
//! All foreign entry runs on the leading pthread's normal C stack; the Rust
//! interpreter remains on private grown stacks on that same operating thread.
use rmake_core::native::NativeHost;
use rmake_core::variables::ExpandError;
use std::ffi::{c_int, c_void};
use std::rc::Rc;

unsafe extern "C" {
    fn rg_dispatch_run(entry: unsafe extern "C" fn(*mut c_void), data: *mut c_void) -> c_int;
    fn rg_dispatch_on_host() -> c_int;
    fn rg_guile_initialized() -> c_int;
    fn rg_guile_component_signal(number: c_int);
    fn rg_guile_notify_signal(number: c_int);
    fn rg_guile_pending_signal() -> c_int;
}

/// Default-off component handler used by isolated signal proofs. No frontend
/// installs this policy; the real host guard must preserve its own lifecycle.
pub fn component_signal_handler() -> unsafe extern "C" fn(c_int) {
    rg_guile_component_signal
}

/// # Safety
/// Only pass a caught host-fatal signal. C uses lock-free atomics and write;
/// calling this does not replace the host's pending flag or perform cleanup.
pub unsafe fn notify_signal(number: i32) {
    unsafe { rg_guile_notify_signal(number) };
}

pub fn pending_signal() -> Option<i32> {
    match unsafe { rg_guile_pending_signal() } {
        0 => None,
        number => Some(number),
    }
}

/// Enter a private Rust host stack while C retains the ordinary leading stack.
///
/// # Safety
/// Invoke this on the process-leading thread before foreign runtime entry. The
/// closure must terminate through `native::Runtime::exit` while its retained
/// NativeHost/context is alive after using native/Guile state. It may return
/// normally only when no foreign lifetime remains. Never call libc/Rust process
/// exit directly from the private Rust stack, or pass SCM objects into Rust.
/// Panic handling is a programmer-error path; it cannot replace engine cleanup.
/// No production entry point enables this component yet.
pub unsafe fn run_host<F, R>(function: F) -> std::io::Result<R>
where
    F: FnOnce() -> R,
{
    struct Host<F, R> {
        function: Option<F>,
        result: Option<std::thread::Result<R>>,
    }
    unsafe extern "C" fn enter<F: FnOnce() -> R, R>(data: *mut c_void) {
        // SAFETY: run_host keeps this owned context alive until C returns;
        // this callback runs only once, on the same OS thread/private stack.
        let host = unsafe { &mut *data.cast::<Host<F, R>>() };
        stacker::grow(8 * 1024 * 1024, || {
            let function = host.function.take().expect("one host entry");
            host.result = Some(std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                function,
            )));
        });
    }
    let mut host = Host {
        function: Some(function),
        result: None,
    };
    // SAFETY: caller accepts the lifecycle contract; C never executes Rust
    // handlers on its ordinary stack or unwinds over these host frames.
    let status = unsafe { rg_dispatch_run(enter::<F, R>, (&mut host as *mut Host<F, R>).cast()) };
    if status < 0 {
        return Err(std::io::Error::last_os_error());
    }
    match host.result.expect("C returned after the host closure") {
        Ok(result) => Ok(result),
        Err(panic) => std::panic::resume_unwind(panic),
    }
}

pub(crate) fn initialized() -> bool {
    // This is an adapter-owned flag, written only on this same interpreter
    // thread before first Guile entry; no library-internal state is accessed.
    unsafe { rg_guile_initialized() != 0 }
}
pub(crate) fn host_active() -> bool {
    unsafe { rg_dispatch_on_host() != 0 }
}

#[derive(Debug)]
pub enum Error {
    Host(ExpandError),
    Scheme(Vec<u8>),
    System(std::io::Error),
}

pub struct Runtime {
    native: Rc<crate::native::Runtime>,
}
impl Runtime {
    pub fn new() -> Result<Self, Error> {
        Self::from_native(Rc::new(crate::native::Runtime::new()))
    }
    pub fn from_native(native: Rc<crate::native::Runtime>) -> Result<Self, Error> {
        if unsafe { rg_dispatch_on_host() } == 0 {
            return Err(Error::System(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "Guile requires the leading-C/private-Rust dispatcher",
            )));
        }
        Ok(Self { native })
    }
    pub fn native(&self) -> Rc<crate::native::Runtime> {
        self.native.clone()
    }
    /// String-valued component evaluation. GNU conversion/error policy remains
    /// independently unqualified; no public function/FEATURES is registered.
    pub fn evaluate(&self, source: &[u8], host: &mut dyn NativeHost) -> Result<Vec<u8>, Error> {
        if unsafe { rg_dispatch_on_host() } == 0 {
            return Err(Error::System(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "Guile host stack is not active",
            )));
        }
        if pending_signal().is_some() {
            return Err(Error::Host(ExpandError::Cancelled));
        }
        let (value, error) = self
            .native
            .evaluate_guile(source, host)
            .map_err(Error::Host)?;
        if error.is_empty() {
            Ok(value)
        } else {
            Err(Error::Scheme(error))
        }
    }
    pub fn initialized(&self) -> bool {
        initialized()
    }
}
