// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Interpreter output focus and recipe-owned spools. Focus holds weak references;
//! buffers keep the controller alive without creating a controller/buffer cycle.
use crate::cli::DirectoryChoice;
use rmake_linux::output::{Coordination, Spool};
use std::cell::{Cell, Ref, RefCell};
use std::io::{self, Write};
use std::rc::{Rc, Weak};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    None,
    Line,
    Target,
    Recurse,
}

impl Mode {
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        Some(match bytes {
            b"none" => Self::None,
            b"line" => Self::Line,
            b"target" => Self::Target,
            b"recurse" => Self::Recurse,
            _ => return None,
        })
    }
    pub fn root_buffered(self) -> bool {
        matches!(self, Self::Line | Self::Target)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum ProcessTrace {
    #[default]
    NotEntered,
    Entered,
    Closed,
}

struct State {
    mode: Cell<Mode>,
    current: RefCell<Option<Weak<Buffer>>>,
    coordination: RefCell<Option<Coordination>>,
    program: RefCell<Vec<u8>>,
    directory_program: Cell<(usize, u32)>,
    interrupted_owner: RefCell<Option<Owner>>,
    directory: RefCell<Option<Vec<u8>>>,
    directory_trace: Cell<ProcessTrace>,
    directory_choice: Cell<DirectoryChoice>,
    silent: Cell<bool>,
    changed_directory: Cell<bool>,
    recursive: Cell<bool>,
    debug: Cell<crate::debug::Mask>,
    debug_signal: Cell<Option<rmake_linux::signals::DebugHandle>>,
    print_database: Cell<bool>,
    compatibility_header: Cell<bool>,
}

#[derive(Clone)]
pub struct Controller(Rc<State>);

impl Controller {
    pub fn new(mode: Mode, program: &[u8]) -> Self {
        Self(Rc::new(State {
            mode: Cell::new(mode),
            current: RefCell::new(None),
            coordination: RefCell::new(None),
            program: RefCell::new(program.to_vec()),
            directory_program: Cell::new((program.len(), 0)),
            interrupted_owner: RefCell::new(None),
            directory: RefCell::new(None),
            directory_trace: Cell::new(ProcessTrace::NotEntered),
            directory_choice: Cell::new(DirectoryChoice::Automatic),
            silent: Cell::new(false),
            changed_directory: Cell::new(false),
            recursive: Cell::new(false),
            debug: Cell::new(crate::debug::Mask::NONE),
            debug_signal: Cell::new(None),
            print_database: Cell::new(false),
            compatibility_header: Cell::new(false),
        }))
    }
    pub fn bind_debug_signal(&self, handle: rmake_linux::signals::DebugHandle) {
        self.0.debug_signal.set(Some(handle));
    }
    pub fn activate_debug_signal(&self) -> std::io::Result<()> {
        if let Some(handle) = self.0.debug_signal.get() {
            handle.activate(self.0.debug.get().bits())?;
        }
        Ok(())
    }
    pub fn debug_mask(&self) -> crate::debug::Mask {
        self.0
            .debug_signal
            .get()
            .and_then(|handle| handle.current())
            .map_or_else(|| self.0.debug.get(), crate::debug::Mask::from_bits)
    }
    pub fn set_debug_mask(&self, mask: crate::debug::Mask) {
        self.0.debug.set(mask);
        if let Some(handle) = self.0.debug_signal.get() {
            handle.publish(mask.bits());
        }
    }
    pub fn database_requested(&self) -> bool {
        self.0.print_database.get()
    }
    pub fn set_database_requested(&self, enabled: bool) {
        self.0.print_database.set(enabled);
    }
    pub fn reset_compatibility_header(&self) {
        self.0.compatibility_header.set(false);
    }
    pub fn print_compatibility_header(&self, commented: bool) {
        if !self.0.compatibility_header.replace(true) {
            crate::debug::print_compatibility_header(commented);
        }
    }
    pub fn process_lifetime(&self) -> ProcessOutput {
        ProcessOutput(self.clone())
    }
    pub fn set_imported_directory(&self, directory: Option<&[u8]>) {
        let mut stored = self.0.directory.borrow_mut();
        match directory {
            None => *stored = None,
            Some(directory) => match stored.as_mut() {
                Some(current) if current.as_slice() == directory => {}
                Some(current) => {
                    current.clear();
                    current.extend_from_slice(directory);
                }
                None => *stored = Some(directory.to_vec()),
            },
        }
    }
    pub fn set_directory_policy(&self, choice: DirectoryChoice, silent: bool) {
        self.0.directory_choice.set(choice);
        self.0.silent.set(silent);
    }
    pub fn set_changed_directory(&self, changed: bool) {
        self.0.changed_directory.set(changed);
    }
    pub fn set_recursive(&self, recursive: bool) {
        self.0.recursive.set(recursive);
    }
    fn should_report_directory(&self) -> bool {
        match self.0.directory_choice.get() {
            DirectoryChoice::On => true,
            DirectoryChoice::Off => false,
            DirectoryChoice::Automatic => {
                !self.0.silent.get() && (self.0.changed_directory.get() || self.0.recursive.get())
            }
        }
    }
    pub fn has_directory_trace(&self) -> bool {
        self.0.directory_trace.get() == ProcessTrace::Entered
    }
    pub fn import_directory_trace(&self) {
        self.0.directory_trace.set(ProcessTrace::Entered);
    }
    /// GNU-managed output and actual child launches share this lazy boundary.
    /// Line/target output brackets belong to each nonempty locked spool dump.
    pub fn start(&self) {
        if !self.mode().root_buffered()
            && self.0.directory_trace.get() == ProcessTrace::NotEntered
            && self.should_report_directory()
        {
            self.0.directory_trace.set(ProcessTrace::Entered);
            self.print_directory(true);
        }
    }
    fn print_directory(&self, entering: bool) {
        if crate::native::has_modules() {
            // Directory messages use stdout even when stderr triggered them.
            let _ = rmake_linux::native::flush_stream(false);
        }
        let program = self.0.program.borrow();
        let directory = self.0.directory.borrow();
        let prefix = if self.database_requested() {
            b"# ".as_slice()
        } else {
            b""
        };
        // Startup publishes the base boundary and depth along with the
        // display name. A translated template can reorder those arguments.
        let (base_length, depth) = self.0.directory_program.get();
        let line = crate::messages::system(rmake_core::messages::Message::Directory {
            entering,
            program: &program[..base_length],
            depth,
            path: directory.as_deref(),
        });
        plain_parts(false, &[prefix, &line]);
    }
    fn dump_frame(&self, entering: bool, active: &mut bool) {
        if entering {
            self.start();
            *active = self.mode().root_buffered()
                && self.0.directory_trace.get() != ProcessTrace::Closed
                && self.should_report_directory();
        }
        if *active {
            self.print_directory(entering);
        }
    }
    fn finish_directory(&self) {
        if self.0.directory_trace.replace(ProcessTrace::Closed) == ProcessTrace::Entered {
            self.print_directory(false);
        }
        self.clear();
    }
    pub fn take_interrupted_owner(&self) -> Option<Owner> {
        self.0.interrupted_owner.borrow_mut().take()
    }
    pub fn mode(&self) -> Mode {
        self.0.mode.get()
    }
    pub fn set_mode(&self, mode: Mode) {
        self.0.mode.set(mode);
    }
    pub fn set_program(&self, program: &[u8], base_length: usize, depth: u32) {
        debug_assert!(base_length <= program.len());
        let mut stored = self.0.program.borrow_mut();
        stored.clear();
        stored.extend_from_slice(program);
        self.0.directory_program.set((base_length, depth));
    }
    fn allocate_buffer(&self, enabled: bool) -> Rc<Buffer> {
        Rc::new(Buffer {
            controller: self.clone(),
            enabled: Cell::new(enabled),
            spool: RefCell::new(None),
            failed: Cell::new(false),
            owner: RefCell::new(None),
            reaped: Cell::new(false),
        })
    }
    pub fn make_buffer(&self) -> Option<Rc<Buffer>> {
        (self.mode() != Mode::None).then(|| self.allocate_buffer(true))
    }
    // A recipe can enable output sync through eval while its body or shell
    // configuration expands. Keep its lazy lifetime even when it starts off.
    pub fn recipe_buffer(&self) -> Rc<Buffer> {
        self.allocate_buffer(self.mode() != Mode::None)
    }
    pub fn select(&self, buffer: Option<&Rc<Buffer>>) {
        self.0.current.replace(buffer.map(Rc::downgrade));
    }
    pub fn clear_buffer(&self, buffer: &Rc<Buffer>) {
        if self
            .current()
            .is_some_and(|current| Rc::ptr_eq(&current, buffer))
        {
            self.select(None);
        }
    }
    pub fn current(&self) -> Option<Rc<Buffer>> {
        self.0
            .current
            .borrow()
            .as_ref()
            .and_then(Weak::upgrade)
            .filter(|buf| buf.enabled.get())
    }
    pub fn write_parts(&self, error: bool, parts: &[&[u8]]) {
        if parts.iter().any(|part| !part.is_empty()) {
            self.start();
        }
        if let Some(buffer) = self.current() {
            for bytes in parts {
                buffer.write(error, bytes);
            }
        } else {
            plain_parts(error, parts);
        }
    }
    pub(crate) fn write_option_notice(&self, message: &[u8]) {
        if crate::native::has_modules() && self.current().is_none() {
            let _ = rmake_linux::native::flush_stream(true);
        }
        self.write_parts(
            true,
            &[self.0.program.borrow().as_slice(), b": ", message, b"\n"],
        );
    }

    pub fn decode_mutex(&self, authentication: &[u8]) -> io::Result<()> {
        match Coordination::join(authentication)? {
            Some(provider) => {
                self.0.coordination.replace(Some(provider));
            }
            None => self.write_parts(
                true,
                &[
                    self.0.program.borrow().as_slice(),
                    b": invalid --sync-mutex string '",
                    authentication,
                    b"'\n",
                ],
            ),
        }
        Ok(())
    }
    pub fn setup(&self, authentication: Option<&[u8]>) -> io::Result<Option<Vec<u8>>> {
        if !self.mode().root_buffered() {
            return Ok(authentication.map(<[u8]>::to_vec));
        }
        if let Some(provider) = self.0.coordination.borrow().as_ref()
            && authentication == Some(provider.authentication())
        {
            return Ok(authentication.map(<[u8]>::to_vec));
        }
        let provider = if let Some(auth) = authentication {
            match Coordination::join(auth)? {
                Some(provider) => provider,
                None => {
                    self.decode_mutex(auth)?;
                    return Ok(None);
                }
            }
        } else {
            Coordination::create()?
        };
        let auth = provider.authentication().to_vec();
        self.0.coordination.replace(Some(provider));
        Ok(Some(auth))
    }
    pub fn clear(&self) {
        self.0.coordination.borrow_mut().take();
    }
    pub fn warning(&self, bytes: &[u8]) {
        self.start();
        plain(
            true,
            &[self.0.program.borrow().as_slice(), b": ", bytes, b"\n"].concat(),
        );
    }
    fn disable(&self) {
        self.0.mode.set(Mode::None);
        self.clear();
    }
}

/// The process owns final output across constructor errors and logical rereads.
/// Recipe spools and successive interpreters may share the controller, but only
/// this outer owner closes the process directory trace.
pub struct ProcessOutput(Controller);
impl ProcessOutput {
    pub fn finish(&self) {
        self.0.finish_directory();
    }
}
impl Drop for ProcessOutput {
    fn drop(&mut self) {
        self.finish();
    }
}

#[derive(Clone)]
pub struct Owner {
    pub target: Vec<u8>,
    pub before: Option<std::time::SystemTime>,
    pub phony: bool,
    reaped: bool,
}

pub struct Buffer {
    controller: Controller,
    enabled: Cell<bool>,
    spool: RefCell<Option<Spool>>,
    failed: Cell<bool>,
    owner: RefCell<Option<Owner>>,
    reaped: Cell<bool>,
}

impl Buffer {
    pub fn set_owner(&self, target: &[u8], before: Option<std::time::SystemTime>, phony: bool) {
        if self.owner.borrow().is_some() {
            return;
        }
        self.owner.replace(Some(Owner {
            target: target.to_vec(),
            before,
            phony,
            reaped: self.reaped.get(),
        }));
    }
    pub fn child_reaped(&self, reaped: bool) {
        self.reaped.set(reaped);
        if let Some(owner) = self.owner.borrow_mut().as_mut() {
            owner.reaped = reaped;
        }
    }
    pub fn has_spool(&self) -> bool {
        self.spool.borrow().is_some()
    }
    pub fn enabled(&self) -> bool {
        self.enabled.get()
    }
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.set(enabled);
    }
    fn prepare(&self) {
        if self.failed.get() || self.spool.borrow().is_some() {
            return;
        }
        match Spool::new() {
            Ok(spool) => {
                self.spool.replace(Some(spool));
            }
            Err(_) => {
                self.failed.set(true);
                self.enabled.set(false);
                self.controller
                    .warning(b"cannot open output-sync lock file, suppressing output-sync.");
                self.controller.disable();
            }
        }
    }
    pub fn write(&self, error: bool, bytes: &[u8]) {
        self.prepare();
        if let Some(spool) = self.spool.borrow().as_ref() {
            let _ = spool.write(error, bytes);
        } else {
            plain(error, bytes);
        }
    }
    pub fn child_spool(&self) -> Option<Ref<'_, Spool>> {
        if !self.enabled() {
            return None;
        }
        self.prepare();
        Ref::filter_map(self.spool.borrow(), Option::as_ref).ok()
    }
    pub fn configure_child(&self, child: &mut std::process::Command) -> io::Result<()> {
        if let Some(spool) = self.child_spool() {
            spool.configure_child(child)?;
        }
        Ok(())
    }
    pub fn configure_stderr(&self, child: &mut std::process::Command) -> io::Result<()> {
        if !self.enabled() {
            return Ok(());
        }
        self.prepare();
        if let Some(spool) = self.spool.borrow().as_ref() {
            spool.configure_stderr(child)?;
        }
        Ok(())
    }
    pub fn dump(&self) {
        let error = {
            let spool = self.spool.borrow();
            let provider = self.controller.0.coordination.borrow();
            spool.as_ref().and_then(|spool| {
                let mut frame = false;
                spool
                    .dump(
                        provider.as_ref(),
                        &mut |entering| self.controller.dump_frame(entering, &mut frame),
                        &mut |operation, error| {
                            plain(
                                true,
                                format!("{operation}: {}\n", crate::engine::os_error(error))
                                    .as_bytes(),
                            );
                        },
                    )
                    .err()
            })
        };
        if let Some(error) = error {
            if rmake_linux::signals::pending().is_some() {
                if let Some(owner) = self.owner.borrow().as_ref().filter(|owner| owner.reaped) {
                    self.controller
                        .0
                        .interrupted_owner
                        .replace(Some(owner.clone()));
                }
                // A fatal handler abandons the interrupted dump. Replaying the
                // spool would expose output GNU never emitted on this path.
                self.spool.borrow_mut().take();
                self.enabled.set(false);
                return;
            }
            if rmake_linux::signals::pending().is_none() {
                plain(
                    true,
                    format!("fcntl(): {}\n", crate::engine::os_error(&error)).as_bytes(),
                );
                self.controller
                    .warning(b"warning: Cannot acquire output lock, disabling output sync.");
            }
            self.controller.clear();
            if let Some(spool) = self.spool.borrow().as_ref() {
                let mut frame = false;
                let _ = spool.dump(
                    None,
                    &mut |entering| self.controller.dump_frame(entering, &mut frame),
                    &mut |operation, error| {
                        plain(
                            true,
                            format!("{operation}: {}\n", crate::engine::os_error(error)).as_bytes(),
                        );
                    },
                );
            }
        }
    }
}

fn plain(error: bool, bytes: &[u8]) {
    plain_parts(error, &[bytes]);
}

fn plain_parts(error: bool, parts: &[&[u8]]) {
    if error {
        let mut stream = io::stderr().lock();
        let _ = parts.iter().try_for_each(|bytes| stream.write_all(bytes));
        let _ = stream.flush();
    } else {
        let mut stream = io::stdout().lock();
        let _ = parts.iter().try_for_each(|bytes| stream.write_all(bytes));
        let _ = stream.flush();
    }
}

/// Preserve output preceding a constructor failure without imposing scope-stack
/// semantics on the scheduler's global output context.
pub struct Construction {
    controller: Controller,
    buffer: Option<Rc<Buffer>>,
    armed: bool,
}
impl Construction {
    pub fn new(controller: Controller, buffer: Option<Rc<Buffer>>) -> Self {
        Self {
            controller,
            buffer,
            armed: true,
        }
    }
    pub fn disarm(&mut self) {
        self.armed = false;
    }
    pub fn replace_buffer(&mut self, buffer: Option<Rc<Buffer>>) {
        self.buffer = buffer;
    }
}
impl Drop for Construction {
    fn drop(&mut self) {
        if self.armed && rmake_linux::signals::pending().is_none() {
            self.controller.select(None);
            if let Some(buffer) = &self.buffer {
                buffer.dump();
            }
        }
    }
}

#[cfg(test)]
mod directory_tests {
    use super::*;

    #[test]
    fn automatic_policy_uses_published_startup_context_and_live_silence() {
        let output = Controller::new(Mode::None, b"make");
        assert!(!output.should_report_directory());
        output.set_changed_directory(true);
        assert!(output.should_report_directory());
        output.set_directory_policy(DirectoryChoice::Automatic, true);
        assert!(!output.should_report_directory());
        output.set_directory_policy(DirectoryChoice::On, true);
        assert!(output.should_report_directory());
        output.set_changed_directory(false);
        output.set_recursive(true);
        output.set_directory_policy(DirectoryChoice::Automatic, false);
        assert!(output.should_report_directory());
        output.set_directory_policy(DirectoryChoice::Off, false);
        assert!(!output.should_report_directory());
    }

    #[test]
    fn directory_close_is_idempotent_and_finalizer_output_cannot_reopen_it() {
        let output = Controller::new(Mode::None, b"make");
        output.set_directory_policy(DirectoryChoice::On, false);
        output.start();
        assert_eq!(output.0.directory_trace.get(), ProcessTrace::Entered);
        // Leave is owed even when later policy suppresses new entries.
        output.set_directory_policy(DirectoryChoice::Off, false);
        output.process_lifetime().finish();
        assert_eq!(output.0.directory_trace.get(), ProcessTrace::Closed);
        output.set_directory_policy(DirectoryChoice::On, false);
        output.start();
        output.process_lifetime().finish();
        assert_eq!(output.0.directory_trace.get(), ProcessTrace::Closed);
    }

    #[test]
    fn synchronized_frames_do_not_open_a_process_trace() {
        let output = Controller::new(Mode::Target, b"make");
        output.set_directory_policy(DirectoryChoice::On, false);
        output.start();
        assert_eq!(output.0.directory_trace.get(), ProcessTrace::NotEntered);
        let mut framed = false;
        output.dump_frame(true, &mut framed);
        assert!(framed);
        output.set_directory_policy(DirectoryChoice::Off, false);
        output.dump_frame(false, &mut framed);
        assert_eq!(output.0.directory_trace.get(), ProcessTrace::NotEntered);
        output.process_lifetime().finish();
        output.set_directory_policy(DirectoryChoice::On, false);
        output.dump_frame(true, &mut framed);
        assert!(!framed);
    }
}
