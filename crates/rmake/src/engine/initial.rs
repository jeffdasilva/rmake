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
    pub(super) exports: HashSet<Vec<u8>>,
    pub(super) seed: startup::Seed,
    pub(super) plan: Box<Plan>,
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
        let warnings = Cell::new(false);
        let mut prefix_count = 0;
        let mut parsed_prefix = None;
        let mut seed = startup::Seed::default();
        let mut seen = HashSet::new();
        let mut include_directories = Vec::new();
        for name in [b"GNUMAKEFLAGS".as_slice(), b"MAKEFLAGS".as_slice()] {
            if name == b"GNUMAKEFLAGS" && database.variables.get(name).is_none() {
                continue;
            }
            let mut emit = |event: Output<'_>| emit_output(&output, &self.program, event);
            let mut receiver = WarningOutput::new(&mut emit, &warnings);
            let expression = [b"$(".as_slice(), name, b")"].concat();
            let value = database
                .expand_with(
                    &expression,
                    runtime_context(None, &mut receiver)
                        .with_filesystem(&mut |effect| filesystem.borrow_mut().execute(effect))
                        .with_shell(&mut |request| {
                            let result =
                                crate::shell::run(request, &self.program, 0, &output, None);
                            filesystem.borrow_mut().child_completed();
                            result
                        })
                        .with_includes(&mut |path| IncludeSearch::default().read(path)),
                )
                .map_err(StartupError::expansion)?;
            let mut incoming = Vec::new();
            Options::append_expanded_flags(&mut incoming, &value)
                .map_err(InvocationError::Options)?;
            // All expansion effects occur even when there are no new tokens.
            if incoming.is_empty() {
                if name == b"GNUMAKEFLAGS" {
                    database
                        .variables
                        .define_literal(name, b"", Origin::Environment)
                        .map_err(StartupError::expansion)?;
                }
                continue;
            }
            let count = incoming.len();
            let parsed = Options::parse_continued(
                incoming,
                parsed_prefix.take().unwrap_or_default(),
                prefix_count,
            )
            .map_err(InvocationError::Options)?;
            prefix_count += count;
            let mut next_include = seed.includes;
            for index in seed.assignments..=parsed.assignments.len() {
                while parsed.include_assignment_positions.get(next_include) == Some(&index) {
                    let directory = crate::makefiles::prepare_argument(
                        parsed.include_directories[next_include]
                            .as_os_str()
                            .as_bytes(),
                        &mut database.variables,
                        runtime_context(None, &mut receiver)
                            .with_filesystem(&mut |effect| filesystem.borrow_mut().execute(effect))
                            .with_shell(&mut |request| {
                                let result =
                                    crate::shell::run(request, &self.program, 0, &output, None);
                                filesystem.borrow_mut().child_completed();
                                result
                            }),
                    )
                    .map_err(StartupError::expansion)?;
                    include_directories.push(directory);
                    next_include += 1;
                }
                let Some(argument) = parsed.assignments.get(index) else {
                    break;
                };
                warnings.set(parsed.assignment_warnings[index]);
                database
                    .variables
                    .set_environment_override(parsed.assignment_context[index].1);
                let variable = database
                    .define_argument_with(
                        argument,
                        Origin::CommandLine,
                        runtime_context(None, &mut receiver)
                            .with_filesystem(&mut |effect| filesystem.borrow_mut().execute(effect))
                            .with_shell(&mut |request| {
                                let result =
                                    crate::shell::run(request, &self.program, 0, &output, None);
                                filesystem.borrow_mut().child_completed();
                                result
                            })
                            .with_includes(&mut |path| IncludeSearch::default().read(path)),
                    )
                    .map_err(StartupError::expansion)?
                    .expect("decoded assignment");
                if seen.insert(variable.clone()) {
                    seed.commands.push(variable.clone());
                }
                if automatic_export_name(&variable) {
                    exports.insert(variable);
                }
            }
            seed.assignments = parsed.assignments.len();
            seed.includes = next_include;
            database
                .variables
                .set_environment_override(parsed.environment_override);
            warnings.set(parsed.warn_undefined);
            if name == b"GNUMAKEFLAGS" {
                database
                    .variables
                    .define_literal(
                        name,
                        b"",
                        if parsed.environment_override {
                            Origin::EnvironmentOverride
                        } else {
                            Origin::Environment
                        },
                    )
                    .map_err(StartupError::expansion)?;
            }
            parsed_prefix = Some(parsed);
        }
        let mut options = if let Some(parsed) = parsed_prefix {
            Options::with_parsed_inherited(self.args.clone(), parsed, prefix_count, false)
        } else {
            Options::with_inherited_tokens(self.args.clone(), Vec::new(), false)
        }
        .map_err(InvocationError::Options)?;
        for (directory, prepared) in options
            .include_directories
            .iter_mut()
            .zip(include_directories)
        {
            *directory = prepared;
        }
        Ok(Invocation {
            options,
            initial: Some(Box::new(Prepared {
                database,
                filesystem,
                output,
                exports,
                seed,
                plan: Box::new(self.clone()),
            })),
        })
    }
}

pub(super) fn emit_error(program: &[u8], error: InvocationError) {
    if rmake_linux::signals::pending().is_some() {
        return;
    }
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
}
