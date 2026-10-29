// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Owned startup assignment state and ordered replay after Makefile remaking.
use super::*;

pub(super) struct Runtime<'a> {
    pub(super) program: &'a [u8],
    pub(super) level: u32,
    pub(super) output: &'a crate::output::Controller,
    pub(super) filesystem: &'a RefCell<FilesystemRuntime>,
    pub(super) warnings: &'a Cell<bool>,
}

pub(super) struct Snapshot {
    database: Database,
    include_directories: Vec<std::path::PathBuf>,
    inherited_exports: HashSet<Vec<u8>>,
    program: Vec<u8>,
    level: u32,
}

impl Snapshot {
    pub(super) fn capture(
        database: &Database,
        options: &Options,
        exports: &HashSet<Vec<u8>>,
        program: &[u8],
        level: u32,
    ) -> Option<Box<Self>> {
        let effects = options.startup_missing_makeflags_warning
            || options.assignments.iter().any(|bytes| {
                bytes.contains(&b'$')
                    || (bytes.contains(&b'!')
                        && rmake_core::parser::assignment(bytes)
                            .is_some_and(|spec| spec.kind == Assignment::Shell))
            })
            || options
                .include_directories
                .iter()
                .any(|path| path.as_os_str().as_bytes().starts_with(b"~"));
        effects.then(|| {
            Box::new(Self {
                database: database.clone(),
                include_directories: options.include_directories.clone(),
                inherited_exports: exports.clone(),
                program: program.to_vec(),
                level,
            })
        })
    }

    pub(super) fn program(&self, level: u32) -> Vec<u8> {
        let mut result = self.program.clone();
        if level != 0 {
            result.extend_from_slice(format!("[{level}]").as_bytes());
        }
        result
    }

    pub(super) fn replay(
        &self,
        options: &mut Options,
        directory: &[u8],
        restarts: u32,
        prior_level: u32,
        output: &crate::output::Controller,
        filesystem: &RefCell<FilesystemRuntime>,
    ) -> Result<Replayed, StartupError> {
        let mut database = self.database.clone();
        database.variables.set_working_directory(directory);
        database
            .variables
            .define_literal(b"CURDIR", directory, Origin::File)
            .map_err(|e| e.diagnostic())?;
        // A fresh process imports the new restart count before CLI expansion.
        // Keep it recursive/environment until ordinary priority promotes it.
        database.variables.set_environment_override(false);
        if database.variables.get(b"MAKELEVEL").is_some() {
            database
                .variables
                .define(
                    b"MAKELEVEL",
                    prior_level.to_string().as_bytes(),
                    Assignment::Recursive,
                    Origin::Environment,
                )
                .map_err(|e| e.diagnostic())?;
        }
        database
            .variables
            .define(
                b"MAKE_RESTARTS",
                restarts.to_string().as_bytes(),
                Assignment::Recursive,
                Origin::Environment,
            )
            .map_err(|e| e.diagnostic())?;
        database
            .variables
            .set_export_policy(b"MAKE_RESTARTS", Some(false));
        database
            .variables
            .set_environment_override(options.environment_override);
        options.include_directories = self.include_directories.clone();
        let mut exports = self.inherited_exports.clone();
        let warnings = Cell::new(options.warn_undefined);
        if options.startup_missing_makeflags_warning {
            emit_output(
                output,
                &self.program,
                Output::Warning {
                    location: None,
                    message: b"warning: undefined variable 'MAKEFLAGS'",
                },
            );
        }
        let setup = prepare(
            &mut database,
            options,
            &mut exports,
            Runtime {
                program: &self.program,
                level: self.level,
                output,
                filesystem,
                warnings: &warnings,
            },
            Seed::default(),
        )?;
        Ok(Replayed {
            database,
            inherited_exports: exports,
            setup,
        })
    }
}

pub(super) struct Setup {
    pub(super) include_search: IncludeSearch,
    pub(super) command_replay: bool,
    pub(super) makelevel: u32,
    pub(super) initial_flags: Vec<u8>,
}

pub(super) struct Replayed {
    pub(super) database: Database,
    pub(super) inherited_exports: HashSet<Vec<u8>>,
    pub(super) setup: Setup,
}

#[derive(Default)]
pub(super) struct Seed {
    pub(super) commands: Vec<Vec<u8>>,
    pub(super) assignments: usize,
    pub(super) includes: usize,
}

pub(super) fn prepare(
    database: &mut Database,
    options: &mut Options,
    inherited_exports: &mut HashSet<Vec<u8>>,
    runtime: Runtime<'_>,
    seed: Seed,
) -> Result<Setup, StartupError> {
    let mut emit = |event: Output<'_>| emit_output(runtime.output, runtime.program, event);
    let mut startup_output = WarningOutput::new(&mut emit, runtime.warnings);
    database
        .variables
        .set_export_policy(b"MAKEFLAGS", Some(true));
    let mut command_names = seed.commands;
    // Prepared arguments already have unique command names. Build the second
    // membership set only when this phase still has definitions to apply.
    let mut seen_commands: HashSet<_> = if seed.assignments < options.assignments.len() {
        command_names.iter().cloned().collect()
    } else {
        HashSet::new()
    };
    let mut next_include = 0;
    for index in 0..=options.assignments.len() {
        while options.include_assignment_positions.get(next_include) == Some(&index) {
            let directory = &mut options.include_directories[next_include];
            next_include += 1;
            if next_include <= seed.includes {
                continue;
            }
            *directory = crate::makefiles::prepare_argument(
                directory.as_os_str().as_bytes(),
                &mut database.variables,
                runtime_context(None, &mut startup_output)
                    .with_filesystem(&mut |effect| runtime.filesystem.borrow_mut().execute(effect))
                    .with_shell(&mut |request| {
                        let result = crate::shell::run(
                            request,
                            runtime.program,
                            runtime.level,
                            runtime.output,
                            None,
                        );
                        runtime.filesystem.borrow_mut().child_completed();
                        result
                    }),
            )
            .map_err(|error| error.diagnostic())?;
        }
        let Some(arg) = options.assignments.get(index) else {
            break;
        };
        if index < seed.assignments {
            continue;
        }
        let name = database
            .define_argument_with(
                arg,
                Origin::CommandLine,
                runtime_context(None, &mut startup_output)
                    .with_filesystem(&mut |effect| runtime.filesystem.borrow_mut().execute(effect))
                    .with_shell(&mut |request| {
                        let result = crate::shell::run(
                            request,
                            runtime.program,
                            runtime.level,
                            runtime.output,
                            None,
                        );
                        runtime.filesystem.borrow_mut().child_completed();
                        result
                    })
                    .with_includes(&mut |name| IncludeSearch::default().read(name)),
            )
            .map_err(StartupError::expansion)?
            .expect("CLI identified assignment");
        if seen_commands.insert(name.clone()) {
            command_names.push(name.clone());
        }
        if automatic_export_name(&name) {
            inherited_exports.insert(name);
        }
    }
    database
        .variables
        .initialize_shell_default(b"/bin/sh")
        .map_err(|error| error.diagnostic())?;
    if !options.no_variables {
        database
            .initialize_builtin_variables()
            .map_err(|error| error.diagnostic())?;
        database
            .variables
            .define(b"GNUMAKEFLAGS", b"", Assignment::Recursive, Origin::Default)
            .map_err(|error| error.diagnostic())?;
    }
    database
        .variables
        .define_literal(b".DEFAULT_GOAL", b"", Origin::File)
        .map_err(|error| error.diagnostic())?;
    let mut seen_include_arguments = HashSet::new();
    options
        .include_directories
        .retain(|directory| seen_include_arguments.insert(directory.as_os_str().to_owned()));
    let include_search = IncludeSearch::new(&options.include_directories);
    database
        .variables
        .define(
            b"MAKEFLAGS",
            &options.makeflags(),
            Assignment::Recursive,
            runtime_options::origin(options.environment_override),
        )
        .map_err(|error| error.diagnostic())?;
    database
        .variables
        .define_literal(b".INCLUDE_DIRS", b"", Origin::Default)
        .map_err(|error| error.diagnostic())?;
    for directory in include_search.directories() {
        database
            .variables
            .define_with(
                b".INCLUDE_DIRS",
                directory,
                Assignment::Append,
                Origin::Default,
                None,
                runtime_context(None, &mut startup_output)
                    .with_filesystem(&mut |effect| runtime.filesystem.borrow_mut().execute(effect))
                    .with_shell(&mut |request| {
                        let result = crate::shell::run(
                            request,
                            runtime.program,
                            runtime.level,
                            runtime.output,
                            None,
                        );
                        runtime.filesystem.borrow_mut().child_completed();
                        result
                    }),
            )
            .map_err(|error| error.diagnostic())?;
    }
    let command_replay = crate::command_variables::prepare(&mut database.variables, &command_names)
        .map_err(|error| error.diagnostic())?;
    let makelevel = startup_makelevel(
        database
            .variables
            .get(b"MAKELEVEL")
            .map_or(b"".as_slice(), |variable| variable.raw_value()),
    );
    // The special binding exists before reading Makefiles, but only a
    // genuinely imported/CLI binding belongs to inherited_exports. A later
    // file assignment must not acquire ordinary export side effects merely
    // because this synthesized variable has environment origin.
    database
        .variables
        .define_literal(
            b"MAKELEVEL",
            makelevel.to_string().as_bytes(),
            if options.environment_override {
                Origin::EnvironmentOverride
            } else {
                Origin::Environment
            },
        )
        .map_err(|e| e.diagnostic())?;
    let mut initial_flags = options.makeflags();
    if command_replay {
        initial_flags.extend_from_slice(b" -- $(MAKEOVERRIDES)");
        database
            .variables
            .define(
                b"MAKEFLAGS",
                &initial_flags,
                Assignment::Recursive,
                runtime_options::origin(options.environment_override),
            )
            .map_err(|error| error.diagnostic())?;
    }
    Ok(Setup {
        include_search,
        command_replay,
        makelevel,
        initial_flags,
    })
}
