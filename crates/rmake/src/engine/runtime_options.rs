// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Runtime switch interpretation and jobserver configuration after reading.
//! Switch values remain distinct from the pool's execution capacity.
use super::*;

pub(super) struct State {
    options: RefCell<Options>,
    dirty: Cell<bool>,
    output: crate::output::Controller,
    includes: std::rc::Rc<RefCell<IncludeSearch>>,
}

impl State {
    pub(super) fn new(
        options: &Options,
        output: crate::output::Controller,
        includes: std::rc::Rc<RefCell<IncludeSearch>>,
    ) -> Self {
        Self {
            options: RefCell::new(options.clone()),
            dirty: Cell::new(false),
            output,
            includes,
        }
    }
    pub(super) fn refresh(&self, options: &Options) {
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
    pub(super) fn assigned(
        &self,
        request: rmake_core::diagnostic::MakeflagsRequest<'_>,
    ) -> Result<rmake_core::diagnostic::MakeflagsResult, Vec<u8>> {
        let current = self.options.borrow();
        let (mut updated, _) = current
            .assigned_flags(&[request.value.to_vec()], request.origin)
            .map_err(String::into_bytes)?;
        let assignments = std::mem::take(&mut updated.assignments)
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
        updated.files = current.files.clone();
        updated.directories = current.directories.clone();
        updated.goals = current.goals.clone();
        updated.assignments = current.assignments.clone();
        updated.assignment_context = current.assignment_context.clone();
        updated.include_assignment_positions = current.include_assignment_positions.clone();
        updated.reset_jobserver = current.reset_jobserver;
        let mode = updated
            .output_sync
            .as_deref()
            .map_or(Some(crate::output::Mode::None), crate::output::Mode::parse)
            .ok_or_else(|| {
                [
                    b"unknown output-sync type '".as_slice(),
                    updated.output_sync.as_deref().unwrap_or_default(),
                    b"'",
                ]
                .concat()
            })?;
        self.output.set_mode(mode);
        let mut seen = HashSet::new();
        updated
            .include_directories
            .retain(|path| seen.insert(path.clone()));
        let search = IncludeSearch::new(&updated.include_directories);
        let include_dirs = search.directories().map(<[u8]>::to_vec).collect();
        self.includes.replace(search);
        let result = rmake_core::diagnostic::MakeflagsResult {
            value: updated.recursive_flags(),
            origin: origin(updated.environment_override),
            environment_override: updated.environment_override,
            include_dirs,
            assignments,
            next_argument: updated.flag_word_count,
        };
        drop(current);
        self.options.replace(updated);
        self.dirty.set(true);
        Ok(result)
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
        let mut changed = Vec::new();
        for name in [b"GNUMAKEFLAGS".as_slice(), b"MAKEFLAGS".as_slice()] {
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
            if unchanged && name == b"MAKEFLAGS" && !changed.is_empty() {
                let prefix = self
                    .initial_flags
                    .strip_suffix(b" -- $(MAKEOVERRIDES)")
                    .or_else(|| {
                        self.initial_flags
                            .strip_suffix(b" -- $(-*-command-variables-*-)")
                    })
                    .unwrap_or(&self.initial_flags);
                changed.push(prefix.to_vec());
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
                        changed.push(value);
                    }
                    Err(error) => {
                        self.expansion_error(None, &error);
                        return Err(());
                    }
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
        if changed.is_empty() {
            return Ok(());
        }
        let (mut updated, file_jobs) = match self.options.file_flags(&changed) {
            Ok(updated) => updated,
            Err(message) => {
                self.fatal(None, message.as_bytes());
                return Err(());
            }
        };
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
            let names = runtime_context(None, &mut |event| {
                emit_output(&self.output, &self.program, event)
            })
            .with_makeflags(&mut |request| self.flag_state.assigned(request))
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
            .apply_flag_assignments(
                &mut self.database,
                &assignments,
                Origin::Environment,
                updated.flag_word_count,
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
        if let Some(jobs) = file_jobs.filter(|jobs| Some(*jobs) != self.flag_entry_jobs) {
            if restarts == 0 && self.options.jobserver_auth.is_some() {
                self.stderr(
                    format!(
                        "warning: -j{} forced in makefile: resetting jobserver mode.",
                        if jobs == usize::MAX { 0 } else { jobs }
                    )
                    .as_bytes(),
                );
            }
            self.jobserver.take();
            updated.jobserver_auth = None;
        }
        updated.files = std::mem::take(&mut self.options.files);
        updated.directories = std::mem::take(&mut self.options.directories);
        updated.goals = std::mem::take(&mut self.options.goals);
        updated.assignments = std::mem::take(&mut self.options.assignments);
        updated.assignment_context = std::mem::take(&mut self.options.assignment_context);
        updated.include_assignment_positions =
            std::mem::take(&mut self.options.include_assignment_positions);
        updated.reset_jobserver = self.options.reset_jobserver;
        let mode = updated
            .output_sync
            .as_deref()
            .map_or(Some(crate::output::Mode::None), crate::output::Mode::parse);
        let Some(mode) = mode else {
            self.fatal(
                None,
                &[
                    b"unknown output-sync type '".as_slice(),
                    updated.output_sync.as_deref().unwrap_or_default(),
                    b"'",
                ]
                .concat(),
            );
            return Err(());
        };
        if updated.jobserver_auth != self.options.jobserver_auth {
            self.jobserver.take();
        }
        let mut seen = HashSet::new();
        updated
            .include_directories
            .retain(|path| seen.insert(path.clone()));
        if let Err(error) = self
            .database
            .variables
            .update_mflags(&updated.recursive_flags(), updated.environment_override)
        {
            self.expansion_error(None, &error);
            return Err(());
        }
        self.options = updated;
        self.database
            .variables
            .set_environment_override(self.options.environment_override);
        self.include_search
            .replace(IncludeSearch::new(&self.options.include_directories));
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
}
