// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Constructor metadata before automatic/default definitions and activation.
use super::*;

pub(super) struct Runtime<'a> {
    pub(super) program: &'a [u8],
    pub(super) level: u32,
    pub(super) output: &'a crate::output::Controller,
    pub(super) filesystem: &'a RefCell<FilesystemRuntime>,
    pub(super) warnings: &'a Cell<bool>,
}

#[derive(Default)]
pub(super) struct Seed {
    /// Unique names already applied by the ordered argument decoder.
    pub(super) commands: Vec<Vec<u8>>,
}

pub(super) fn include_metadata(
    database: &mut Database,
    options: &Options,
    runtime: Runtime<'_>,
) -> Result<IncludeSearch, StartupError> {
    let search = if options
        .include_directories
        .iter()
        .any(|name| name.as_os_str().as_bytes().starts_with(b"~"))
    {
        let mut builder = crate::makefiles::IncludeSearchBuilder::default();
        for index in 0..options.include_directories.len() {
            if rmake_linux::signals::pending().is_some() {
                return Err(StartupError::expansion(ExpandError::Cancelled));
            }
            // HOME callbacks run before flag-assignment activation. Own the
            // stored operand while evaluating its current database binding.
            let name = options.include_directories[index]
                .as_os_str()
                .as_bytes()
                .to_vec();
            if name == b"-" {
                builder.push(&name);
                continue;
            }
            let expanded = crate::makefiles::expand_tilde_with(&name, || {
                let warning = runtime.warnings.replace(false);
                let result = evaluate(database, &runtime, |database, context| {
                    database.expand_with(b"$(HOME)", context)
                });
                runtime.warnings.set(warning);
                result
            })
            .map_err(|error| StartupError::expansion(error).with_program(runtime.program))?;
            builder.push_resolved(&expanded);
        }
        builder.finish()
    } else {
        IncludeSearch::new(&options.include_directories)
    };
    if rmake_linux::signals::pending().is_some() {
        return Err(StartupError::expansion(ExpandError::Cancelled));
    }
    database
        .variables
        .define_literal(b".INCLUDE_DIRS", b"", Origin::Default)
        .map_err(StartupError::expansion)?;
    for directory in search.directories() {
        evaluate(database, &runtime, |database, context| {
            database.define_variable_with(
                rmake_core::parser::AssignmentSpec {
                    name: b".INCLUDE_DIRS",
                    value: directory,
                    kind: Assignment::Append,
                },
                Origin::Default,
                None,
                context,
            )
        })
        .map_err(|error| StartupError::expansion(error).with_program(runtime.program))?;
    }
    Ok(search)
}

fn evaluate<T>(
    database: &mut Database,
    runtime: &Runtime<'_>,
    operation: impl FnOnce(&mut Database, ExpansionContext<'_>) -> Result<T, ExpandError>,
) -> Result<T, ExpandError> {
    let mut emit = |event: Output<'_>| emit_output(runtime.output, runtime.program, event);
    let mut receiver = WarningOutput::new(&mut emit, runtime.warnings);
    operation(
        database,
        runtime_context(None, &mut receiver)
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
}
