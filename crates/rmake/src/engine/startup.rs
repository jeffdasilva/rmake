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
    options: &mut Options,
    runtime: Runtime<'_>,
) -> Result<IncludeSearch, StartupError> {
    // Operand-history multiplicity is a separate decoder follow-up. Preserve
    // the existing list policy while changing the definition/effect phase.
    let mut seen = HashSet::new();
    options
        .include_directories
        .retain(|directory| seen.insert(directory.as_os_str().to_owned()));
    let search = IncludeSearch::new(&options.include_directories);
    if rmake_linux::signals::pending().is_some() {
        return Err(StartupError::expansion(ExpandError::Cancelled));
    }
    database
        .variables
        .define_literal(b".INCLUDE_DIRS", b"", Origin::Default)
        .map_err(StartupError::expansion)?;
    let mut emit = |event: Output<'_>| emit_output(runtime.output, runtime.program, event);
    let mut receiver = WarningOutput::new(&mut emit, runtime.warnings);
    for directory in search.directories() {
        database
            .define_variable_with(
                rmake_core::parser::AssignmentSpec {
                    name: b".INCLUDE_DIRS",
                    value: directory,
                    kind: Assignment::Append,
                },
                Origin::Default,
                None,
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
            .map_err(|error| StartupError::expansion(error).with_program(runtime.program))?;
    }
    Ok(search)
}
