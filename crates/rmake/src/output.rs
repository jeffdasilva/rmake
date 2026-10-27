// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Interpreter output focus and recipe-owned spools. Focus holds weak references;
//! buffers keep the controller alive without creating a controller/buffer cycle.
use rmake_linux::output::{Coordination, Spool};
use std::cell::{Cell, RefCell};
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

struct State {
    mode: Cell<Mode>,
    current: RefCell<Option<Weak<Buffer>>>,
    coordination: RefCell<Option<Coordination>>,
    program: Vec<u8>,
    interrupted_owner: RefCell<Option<Owner>>,
}

#[derive(Clone)]
pub struct Controller(Rc<State>);

impl Controller {
    pub fn new(mode: Mode, program: &[u8]) -> Self {
        Self(Rc::new(State {
            mode: Cell::new(mode),
            current: RefCell::new(None),
            coordination: RefCell::new(None),
            program: program.to_vec(),
            interrupted_owner: RefCell::new(None),
        }))
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
        if let Some(buffer) = self.current() {
            for bytes in parts {
                buffer.write(error, bytes);
            }
        } else {
            plain_parts(error, parts);
        }
    }
    pub fn decode_mutex(&self, authentication: &[u8]) -> io::Result<()> {
        match Coordination::join(authentication)? {
            Some(provider) => {
                self.0.coordination.replace(Some(provider));
            }
            None => self.write_parts(
                true,
                &[
                    &self.0.program,
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
        plain(
            true,
            &[self.0.program.as_slice(), b": ", bytes, b"\n"].concat(),
        );
    }
    fn disable(&self) {
        self.0.mode.set(Mode::None);
        self.clear();
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
    pub fn configure_child(&self, child: &mut std::process::Command) -> io::Result<()> {
        if !self.enabled() {
            return Ok(());
        }
        self.prepare();
        if let Some(spool) = self.spool.borrow().as_ref() {
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
                spool
                    .dump(provider.as_ref(), &mut |operation, error| {
                        plain(
                            true,
                            format!("{operation}: {}\n", crate::engine::os_error(error)).as_bytes(),
                        );
                    })
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
                let _ = spool.dump(None, &mut |operation, error| {
                    plain(
                        true,
                        format!("{operation}: {}\n", crate::engine::os_error(error)).as_bytes(),
                    );
                });
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
