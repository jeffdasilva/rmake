// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Initial inherited bindings are expanded before their tokens are decoded.
//! The prepared database/effect runtime is transferred, never replayed in-place.
use super::*;

pub(super) struct Plan {
    args: Vec<OsString>,
    pub(super) stdin_source: Option<rmake_linux::stdin_source::Source>,
    stdin_argument: Option<crate::cli::StdinArgument>,
    stdin_index: Option<usize>,
    environment: Vec<(OsString, OsString)>,
    pub(super) startup_restarts: u32,
    pub(super) program: Vec<u8>,
    pub(super) invocation: OsString,
    restart_invocation: Option<OsString>,
    output: Option<crate::output::Controller>,
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
    pub(super) initial_directory: Option<std::path::PathBuf>,
    pub(super) shuffle: crate::shuffle::Mode,
    pub(super) warnings: Cell<bool>,
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
        output: Option<crate::output::Controller>,
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
        let restart_invocation =
            if !invocation.as_bytes().starts_with(b"/") && invocation.as_bytes().contains(&b'/') {
                std::env::current_dir()
                    .ok()
                    .map(|directory| directory.join(invocation).into_os_string())
            } else {
                None
            };
        Self {
            args,
            stdin_source: None,
            stdin_argument: None,
            stdin_index: None,
            environment,
            startup_restarts: 0,
            program: program.to_vec(),
            invocation: invocation.to_owned(),
            restart_invocation,
            output,
        }
    }

    /// Retain the process-entry path before any -C changes. Bare names keep
    /// their PATH search; relative paths containing '/' keep their spelling.
    pub(super) fn restart_program(&self) -> &OsStr {
        self.restart_invocation
            .as_deref()
            .unwrap_or(&self.invocation)
    }

    pub(super) fn debug_restart(&self, count: u32) {
        let mut line = crate::debug::restart_prefix(count);
        line.push(b' ');
        line.extend_from_slice(self.restart_program().as_bytes());
        let rewritten = self.restart_arguments();
        for argument in rewritten.as_deref().unwrap_or(&self.args) {
            line.push(b' ');
            line.extend_from_slice(argument.as_bytes());
        }
        line.push(b'\n');
        crate::debug::raw(&[&line]);
    }

    /// Restart in a fresh program image using retained argv and the live
    /// process environment. Dynamic-loader and startup hooks run again.
    pub(super) fn exec_restart(
        &self,
        count: u32,
        level: u32,
        jobserver: Option<&Jobserver>,
    ) -> io::Error {
        let rewritten = self.restart_arguments();
        let traced = self
            .output
            .as_ref()
            .is_some_and(|output| output.has_directory_trace());
        // Only flush the public streams here. Exec discards plugin-owned
        // buffers, and a standard-stream write failure does not cancel it.
        let _ = rmake_linux::native::flush_stream(false);
        let _ = rmake_linux::native::flush_stream(true);
        rmake_linux::environment::exec_live_restart(
            self.restart_program(),
            rewritten.as_deref().unwrap_or(&self.args),
            count,
            level,
            traced,
            jobserver,
        )
    }

    fn restart_arguments(&self) -> Option<Vec<OsString>> {
        Some(crate::stdin_source::restart_arguments(
            &self.args,
            self.stdin_argument?,
            self.stdin_source.as_ref()?.path(),
        ))
    }

    fn argument(
        &mut self,
        state: &mut State,
        options: &mut Options,
        argument: crate::cli::Argument,
        explicit: bool,
    ) -> Result<(), InvocationError> {
        if let crate::cli::Argument::Usage(_, issue) = argument {
            let name = if explicit {
                self.invocation.as_bytes()
            } else {
                b""
            };
            if let Some(message) = issue.getopt_line(name, true) {
                let _ = std::io::Write::write_all(&mut std::io::stderr().lock(), &message);
            } else {
                state
                    .output
                    .write_parts(true, &[&self.program, b": ", &issue.message(), b"\n"]);
            }
            return Ok(());
        }
        if let crate::cli::Argument::File(index) = argument
            && options.temp_stdin_file == Some(index)
        {
            if self.stdin_index.is_some_and(|index| index > 0) {
                return Err(StartupError::from(Diagnostic::from(
                    "INTERNAL: multiple --temp-stdin options provided!",
                ))
                .into());
            }
            self.stdin_index = Some(index);
            if let Some(previous) =
                self.stdin_source
                    .replace(rmake_linux::stdin_source::Source::adopt(
                        options.files[index].clone(),
                    ))
            {
                previous.disarm();
            }
        }
        state.argument(&self.program, options, argument)
    }

    pub(super) fn decode(mut self: Box<Self>) -> Result<Invocation, Failure<InvocationError>> {
        crate::native::reset();
        #[cfg(feature = "guile")]
        crate::native::register_guile().map_err(StartupError::expansion)?;
        let output = if let Some(output) = &self.output {
            output.set_mode(crate::output::Mode::None);
            output.set_program(&self.program, self.program.len(), 0);
            output.set_imported_directory(None);
            output.clone()
        } else {
            crate::output::Controller::new(crate::output::Mode::None, &self.program)
        };
        output.reset_compatibility_header();
        output.set_database_requested(false);
        output.set_debug_mask(crate::debug::Mask::NONE);
        output.set_directory_policy(crate::cli::DirectoryChoice::Automatic, false);
        output.set_changed_directory(false);
        output.set_recursive(false);
        self.output = Some(output.clone());
        let mut database = Database::default();
        database
            .enable_database_history()
            .expect("fresh invocation database");
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
        for (name, value) in std::mem::take(&mut self.environment) {
            let name = name.as_bytes();
            if name == b"MAKE_RESTARTS" && value.as_bytes().starts_with(b"-") {
                output.import_directory_trace();
            }
            let value = value.as_bytes();
            let value = if name == b"MAKE_RESTARTS" {
                let value = startup_restart_value(value);
                // Import in vector order: the last binding controls startup.
                // Later native edits affect the live exec image, not this seed.
                self.startup_restarts = startup_unsigned(value);
                value
            } else {
                value
            };
            database
                .variables
                .define(name, value, Assignment::Recursive, Origin::Environment)
                .map_err(StartupError::expansion)?;
            if name == b"SHELL" {
                database.variables.set_original_shell(value);
                database.variables.set_export_policy(name, Some(false));
            } else if name == b"MAKE_RESTARTS" {
                database.variables.set_export_policy(name, Some(false));
            } else {
                database.variables.set_export_policy(name, Some(true));
                exports.insert(name.to_vec());
            }
        }
        let initial_directory = match std::env::current_dir() {
            Ok(directory) => Some(directory),
            Err(error) => {
                emit_getcwd_error(&output, &self.program, &error);
                None
            }
        };
        database.variables.set_startup_directory_pending();
        let filesystem = RefCell::new(FilesystemRuntime::default());
        filesystem.borrow_mut().set_database_history(
            database
                .database_history()
                .expect("enabled before defaults")
                .clone(),
        );
        let mut state = State {
            database,
            filesystem,
            output,
            exports,
            seed: startup::Seed::default(),
            warnings: Cell::new(false),
            seen_commands: HashSet::new(),
            seen_includes: HashSet::new(),
            seen_directories: HashSet::new(),
        };
        let policy_output = state.output.clone();
        let decoded = (|| -> Result<_, InvocationError> {
            let mut prefix_count = 0;
            let mut parsed_prefix = Options::default();
            for name in [b"GNUMAKEFLAGS".as_slice(), b"MAKEFLAGS".as_slice()] {
                if name == b"GNUMAKEFLAGS"
                    && state.database.variables.database_lookup(name).is_none()
                {
                    continue;
                }
                let expression = [b"$(".as_slice(), name, b")"].concat();
                let value = state
                    .evaluate(&self.program, |database, context| {
                        database.expand_with(&expression, context)
                    })
                    .map_err(StartupError::expansion)?;
                let mut incoming = Vec::new();
                Options::append_expanded_flags(&mut incoming, &value);
                // Empty sources retain expansion effects without another parse.
                if !incoming.is_empty() {
                    let count = incoming.len();
                    let mut selected_policy = (false, false);
                    let parsed = Options::parse_continued_with(
                        incoming,
                        parsed_prefix,
                        prefix_count,
                        &mut |options, argument| {
                            self.argument(&mut state, options, argument, false)
                        },
                        &mut |options| {
                            selected_policy =
                                (options.warn_undefined, options.environment_override);
                            policy_output
                                .set_directory_policy(options.directory_choice, options.silent);
                            policy_output.set_database_requested(options.print_database);
                            policy_output.set_changed_directory(!options.directories.is_empty());
                        },
                    );
                    state.warnings.set(selected_policy.0);
                    state
                        .database
                        .variables
                        .set_environment_override(selected_policy.1);
                    parsed_prefix = parsed?;
                    if parsed_prefix.invalid_usage {
                        return Err(InvocationError::Usage {
                            version: parsed_prefix.version,
                        });
                    }
                    parsed_prefix
                        .decode_debug()
                        .map_err(|error| StartupError::from(Diagnostic::from(error)))?;
                    if let Some(authentication) = &parsed_prefix.sync_mutex {
                        output_mode(parsed_prefix.output_sync.as_deref())?;
                        state.complete_mutex(authentication)?;
                    }
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
            let mut selected_policy = (false, false);
            let decoded = Options::with_parsed_inherited_with(
                self.args.clone(),
                parsed_prefix,
                prefix_count,
                false,
                &mut |options, argument| self.argument(&mut state, options, argument, true),
                &mut |options| {
                    selected_policy = (options.warn_undefined, options.environment_override);
                    policy_output.set_directory_policy(options.directory_choice, options.silent);
                    policy_output.set_database_requested(options.print_database);
                    policy_output.set_changed_directory(!options.directories.is_empty());
                },
            );
            state.warnings.set(selected_policy.0);
            state
                .database
                .variables
                .set_environment_override(selected_policy.1);
            let mut buffered_failure = |mut error: StartupError| {
                if rmake_linux::signals::pending().is_none()
                    && let Some(buffer) = &reader_output
                {
                    state.output.write_parts(
                        true,
                        &[
                            &error.prefix(&self.program),
                            b"*** ",
                            &crate::messages::diagnostic(&error.message),
                            &crate::messages::stop_for(&error.message),
                        ],
                    );
                    error.buffered = Some(buffer.clone());
                    construction.disarm();
                }
                InvocationError::Expansion(error)
            };
            let mut options = match decoded {
                Ok(options) => options,
                Err(InvocationError::Expansion(error)) => return Err(buffered_failure(error)),
                Err(error) => return Err(error),
            };
            if options.invalid_usage {
                return Err(InvocationError::Usage {
                    version: options.version,
                });
            }
            options.decode_debug().map_err(|error| {
                let mut error = StartupError::from(Diagnostic::from(error));
                error.version = options.version;
                error
            })?;
            output_mode(options.output_sync.as_deref())?;
            if let Some(authentication) = &options.sync_mutex {
                state
                    .complete_mutex(authentication)
                    .map_err(buffered_failure)?;
            }
            let shuffle = if options.help || options.version {
                crate::shuffle::Mode::None
            } else {
                options
                    .shuffle
                    .as_deref()
                    .map(crate::shuffle::Mode::parse)
                    .transpose()
                    .map_err(|message| StartupError::from(Diagnostic::from(message)))?
                    .unwrap_or_default()
            };
            if !options.help && !options.version {
                options.shuffle = shuffle.spelling();
            }
            Ok((options, reader_output, construction, shuffle))
        })();
        let (options, reader_output, construction, shuffle) = match decoded {
            Ok(decoded) => decoded,
            Err(error) => {
                let stdin_source = self.stdin_source.take();
                let exit = if crate::native::has_modules() || state.output.database_requested() {
                    Some(Box::new(NativeExit::Startup(Box::new(
                        startup_exit::Host {
                            stdin_source,
                            database: state.database,
                            filesystem: state.filesystem,
                            output: state.output,
                            warnings: state.warnings,
                            program: self.program.clone(),
                            level: 0,
                            include_search: None,
                            reader_output: None,
                            construction: None,
                            restore_directory: None,
                            database_dumped: Cell::new(false),
                        },
                    ))))
                } else {
                    stdin_source.map(|source| {
                        NativeExit::source(source, state.output.clone(), self.program.clone())
                    })
                };
                return Err(Failure { error, exit });
            }
        };
        self.stdin_argument = options.stdin_argument;
        Ok(Invocation {
            options,
            initial: Box::new(Prepared {
                database: state.database,
                filesystem: state.filesystem,
                output: state.output,
                exports: state.exports,
                seed: state.seed,
                reader_output,
                construction,
                plan: self,
                initial_directory,
                shuffle,
                warnings: state.warnings,
            }),
        })
    }
}

fn output_mode(raw: Option<&[u8]>) -> Result<crate::output::Mode, StartupError> {
    raw.map_or(Ok(crate::output::Mode::None), |raw| {
        crate::output::Mode::parse(raw)
            .ok_or_else(|| Diagnostic::from(crate::usage::output_sync_mode(raw)).into())
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
    seen_includes: HashSet<OsString>,
    seen_directories: HashSet<OsString>,
}

impl State {
    fn complete_mutex(&self, authentication: &[u8]) -> Result<(), StartupError> {
        self.output.decode_mutex(authentication).map_err(|error| {
            // A failed open leaves no usable synchronization handle for
            // startup diagnostics, even if an earlier source opened one.
            self.output.clear();
            Diagnostic::from(crate::usage::sync_mutex_open(authentication, &error)).into()
        })
    }

    fn evaluate<T>(
        &mut self,
        program: &[u8],
        action: impl FnOnce(&mut Database, ExpansionContext<'_>) -> Result<T, ExpandError>,
    ) -> Result<T, ExpandError> {
        let mut receiver = RuntimeOutput::new(&self.output, program, &self.warnings);
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
        self.output
            .set_directory_policy(options.directory_choice, options.silent);
        self.output.set_database_requested(options.print_database);
        self.output
            .set_changed_directory(!options.directories.is_empty());
        self.database
            .variables
            .set_environment_override(options.environment_override);
        let result: Result<(), ExpandError> = (|| {
            match argument {
                // The startup decoder does not emit live policy events.
                crate::cli::Argument::Warning(_) | crate::cli::Argument::Usage(..) => {}
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
                }
                crate::cli::Argument::Include(index) => {
                    let raw = options.include_directories[index].as_os_str().as_bytes();
                    if !self.seen_includes.contains(OsStr::from_bytes(raw)) {
                        let stored = self.path(program, raw)?;
                        self.seen_includes.insert(stored.as_os_str().to_owned());
                        options.include_directories[index] = stored;
                    } else {
                        options.include_directories.pop();
                        options.include_assignment_positions.pop();
                    }
                }
                crate::cli::Argument::Directory(index) => {
                    let raw = options.directories[index].as_os_str().as_bytes();
                    if !self.seen_directories.contains(OsStr::from_bytes(raw)) {
                        let stored = self.path(program, raw)?;
                        self.seen_directories.insert(stored.as_os_str().to_owned());
                        options.directories[index] = stored;
                    } else {
                        options.directories.pop();
                    }
                }
                crate::cli::Argument::File(index) if options.temp_stdin_file == Some(index) => {
                    // The internal source operand is already a prepared raw
                    // filename. Retain its selection without tilde expansion.
                    if let Some(history) = self.database.database_history() {
                        history.intern(options.files[index].as_os_str().as_bytes());
                    }
                }
                crate::cli::Argument::File(index) => {
                    let raw = options.files[index].as_os_str().as_bytes();
                    options.files[index] = self.path(program, raw)?;
                    if let Some(history) = self.database.database_history() {
                        history.intern(options.files[index].as_os_str().as_bytes());
                    }
                }
                crate::cli::Argument::OldFile(index) => {
                    let raw = options.old_files[index].as_os_str().as_bytes();
                    options.old_files[index] = self.path(program, raw)?;
                    if let Some(history) = self.database.database_history() {
                        history.intern(options.old_files[index].as_os_str().as_bytes());
                    }
                }
                crate::cli::Argument::NewFile(index) => {
                    let raw = options.new_files[index].as_os_str().as_bytes();
                    options.new_files[index] = self.path(program, raw)?;
                    if let Some(history) = self.database.database_history() {
                        history.intern(options.new_files[index].as_os_str().as_bytes());
                    }
                }
                crate::cli::Argument::Goal(index) => {
                    let path = self.path(program, &options.goals[index])?;
                    let name = path.as_os_str().as_bytes();
                    if let Some(history) = self.database.database_history() {
                        // Filename preparation and goal entry each retain the
                        // prepared name, including repeated argv goals.
                        history.intern(name);
                        history.intern(name);
                    }
                    self.database.file_records.insert(name);
                    let previous = self.database.variables.database_lookup(b"MAKECMDGOALS");
                    let had_previous = previous.is_some();
                    let mut value = previous.map(|v| v.raw_value().to_vec()).unwrap_or_default();
                    if had_previous {
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

pub(super) fn emit_error(
    output: &crate::output::Controller,
    program: &[u8],
    error: InvocationError,
) {
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
    match error {
        InvocationError::Options(message) => {
            output.write_parts(true, &[program, b": ", message.as_bytes(), b"\n"])
        }
        InvocationError::Usage { version } => {
            if version {
                crate::print_version();
                let _ = rmake_linux::output::write_standard(false, b"\n");
            }
            output.write_parts(true, &[&crate::usage::text(program)]);
        }
        InvocationError::Expansion(error) => output.write_parts(
            true,
            &[
                &error.prefix(program),
                b"*** ",
                &crate::messages::diagnostic(&error.message),
                &crate::messages::stop_for(&error.message),
            ],
        ),
    }
    if version {
        crate::print_version();
    }
}

#[cfg(feature = "test-support")]
pub(super) fn run_native_ownership_check(name: &str) -> bool {
    match name {
        "decoder_failure_retains_effects_and_last_policy_without_defaults_or_replay" => exit_ownership_tests::decoder_failure_retains_effects_and_last_policy_without_defaults_or_replay(),
        "prepared_help_pass_keeps_constructor_unstarted_and_actual_bindings" => exit_ownership_tests::prepared_help_pass_keeps_constructor_unstarted_and_actual_bindings(),
        "failed_constructor_keeps_only_metadata_published_before_home_expansion" => exit_ownership_tests::failed_constructor_keeps_only_metadata_published_before_home_expansion(),
        "failed_chdir_keeps_early_database_without_late_constructor_metadata" => exit_ownership_tests::failed_chdir_keeps_early_database_without_late_constructor_metadata(),
        "restarted_help_owns_fresh_decoder_bindings_and_restart_count" => exit_ownership_tests::restarted_help_owns_fresh_decoder_bindings_and_restart_count(),
        _ => return false,
    }
    true
}

#[cfg(any(all(test, not(feature = "guile")), feature = "test-support"))]
mod exit_ownership_tests {
    use super::*;
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);

    // Each native runtime owns one interpreter thread for the process. Unit
    // subprocesses exercise the real loader without sharing that global gate.
    fn isolated(action: impl FnOnce()) {
        if std::env::var_os("RMAKE_STARTUP_OWNER_UNIT_CHILD").is_some() {
            action();
            return;
        }
        let thread = std::thread::current();
        let name = thread.name().expect("named test worker");
        let result = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", name, "--nocapture"])
            .env("RMAKE_STARTUP_OWNER_UNIT_CHILD", "1")
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }

    struct Fixture(std::path::PathBuf);
    impl Fixture {
        fn new() -> Self {
            let directory = std::env::temp_dir().join(format!(
                "rmake-startup-owner-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed),
            ));
            fs::create_dir(&directory).unwrap();
            let module = directory.join("startup_owner_unit.so");
            let mut compiler = Command::new("cc")
                .args(["-shared", "-fPIC", "-x", "c", "-o"])
                .arg(module)
                .arg("-")
                .stdin(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap();
            compiler
                .stdin
                .take()
                .unwrap()
                .write_all(
                    br#"
/* Original setup-only module; no GNU headers or finalizers. */
struct Place { const char *file; unsigned long line; };
extern void gmk_eval(const char *, const struct Place *);
int plugin_is_GPL_compatible = 1;
int startup_owner_unit_gmk_setup(const struct Place *place) {
    (void)place;
    gmk_eval("NATIVE_SETUP_COUNT += once\nNATIVE_SETUP_VALUE := setup\n", 0);
    return 1;
}
"#,
                )
                .unwrap();
            let output = compiler.wait_with_output().unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            Self(directory)
        }
        fn plan(&self, args: &[&str]) -> Box<Plan> {
            let mut plan = Plan::new(
                args.iter().map(OsString::from).collect(),
                b"rmake",
                OsStr::new("rmake"),
                None,
                None,
                None,
            );
            plan.environment = vec![(
                "GNUMAKEFLAGS".into(),
                format!(
                    "$(eval load {})",
                    self.0.join("startup_owner_unit.so").display()
                )
                .into(),
            )];
            Box::new(plan)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn finish_owned_check(host: startup_exit::Host, fixture: Fixture) {
        #[cfg(feature = "guile")]
        {
            // Every feature child retains its actual phase through C exit.
            // Delete only the owned fixture before that non-returning boundary.
            drop(fixture);
            let mut exit = NativeExit::Startup(Box::new(host));
            exit.prepare();
            exit.finish(0);
            rmake_linux::guile::abort_host();
        }
        #[cfg(not(feature = "guile"))]
        {
            drop(host);
            drop(fixture);
        }
    }

    fn startup(exit: Option<Box<NativeExit>>) -> startup_exit::Host {
        match *exit.expect("native phase must remain owned") {
            NativeExit::Startup(host) => *host,
            NativeExit::Engine(_) | NativeExit::Source(_) => {
                panic!("constructor was not completed")
            }
        }
    }
    fn value(database: &Database, name: &[u8]) -> Option<Vec<u8>> {
        database
            .variables
            .get(name)
            .map(|value| value.raw_value().to_vec())
    }
    fn check_setup(database: &Database) {
        assert_eq!(
            value(database, b"NATIVE_SETUP_COUNT"),
            Some(b"once".to_vec())
        );
        assert_eq!(
            value(database, b"NATIVE_SETUP_VALUE"),
            Some(b"setup".to_vec())
        );
    }

    #[cfg_attr(all(test, not(feature = "guile")), test)]
    pub(super) fn decoder_failure_retains_effects_and_last_policy_without_defaults_or_replay() {
        isolated(|| {
            let fixture = Fixture::new();
            let failure = match fixture
                .plan(&[
                    "USER_VALUE=command",
                    "--warn-undefined-variables",
                    "-e",
                    "--not-an-option",
                ])
                .decode()
            {
                Ok(_) => panic!("invalid option accepted"),
                Err(error) => error,
            };
            assert!(matches!(failure.error, InvocationError::Usage { .. }));
            let mut host = startup(failure.exit);
            check_setup(&host.database);
            assert_eq!(
                value(&host.database, b"USER_VALUE"),
                Some(b"command".to_vec())
            );
            assert!(host.warnings.get());
            host.database
                .variables
                .define_literal(b"GNUMAKEFLAGS", b"changed", Origin::File)
                .unwrap();
            assert_eq!(
                host.database
                    .variables
                    .get(b"GNUMAKEFLAGS")
                    .unwrap()
                    .origin(),
                Origin::EnvironmentOverride
            );
            assert_eq!(value(&host.database, b"GNUMAKEFLAGS"), Some(Vec::new()));
            assert!(host.database.variables.get(b"CURDIR").is_none());
            assert!(host.database.variables.get(b".INCLUDE_DIRS").is_none());
            assert!(host.include_search.is_none());
            finish_owned_check(host, fixture);
        });
    }

    #[cfg_attr(all(test, not(feature = "guile")), test)]
    pub(super) fn prepared_help_pass_keeps_constructor_unstarted_and_actual_bindings() {
        isolated(|| {
            let fixture = Fixture::new();
            let launch = fixture
                .plan(&["USER_VALUE=command", "--help"])
                .decode()
                .unwrap();
            assert!(launch.options.help);
            let host = startup(launch.into_native_exit(b"rmake".to_vec()));
            check_setup(&host.database);
            assert!(host.construction.is_some());
            assert!(host.database.variables.get(b"MAKE_COMMAND").is_none());
            assert!(host.database.variables.get(b"CURDIR").is_none());
            assert_eq!(
                value(&host.database, b"USER_VALUE"),
                Some(b"command".to_vec())
            );
            finish_owned_check(host, fixture);
        });
    }

    #[cfg_attr(all(test, not(feature = "guile")), test)]
    pub(super) fn failed_constructor_keeps_only_metadata_published_before_home_expansion() {
        isolated(|| {
            let fixture = Fixture::new();
            let launch = fixture
                .plan(&[
                    "HOME=$(if $(CURDIR),$(error HOME-STOP))~",
                    "-I~/inc",
                    "USER_VALUE=command",
                ])
                .decode()
                .unwrap();
            let failure = Engine::from_invocation(launch, b"rmake".to_vec(), OsStr::new("rmake"))
                .err()
                .expect("HOME must fail in constructor");
            assert_eq!(failure.message.as_bytes(), b"HOME-STOP");
            let host = startup(failure.exit);
            check_setup(&host.database);
            assert!(host.database.variables.get(b"CURDIR").is_some());
            assert!(host.database.variables.get(b".INCLUDE_DIRS").is_none());
            assert!(host.database.variables.get(b"MAKE_COMMAND").is_none());
            assert!(host.include_search.is_none());
            assert_eq!(
                value(&host.database, b"USER_VALUE"),
                Some(b"command".to_vec())
            );
            finish_owned_check(host, fixture);
        });
    }

    #[cfg_attr(all(test, not(feature = "guile")), test)]
    pub(super) fn failed_chdir_keeps_early_database_without_late_constructor_metadata() {
        isolated(|| {
            let fixture = Fixture::new();
            let missing = fixture.0.join("missing");
            let missing = missing.to_str().unwrap();
            let launch = fixture.plan(&["-C", missing]).decode().unwrap();
            let failure = Engine::from_invocation(launch, b"rmake".to_vec(), OsStr::new("rmake"))
                .err()
                .expect("missing directory accepted");
            let host = startup(failure.exit);
            check_setup(&host.database);
            assert!(host.database.variables.get(b"CURDIR").is_none());
            assert!(host.database.variables.get(b"MAKE_COMMAND").is_none());
            assert!(host.restore_directory.is_some());
            finish_owned_check(host, fixture);
        });
    }

    #[cfg_attr(all(test, not(feature = "guile")), test)]
    pub(super) fn restarted_help_owns_fresh_decoder_bindings_and_restart_count() {
        isolated(|| {
            let fixture = Fixture::new();
            let mut plan = fixture.plan(&[]);
            plan.environment[0].1 = format!(
                "$(if $(MAKE_RESTARTS),$(eval load {}) --help)",
                fixture.0.join("startup_owner_unit.so").display()
            )
            .into();
            plan.environment.push(("MAKELEVEL".into(), "3".into()));
            plan.environment.push(("MAKE_RESTARTS".into(), "1".into()));
            let launch = plan.decode().unwrap();
            assert!(launch.options.help);
            let host = startup(launch.into_native_exit(b"rmake".to_vec()));
            check_setup(&host.database);
            assert_eq!(value(&host.database, b"MAKE_RESTARTS"), Some(b"1".to_vec()));
            assert!(host.database.variables.get(b"MAKE_COMMAND").is_none());
            assert!(host.database.variables.get(b"CURDIR").is_none());
            finish_owned_check(host, fixture);
        });
    }
}
