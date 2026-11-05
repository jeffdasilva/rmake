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
    Source(Box<SourceExit>),
}
pub struct SourceExit {
    source: rmake_linux::stdin_source::Source,
    output: crate::output::Controller,
    program: Vec<u8>,
}
impl NativeExit {
    pub(super) fn source(
        source: rmake_linux::stdin_source::Source,
        output: crate::output::Controller,
        program: Vec<u8>,
    ) -> Box<Self> {
        Box::new(Self::Source(Box::new(SourceExit {
            source,
            output,
            program,
        })))
    }
    pub fn prepare(&mut self) {
        match self {
            Self::Startup(host) => host.prepare(),
            Self::Source(exit) => {
                crate::stdin_source::unlink(Some(&exit.source), &exit.output, &exit.program)
            }
            Self::Engine(engine) => {
                engine.prepare_native_exit();
                engine.print_database();
            }
        }
    }

    #[cfg(feature = "guile")]
    pub fn finish_signal(&mut self, number: i32) -> u8 {
        match self {
            Self::Startup(host) => host.finish_signal(number),
            Self::Engine(engine) => engine.native_signal_exit(number),
            // A source-only exit never owns a live native phase.
            Self::Source(_) => rmake_linux::guile::abort_host(),
        }
    }

    pub fn finish(&mut self, status: u8) -> u8 {
        match self {
            Self::Startup(host) => host.finish(status),
            Self::Source(_) => status,
            Self::Engine(engine) => engine.native_exit(status),
        }
    }
}

/// These fields are moved from the phase that actually ran. No defaults or
/// option receivers are installed merely to make a finalization host.
pub struct Host {
    pub(super) stdin_source: Option<rmake_linux::stdin_source::Source>,
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
    pub(super) database_dumped: Cell<bool>,
}
impl Host {
    fn evaluate<T>(
        &mut self,
        action: impl FnOnce(&mut Database, ExpansionContext<'_>) -> Result<T, ExpandError>,
    ) -> Result<T, ExpandError> {
        let mut output = RuntimeOutput::new(&self.output, &self.program, &self.warnings);
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
        crate::stdin_source::unlink(self.stdin_source.as_ref(), &self.output, &self.program);
        // A successful prepared help/version pass keeps this guard until after
        // its banner. Error paths already let construction unwinding flush it.
        self.construction.take();
        self.output.select(None);
        if rmake_linux::signals::pending().is_none()
            && let Some(buffer) = self.reader_output.take()
        {
            buffer.dump();
        }
        database_output::print_startup(self);
    }

    #[cfg(feature = "guile")]
    fn finish_signal(&mut self, number: i32) -> u8 {
        self.output.process_lifetime().finish();
        if let Some(directory) = self.restore_directory.take() {
            let _ = std::env::set_current_dir(directory);
        }
        let result = self.evaluate(|database, mut context| {
            database.terminate_native_signal_with(number, &mut context)
        });
        if let Err(error) = result {
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
        2
    }

    fn finish(&mut self, mut status: u8) -> u8 {
        self.output.process_lifetime().finish();
        if let Some(directory) = self.restore_directory.take() {
            let _ = std::env::set_current_dir(directory);
        }
        if !crate::native::has_modules() {
            return status;
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
