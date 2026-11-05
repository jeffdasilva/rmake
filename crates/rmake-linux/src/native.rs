// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Native object loading and the GNU-compatible public allocation/callback ABI.
//! Foreign execution stays on a C-owned stack; interpreter requests return to
//! the original Rust stack. Errors abandon that stack and use ordinary cleanup.
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ffi::{CStr, CString, c_char, c_int, c_uint, c_ulong, c_void};
use std::mem::ManuallyDrop;
use std::ptr::{self, NonNull};
use std::rc::{Rc, Weak};
use std::sync::OnceLock;

use rmake_core::diagnostic::{Location, Output};
use rmake_core::messages::Message;
use rmake_core::native::{
    LoadOutcome, NativeCallback, NativeFunction, NativeHost, NativeLoadStatus, NativeRuntime,
};
use rmake_core::variables::{ExpandError, MAX_OUTPUT};

#[repr(C)]
struct ForeignLocation {
    file: *const c_char,
    line: c_ulong,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct Request {
    kind: c_int,
    text: *const c_char,
    location: *const ForeignLocation,
    function: *mut c_void,
    minimum: c_uint,
    maximum: c_uint,
    flags: c_uint,
}
unsafe extern "C" {
    fn rm_native_create(
        operation: c_int,
        function: *mut c_void,
        name: *const c_char,
        number: c_int,
        file: *const c_char,
        line: c_ulong,
        argc: c_uint,
        argv: *const *const c_char,
    ) -> *mut c_void;
    fn rm_native_drive(session: *mut c_void, reply: *mut c_void) -> *const Request;
    fn rm_native_result(session: *const c_void) -> *mut c_void;
    fn rm_native_number(session: *const c_void) -> c_int;
    fn rm_native_error(session: *const c_void) -> *const c_char;
    fn rm_native_destroy(session: *mut c_void);
    fn rm_native_abandon(session: *mut c_void);
    fn rm_native_base(address: *mut c_void) -> *mut c_void;
    fn rm_native_flush() -> c_int;
    fn rm_native_stdio_initialize();
    fn rm_native_stdout_line_buffered();
    fn rm_native_flush_stream(error: c_int) -> c_int;
    fn rm_native_stdout_closed() -> c_int;
    fn rm_native_thread_id() -> usize;
    #[cfg(feature = "guile")]
    fn rg_guile_callback(name: *const c_char, argc: c_uint, argv: *mut *mut c_char) -> *mut c_char;
    #[cfg(feature = "guile")]
    fn rg_guile_public_callback(
        name: *const c_char,
        argc: c_uint,
        argv: *mut *mut c_char,
    ) -> *mut c_char;
    #[cfg(feature = "guile")]
    fn rg_workers_function_base(function: *mut c_void, base: *mut usize) -> c_int;
    fn malloc(size: usize) -> *mut c_void;
    fn free(pointer: *mut c_void);
}

#[derive(Default)]
struct State {
    functions: RefCell<HashMap<Vec<u8>, Rc<NativeFunction>>>,
    loaded: RefCell<HashMap<Vec<u8>, Option<Rc<Image>>>>,
    owned: RefCell<HashMap<Vec<u8>, Vec<Rc<Image>>>>,
    images: RefCell<Vec<Rc<Image>>>,
    unloaded: RefCell<Vec<Vec<u8>>>,
    global: Cell<usize>,
    poisoned: Cell<bool>,
    exiting: Cell<bool>,
    abandoned_count: Cell<u64>,
    retry_exit: Cell<bool>,
    foreign_depth: Cell<usize>,
}
struct Image {
    handle: Cell<usize>,
    base: Cell<usize>,
    open: Cell<bool>,
    active_invocations: Cell<usize>,
}
impl Image {
    fn new() -> Self {
        Self {
            handle: Cell::new(0),
            base: Cell::new(0),
            open: Cell::new(true),
            active_invocations: Cell::new(0),
        }
    }
}
// GNU API interpretation is synchronous on one process-owned thread. Runtime
// instances share image identity on that thread; their variable stores remain
// separate. Claiming only at actual FFI entry keeps pure construction harmless.
static INTERPRETER_THREAD: OnceLock<usize> = OnceLock::new();
thread_local! {
    // libc runs DSO finalizers after Rust TLS destruction. Retain these weak
    // image records without a TLS destructor so late gmk_eval/load stays valid.
    static KNOWN_IMAGES:RefCell<ManuallyDrop<Vec<Weak<Image>>>>=const {RefCell::new(ManuallyDrop::new(Vec::new()))};
    static FOREIGN_DEPTH:Cell<usize>=const {Cell::new(0)};
}
fn claim_interpreter_thread() -> Result<(), ExpandError> {
    // pthread identity does not access Rust's already-destroyed thread object
    // when a DSO finalizer calls back into the interpreter.
    let current = unsafe { rm_native_thread_id() };
    if *INTERPRETER_THREAD.get_or_init(|| current) != current {
        return Err(error(
            b"rmake: native operations require the process interpreter thread".to_vec(),
        ));
    }
    Ok(())
}

/// Handles intentionally remain resident until explicit unload or process exit.
/// Logical Makefile restarts involving native objects must self-exec.
/// Actual native operations are restricted to the first interpreter thread;
/// another thread receives an error before entering foreign code. Independent
/// instances on that thread share image pins and active-execution protection.
#[derive(Clone, Default)]
pub struct Runtime(Rc<State>);

fn error(message: impl Into<Vec<u8>>) -> ExpandError {
    ExpandError::FileError(message.into())
}
fn bytes(pointer: *const c_char) -> Vec<u8> {
    if pointer.is_null() {
        Vec::new()
    } else {
        // Public API strings remain valid while their C session is suspended.
        unsafe { CStr::from_ptr(pointer) }.to_bytes().to_vec()
    }
}
fn cstring(value: &[u8]) -> CString {
    let end = value.iter().position(|&b| b == 0).unwrap_or(value.len());
    CString::new(&value[..end]).expect("NUL prefix removed")
}
fn allocated(value: &[u8]) -> Result<*mut c_void, ExpandError> {
    let size = value
        .len()
        .checked_add(1)
        .ok_or(ExpandError::ExpansionLimit)?;
    // malloc/free are the allocator contract shared with GNU-compatible modules.
    let result = unsafe { malloc(size) };
    if result.is_null() {
        return Err(error(b"virtual memory exhausted".to_vec()));
    }
    unsafe {
        ptr::copy_nonoverlapping(value.as_ptr(), result.cast(), value.len());
        *result.cast::<u8>().add(value.len()) = 0;
    }
    Ok(result)
}
struct Session {
    pointer: NonNull<c_void>,
    completed: bool,
    state: Rc<State>,
    owner: Option<Rc<Image>>,
}
impl Drop for Session {
    fn drop(&mut self) {
        if self.completed {
            unsafe { rm_native_destroy(self.pointer.as_ptr()) };
            self.state
                .foreign_depth
                .set(self.state.foreign_depth.get() - 1);
            FOREIGN_DEPTH.with(|depth| depth.set(depth.get() - 1));
            if let Some(owner) = &self.owner {
                owner
                    .active_invocations
                    .set(owner.active_invocations.get() - 1);
            }
        } else {
            self.state.poisoned.set(true);
            self.state
                .abandoned_count
                .set(self.state.abandoned_count.get().wrapping_add(1));
            // This session may contain live C++/foreign automatic objects. Do
            // not unwind, unmap, free arguments, or resume after interpreter Err.
            unsafe { rm_native_abandon(self.pointer.as_ptr()) };
            // Keep the active counts and ownership alive as long as the
            // retained foreign frames. Even a later safe-host call must not
            // dlclose code still represented by this suspended invocation.
            std::mem::forget(self.state.clone());
            if let Some(owner) = self.owner.take() {
                std::mem::forget(owner);
            }
        }
    }
}
struct ResultValue {
    pointer: *mut c_void,
    number: c_int,
    message: Vec<u8>,
}
impl Runtime {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn has_modules(&self) -> bool {
        #[cfg(feature = "guile")]
        if crate::guile::initialized() {
            return true;
        }
        !self.0.images.borrow().is_empty()
            || !self.0.loaded.borrow().is_empty()
            || !self.0.unloaded.borrow().is_empty()
    }
    pub fn unloaded(&self) -> Vec<Vec<u8>> {
        self.0.unloaded.borrow().clone()
    }
    /// Register the opt-in Guile function without initializing the language.
    /// This trusted executable-resident callback needs no foreign image lookup
    /// or interpreter-thread claim. Actual invocation still requires an active
    /// dispatcher host. Frontends control their own feature publication.
    #[cfg(feature = "guile")]
    pub fn register_guile(&self) -> Result<(), ExpandError> {
        if self.0.poisoned.get() && !self.0.exiting.get() {
            return Err(error(
                b"rmake: native invocation stopped after a fatal error".to_vec(),
            ));
        }
        self.0.functions.borrow_mut().insert(
            b"guile".to_vec(),
            Rc::new(NativeFunction {
                minimum: 0,
                maximum: Some(1),
                expand_arguments: true,
                callback: Rc::new(Callback {
                    state: Rc::downgrade(&self.0),
                    function: rg_guile_public_callback as *const () as usize,
                    owner: None,
                }),
            }),
        );
        Ok(())
    }
    #[cfg(feature = "guile")]
    pub(crate) fn evaluate_guile(
        &self,
        source: &[u8],
        host: &mut dyn NativeHost,
    ) -> Result<(Vec<u8>, Vec<u8>), ExpandError> {
        let result = self.run(
            3,
            rg_guile_callback as *const () as usize,
            Some(b"guile"),
            0,
            None,
            &[source.to_vec()],
            None,
            host,
        )?;
        let value = if result.pointer.is_null() {
            Vec::new()
        } else {
            let value = bytes(result.pointer.cast());
            unsafe { free(result.pointer) };
            value
        };
        if value.len() > MAX_OUTPUT {
            return Err(ExpandError::ExpansionLimit);
        }
        Ok((value, result.message))
    }
    fn check(&self) -> Result<(), ExpandError> {
        #[cfg(feature = "guile")]
        if !crate::guile::host_active() {
            return Err(error(
                b"rmake: Guile-enabled native operations require the leading-C dispatcher".to_vec(),
            ));
        }
        if self.0.poisoned.get() && !self.0.exiting.get() {
            Err(error(
                b"rmake: native invocation stopped after a fatal error".to_vec(),
            ))
        } else {
            Ok(())
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn run(
        &self,
        operation: c_int,
        function: usize,
        name: Option<&[u8]>,
        number: c_int,
        location: Option<&Location>,
        args: &[Vec<u8>],
        owner: Option<Rc<Image>>,
        host: &mut dyn NativeHost,
    ) -> Result<ResultValue, ExpandError> {
        self.check()?;
        claim_interpreter_thread()?;
        let next_global_depth = FOREIGN_DEPTH
            .with(|depth| depth.get().checked_add(1))
            .ok_or(ExpandError::ExpansionLimit)?;
        let next_depth = self
            .0
            .foreign_depth
            .get()
            .checked_add(1)
            .ok_or(ExpandError::ExpansionLimit)?;
        let next_image_depth = owner
            .as_ref()
            .map(|image| {
                image
                    .active_invocations
                    .get()
                    .checked_add(1)
                    .ok_or(ExpandError::ExpansionLimit)
            })
            .transpose()?;
        let name = name.map(cstring);
        let file = location.and_then(|l| l.file.as_option()).map(cstring);
        let args: Vec<_> = args.iter().map(|a| cstring(a)).collect();
        let pointers: Vec<_> = args.iter().map(|a| a.as_ptr()).collect();
        // C owns copies of all arguments before makecontext can enter a module.
        let raw = unsafe {
            rm_native_create(
                operation,
                function as *mut c_void,
                name.as_ref().map_or(ptr::null(), |s| s.as_ptr()),
                number,
                file.as_ref().map_or(ptr::null(), |s| s.as_ptr()),
                location.map_or(0, |l| l.line as c_ulong),
                pointers
                    .len()
                    .try_into()
                    .map_err(|_| ExpandError::ExpansionLimit)?,
                pointers.as_ptr(),
            )
        };
        let raw = NonNull::new(raw)
            .ok_or_else(|| error(b"rmake: unable to reserve native invocation stack".to_vec()))?;
        self.0.foreign_depth.set(next_depth);
        FOREIGN_DEPTH.with(|depth| depth.set(next_global_depth));
        if let Some(image) = &owner {
            image
                .active_invocations
                .set(next_image_depth.expect("image depth computed"));
        }
        let mut session = Session {
            pointer: raw,
            completed: false,
            state: self.0.clone(),
            owner: owner.clone(),
        };
        let initial_abandoned = self.0.abandoned_count.get();
        let mut reply = ptr::null_mut();
        loop {
            if self.0.abandoned_count.get() != initial_abandoned {
                return Err(error(
                    b"rmake: native invocation stopped after a nested fatal error".to_vec(),
                ));
            }
            // Only foreign code and its C ABI shim execute here. On a yielded operation,
            // no Rust frame or borrow lives on the foreign stack.
            let request = unsafe { *rm_native_drive(raw.as_ptr(), reply) };
            reply = ptr::null_mut();
            #[cfg(feature = "guile")]
            if !self.0.exiting.get() && crate::guile::pending_signal().is_some() {
                return Err(ExpandError::Cancelled);
            }
            match request.kind {
                0 => {
                    session.completed = true;
                    return Ok(ResultValue {
                        pointer: unsafe { rm_native_result(raw.as_ptr()) },
                        number: unsafe { rm_native_number(raw.as_ptr()) },
                        message: bytes(unsafe { rm_native_error(raw.as_ptr()) }),
                    });
                }
                1 => {
                    let input = bytes(request.text);
                    reply = allocated(&host.expand(&input)?)?;
                }
                2 => {
                    let input = bytes(request.text);
                    let location = if request.location.is_null() {
                        None
                    } else {
                        let l = unsafe { &*request.location };
                        Some(Location {
                            file: if l.file.is_null() {
                                rmake_core::diagnostic::SourceName::null()
                            } else {
                                bytes(l.file).into()
                            },
                            line: l.line as usize,
                        })
                    };
                    host.evaluate(&input, location.as_ref())?;
                }
                3 => self.register(request, owner.clone(), host)?,
                5 => {
                    let message = bytes(request.text);
                    if !host.emit(Output::Warning {
                        location: None,
                        message: &message,
                    }) {
                        return Err(error(message));
                    }
                }
                #[cfg(feature = "guile")]
                6 => return Err(ExpandError::Cancelled),
                _ => return Err(error(b"virtual memory exhausted".to_vec())),
            }
        }
    }
    fn function_base(&self, function: usize) -> usize {
        #[cfg(feature = "guile")]
        {
            let mut base = 0;
            // The worker resolves this while it owns any constructor loader
            // lock; only an owned integer address crosses back to this thread.
            if unsafe { rg_workers_function_base(function as *mut c_void, &mut base) } != 0 {
                return base;
            }
        }
        unsafe { rm_native_base(function as *mut c_void) as usize }
    }
    fn image_for(&self, function: usize) -> Option<Rc<Image>> {
        let base = self.function_base(function);
        KNOWN_IMAGES.with(|known| {
            let mut known = known.borrow_mut();
            known.retain(|image| image.strong_count() != 0);
            known
                .iter()
                .filter_map(Weak::upgrade)
                .find(|image| base != 0 && image.open.get() && image.base.get() == base)
        })
    }
    fn latest_owned(&self, name: &[u8]) -> Option<Rc<Image>> {
        self.0
            .owned
            .borrow()
            .get(name)
            .and_then(|images| images.iter().rev().find(|image| image.open.get()).cloned())
    }

    fn register(
        &self,
        request: Request,
        owner: Option<Rc<Image>>,
        host: &mut dyn NativeHost,
    ) -> Result<(), ExpandError> {
        let name = bytes(request.text);
        let prefix = name
            .iter()
            .take_while(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
            .count();
        if prefix == 0 {
            return Err(ExpandError::system(Message::EmptyNativeFunction));
        }
        if name[0] == b'.' || prefix != name.len() {
            return Err(ExpandError::system(Message::InvalidNativeFunction(&name)));
        }
        if name.len() > 255 {
            return Err(ExpandError::system(Message::LongNativeFunction(&name)));
        }
        if request.minimum > 255 {
            return Err(ExpandError::system(Message::NativeArgumentCount {
                minimum: true,
                count: request.minimum,
                name: &name,
            }));
        }
        if request.maximum > 255 || (request.maximum != 0 && request.maximum < request.minimum) {
            return Err(ExpandError::system(Message::NativeArgumentCount {
                minimum: false,
                count: request.maximum,
                name: &name,
            }));
        }
        if rmake_core::native::is_builtin_function(&name) {
            return Err(error(
                [
                    b"rmake: native registration cannot replace built-in function '".as_slice(),
                    &name,
                    b"'",
                ]
                .concat(),
            ));
        }
        if request.function.is_null() {
            return Err(error(
                b"rmake: native function has a null callback".to_vec(),
            ));
        }
        let owner = self.image_for(request.function as usize).or(owner);
        if let Some(image) = &owner
            && image.base.get() == 0
        {
            image
                .base
                .set(self.function_base(request.function as usize));
        }
        host.registered_function_name(&name);
        let callback = Rc::new(Callback {
            state: Rc::downgrade(&self.0),
            function: request.function as usize,
            owner,
        });
        self.0.functions.borrow_mut().insert(
            name,
            Rc::new(NativeFunction {
                minimum: request.minimum as u8,
                maximum: (request.maximum != 0).then_some(request.maximum as u8),
                expand_arguments: request.flags & 1 == 0,
                callback,
            }),
        );
        Ok(())
    }
    /// Run libc finalizers while a live interpreter host can service their API
    /// requests. A successful libc exit terminates the process. On a finalizer
    /// error its stack remains suspended; the application reports and retries
    /// exit with failure, letting libc advance its remaining callback list.
    pub fn exit(&self, status: u8, host: &mut dyn NativeHost) -> Result<(), ExpandError> {
        self.0.exiting.set(true);
        self.0.retry_exit.set(false);
        let before = self.0.abandoned_count.get();
        match self.run(5, 0, None, c_int::from(status), None, &[], None, host) {
            Err(error) => {
                self.0
                    .retry_exit
                    .set(self.0.abandoned_count.get() != before);
                Err(error)
            }
            Ok(_) => Err(error(
                b"rmake: native process exit unexpectedly returned".to_vec(),
            )),
        }
    }
    /// Finish an already-cleaned-up fatal signal on the ordinary C stack.
    /// Signal termination skips ELF finalizers, matching the host signal path.
    #[cfg(feature = "guile")]
    pub fn terminate_signal(
        &self,
        number: i32,
        host: &mut dyn NativeHost,
    ) -> Result<(), ExpandError> {
        if !matches!(number, 1 | 2 | 3 | 13 | 15 | 24 | 25) {
            return Err(error(b"unsupported native termination signal".to_vec()));
        }
        self.0.exiting.set(true);
        self.0.retry_exit.set(false);
        self.run(6, 0, None, number, None, &[], None, host)?;
        Err(error(
            b"native signal termination unexpectedly returned".to_vec(),
        ))
    }

    pub fn can_retry_exit(&self) -> bool {
        self.0.retry_exit.get()
    }

    pub fn unload(&self, name: &[u8]) -> Result<bool, Vec<u8>> {
        self.unload_with_host(name, &mut NoHost)
            .map_err(|e| e.diagnostic().as_bytes().to_vec())
    }
    pub fn unload_with_host(
        &self,
        name: &[u8],
        host: &mut dyn NativeHost,
    ) -> Result<bool, ExpandError> {
        self.check()?;
        // An arbitrary safe NativeHost can re-enter this public method while
        // setup/callback/loader/finalizer code is suspended. An Rc alone does
        // not prevent explicit dlclose; reject before changing any state.
        if FOREIGN_DEPTH.with(|depth| depth.get() != 0) {
            return Err(error(
                b"rmake: cannot unload a native object during foreign execution".to_vec(),
            ));
        }
        let Some(image) = self.0.loaded.borrow().get(name).cloned() else {
            return Ok(false);
        };
        if image
            .as_ref()
            .is_some_and(|image| image.active_invocations.get() != 0)
        {
            return Err(error(
                b"rmake: cannot unload an active native object".to_vec(),
            ));
        }
        if let Some(image) = image
            && image.open.get()
            && image.handle.get() != 0
        {
            let result = self.run(
                4,
                image.handle.get(),
                None,
                0,
                None,
                &[],
                Some(image.clone()),
                host,
            )?;
            if result.number != 0 {
                return Err(error(b"rmake: dlclose failed".to_vec()));
            }
            image.open.set(false);
        }
        self.0.loaded.borrow_mut().remove(name);
        if !self.0.unloaded.borrow().iter().any(|n| n == name) {
            self.0.unloaded.borrow_mut().push(name.to_vec());
        }
        Ok(true)
    }
}
struct Callback {
    state: Weak<State>,
    function: usize,
    owner: Option<Rc<Image>>,
}
impl NativeCallback for Callback {
    fn invoke(
        &self,
        name: &[u8],
        args: &[Vec<u8>],
        host: &mut dyn NativeHost,
    ) -> Result<Vec<u8>, ExpandError> {
        if self.owner.as_ref().is_some_and(|i| !i.open.get()) {
            return Err(error(
                b"rmake: native function belongs to an unloaded object".to_vec(),
            ));
        }
        let runtime = Runtime(
            self.state
                .upgrade()
                .ok_or_else(|| error(b"rmake: native runtime is unavailable".to_vec()))?,
        );
        let result = runtime.run(
            3,
            self.function,
            Some(name),
            0,
            None,
            args,
            self.owner.clone(),
            host,
        )?;
        if result.pointer.is_null() {
            return Ok(Vec::new());
        }
        let value = bytes(result.pointer.cast());
        unsafe { free(result.pointer) };
        if value.len() > MAX_OUTPUT {
            return Err(ExpandError::ExpansionLimit);
        }
        Ok(value)
    }
}
struct NoHost;
impl NativeHost for NoHost {
    fn expand(&mut self, _: &[u8]) -> Result<Vec<u8>, ExpandError> {
        Err(error(
            b"rmake: native destructor requires an interpreter context".to_vec(),
        ))
    }
    fn evaluate(&mut self, _: &[u8], _: Option<&Location>) -> Result<(), ExpandError> {
        Err(error(
            b"rmake: native destructor requires an interpreter context".to_vec(),
        ))
    }
    fn emit(&mut self, _: Output<'_>) -> bool {
        false
    }
}
impl NativeRuntime for Runtime {
    fn terminate(&self, status: u8, host: &mut dyn NativeHost) -> Result<(), ExpandError> {
        Runtime::exit(self, status, host)
    }
    fn unload(&self, name: &[u8], host: &mut dyn NativeHost) -> Result<bool, ExpandError> {
        self.unload_with_host(name, host)
    }
    fn unloaded(&self) -> Vec<Vec<u8>> {
        Runtime::unloaded(self)
    }
    fn lookup(&self, name: &[u8]) -> Option<Rc<NativeFunction>> {
        self.0.functions.borrow().get(name).cloned()
    }
    fn load(
        &self,
        name: &[u8],
        optional: bool,
        location: &Location,
        host: &mut dyn NativeHost,
    ) -> Result<LoadOutcome, ExpandError> {
        self.check()?;
        let (name, symbol) = setup_name(name)?;
        if self.0.loaded.borrow().contains_key(&name) {
            return Ok(LoadOutcome {
                name,
                status: NativeLoadStatus::Skipped,
            });
        }
        let global = if self.0.global.get() != 0 {
            self.0.global.get()
        } else {
            let result = self.run(0, 0, None, 2 | 0x100, Some(location), &[], None, host)?;
            if result.pointer.is_null() {
                return Err(error(
                    [
                        b"Failed to open global symbol table: ".as_slice(),
                        &result.message,
                    ]
                    .concat(),
                ));
            }
            let value = result.pointer as usize;
            self.0.global.set(value);
            value
        };
        let mut function = self
            .run(1, global, Some(&symbol), 0, Some(location), &[], None, host)?
            .pointer as usize;
        let mut owner = self.image_for(function);
        // Global setup lookup acquires no new handle. Actual opens are tracked
        // separately from completed load records, because a required failed
        // setup may later be retried from a native finalizer.

        if function == 0 {
            let image = Rc::new(Image::new());
            self.0.images.borrow_mut().push(image.clone());
            KNOWN_IMAGES.with(|known| known.borrow_mut().push(Rc::downgrade(&image)));
            let first = if !name.contains(&b'/') {
                Some(self.run(
                    0,
                    0,
                    Some(&[b"./".as_slice(), &name].concat()),
                    1 | 0x100,
                    Some(location),
                    &[],
                    Some(image.clone()),
                    host,
                )?)
            } else {
                None
            };
            let opened = match first {
                Some(result) if !result.pointer.is_null() => result,
                _ => self.run(
                    0,
                    0,
                    Some(&name),
                    1 | 0x100,
                    Some(location),
                    &[],
                    Some(image.clone()),
                    host,
                )?,
            };
            if opened.pointer.is_null() {
                image.open.set(false);
                if !optional
                    && !host.emit(Output::Warning {
                        location: Some(location),
                        message: &opened.message,
                    })
                {
                    return Err(error(opened.message));
                }
                if optional {
                    self.0
                        .loaded
                        .borrow_mut()
                        .insert(name.clone(), self.latest_owned(&name));
                }
                return Ok(LoadOutcome {
                    name,
                    status: NativeLoadStatus::Failed,
                });
            }
            image.handle.set(opened.pointer as usize);
            let marker = self.run(
                1,
                image.handle.get(),
                Some(b"plugin_is_GPL_compatible"),
                0,
                Some(location),
                &[],
                Some(image.clone()),
                host,
            )?;
            if marker.pointer.is_null() {
                return Err(ExpandError::system(Message::MissingNativeLicense(&name)));
            }
            let found = self.run(
                1,
                image.handle.get(),
                Some(&symbol),
                0,
                Some(location),
                &[],
                Some(image.clone()),
                host,
            )?;
            if found.pointer.is_null() {
                return Err(ExpandError::system(Message::NativeSymbolFailure {
                    symbol: &symbol,
                    object: &name,
                    detail: &found.message,
                }));
            }
            function = found.pointer as usize;
            image
                .base
                .set(unsafe { rm_native_base(found.pointer) } as usize);
            // A valid setup symbol has an owned object even if setup returns
            // zero or aborts interpretation. Keep that reference available for
            // a later global-symbol retry; aliases own no such reference.
            self.0
                .owned
                .borrow_mut()
                .entry(name.clone())
                .or_default()
                .push(image.clone());
            owner = Some(image);
        }
        let result = self.run(
            2,
            function,
            None,
            0,
            Some(location),
            &[],
            owner.clone(),
            host,
        )?;
        if result.number != 0 || optional {
            self.0
                .loaded
                .borrow_mut()
                .insert(name.clone(), self.latest_owned(&name));
            self.0.unloaded.borrow_mut().retain(|n| n != &name);
        }
        Ok(LoadOutcome {
            name,
            status: if result.number != 0 {
                NativeLoadStatus::Loaded
            } else {
                NativeLoadStatus::Failed
            },
        })
    }
}
fn setup_name(input: &[u8]) -> Result<(Vec<u8>, Vec<u8>), ExpandError> {
    if let Some(open) = input.iter().position(|&b| b == b'(')
        && let Some(close) = input[open + 1..]
            .iter()
            .position(|&b| b == b')')
            .map(|i| open + 1 + i)
        && close + 1 == input.len()
    {
        if close == open + 1 {
            return Err(ExpandError::system(Message::EmptyLoadSymbol(input)));
        }
        return Ok((input[..open].to_vec(), input[open + 1..close].to_vec()));
    }
    let base = input.rsplit(|&b| b == b'/').next().unwrap_or(input);
    let end = base
        .iter()
        .position(|b| !b.is_ascii_alphanumeric() && *b != b'_')
        .unwrap_or(base.len());
    Ok((input.to_vec(), [&base[..end], b"_gmk_setup"].concat()))
}

/// Flush C stdio before an exec, which does not run libc's exit handlers.
pub fn flush_standard_streams() -> std::io::Result<()> {
    if unsafe { rm_native_flush() } == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

/// GNU's native-visible stdout is line buffered independently of Rust output.
/// Sessions initialize this lazily before the first object constructor executes.
pub fn initialize_stdio() {
    unsafe { rm_native_stdio_initialize() };
}

/// Publish the running application's C stdout policy after early CLI exits.
/// Registering final stdout cleanup and loading a native module do not select it.
pub fn line_buffer_stdout() {
    unsafe { rm_native_stdout_line_buffered() };
}

/// Flush only the selected libc stream before a direct host write to that
/// stream. Synchronized host writes to separate spool files must not use this.
pub fn flush_stream(error: bool) -> std::io::Result<()> {
    if unsafe { rm_native_flush_stream(c_int::from(error)) } == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

/// Whether the native-visible stdout stream has reached its final close.
/// Descriptor reuse after that point does not reopen the original C stream.
/// This query never inspects a FILE object after fclose.
pub fn stdout_closed() -> bool {
    unsafe { rm_native_stdout_closed() != 0 }
}
