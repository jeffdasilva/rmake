// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Owned interpreter state for native finalizers reached before normal execution.
use super::*;

pub struct Failure<E> {
    pub error: E,
    pub(super) exit: Option<Box<NativeExit>>,
}
impl<E> Failure<E> {
    pub fn into_parts(self) -> (E, Option<Box<NativeExit>>) {
        (self.error, self.exit)
    }
    pub(super) fn plain(error: E) -> Self {
        Self { error, exit: None }
    }
}
impl<E: std::fmt::Debug> std::fmt::Debug for Failure<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(f)
    }
}
impl<E> std::ops::Deref for Failure<E> {
    type Target = E;
    fn deref(&self) -> &E {
        &self.error
    }
}
impl From<StartupError> for Failure<InvocationError> {
    fn from(error: StartupError) -> Self {
        Self::plain(InvocationError::Expansion(error))
    }
}
impl From<InvocationError> for Failure<InvocationError> {
    fn from(error: InvocationError) -> Self {
        Self::plain(error)
    }
}

pub enum NativeExit {
    Startup(Box<Host>),
    Engine(Box<Engine>),
}
impl NativeExit {
    pub fn prepare(&mut self) {
        match self {
            Self::Startup(host) => host.prepare(),
            Self::Engine(engine) => {
                engine.prepare_native_exit();
                if let Some(exit) = &mut engine.native_exit_override {
                    exit.prepare();
                }
            }
        }
    }

    pub fn finish(&mut self, status: u8) -> u8 {
        match self {
            Self::Startup(host) => host.finish(status),
            Self::Engine(engine) => engine.native_exit(status),
        }
    }
}

/// These fields are moved from the phase that actually ran. No defaults or
/// option receivers are installed merely to make a finalization host.
pub struct Host {
    pub(super) database: Database,
    pub(super) filesystem: RefCell<FilesystemRuntime>,
    pub(super) output: crate::output::Controller,
    pub(super) warnings: Cell<bool>,
    pub(super) program: Vec<u8>,
    pub(super) level: u32,
    pub(super) include_search: Option<std::rc::Rc<RefCell<IncludeSearch>>>,
    pub(super) reader_output: Option<std::rc::Rc<crate::output::Buffer>>,
    pub(super) construction: Option<crate::output::Construction>,
    pub(super) restore_directory: Option<std::path::PathBuf>,
}
impl Host {
    fn evaluate<T>(
        &mut self,
        action: impl FnOnce(&mut Database, ExpansionContext<'_>) -> Result<T, ExpandError>,
    ) -> Result<T, ExpandError> {
        let mut emit = |event: Output<'_>| emit_output(&self.output, &self.program, event);
        let mut output = WarningOutput::new(&mut emit, &self.warnings);
        let mut filesystem = |effect: rmake_core::diagnostic::FilesystemEffect<'_>| {
            self.filesystem.borrow_mut().execute(effect)
        };
        let mut shell = |request: rmake_core::diagnostic::ShellRequest<'_>| {
            let result = crate::shell::run(request, &self.program, self.level, &self.output, None);
            self.filesystem.borrow_mut().child_completed();
            result
        };
        let mut include = |name: &[u8]| {
            self.include_search.as_ref().map_or_else(
                || IncludeSearch::default().read(name),
                |search| search.borrow().read(name),
            )
        };
        action(
            &mut self.database,
            runtime_context(None, &mut output)
                .with_filesystem(&mut filesystem)
                .with_shell(&mut shell)
                .with_includes(&mut include),
        )
    }

    fn prepare(&mut self) {
        // A successful prepared help/version pass keeps this guard until after
        // its banner. Error paths already let construction unwinding flush it.
        self.construction.take();
        self.output.select(None);
        if rmake_linux::signals::pending().is_none()
            && let Some(buffer) = self.reader_output.take()
        {
            buffer.dump();
        }
    }

    fn finish(&mut self, mut status: u8) -> u8 {
        self.output.process_lifetime().finish();
        if let Some(directory) = self.restore_directory.take() {
            let _ = std::env::set_current_dir(directory);
        }
        loop {
            let result = self.evaluate(|database, mut context| {
                database.terminate_native_with(status, &mut context)
            });
            match result {
                Ok(()) => return status,
                Err(error) => {
                    if rmake_linux::signals::pending().is_none() {
                        let error = StartupError::expansion(error);
                        self.output.write_parts(
                            true,
                            &[
                                &error.prefix(&self.program),
                                b"*** ",
                                error.message.as_bytes(),
                                b".  Stop.\n",
                            ],
                        );
                    }
                    status = 2;
                    if !crate::native::can_retry_exit() {
                        return status;
                    }
                }
            }
        }
    }
}
