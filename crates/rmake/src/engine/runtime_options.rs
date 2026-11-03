// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Runtime switch interpretation and jobserver configuration after reading.
//! Switch values remain distinct from the pool's execution capacity.
use super::*;

pub(super) struct State {
    options: std::rc::Rc<RefCell<Options>>,
    dirty: Cell<bool>,
    pub(super) warn_undefined: Cell<bool>,
    pub(super) trace_enabled: Cell<bool>,
    output: crate::output::Controller,
    includes: std::rc::Rc<RefCell<IncludeSearch>>,
}

pub(super) struct Receiver<'a>(&'a State);

impl rmake_core::diagnostic::MakeflagsReceiver for Receiver<'_> {
    fn decode(
        &mut self,
        request: rmake_core::diagnostic::MakeflagsRequest<'_>,
    ) -> Result<rmake_core::diagnostic::MakeflagsResult, Vec<u8>> {
        self.0.assigned(request)
    }

    fn begin(
        &mut self,
        request: rmake_core::diagnostic::MakeflagsRequest<'_>,
        passive: bool,
    ) -> Result<Option<Box<dyn rmake_core::diagnostic::FlagDecoder>>, Vec<u8>> {
        self.0.begin_include_decoder(request, passive)
    }
    fn sync_decoded_flags(&mut self) {
        let options = self.0.options.borrow();
        self.0.warn_undefined.set(options.warn_undefined);
        self.0.trace_enabled.set(options.trace);
        self.0.dirty.set(true);
    }

    fn set_warning_policy(&mut self, enabled: bool) {
        self.0.options.borrow_mut().warn_undefined = enabled;
        self.0.warn_undefined.set(enabled);
        self.0.dirty.set(true);
    }
}

impl State {
    pub(super) fn receiver(&self) -> Receiver<'_> {
        Receiver(self)
    }
    pub(super) fn new(
        options: &Options,
        output: crate::output::Controller,
        includes: std::rc::Rc<RefCell<IncludeSearch>>,
    ) -> Self {
        Self {
            options: std::rc::Rc::new(RefCell::new(options.clone())),
            dirty: Cell::new(false),
            warn_undefined: Cell::new(options.warn_undefined),
            trace_enabled: Cell::new(options.trace),
            output,
            includes,
        }
    }
    pub(super) fn refresh(&self, options: &Options) {
        self.trace_enabled.set(options.trace);
        self.warn_undefined.set(options.warn_undefined);
        self.options.replace(options.clone());
        self.dirty.set(false);
    }
    pub(super) fn take(&self) -> Option<Options> {
        if !self.dirty.get() {
            return None;
        }
        self.dirty.set(false);
        Some(self.options.borrow().clone())
    }
    fn begin_include_decoder(
        &self,
        request: rmake_core::diagnostic::MakeflagsRequest<'_>,
        passive: bool,
    ) -> Result<Option<Box<dyn rmake_core::diagnostic::FlagDecoder>>, Vec<u8>> {
        let prepared = self
            .options
            .borrow()
            .include_decoder(request.value)
            .map_err(String::into_bytes)?;
        let Some(mut decoder) = prepared else {
            return Ok(None);
        };
        let mut options = self.options.borrow_mut();
        let invocation = InvocationInputs::take(&mut options);
        let initial = std::mem::take(&mut decoder.options);
        *options = initial;
        drop(options);
        self.dirty.set(true);
        Ok(Some(Box::new(IncludeDecoder {
            decoder,
            options: self.options.clone(),
            includes: self.includes.clone(),
            output: self.output.clone(),
            invocation: Some(invocation),
            active: false,
            pending: None,
            origin: request.origin,
            passive,
            file_jobs: None,
        })))
    }

    pub(super) fn assigned(
        &self,
        request: rmake_core::diagnostic::MakeflagsRequest<'_>,
    ) -> Result<rmake_core::diagnostic::MakeflagsResult, Vec<u8>> {
        let current = self.options.borrow();
        let (mut updated, _, warning_positions) = current
            .assigned_flags_with_positions(&[request.value.to_vec()], request.origin)
            .map_err(String::into_bytes)?;
        let assignments: Vec<_> = std::mem::take(&mut updated.assignments)
            .into_iter()
            .zip(std::mem::take(&mut updated.assignment_context))
            .map(|(argument, (position, environment_override))| {
                rmake_core::diagnostic::FlagAssignment {
                    argument,
                    environment_override,
                    position,
                }
            })
            .collect();
        updated.reset_jobserver = current.reset_jobserver;
        let mode = updated
            .output_sync
            .as_deref()
            .map_or(Some(crate::output::Mode::None), crate::output::Mode::parse);
        if mode.is_none() && assignments.is_empty() {
            return Err([
                b"unknown output-sync type '".as_slice(),
                updated.output_sync.as_deref().unwrap_or_default(),
                b"'",
            ]
            .concat());
        }
        // GNU validates output synchronization after payload definitions. The
        // core refreshes this receiver when they finish, so an invalid pending
        // selection must not skip their effects or construct a new search yet.
        let search = mode.map(|mode| {
            self.output.set_mode(mode);
            IncludeSearch::new(&updated.include_directories)
        });
        let include_dirs = search.as_ref().map_or_else(Vec::new, |search| {
            search.directories().map(<[u8]>::to_vec).collect()
        });
        let result = rmake_core::diagnostic::MakeflagsResult {
            eval_reference: !updated.evals.is_empty(),
            // Finalization selects current flags after nested metadata effects.
            // Avoid allocating a string that this owned callback replaces.
            value: Vec::new(),
            origin: origin(updated.environment_override),
            environment_override: updated.environment_override,
            include_dirs,
            publish_includes: if let Some(search) = search {
                Some(Box::new({
                    let includes = self.includes.clone();
                    let options = self.options.clone();
                    move || {
                        includes.replace(search);
                        let options = options.borrow();
                        rmake_core::diagnostic::MakeflagsMetadata {
                            eval_reference: !options.evals.is_empty(),
                            value: options.recursive_flags(),
                            origin: origin(options.environment_override),
                            environment_override: options.environment_override,
                        }
                    }
                }))
            } else {
                None
            },
            assignments,
            warning_positions,
            warning_entry: current.warn_undefined,
            next_argument: updated.flag_word_count,
        };
        drop(current);
        self.trace_enabled.set(updated.trace);
        self.warn_undefined.set(updated.warn_undefined);
        // These invocation inputs survive a live reset unchanged. Move them
        // only after fallible decoding and search preparation have succeeded.
        // No evaluator callback runs while this exclusive borrow is held.
        let mut previous = self.options.borrow_mut();
        updated.files = std::mem::take(&mut previous.files);
        updated.directories = std::mem::take(&mut previous.directories);
        updated.goals = std::mem::take(&mut previous.goals);
        updated.assignments = std::mem::take(&mut previous.assignments);
        updated.assignment_context = std::mem::take(&mut previous.assignment_context);
        updated.assignment_warnings = std::mem::take(&mut previous.assignment_warnings);
        *previous = updated;
        self.dirty.set(true);
        Ok(result)
    }
}

/// Invocation operands live outside the suspended stream and are restored once.
#[derive(Default)]
struct InvocationInputs {
    files: Vec<std::path::PathBuf>,
    directories: Vec<std::path::PathBuf>,
    old_files: Vec<std::path::PathBuf>,
    new_files: Vec<std::path::PathBuf>,
    goals: Vec<Vec<u8>>,
    assignments: Vec<Vec<u8>>,
    assignment_context: Vec<(usize, bool)>,
    assignment_warnings: Vec<bool>,
}
impl InvocationInputs {
    fn take(options: &mut Options) -> Self {
        Self {
            files: std::mem::take(&mut options.files),
            directories: std::mem::take(&mut options.directories),
            old_files: std::mem::take(&mut options.old_files),
            new_files: std::mem::take(&mut options.new_files),
            goals: std::mem::take(&mut options.goals),
            assignments: std::mem::take(&mut options.assignments),
            assignment_context: std::mem::take(&mut options.assignment_context),
            assignment_warnings: std::mem::take(&mut options.assignment_warnings),
        }
    }
    fn restore(self, options: &mut Options) {
        options.files = self.files;
        options.directories = self.directories;
        options.old_files = self.old_files;
        options.new_files = self.new_files;
        options.goals = self.goals;
        options.assignments = self.assignments;
        options.assignment_context = self.assignment_context;
        options.assignment_warnings = self.assignment_warnings;
    }
}

struct IncludeDecoder {
    decoder: crate::cli::Decoder,
    options: std::rc::Rc<RefCell<Options>>,
    includes: std::rc::Rc<RefCell<IncludeSearch>>,
    output: crate::output::Controller,
    invocation: Option<InvocationInputs>,
    active: bool,
    pending: Option<crate::cli::Argument>,
    origin: Origin,
    passive: bool,
    file_jobs: Option<usize>,
}

impl rmake_core::diagnostic::FlagDecoder for IncludeDecoder {
    fn terminal(&self) -> usize {
        self.decoder.terminal()
    }
    fn requested_jobs(&self) -> Option<usize> {
        self.file_jobs
            .filter(|_| self.options.borrow().command_jobs.is_none())
    }
    fn next(&mut self, cursor: usize) -> Result<Option<rmake_core::diagnostic::FlagStep>, Vec<u8>> {
        use crate::cli::Argument;
        use rmake_core::diagnostic::{FlagAction, FlagStep};
        self.active = true;
        self.decoder.options = std::mem::take(&mut *self.options.borrow_mut());
        let options = &mut self.decoder.options;
        let silent = (options.silent, options.silent_origin.take());
        let keep = (options.keep_going, options.keep_going_origin.take());
        let jobs = options.jobs.take();
        self.decoder.cursor = cursor.saturating_sub(1);
        let next = self
            .decoder
            .advance::<false>()
            .map_err(String::into_bytes)?;
        let options = &mut self.decoder.options;
        if options.silent_origin.is_some()
            && (self.origin == Origin::CommandLine || silent.1.is_none_or(|old| self.origin >= old))
        {
            options.silent_origin = Some(self.origin);
        } else {
            options.silent = silent.0;
            options.silent_origin = silent.1;
        }
        if options.keep_going_origin.is_some()
            && (self.origin == Origin::CommandLine || keep.1.is_none_or(|old| self.origin >= old))
        {
            options.keep_going_origin = Some(self.origin);
        } else {
            options.keep_going = keep.0;
            options.keep_going_origin = keep.1;
        }
        if options.jobs.is_some() {
            self.file_jobs = options.jobs;
        } else {
            options.jobs = jobs;
        }
        let action = match next.as_ref().and_then(|step| step.argument) {
            Some(Argument::Definition(index)) => {
                FlagAction::Definition(options.assignments[index].clone())
            }
            Some(argument @ Argument::Include(_)) => {
                let path = options
                    .include_directories
                    .pop()
                    .expect("just decoded operand");
                options.include_assignment_positions.pop();
                if options
                    .include_directories
                    .iter()
                    .any(|stored| stored.as_os_str().as_bytes() == path.as_os_str().as_bytes())
                {
                    FlagAction::Policy
                } else {
                    self.pending = Some(argument);
                    FlagAction::Filename(path.into_os_string().into_vec())
                }
            }
            // GNU ignores invocation-only switches in runtime sources. Their
            // operands must not expand HOME, change directories or open files.
            _ => FlagAction::Policy,
        };
        let environment_override = options.environment_override;
        let following_definition =
            matches!(&action, FlagAction::Definition(_)) && self.decoder.following_definition();
        *self.options.borrow_mut() = std::mem::take(&mut self.decoder.options);
        self.active = false;
        Ok(next.map(|step| FlagStep {
            position: step.position,
            environment_override,
            following_definition,
            action,
        }))
    }
    fn filename(&mut self, value: Vec<u8>) -> Result<(), Vec<u8>> {
        let mut options = self.options.borrow_mut();
        match self.pending.take().expect("suspended filename") {
            crate::cli::Argument::Include(_) => {
                options
                    .include_directories
                    .push(std::ffi::OsString::from_vec(value).into());
                let count = options.assignments.len();
                options.include_assignment_positions.push(count);
            }
            _ => unreachable!(),
        }
        Ok(())
    }
    fn finish(mut self: Box<Self>) -> Result<rmake_core::diagnostic::MakeflagsResult, Vec<u8>> {
        let terminal = self.decoder.terminal();
        let options = self.options.clone();
        let includes = self.includes.clone();
        let output = self.output.clone();
        let passive = self.passive;
        let invocation = self.invocation.take().expect("owned invocation");
        let mut current = options.borrow_mut();
        invocation.restore(&mut current);
        current.flag_word_count = terminal;
        if passive && let Some(jobs) = current.command_jobs {
            current.jobs = Some(jobs);
        }
        let mode = current
            .output_sync
            .as_deref()
            .map_or(Some(crate::output::Mode::None), crate::output::Mode::parse)
            .ok_or_else(|| {
                [
                    b"unknown output-sync type '".as_slice(),
                    current.output_sync.as_deref().unwrap_or_default(),
                    b"'",
                ]
                .concat()
            })?;
        let mut result = rmake_core::diagnostic::MakeflagsResult {
            eval_reference: !current.evals.is_empty(),
            value: current.recursive_flags(),
            origin: origin(current.environment_override),
            environment_override: current.environment_override,
            include_dirs: Vec::new(),
            publish_includes: None,
            assignments: Vec::new(),
            warning_positions: Vec::new(),
            warning_entry: current.warn_undefined,
            next_argument: current.flag_word_count,
        };
        if !passive {
            output.set_mode(mode);
            let search = IncludeSearch::new(&current.include_directories);
            result.include_dirs = search.directories().map(<[u8]>::to_vec).collect();
            let captured = options.clone();
            result.publish_includes = Some(Box::new(move || {
                includes.replace(search);
                let options = captured.borrow();
                rmake_core::diagnostic::MakeflagsMetadata {
                    eval_reference: !options.evals.is_empty(),
                    value: options.recursive_flags(),
                    origin: origin(options.environment_override),
                    environment_override: options.environment_override,
                }
            }));
        }
        Ok(result)
    }
}

impl Drop for IncludeDecoder {
    fn drop(&mut self) {
        // Restore invocation ownership on errors and cancellation as well as
        // success. Previously completed option/variable effects remain live.
        if self.active {
            *self.options.borrow_mut() = std::mem::take(&mut self.decoder.options);
        }
        if let Some(invocation) = self.invocation.take() {
            invocation.restore(&mut self.options.borrow_mut());
        }
    }
}

pub(super) fn origin(environment_override: bool) -> Origin {
    if environment_override {
        Origin::EnvironmentOverride
    } else {
        Origin::File
    }
}

impl Engine {
    pub(super) fn setup_jobserver(&mut self, create: bool) -> Result<(), Diagnostic> {
        if create {
            if let Some(options) = self.flag_state.take() {
                self.options = options;
            }
            if let Some(jobs) = self.options.command_jobs {
                self.options.jobs = Some(jobs);
            }
        }
        if self.jobserver.is_some() {
            return Ok(());
        }
        if self.options.reset_jobserver {
            emit_output(
                &self.output,
                &self.program,
                Output::Warning {
                    location: None,
                    message: format!(
                        "warning: -j{} forced in submake: resetting jobserver mode.",
                        self.options
                            .jobs
                            .filter(|jobs| *jobs != usize::MAX)
                            .unwrap_or(0)
                    )
                    .as_bytes(),
                },
            );
        }
        self.jobserver = if self.options.jobs == Some(1) {
            self.options.jobserver_auth = None;
            None
        } else if let Some(auth) = &self.options.jobserver_auth {
            match Jobserver::join(auth) {
                Ok(server) => Some(server),
                Err(error) => {
                    if let Some(path) = auth.strip_prefix(b"fifo:") {
                        emit_output(
                            &self.output,
                            &self.program,
                            Output::Warning {
                                location: None,
                                message: &[
                                    b"cannot open jobserver ".as_slice(),
                                    path,
                                    b": ",
                                    os_error(&error).as_bytes(),
                                ]
                                .concat(),
                            },
                        );
                    }
                    emit_output(&self.output, &self.program, Output::Warning { location: None,
                        message: b"warning: jobserver unavailable: using -j1.  Add '+' to parent make rule." });
                    self.options.jobs = Some(1);
                    self.options.jobserver_auth = None;
                    None
                }
            }
        } else if let Some(jobs) = self
            .options
            .jobs
            .filter(|jobs| create && *jobs > 1 && *jobs != usize::MAX)
        {
            Some(
                Jobserver::create(jobs, self.options.jobserver_pipe).map_err(|error| {
                    Diagnostic::from(format!("jobserver: {}", os_error(&error)))
                })?,
            )
        } else {
            None
        };
        if let Some(server) = &self.jobserver {
            self.options.jobserver_auth = Some(server.auth().to_vec());
        }
        self.options.inherited_jobs = false;
        self.options.reset_jobserver = false;
        Ok(())
    }

    pub(super) fn read_runtime_flags(&mut self, restarts: u32) -> Result<(), ()> {
        if let Some(options) = self.flag_state.take() {
            self.options = options;
        }
        let entry_auth = self.options.jobserver_auth.clone();
        let mut changed = false;
        let mut file_jobs = None;
        for name in [b"GNUMAKEFLAGS".as_slice(), b"MAKEFLAGS".as_slice()] {
            if self.flag_state.warn_undefined.get() && self.database.variables.get(name).is_none() {
                let expression = [b"$(".as_slice(), name, b")"].concat();
                if let Err(error) = self.expand_global(&expression, None) {
                    self.expansion_error(None, &error);
                    return Err(());
                }
            }
            let initial = if name == b"GNUMAKEFLAGS" {
                &self.initial_gnu_flags
            } else {
                &self.initial_flags
            };
            let unchanged = self
                .database
                .variables
                .get(name)
                .is_none_or(|value| value.raw_value() == *initial);
            let value = if unchanged && name == b"MAKEFLAGS" && changed {
                let prefix = self
                    .initial_flags
                    .strip_suffix(b" -- $(MAKEOVERRIDES)")
                    .or_else(|| {
                        self.initial_flags
                            .strip_suffix(b" -- $(-*-command-variables-*-)")
                    })
                    .unwrap_or(&self.initial_flags);
                Some(prefix.to_vec())
            } else if !unchanged {
                let expression = [b"$(".as_slice(), name, b")"].concat();
                match self.expand_global(&expression, None) {
                    Ok(mut value) => {
                        let encoded = name == b"MAKEFLAGS"
                            && self.database.variables.get(name).is_some_and(|variable| {
                                Database::has_command_overrides_reference(variable.raw_value())
                            });
                        if encoded
                            && let Some(boundary) =
                                value.windows(4).position(|part| part == b" -- ")
                        {
                            value.truncate(boundary);
                        }
                        Some(value)
                    }
                    Err(error) => {
                        self.expansion_error(None, &error);
                        return Err(());
                    }
                }
            } else {
                None
            };
            if let Some(value) = value {
                changed = true;
                // Finish this owned source before expanding the next one.
                // Nested effects therefore see and publish current state.
                if let Some(jobs) = self.apply_runtime_flag_source(value)? {
                    file_jobs = Some(jobs);
                }
            }
            if name == b"GNUMAKEFLAGS"
                && let Err(error) =
                    self.database
                        .variables
                        .define_literal(name, b"", Origin::Override)
            {
                self.expansion_error(None, &error);
                return Err(());
            }
        }
        if !changed {
            return Ok(());
        }
        if let Some(jobs) = file_jobs.filter(|jobs| Some(*jobs) != self.flag_entry_jobs) {
            if restarts == 0 && entry_auth.is_some() {
                self.stderr(
                    format!(
                        "warning: -j{} forced in makefile: resetting jobserver mode.",
                        if jobs == usize::MAX { 0 } else { jobs }
                    )
                    .as_bytes(),
                );
            }
            self.jobserver.take();
            self.options.jobserver_auth = None;
        }
        let mode = self
            .options
            .output_sync
            .as_deref()
            .map_or(Some(crate::output::Mode::None), crate::output::Mode::parse);
        let Some(mode) = mode else {
            self.fatal(
                None,
                &[
                    b"unknown output-sync type '".as_slice(),
                    self.options.output_sync.as_deref().unwrap_or_default(),
                    b"'",
                ]
                .concat(),
            );
            return Err(());
        };
        if self.options.jobserver_auth != entry_auth {
            self.jobserver.take();
        }
        if let Err(error) = self.database.variables.update_mflags(
            &self.options.recursive_flags(),
            self.options.environment_override,
        ) {
            self.expansion_error(None, &error);
            return Err(());
        }
        // Passive operands accumulate without rebuilding include search.
        // Live MAKEFLAGS resets retain their separate publication boundary.
        self.output.set_mode(mode);
        if !mode.root_buffered() {
            self.output.select(None);
            if let Some(buffer) = self.reader_output.take() {
                buffer.dump();
            }
        } else if self.reader_output.is_none() {
            self.reader_output = self.output.make_buffer();
            self.output.select(self.reader_output.as_ref());
        }
        Ok(())
    }

    fn apply_runtime_flag_source(&mut self, value: Vec<u8>) -> Result<Option<usize>, ()> {
        // Source expansion can itself change policy; capture its completed state.
        let warning_entry = self.flag_state.warn_undefined.get();
        let (mut updated, file_jobs, warning_positions) = match self
            .options
            .file_flags_with_positions(std::slice::from_ref(&value))
        {
            Ok(updated) => updated,
            Err(message) => {
                self.fatal(None, message.as_bytes());
                return Err(());
            }
        };
        if value.contains(&b'~') {
            self.flag_state.refresh(&self.options);
            let decoder = match self.flag_state.begin_include_decoder(
                rmake_core::diagnostic::MakeflagsRequest {
                    value: &value,
                    origin: Origin::Environment,
                },
                true,
            ) {
                Ok(decoder) => decoder,
                Err(error) => {
                    self.fatal(None, &error);
                    return Err(());
                }
            };
            if let Some(decoder) = decoder {
                let driven = runtime_context(
                    None,
                    &mut WarningOutput::new(
                        &mut |event| emit_output(&self.output, &self.program, event),
                        &self.flag_state.warn_undefined,
                    ),
                )
                .with_makeflags(&mut self.flag_state.receiver())
                .with_filesystem(&mut |effect| self.filesystem.borrow_mut().execute(effect))
                .with_shell(&mut |request| {
                    let result = crate::shell::run(
                        request,
                        &self.program,
                        self.makelevel,
                        &self.output,
                        self.jobserver.as_ref(),
                    );
                    self.filesystem.borrow_mut().child_completed();
                    result
                })
                .drive_flag_decoder(
                    &mut self.database,
                    decoder,
                    Origin::Environment,
                );
                let rmake_core::diagnostic::DecodedFlags { names, jobs, .. } = match driven {
                    Ok(value) => value,
                    Err(error) => {
                        self.expansion_error(None, &error);
                        return Err(());
                    }
                };
                for name in names {
                    if self.database.variables.get(&name).is_some_and(|v| {
                        matches!(
                            v.origin(),
                            Origin::Environment | Origin::EnvironmentOverride
                        )
                    }) {
                        self.inherited_exports.insert(name);
                    }
                }
                if let Some(options) = self.flag_state.take() {
                    self.options = options;
                }
                self.database
                    .variables
                    .set_environment_override(self.options.environment_override);
                return Ok(jobs);
            }
        }
        // A flag-only stream has no payload effects to interleave. Keep that
        // path unchanged instead of cloning and replaying an inert policy.
        if !updated.assignments.is_empty() {
            let assignments = std::mem::take(&mut updated.assignments);
            self.flag_state.refresh(&updated);
            let assignments = assignments
                .into_iter()
                .zip(std::mem::take(&mut updated.assignment_context))
                .map(|(argument, (position, environment_override))| {
                    rmake_core::diagnostic::FlagAssignment {
                        argument,
                        position,
                        environment_override,
                    }
                })
                .collect::<Vec<_>>();
            let names = runtime_context(
                None,
                &mut WarningOutput::new(
                    &mut |event| emit_output(&self.output, &self.program, event),
                    &self.flag_state.warn_undefined,
                ),
            )
            .with_makeflags(&mut self.flag_state.receiver())
            .with_filesystem(&mut |effect| self.filesystem.borrow_mut().execute(effect))
            .with_shell(&mut |request| {
                let result = crate::shell::run(
                    request,
                    &self.program,
                    self.makelevel,
                    &self.output,
                    self.jobserver.as_ref(),
                );
                self.filesystem.borrow_mut().child_completed();
                result
            })
            .apply_flag_steps(
                &mut self.database,
                &assignments,
                Origin::Environment,
                updated.flag_word_count,
                &warning_positions,
                warning_entry,
            );
            match names {
                Ok(names) => {
                    for name in names {
                        if self.database.variables.get(&name).is_some_and(|variable| {
                            matches!(
                                variable.origin(),
                                Origin::Environment | Origin::EnvironmentOverride
                            )
                        }) {
                            self.inherited_exports.insert(name);
                        }
                    }
                }
                Err(error) => {
                    self.expansion_error(None, &error);
                    return Err(());
                }
            }
            if let Some(options) = self.flag_state.take() {
                updated = options;
            }
        }
        // Each source validates its final selection before the next source is
        // expanded. A later MAKEFLAGS value cannot rescue invalid GNU flags or
        // observe their post-source clearing after GNU has already stopped.
        if let Some(value) = updated.output_sync.as_deref()
            && crate::output::Mode::parse(value).is_none()
        {
            self.fatal(
                None,
                &[b"unknown output-sync type '".as_slice(), value, b"'"].concat(),
            );
            return Err(());
        }
        updated.files = std::mem::take(&mut self.options.files);
        updated.directories = std::mem::take(&mut self.options.directories);
        updated.goals = std::mem::take(&mut self.options.goals);
        updated.assignments = std::mem::take(&mut self.options.assignments);
        updated.assignment_context = std::mem::take(&mut self.options.assignment_context);
        updated.assignment_warnings = std::mem::take(&mut self.options.assignment_warnings);
        updated.reset_jobserver = self.options.reset_jobserver;
        self.options = updated;
        self.database
            .variables
            .set_environment_override(self.options.environment_override);
        // The following source may re-enter the live receiver or take its state.
        self.flag_state.refresh(&self.options);
        Ok(file_jobs)
    }
}

#[cfg(test)]
mod decoder_ownership_tests {
    use super::*;
    use rmake_core::diagnostic::{FlagAction, MakeflagsRequest};

    #[test]
    fn suspended_and_failed_decoders_restore_invocation_and_keep_completed_effects() {
        for fail in [false, true] {
            let original = Options {
                files: vec!["main.mk".into()],
                directories: vec!["working".into()],
                old_files: vec!["old".into()],
                new_files: vec!["new".into()],
                goals: vec![b"requested".to_vec()],
                assignments: vec![b"X=one".to_vec()],
                assignment_context: vec![(1, false)],
                assignment_warnings: vec![false],
                ..Options::default()
            };
            let state = State::new(
                &original,
                crate::output::Controller::new(crate::output::Mode::None, b"make"),
                std::rc::Rc::new(RefCell::new(IncludeSearch::default())),
            );
            let mut decoder = state
                .begin_include_decoder(
                    MakeflagsRequest {
                        value: b"-I~/inc -f -jbad",
                        origin: Origin::File,
                    },
                    false,
                )
                .unwrap()
                .unwrap();
            assert!(matches!(
                decoder.next(1).unwrap().unwrap().action,
                FlagAction::Filename(_)
            ));
            if fail {
                decoder.filename(b"hdir/inc".to_vec()).unwrap();
                // A nested reset can move the observable cursor into what was
                // previously a consumed filename operand.
                assert!(decoder.next(3).is_err());
            }
            drop(decoder);
            let restored = state.take().unwrap();
            assert_eq!(restored.files, original.files);
            assert_eq!(restored.directories, original.directories);
            assert_eq!(restored.old_files, original.old_files);
            assert_eq!(restored.new_files, original.new_files);
            assert_eq!(restored.goals, original.goals);
            assert_eq!(restored.assignments, original.assignments);
            assert_eq!(restored.assignment_context, original.assignment_context);
            assert_eq!(restored.assignment_warnings, original.assignment_warnings);
            assert_eq!(
                restored.include_directories,
                if fail {
                    vec![std::path::PathBuf::from("hdir/inc")]
                } else {
                    vec![]
                }
            );
        }
    }
}

#[cfg(test)]
mod raw_include_history_tests {
    use super::*;
    use rmake_core::diagnostic::{FlagAction, MakeflagsRequest};

    #[test]
    fn a_distinct_raw_trailing_slash_operand_is_not_discarded_before_expansion() {
        let options = Options {
            include_directories: vec!["same/inc".into()],
            include_assignment_positions: vec![0],
            ..Options::default()
        };
        let state = State::new(
            &options,
            crate::output::Controller::new(crate::output::Mode::None, b"make"),
            std::rc::Rc::new(RefCell::new(IncludeSearch::default())),
        );
        let mut decoder = state
            .begin_include_decoder(
                MakeflagsRequest {
                    value: b"-Isame/inc/ -I~/inc",
                    origin: Origin::File,
                },
                false,
            )
            .unwrap()
            .unwrap();
        let step = decoder.next(1).unwrap().unwrap();
        assert!(matches!(step.action, FlagAction::Filename(ref name) if name == b"same/inc/"));
        decoder.filename(b"same/inc/".to_vec()).unwrap();
        drop(decoder);
        let selected = state.take().unwrap();
        assert_eq!(selected.include_directories.len(), 2);
        assert_eq!(
            selected.include_directories[0].as_os_str().as_bytes(),
            b"same/inc"
        );
        assert_eq!(
            selected.include_directories[1].as_os_str().as_bytes(),
            b"same/inc/"
        );
    }
}
