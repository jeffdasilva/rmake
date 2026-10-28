// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Initial inherited bindings are expanded before their tokens are decoded.
//! The prepared database/effect runtime is transferred, never replayed in-place.
use super::*;

#[derive(Clone)]
pub(super) struct Plan {
    args: Vec<OsString>,
    environment: Vec<(OsString, OsString)>,
    pub(super) program: Vec<u8>,
    pub(super) invocation: OsString,
}

pub(super) struct Prepared {
    pub(super) database: Database,
    pub(super) filesystem: RefCell<FilesystemRuntime>,
    pub(super) output: crate::output::Controller,
    pub(super) reader_output: Option<std::rc::Rc<crate::output::Buffer>>,
    pub(super) construction: crate::output::Construction,
    pub(super) exports: HashSet<Vec<u8>>,
    pub(super) seed: startup::Seed,
    pub(super) plan: Box<Plan>,
}

impl From<String> for InvocationError {
    fn from(message: String) -> Self {
        Self::Options(message)
    }
}
impl From<&'static str> for InvocationError {
    fn from(message: &'static str) -> Self {
        Self::Options(message.into())
    }
}

impl From<StartupError> for InvocationError {
    fn from(error: StartupError) -> Self {
        Self::Expansion(error)
    }
}

impl Plan {
    pub(super) fn new(
        args: Vec<OsString>,
        program: &[u8],
        invocation: &OsStr,
        makeflags: Option<OsString>,
        gnumakeflags: Option<OsString>,
    ) -> Self {
        let mut environment: Vec<_> = std::env::vars_os().collect();
        for (name, value) in [("MAKEFLAGS", makeflags), ("GNUMAKEFLAGS", gnumakeflags)] {
            if let Some(index) = environment.iter().position(|(key, _)| key == name) {
                if let Some(value) = value {
                    environment[index].1 = value;
                } else {
                    environment.remove(index);
                }
            } else if let Some(value) = value {
                environment.push((name.into(), value));
            }
        }
        Self {
            args,
            environment,
            program: program.to_vec(),
            invocation: invocation.to_owned(),
        }
    }

    pub(super) fn decode(
        &self,
        restart: Option<(u32, u32)>,
    ) -> Result<Invocation, InvocationError> {
        let mut database = Database::default();
        database.variables.reserve_environment_layout(
            &[
                b".VARIABLES",
                b".RECIPEPREFIX",
                b".SHELLFLAGS",
                b".LOADED",
                b".FEATURES",
            ],
            false,
        );
        database
            .variables
            .enable_variable_names()
            .map_err(StartupError::expansion)?;
        for (name, value) in [
            (b".RECIPEPREFIX".as_slice(), b"".as_slice()),
            (b".SHELLFLAGS", b"-c"),
            (b".LOADED", b""),
            (b".FEATURES", FEATURES),
        ] {
            database
                .variables
                .define_literal(name, value, Origin::Default)
                .map_err(StartupError::expansion)?;
        }
        let mut exports = HashSet::new();
        for (name, value) in &self.environment {
            let name = name.as_bytes();
            let updated_level = restart
                .filter(|_| name == b"MAKELEVEL")
                .map(|(_, level)| level.to_string());
            let updated_restart = restart
                .filter(|_| name == b"MAKE_RESTARTS")
                .map(|(count, _)| count.to_string());
            let value = updated_level
                .as_ref()
                .or(updated_restart.as_ref())
                .map_or(value.as_bytes(), |value| value.as_bytes());
            database
                .variables
                .define(name, value, Assignment::Recursive, Origin::Environment)
                .map_err(StartupError::expansion)?;
            if name == b"SHELL" {
                database.variables.set_original_shell(value);
                database.variables.set_export_policy(name, Some(false));
            } else {
                database.variables.set_export_policy(name, Some(true));
                exports.insert(name.to_vec());
            }
        }
        if let Some((count, _)) = restart
            && database.variables.get(b"MAKE_RESTARTS").is_none()
        {
            database
                .variables
                .define(
                    b"MAKE_RESTARTS",
                    count.to_string().as_bytes(),
                    Assignment::Recursive,
                    Origin::Environment,
                )
                .map_err(StartupError::expansion)?;
            database
                .variables
                .set_export_policy(b"MAKE_RESTARTS", Some(true));
            exports.insert(b"MAKE_RESTARTS".to_vec());
        }
        if let Ok(directory) = std::env::current_dir() {
            database
                .variables
                .set_working_directory(directory.as_os_str().as_bytes());
        }
        let filesystem = RefCell::new(FilesystemRuntime::default());
        let output = crate::output::Controller::new(crate::output::Mode::None, &self.program);
        let mut state = State {
            database,
            filesystem,
            output,
            exports,
            seed: startup::Seed::default(),
            warnings: Cell::new(false),
            seen_commands: HashSet::new(),
        };
        let mut prefix_count = 0;
        let mut parsed_prefix = Options::default();
        for name in [b"GNUMAKEFLAGS".as_slice(), b"MAKEFLAGS".as_slice()] {
            if name == b"GNUMAKEFLAGS" && state.database.variables.get(name).is_none() {
                continue;
            }
            let expression = [b"$(".as_slice(), name, b")"].concat();
            let value = state
                .evaluate(&self.program, |database, context| {
                    database.expand_with(&expression, context)
                })
                .map_err(StartupError::expansion)?;
            let mut incoming = Vec::new();
            Options::append_expanded_flags(&mut incoming, &value)?;
            // Empty sources retain expansion effects without another parse.
            if !incoming.is_empty() {
                let count = incoming.len();
                parsed_prefix = Options::parse_continued_with(
                    incoming,
                    parsed_prefix,
                    prefix_count,
                    &mut |options, argument| state.argument(&self.program, options, argument),
                )?;
                prefix_count += count;
                state
                    .database
                    .variables
                    .set_environment_override(parsed_prefix.environment_override);
                state.warnings.set(parsed_prefix.warn_undefined);
            }
            if name == b"GNUMAKEFLAGS" {
                state
                    .database
                    .variables
                    .define_literal(
                        name,
                        b"",
                        if parsed_prefix.environment_override {
                            Origin::EnvironmentOverride
                        } else {
                            Origin::Environment
                        },
                    )
                    .map_err(StartupError::expansion)?;
            }
        }
        // Initial inherited expansion precedes synchronization; explicit CLI
        // effects use the inherited mode until the whole explicit stream ends.
        let inherited_mode = output_mode(parsed_prefix.output_sync.as_deref())?;
        state.output.set_mode(inherited_mode);
        let reader_output = inherited_mode
            .root_buffered()
            .then(|| state.output.make_buffer())
            .flatten();
        state.output.select(reader_output.as_ref());
        let mut construction =
            crate::output::Construction::new(state.output.clone(), reader_output.clone());
        let decoded = Options::with_parsed_inherited_with(
            self.args.clone(),
            parsed_prefix,
            prefix_count,
            false,
            &mut |options, argument| state.argument(&self.program, options, argument),
        );
        let options = match decoded {
            Ok(options) => options,
            Err(InvocationError::Expansion(mut error)) => {
                if rmake_linux::signals::pending().is_none()
                    && let Some(buffer) = &reader_output
                {
                    state.output.write_parts(
                        true,
                        &[
                            &error.prefix(&self.program),
                            b"*** ",
                            error.message.as_bytes(),
                            b".  Stop.\n",
                        ],
                    );
                    error.buffered = Some(buffer.clone());
                    construction.disarm();
                }
                return Err(InvocationError::Expansion(error));
            }
            Err(error) => return Err(error),
        };
        output_mode(options.output_sync.as_deref())?;
        Ok(Invocation {
            options,
            initial: Some(Box::new(Prepared {
                database: state.database,
                filesystem: state.filesystem,
                output: state.output,
                exports: state.exports,
                seed: state.seed,
                reader_output,
                construction,
                plan: Box::new(self.clone()),
            })),
        })
    }
}

fn output_mode(raw: Option<&[u8]>) -> Result<crate::output::Mode, StartupError> {
    raw.map_or(Ok(crate::output::Mode::None), |raw| {
        crate::output::Mode::parse(raw).ok_or_else(|| {
            Diagnostic::from([b"unknown output-sync type '".as_slice(), raw, b"'"].concat()).into()
        })
    })
}

/// One early database and effect runtime, shared by all ordered argument sources.
struct State {
    database: Database,
    filesystem: RefCell<FilesystemRuntime>,
    output: crate::output::Controller,
    exports: HashSet<Vec<u8>>,
    seed: startup::Seed,
    warnings: Cell<bool>,
    seen_commands: HashSet<Vec<u8>>,
}

impl State {
    fn evaluate<T>(
        &mut self,
        program: &[u8],
        action: impl FnOnce(&mut Database, ExpansionContext<'_>) -> Result<T, ExpandError>,
    ) -> Result<T, ExpandError> {
        let mut emit = |event: Output<'_>| emit_output(&self.output, program, event);
        let mut receiver = WarningOutput::new(&mut emit, &self.warnings);
        let mut filesystem = |effect: rmake_core::diagnostic::FilesystemEffect<'_>| {
            self.filesystem.borrow_mut().execute(effect)
        };
        let mut shell = |request: rmake_core::diagnostic::ShellRequest<'_>| {
            let result = crate::shell::run(request, program, 0, &self.output, None);
            self.filesystem.borrow_mut().child_completed();
            result
        };
        let mut include = |name: &[u8]| IncludeSearch::default().read(name);
        action(
            &mut self.database,
            runtime_context(None, &mut receiver)
                .with_filesystem(&mut filesystem)
                .with_shell(&mut shell)
                .with_includes(&mut include),
        )
    }

    fn path(&mut self, program: &[u8], raw: &[u8]) -> Result<std::path::PathBuf, ExpandError> {
        let warnings = self.warnings.replace(false);
        let result = crate::makefiles::prepare_argument_with(raw, || {
            self.evaluate(program, |database, context| {
                database.expand_with(b"$(HOME)", context)
            })
        });
        self.warnings.set(warnings);
        result
    }

    fn argument(
        &mut self,
        program: &[u8],
        options: &mut Options,
        argument: crate::cli::Argument,
    ) -> Result<(), InvocationError> {
        self.warnings.set(options.warn_undefined);
        self.database
            .variables
            .set_environment_override(options.environment_override);
        let result: Result<(), ExpandError> = (|| {
            match argument {
                crate::cli::Argument::Definition(index) => {
                    let name = self
                        .evaluate(program, |database, context| {
                            database.define_argument_with(
                                &options.assignments[index],
                                Origin::CommandLine,
                                context,
                            )
                        })?
                        .expect("decoded assignment");
                    if self.seen_commands.insert(name.clone()) {
                        self.seed.commands.push(name.clone());
                    }
                    if automatic_export_name(&name) {
                        self.exports.insert(name);
                    }
                    self.seed.assignments = options.assignments.len();
                }
                crate::cli::Argument::Include(index) => {
                    let raw = options.include_directories[index].as_os_str().as_bytes();
                    if !options.include_directories[..index]
                        .iter()
                        .any(|path| path.as_os_str().as_bytes() == raw)
                    {
                        options.include_directories[index] = self.path(program, raw)?;
                    } else {
                        options.include_directories.pop();
                        options.include_assignment_positions.pop();
                    }
                    self.seed.includes = options.include_directories.len();
                }
                crate::cli::Argument::Directory(index) => {
                    let raw = options.directories[index].as_os_str().as_bytes();
                    if !options.directories[..index]
                        .iter()
                        .any(|path| path.as_os_str().as_bytes() == raw)
                    {
                        options.directories[index] = self.path(program, raw)?;
                    } else {
                        options.directories.pop();
                    }
                }
                crate::cli::Argument::File(index) => {
                    let raw = options.files[index].as_os_str().as_bytes();
                    options.files[index] = self.path(program, raw)?;
                }
                crate::cli::Argument::Goal(index) => {
                    let path = self.path(program, &options.goals[index])?;
                    let name = path.as_os_str().as_bytes();
                    self.database.file_records.insert(name);
                    let mut value = self
                        .database
                        .variables
                        .get(b"MAKECMDGOALS")
                        .map(|v| v.raw_value().to_vec())
                        .unwrap_or_default();
                    if self.database.variables.get(b"MAKECMDGOALS").is_some() {
                        value.push(b' ');
                    }
                    value.extend_from_slice(name);
                    self.database.variables.define_literal(
                        b"MAKECMDGOALS",
                        &value,
                        Origin::Default,
                    )?;
                    options.goals[index] = name.to_vec();
                }
            }
            Ok(())
        })();
        result.map_err(|error| {
            let mut error = StartupError::expansion(error);
            error.version = options.version;
            error.into()
        })
    }
}

pub(super) fn emit_error(program: &[u8], error: InvocationError) {
    if rmake_linux::signals::pending().is_some() {
        return;
    }
    if let InvocationError::Expansion(error) = &error
        && let Some(buffer) = &error.buffered
    {
        if error.version {
            crate::print_version();
        }
        buffer.dump();
        return;
    }
    let version = matches!(&error, InvocationError::Expansion(error) if error.version);
    let mut stderr = io::stderr().lock();
    match error {
        InvocationError::Options(message) => {
            let _ = stderr.write_all(program);
            let _ = stderr.write_all(b": ");
            let _ = stderr.write_all(message.as_bytes());
            let _ = stderr.write_all(b"\n");
        }
        InvocationError::Expansion(error) => {
            let _ = stderr.write_all(&error.prefix(program));
            let _ = stderr.write_all(b"*** ");
            let _ = stderr.write_all(error.message.as_bytes());
            let _ = stderr.write_all(b".  Stop.\n");
        }
    }
    let _ = stderr.flush();
    drop(stderr);
    if version {
        crate::print_version();
    }
}
