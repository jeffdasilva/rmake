// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! One extension namespace per prepared invocation, shared by every effect context.
use rmake_core::native::NativeRuntime;
use rmake_linux::native::Runtime;
use std::cell::RefCell;
use std::mem::ManuallyDrop;
use std::rc::Rc;

thread_local! {
    // libc destroys ordinary Rust TLS before DSO finalizers. The namespace
    // owner must remain available while those finalizers call the make API.
    static RUNTIME: RefCell<Option<ManuallyDrop<Rc<Runtime>>>> =
        const { RefCell::new(None) };
}

pub fn current() -> Rc<dyn NativeRuntime> {
    RUNTIME.with(|runtime| {
        let mut runtime = runtime.borrow_mut();
        let owner = runtime.get_or_insert_with(|| ManuallyDrop::new(Rc::new(Runtime::default())));
        Rc::clone(owner) as Rc<dyn NativeRuntime>
    })
}

pub fn reset() {
    let old = RUNTIME.with(|runtime| {
        runtime
            .borrow_mut()
            .replace(ManuallyDrop::new(Rc::new(Runtime::default())))
    });
    if let Some(old) = old {
        drop(ManuallyDrop::into_inner(old));
    }
}

#[cfg(feature = "guile")]
pub fn register_guile() -> Result<(), rmake_core::variables::ExpandError> {
    RUNTIME.with(|runtime| {
        let runtime = runtime.borrow();
        runtime
            .as_ref()
            .expect("fresh invocation namespace")
            .register_guile()
    })
}

pub fn has_modules() -> bool {
    RUNTIME.with(|runtime| {
        runtime
            .borrow()
            .as_ref()
            .is_some_and(|runtime| runtime.has_modules())
    })
}

pub fn unloaded() -> Vec<Vec<u8>> {
    RUNTIME.with(|runtime| {
        runtime
            .borrow()
            .as_ref()
            .map_or_else(Vec::new, |runtime| runtime.unloaded())
    })
}

pub fn can_retry_exit() -> bool {
    RUNTIME.with(|runtime| {
        runtime
            .borrow()
            .as_ref()
            .is_some_and(|runtime| runtime.can_retry_exit())
    })
}

#[cfg(feature = "test-support")]
pub(crate) fn run_native_safety_check() {
    tests::independent_runtimes_pin_active_code_and_alias_records_do_not_own_handles();
}

#[cfg(any(all(test, not(feature = "guile")), feature = "test-support"))]
mod tests;
