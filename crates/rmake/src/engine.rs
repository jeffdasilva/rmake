// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Linux filesystem and serial process boundary for the initial explicit-rule slice.
//! No persistent cache and no delegation to GNU Make are involved.

use crate::cli::Options;
use crate::recipe_flow::{Cleanup, Execution, Flow, Status};
use rmake_core::diagnostic::{Diagnostic, ExpansionContext, IncludedFile, Output};
use rmake_core::parser::{
    Database, Dependency, Location, Makefile, Recipe, Rule, assignment, trim, words,
};
use rmake_core::variables::{Assignment, ExpandError, Flavor, Origin, VariableStore};
use rmake_linux::filesystem::Runtime as FilesystemRuntime;
use rmake_linux::jobserver::{Jobserver, Permit};
use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::ffi::{OsStr, OsString};
use std::fs::{self, FileTimes, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::Path;
use std::process::Command;
use std::time::SystemTime;

fn os(bytes: &[u8]) -> &OsStr {
    OsStr::from_bytes(bytes)
}

fn joined(items: impl IntoIterator<Item = impl AsRef<[u8]>>) -> Vec<u8> {
    let mut result = Vec::new();
    for item in items {
        if !result.is_empty() {
            result.push(b' ');
        }
        result.extend_from_slice(item.as_ref());
    }
    result
}

fn unique(items: &[Vec<u8>]) -> Vec<Vec<u8>> {
    let mut seen = HashSet::new();
    items
        .iter()
        .filter(|v| seen.insert(v.as_slice()))
        .cloned()
        .collect()
}

fn modified(name: &[u8]) -> Option<SystemTime> {
    fs::metadata(os(name)).and_then(|m| m.modified()).ok()
}

fn location_prefix(location: &Location) -> Vec<u8> {
    let mut bytes = location.file.clone();
    if location.line != 0 {
        bytes.extend_from_slice(format!(":{}", location.line).as_bytes());
    }
    bytes.extend_from_slice(b": ");
    bytes
}

/// GNU exposes these recursive definitions even while reading makefiles and
/// with -R. Their automatic origin also protects them from override/undefine.
/// Keep the formulas, rather than precomputing paths: origin/flavor/value and
/// references to user-defined base variables during parsing are observable.
fn define_automatic_variants(variables: &mut VariableStore) -> Result<(), ExpandError> {
    for base in b"@<^+?*%" {
        for (suffix, prefix, ending) in [
            (
                b'D',
                b"$(patsubst %/,%,$(dir $".as_slice(),
                b"))".as_slice(),
            ),
            (b'F', b"$(notdir $".as_slice(), b")".as_slice()),
        ] {
            let value = [prefix, &[*base], ending].concat();
            variables.define(
                &[*base, suffix],
                &value,
                Assignment::Recursive,
                Origin::Automatic,
            )?;
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Default)]
struct Outcome {
    time: Option<SystemTime>,
    ran: bool,
    failed: bool,
    fatal: bool,
    question_outdated: bool,
    blocked: bool,
}

fn automatic_export_name(name: &[u8]) -> bool {
    name.first()
        .is_some_and(|byte| byte.is_ascii_alphabetic() || *byte == b'_')
        && name
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
}

/// Original probes of the pinned 64-bit Linux GNU profile establish this
/// unsigned conversion, including its first-byte minus guard and overflow.
/// The stored value is read directly: recursive make expressions stay deferred.
fn startup_makelevel(value: &[u8]) -> u32 {
    if value.first() == Some(&b'-') {
        return 0;
    }
    // Rust's ASCII whitespace deliberately excludes vertical tab; GNU's C
    // whitespace includes it, including mixed runs with other whitespace.
    let value = &value[value
        .iter()
        .position(|byte| !matches!(byte, b' ' | b'\t'..=b'\r'))
        .unwrap_or(value.len())..];
    let (negative, digits) = match value.first() {
        Some(b'-') => (true, &value[1..]),
        Some(b'+') => (false, &value[1..]),
        _ => (false, value),
    };
    let mut magnitude = 0_u64;
    let mut overflow = false;
    for &byte in digits.iter().take_while(|byte| byte.is_ascii_digit()) {
        match magnitude
            .checked_mul(10)
            .and_then(|value| value.checked_add(u64::from(byte - b'0')))
        {
            Some(value) => magnitude = value,
            None => {
                magnitude = u64::MAX;
                overflow = true;
            }
        }
    }
    if negative && !overflow {
        magnitude = magnitude.wrapping_neg();
    }
    magnitude as u32
}

/// The only I/O boundary for parser and expansion output. Flush each message
/// before subsequent diagnostics or child execution can write either stream.
fn emit_output(program: &[u8], output: Output<'_>) {
    match output {
        Output::Info(message) => {
            let mut out = io::stdout().lock();
            let _ = out.write_all(message);
            let _ = out.write_all(b"\n");
            let _ = out.flush();
        }
        Output::Warning { location, message } => {
            let mut out = io::stderr().lock();
            if let Some(location) = location {
                let _ = out.write_all(&location_prefix(location));
            } else {
                let _ = out.write_all(program);
                let _ = out.write_all(b": ");
            }
            let _ = out.write_all(message);
            let _ = out.write_all(b"\n");
            let _ = out.flush();
        }
    }
}

struct ParallelJob {
    program: Vec<u8>,
    location: Location,
    ignore: bool,
    before: Option<SystemTime>,
    phony: bool,
    continuation: RecipeState,
}

/// Interpreter-owned continuation. Workers receive only one prepared Command;
/// subsequent command configuration and exports are evaluated after completion.
struct RecipeState {
    expanded: std::collections::VecDeque<(usize, Vec<u8>, Recipe)>,
    variables: VariableStore,
    flow: Flow,
    ran: bool,
    one_shell: bool,
    stdin_owner: bool,
    environment: Option<Vec<(OsString, Option<OsString>)>>,
    permit: Option<Permit>,
}

type JobResult = (Vec<u8>, io::Result<std::process::ExitStatus>);

pub struct Engine {
    options: Options,
    filesystem: RefCell<FilesystemRuntime>,
    database: Database,
    program: Vec<u8>,
    makelevel: u32,
    inherited_exports: HashSet<Vec<u8>>,
    command_replay: bool,
    done: HashMap<Vec<u8>, Outcome>,
    visiting: HashSet<Vec<u8>>,
    scope_parents: HashMap<Vec<u8>, Option<Vec<u8>>>,
    target_contexts: HashMap<Vec<u8>, VariableStore>,
    would_build: bool,
    parallel_jobs: HashMap<Vec<u8>, ParallelJob>,
    completed_jobs: std::sync::mpsc::Receiver<JobResult>,
    job_sender: std::sync::mpsc::Sender<JobResult>,
    parallel_failed: bool,
    remaking: bool,
    vpath_dirs: Vec<Vec<u8>>,
    resolved_names: HashMap<Vec<u8>, Vec<u8>>,
    makefile_error: RefCell<Option<(Location, Vec<u8>)>>,
    initial_flags: Vec<u8>,
    initial_gnu_flags: Vec<u8>,
    intermediates: HashSet<Vec<u8>>,
    created_intermediates: Vec<Vec<u8>>,
    jobserver: Option<Jobserver>,
}

impl Engine {
    pub fn new(
        mut options: Options,
        program: Vec<u8>,
        invocation: &OsStr,
    ) -> Result<Self, Diagnostic> {
        for dir in &options.directories {
            std::env::set_current_dir(dir)
                .map_err(|e| Diagnostic::from(format!("{}: {e}", dir.display())))?;
        }
        let startup_level = startup_makelevel(
            std::env::var_os("MAKELEVEL")
                .as_deref()
                .map_or(b"".as_slice(), OsStrExt::as_bytes),
        );
        let warning_program = if startup_level == 0 {
            program.clone()
        } else {
            [program.as_slice(), format!("[{startup_level}]").as_bytes()].concat()
        };
        if options.reset_jobserver {
            emit_output(
                &warning_program,
                Output::Warning {
                    location: None,
                    message: format!(
                        "warning: -j{} forced in submake: resetting jobserver mode.",
                        options.jobs.filter(|jobs| *jobs != usize::MAX).unwrap_or(0)
                    )
                    .as_bytes(),
                },
            );
        }
        let jobserver = if options.jobs == Some(1) {
            options.jobserver_auth = None;
            None
        } else if let Some(auth) = &options.jobserver_auth {
            match Jobserver::join(auth) {
                Ok(server) => Some(server),
                Err(error) => {
                    if let Some(path) = auth.strip_prefix(b"fifo:") {
                        emit_output(
                            &warning_program,
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
                    emit_output(&warning_program, Output::Warning { location: None,
                        message: b"warning: jobserver unavailable: using -j1.  Add '+' to parent make rule." });
                    options.jobs = Some(1);
                    options.jobserver_auth = None;
                    None
                }
            }
        } else if let Some(jobs) = options.jobs.filter(|jobs| *jobs > 1 && *jobs != usize::MAX) {
            Some(
                Jobserver::create(jobs, options.jobserver_pipe).map_err(|error| {
                    Diagnostic::from(format!("jobserver: {}", os_error(&error)))
                })?,
            )
        } else {
            None
        };
        if let Some(server) = &jobserver {
            options.jobs = Some(usize::MAX);
            options.jobserver_auth = Some(server.auth().to_vec());
        }
        options.inherited_jobs = false;
        let filesystem = RefCell::new(FilesystemRuntime::default());
        let mut database = Database::default();
        database
            .variables
            .set_environment_override(options.environment_override);
        let mut inherited_exports = HashSet::new();
        for (name, value) in std::env::vars_os() {
            let name = name.as_bytes();
            if name == b"SHELL" {
                continue;
            }
            database
                .variables
                .define(
                    name,
                    value.as_bytes(),
                    Assignment::Recursive,
                    Origin::Environment,
                )
                .map_err(|e| e.diagnostic())?;
            // Imported export policy belongs to this binding. Ordinary
            // reassignment retains it; undefine destroys it with the binding.
            database.variables.set_export_policy(name, Some(true));
            inherited_exports.insert(name.to_vec());
        }
        database
            .initialize_suffixes(!options.no_rules)
            .map_err(|error| error.diagnostic())?;
        if !options.no_variables {
            database
                .initialize_builtin_variables()
                .map_err(|error| error.diagnostic())?;
        }
        if !options.no_rules {
            database
                .initialize_builtin_rules()
                .map_err(|error| error.message)?;
        }
        define_automatic_variants(&mut database.variables).map_err(|error| error.diagnostic())?;
        for (name, value) in [
            (b"SHELL".as_slice(), b"/bin/sh".as_slice()),
            (b".SHELLFLAGS", b"-c"),
            (b"MAKE_VERSION", b"4.4.1"),
            (b"MAKE", invocation.as_bytes()),
            (b"MAKE_COMMAND", invocation.as_bytes()),
        ] {
            database
                .variables
                .define_literal(name, value, Origin::Default)
                .map_err(|e| e.diagnostic())?;
        }
        let cwd = std::env::current_dir().map_err(|e| Diagnostic::from(e.to_string()))?;
        database
            .variables
            .define_literal(b"CURDIR", cwd.as_os_str().as_bytes(), Origin::File)
            .map_err(|e| e.diagnostic())?;
        if !options.goals.is_empty() {
            database
                .variables
                .define_literal(b"MAKECMDGOALS", &joined(&options.goals), Origin::Default)
                .map_err(|e| e.diagnostic())?;
        }
        database
            .variables
            .set_working_directory(cwd.as_os_str().as_bytes());
        database.variables.remove(b"MAKEFLAGS");
        database
            .variables
            .define_literal(b"MAKEFLAGS", &options.flags(), Origin::File)
            .map_err(|e| e.diagnostic())?;
        let mut output = |event: Output<'_>| emit_output(&program, event);
        database
            .variables
            .set_export_policy(b"MAKEFLAGS", Some(true));
        let mut command_names = Vec::new();
        let mut seen_commands = HashSet::new();
        for arg in &options.assignments {
            let spec = assignment(arg).expect("CLI identified assignment");
            if [b"MAKEFLAGS".as_slice(), b"GNUMAKEFLAGS"].contains(&trim(spec.name)) {
                return Err("assigning make flags is not implemented yet".into());
            }
            let name = database
                .variables
                .expand_mut_with(
                    spec.name,
                    ExpansionContext::new(None, &mut output)
                        .with_filesystem(&mut |effect| filesystem.borrow_mut().execute(effect))
                        .with_shell(&mut |request| {
                            let result = crate::shell::run(request, &program, startup_level);
                            filesystem.borrow_mut().child_completed();
                            result
                        }),
                )
                .map_err(|e| e.diagnostic())?;
            database
                .variables
                .define_with(
                    &name,
                    spec.value,
                    spec.kind,
                    Origin::CommandLine,
                    None,
                    ExpansionContext::new(None, &mut output)
                        .with_filesystem(&mut |effect| filesystem.borrow_mut().execute(effect))
                        .with_shell(&mut |request| {
                            let result = crate::shell::run(request, &program, startup_level);
                            filesystem.borrow_mut().child_completed();
                            result
                        }),
                )
                .map_err(|e| e.diagnostic())?;
            if seen_commands.insert(name.clone()) {
                command_names.push(name.clone());
            }
            if automatic_export_name(&name) {
                inherited_exports.insert(name);
            }
        }
        let command_replay =
            crate::command_variables::prepare(&mut database.variables, &command_names)
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
        let mut program = program;
        if makelevel != 0 {
            program.extend_from_slice(format!("[{makelevel}]").as_bytes());
        }
        database.variables.remove(b"GNUMAKEFLAGS");
        database
            .variables
            .define_literal(b"GNUMAKEFLAGS", b"", Origin::Override)
            .map_err(|e| e.diagnostic())?;
        let initial_flags = options.flags();
        let (job_sender, completed_jobs) = std::sync::mpsc::channel();
        Ok(Self {
            options,
            filesystem,
            database,
            program,
            makelevel,
            inherited_exports,
            command_replay,
            done: HashMap::new(),
            visiting: HashSet::new(),
            scope_parents: HashMap::new(),
            target_contexts: HashMap::new(),
            would_build: false,
            parallel_jobs: HashMap::new(),
            completed_jobs,
            job_sender,
            parallel_failed: false,
            remaking: false,
            vpath_dirs: Vec::new(),
            resolved_names: HashMap::new(),
            makefile_error: RefCell::new(None),
            initial_flags,
            initial_gnu_flags: Vec::new(),
            intermediates: HashSet::new(),
            created_intermediates: Vec::new(),
            jobserver,
        })
    }

    fn expand(
        &mut self,
        variables: &mut VariableStore,
        bytes: &[u8],
        location: Option<&Location>,
    ) -> Result<Vec<u8>, ExpandError> {
        self.database.expand_in(
            variables,
            bytes,
            ExpansionContext::new(location, &mut |event| emit_output(&self.program, event))
                .with_filesystem(&mut |effect| self.filesystem.borrow_mut().execute(effect))
                .with_shell(&mut |request| {
                    let result = crate::shell::run(request, &self.program, self.makelevel);
                    self.filesystem.borrow_mut().child_completed();
                    result
                })
                .with_includes(&mut read_include),
        )
    }

    fn expand_global(
        &mut self,
        bytes: &[u8],
        location: Option<&Location>,
    ) -> Result<Vec<u8>, ExpandError> {
        self.database.expand_with(
            bytes,
            ExpansionContext::new(location, &mut |event| emit_output(&self.program, event))
                .with_filesystem(&mut |effect| self.filesystem.borrow_mut().execute(effect))
                .with_shell(&mut |request| {
                    let result = crate::shell::run(request, &self.program, self.makelevel);
                    self.filesystem.borrow_mut().child_completed();
                    result
                })
                .with_includes(&mut read_include),
        )
    }

    fn show_makefile_error(&self) {
        if let Some((location, message)) = self.makefile_error.borrow_mut().take() {
            emit_output(
                &self.program,
                Output::Warning {
                    location: Some(&location),
                    message: &message,
                },
            );
        }
    }

    fn stderr(&self, body: &[u8]) {
        let mut out = io::stderr().lock();
        let _ = out.write_all(&self.program);
        let _ = out.write_all(b": ");
        let _ = out.write_all(body);
        let _ = out.write_all(b"\n");
    }

    fn stdout(&self, body: &[u8]) {
        let mut out = io::stdout().lock();
        let _ = out.write_all(body);
        let _ = out.write_all(b"\n");
        let _ = out.flush();
    }

    fn fatal(&self, location: Option<&Location>, message: &[u8]) {
        if let Some(location) = location {
            let mut out = io::stderr().lock();
            let _ = out.write_all(&location_prefix(location));
            let _ = out.write_all(b"*** ");
            let _ = out.write_all(message);
            let _ = out.write_all(b".  Stop.\n");
        } else {
            self.stderr(&[b"*** ".as_slice(), message, b".  Stop."].concat());
        }
    }

    fn expansion_error(&self, fallback: Option<&Location>, error: &ExpandError) {
        self.fatal(
            error.diagnostic_location(fallback),
            error.diagnostic().as_bytes(),
        );
    }

    pub fn run(mut self) -> u8 {
        let mut files = self.options.files.clone();
        if files.is_empty()
            && let Some(file) = ["GNUmakefile", "makefile", "Makefile"]
                .into_iter()
                .find(|file| Path::new(file).is_file())
        {
            files.push(file.into());
        }
        let found_makefile = !files.is_empty();
        // GNU initializes this metadata after command-line assignment expansion.
        // Higher-priority CLI/-e bindings remain intact and reject file appends.
        if let Err(error) =
            self.database
                .variables
                .define_literal(b"MAKEFILE_LIST", b"", Origin::File)
        {
            self.expansion_error(None, &error);
            return 2;
        }
        let initial_database = self.database.clone();
        let mut restarts = 0_u32;
        let mut stdin_bytes = None;
        loop {
            if restarts > 0 {
                self.database = initial_database.clone();
                self.database
                    .variables
                    .define_literal(
                        b"MAKE_RESTARTS",
                        restarts.to_string().as_bytes(),
                        Origin::Environment,
                    )
                    .expect("bounded restart count");
                self.done.clear();
                self.intermediates.clear();
                self.visiting.clear();
                self.scope_parents.clear();
                self.target_contexts.clear();
                self.resolved_names.clear();
            }
            for file in &files {
                let mut bytes = Vec::new();
                let read = if file == Path::new("-") {
                    if let Some(saved) = &stdin_bytes {
                        bytes = Vec::clone(saved);
                        Ok(bytes.len())
                    } else {
                        let result = io::stdin().read_to_end(&mut bytes);
                        stdin_bytes = Some(bytes.clone());
                        result
                    }
                } else {
                    fs::read(file).map(|content| {
                        bytes = content;
                        bytes.len()
                    })
                };
                if let Err(err) = read {
                    emit_output(
                        &self.program,
                        Output::Warning {
                            location: None,
                            message: format!("{}: {}", file.display(), os_error(&err)).as_bytes(),
                        },
                    );
                    self.database.makefiles.push(Makefile {
                        name: file.as_os_str().as_bytes().to_vec(),
                        optional: false,
                        missing: true,
                        read_error: None,
                    });
                    continue;
                }
                if file != Path::new("-") {
                    self.database.makefiles.push(Makefile {
                        name: file.as_os_str().as_bytes().to_vec(),
                        optional: false,
                        missing: false,
                        read_error: None,
                    });
                }
                if let Err(err) = self.database.variables.append_literal_at(
                    b"MAKEFILE_LIST",
                    file.as_os_str().as_bytes(),
                    Origin::File,
                    &Location {
                        file: file.as_os_str().as_bytes().to_vec(),
                        line: 1,
                    },
                ) {
                    self.expansion_error(None, &err);
                    return 2;
                }
                if let Err(err) = self.database.read_with(
                    &bytes,
                    file.as_os_str().as_bytes(),
                    ExpansionContext::new(None, &mut |event| emit_output(&self.program, event))
                        .with_filesystem(&mut |effect| self.filesystem.borrow_mut().execute(effect))
                        .with_shell(&mut |request| {
                            let result = crate::shell::run(request, &self.program, self.makelevel);
                            self.filesystem.borrow_mut().child_completed();
                            result
                        })
                        .with_includes(&mut |name| {
                            fs::read(os(name))
                                .map(|bytes| IncludedFile {
                                    name: name.to_vec(),
                                    bytes,
                                })
                                .map_err(|error| os_error(&error).into_bytes())
                        }),
                ) {
                    self.fatal(err.source_location(), err.message.as_bytes());
                    return 2;
                }
            }
            for (name, initial) in [
                (b"MAKEFLAGS".as_slice(), self.initial_flags.clone()),
                (b"GNUMAKEFLAGS", self.initial_gnu_flags.clone()),
            ] {
                match self.expand_global(&[b"$(".as_slice(), name, b")"].concat(), None) {
                    Ok(current) if current == initial => {}
                    _ => {
                        self.fatal(
                            None,
                            b"rmake: changing make flags in a Makefile is not implemented yet",
                        );
                        return 2;
                    }
                }
            }
            match crate::command_variables::replay(
                &mut self.database.variables,
                self.command_replay,
                if self.options.environment_override {
                    Origin::EnvironmentOverride
                } else {
                    Origin::Environment
                },
                ExpansionContext::new(None, &mut |event| emit_output(&self.program, event))
                    .with_filesystem(&mut |effect| self.filesystem.borrow_mut().execute(effect))
                    .with_shell(&mut |request| {
                        let result = crate::shell::run(request, &self.program, self.makelevel);
                        self.filesystem.borrow_mut().child_completed();
                        result
                    }),
            ) {
                Ok(names) => self.inherited_exports.extend(names),
                Err(error) => {
                    self.expansion_error(None, &error);
                    return 2;
                }
            }
            if self
                .database
                .variables
                .get(b"MAKEOVERRIDES")
                .is_some_and(|value| !value.raw_value().is_empty())
            {
                let raw = [self.options.flags(), b" -- $(MAKEOVERRIDES)".to_vec()].concat();
                if let Err(error) = self.database.variables.define(
                    b"MAKEFLAGS",
                    &raw,
                    Assignment::Recursive,
                    Origin::File,
                ) {
                    self.expansion_error(None, &error);
                    return 2;
                }
            }
            let vpath = match self.expand_global(b"$(VPATH)", None) {
                Ok(value) => value,
                Err(error) => {
                    self.expansion_error(None, &error);
                    return 2;
                }
            };
            if let Err(error) = self.database.finalize_implicit_rules(
                !self.options.no_rules,
                &mut ExpansionContext::new(None, &mut |event| emit_output(&self.program, event)),
            ) {
                self.fatal(error.source_location(), error.message.as_bytes());
                return 2;
            }
            self.vpath_dirs = vpath
                .split(|byte| *byte == b':' || byte.is_ascii_whitespace() || *byte == 0x0b)
                .filter(|path| !path.is_empty())
                .map(Vec::from)
                .collect();
            // Makefiles are ordinary build goals, but -n/-q/-t never suppress their
            // recipes. A fresh database is constructed only when a file changed.
            let modes = (
                self.options.dry_run,
                self.options.question,
                self.options.touch,
                self.options.always,
            );
            self.options.dry_run = false;
            self.options.question = false;
            self.options.touch = false;
            if restarts > 0 {
                self.options.always = false;
            }
            self.remaking = true;
            let mut changed = false;
            let mut failed = false;
            let mut seen = HashSet::new();
            for makefile in self.database.makefiles.clone() {
                if !seen.insert(makefile.name.clone())
                    || self.special(b".PHONY", &makefile.name, false)
                    || self
                        .database
                        .rules
                        .get(&makefile.name)
                        .and_then(|rule| rule.double_colon.as_ref())
                        .is_some_and(|branches| {
                            branches
                                .iter()
                                .any(|rule| rule.dependencies.is_empty() && rule.recipes.is_some())
                        })
                {
                    continue;
                }
                self.resolve_implicit(&makefile.name);
                if !self.database.rules.contains_key(&makefile.name)
                    && (!makefile.missing || makefile.optional)
                {
                    continue;
                }
                let before = modified(&makefile.name);
                *self.makefile_error.borrow_mut() = if makefile.optional {
                    None
                } else {
                    makefile.read_error.map(|(location, error)| {
                        (location, [makefile.name.as_slice(), b": ", &error].concat())
                    })
                };
                let result = self.build(&makefile.name, None, 0);
                self.makefile_error.borrow_mut().take();
                failed |= result.failed && !makefile.optional;
                changed |= !result.failed && modified(&makefile.name) != before;
                if result.fatal {
                    failed = true;
                    break;
                }
            }
            (
                self.options.dry_run,
                self.options.question,
                self.options.touch,
                self.options.always,
            ) = modes;
            self.remaking = false;
            if changed {
                self.remove_intermediates();
                restarts += 1;
                if restarts > 128 {
                    self.fatal(None, b"rmake: makefile restart limit exceeded");
                    return 2;
                }
                continue;
            }
            if failed {
                self.remove_intermediates();
                return 2;
            }
            break;
        }
        // Reject special execution modes until their semantics are implemented,
        // rather than accidentally passing cases under serial defaults.
        for name in [b".POSIX".as_slice(), b".LOW_RESOLUTION_TIME"] {
            if self.database.rules.contains_key(name) {
                self.fatal(
                    None,
                    &[b"rmake: unsupported special target ".as_slice(), name].concat(),
                );
                return 2;
            }
        }
        let goals = if self.options.goals.is_empty() {
            // GNU expands a recursive default goal's raw value here, without
            // entering the .DEFAULT_GOAL binding's definition context. Nested
            // references still carry their own definition locations.
            let binding = self.database.variables.get(b".DEFAULT_GOAL").cloned();
            let goal = match binding {
                None => Ok(Vec::new()),
                Some(variable) if variable.flavor() == Flavor::Simple => {
                    Ok(variable.raw_value().to_vec())
                }
                Some(variable) => self.expand_global(variable.raw_value(), None),
            };
            match goal {
                Ok(goal) if !trim(&goal).is_empty() => {
                    let goals: Vec<_> = words(&goal).map(Vec::from).collect();
                    if goals.len() != 1 {
                        self.fatal(None, b".DEFAULT_GOAL contains more than one target");
                        return 2;
                    }
                    goals
                }
                Ok(_) => {
                    self.fatal(
                        None,
                        if found_makefile {
                            b"No targets"
                        } else {
                            b"No targets specified and no makefile found"
                        },
                    );
                    return 2;
                }
                Err(err) => {
                    self.expansion_error(None, &err);
                    return 2;
                }
            }
        } else {
            self.options.goals.clone()
        };
        let mut failed = false;
        for goal in goals {
            let already_done = self.done.contains_key(&goal);
            if already_done {
                self.wait_target(&goal);
            }
            let outcome = self.build(&goal, None, 0);
            failed |= outcome.failed;
            if outcome.fatal || (outcome.failed && !self.options.keep_going) {
                break;
            }
            if outcome.failed
                && self.options.keep_going
                && outcome.blocked
                && !self.options.question
            {
                self.stderr(
                    &[
                        b"Target '".as_slice(),
                        &goal,
                        b"' not remade because of errors.",
                    ]
                    .concat(),
                );
            } else if !outcome.failed
                && (!outcome.ran || already_done)
                && !self.options.question
                && !self.globally_silent()
            {
                let has_recipe = self.database.rules.get(&goal).is_some_and(|rule| {
                    rule.recipes.is_some()
                        || rule.double_colon.as_ref().is_some_and(|branches| {
                            branches.iter().any(|rule| rule.recipes.is_some())
                        })
                }) && !self.special(b".PHONY", &goal, false);
                let body = if has_recipe {
                    [b"'".as_slice(), &goal, b"' is up to date."].concat()
                } else {
                    [b"Nothing to be done for '".as_slice(), &goal, b"'."].concat()
                };
                self.stdout(&[self.program.as_slice(), b": ", &body].concat());
            }
        }
        self.wait_all_jobs();
        self.remove_intermediates();
        if let Some(error) = self.jobserver.as_ref().and_then(Jobserver::take_error) {
            self.fatal(
                None,
                format!("jobserver token return: {}", os_error(&error)).as_bytes(),
            );
            return 2;
        }
        failed |= self.parallel_failed;
        if failed {
            2
        } else if self.would_build {
            1
        } else {
            0
        }
    }

    fn special(&self, special: &[u8], target: &[u8], empty_means_all: bool) -> bool {
        self.database.rules.get(special).is_some_and(|rule| {
            (empty_means_all && rule.dependencies.is_empty())
                || rule
                    .dependencies
                    .iter()
                    .any(|dependency| dependency.name == target)
        })
    }

    fn intermediate(&self, target: &[u8]) -> bool {
        !self.options.goals.iter().any(|goal| goal == target)
            && !self
                .database
                .matches_special_pattern(b".NOTINTERMEDIATE", target)
            && !self
                .database
                .rules
                .get(b".NOTINTERMEDIATE".as_slice())
                .is_some_and(|rule| rule.dependencies.is_empty())
            && (self.intermediates.contains(target)
                || self.special(b".INTERMEDIATE", target, false)
                || self.special(b".SECONDARY", target, false))
    }

    fn remove_intermediates(&mut self) {
        let mut removed = Vec::new();
        for target in std::mem::take(&mut self.created_intermediates) {
            if removed.contains(&target)
                || self.special(b".SECONDARY", &target, true)
                || self.database.matches_special_pattern(b".PRECIOUS", &target)
                || self.special(b".PHONY", &target, false)
            {
                continue;
            }
            if self.options.question || self.options.touch {
                continue;
            }
            if self.options.dry_run
                || (fs::symlink_metadata(os(&target)).is_ok_and(|m| !m.is_dir())
                    && fs::remove_file(os(&target)).is_ok())
            {
                removed.push(target);
            }
        }
        if !removed.is_empty() && !self.globally_silent() {
            self.stdout(&[b"rm ".as_slice(), &joined(removed)].concat());
        }
    }

    /// Missing intermediate files do not by themselves make a parent stale.
    /// Update their real inputs first, and materialize the chain only if an
    /// input is newer than the parent (or another prerequisite requires it).
    fn check_intermediate(
        &mut self,
        target: &[u8],
        parent: &[u8],
        time: SystemTime,
        depth: usize,
    ) -> Outcome {
        if depth >= 512
            || !self.intermediate(target)
            || modified(target).is_some()
            || self.special(b".PHONY", target, false)
        {
            return self.build(target, Some(parent), depth);
        }
        let Some(rule) = self.database.rules.get(target).cloned() else {
            return self.build(target, Some(parent), depth);
        };
        self.scope_parents
            .entry(target.to_vec())
            .or_insert_with(|| Some(parent.to_vec()));
        if rule.deferred {
            return self.build(target, Some(parent), depth);
        }
        let mut result = Outcome {
            time: Some(time),
            ..Outcome::default()
        };
        let mut stale = self.options.always;
        self.visiting.insert(target.to_vec());
        for dep in &rule.dependencies {
            if dep.name == b".WAIT" {
                continue;
            }
            if self.visiting.contains(&dep.name) {
                continue;
            }
            let child = self.check_intermediate(&dep.name, target, time, depth + 1);
            self.wait_target(&dep.name);
            let child = self.done.get(&dep.name).copied().unwrap_or(child);
            result.ran |= child.ran;
            result.failed |= child.failed;
            result.fatal |= child.fatal;
            result.question_outdated |= child.question_outdated;
            stale |= !dep.order_only && (child.time.is_none() || child.time > Some(time));
            if child.failed && !self.options.keep_going {
                break;
            }
        }
        self.visiting.remove(target);
        if stale && !result.failed {
            self.build(target, Some(parent), depth)
        } else {
            result
        }
    }

    /// An empty .SILENT affects global diagnostics and touch messages, but does
    /// not add -s to MAKEFLAGS or propagate that option to recursive children.
    fn globally_silent(&self) -> bool {
        self.options.silent
            || self
                .database
                .rules
                .get(b".SILENT".as_slice())
                .is_some_and(|rule| rule.dependencies.is_empty())
    }

    fn job_slots(&self) -> usize {
        if self.remaking
            || self.options.inherited_jobs
            || self
                .database
                .rules
                .get(b".NOTPARALLEL".as_slice())
                .is_some_and(|rule| rule.dependencies.is_empty())
        {
            1
        } else {
            self.options.jobs.unwrap_or(1)
        }
    }

    fn reserve_job(&mut self) -> io::Result<Option<Permit>> {
        let Some(server) = self.jobserver.clone() else {
            return Ok(None);
        };
        loop {
            if !self.parallel_jobs.values().any(|job| {
                job.continuation
                    .permit
                    .as_ref()
                    .is_some_and(Permit::is_implicit)
            }) {
                return Ok(Some(Permit::implicit()));
            }
            if let Some(permit) = server.try_acquire()? {
                return Ok(Some(permit));
            }
            // Local completions wake immediately. Tokens returned by another
            // process are checked within 5 ms without changing shared FD flags.
            match self
                .completed_jobs
                .recv_timeout(std::time::Duration::from_millis(5))
            {
                Ok(result) => self.complete_job(result),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(_) => return Err(io::Error::from(io::ErrorKind::BrokenPipe)),
            }
            if self.parallel_failed && !self.options.keep_going {
                return Ok(None);
            }
        }
    }

    fn wait_target(&mut self, target: &[u8]) {
        while self.parallel_jobs.contains_key(target) {
            self.reap_job();
        }
    }

    fn wait_all_jobs(&mut self) {
        while !self.parallel_jobs.is_empty() {
            self.reap_job();
        }
    }

    fn poll_jobs(&mut self) {
        while let Ok(result) = self.completed_jobs.try_recv() {
            self.complete_job(result);
        }
    }

    fn reap_job(&mut self) {
        let result = self
            .completed_jobs
            .recv()
            .expect("a live worker owns its completion sender");
        self.complete_job(result);
    }

    fn complete_job(&mut self, (target, result): JobResult) {
        let mut job = self
            .parallel_jobs
            .remove(&target)
            .expect("reserved parallel target");
        self.filesystem.borrow_mut().child_completed();
        let launched = result.is_ok();
        let error = match result {
            Ok(status) if status.success() => None,
            Ok(status) => Some(status.code().map_or_else(
                || format!("Signal {}", status.signal().unwrap_or(0)),
                |code| format!("Error {code}"),
            )),
            Err(error) => {
                self.stderr(&[job.program.as_slice(), b": ", os_error(&error).as_bytes()].concat());
                Some("Error 127".to_string())
            }
        };
        job.continuation.flow.attempted(launched);
        let mut failed = error.is_some() && !job.ignore;
        if let Some(error) = error
            && (!job.ignore || !self.globally_silent())
        {
            let message = [
                if job.ignore { b"".as_slice() } else { b"*** " },
                b"[",
                &location_prefix(&job.location),
                &target,
                format!("] {error}{}", if job.ignore { " (ignored)" } else { "" }).as_bytes(),
            ]
            .concat();
            self.stderr(&message);
        }
        if failed && launched {
            self.cleanup_target(&target, job.before, job.phony, Cleanup::ChangedOnError);
        }
        let mut outcome = self.done.get(&target).copied().unwrap_or_default();
        if !failed {
            let execution =
                self.continue_recipe(&target, job.continuation, job.phony, job.before, true);
            failed |= matches!(execution.status, Status::Failure | Status::Fatal);
            outcome.fatal |= execution.status == Status::Fatal;
            self.cleanup_target(&target, job.before, job.phony, execution.cleanup);
        }
        if failed
            && !self.options.keep_going
            && !self.parallel_failed
            && !self.parallel_jobs.is_empty()
        {
            self.stderr(b"*** Waiting for unfinished jobs....");
        }
        self.parallel_failed |= failed;
        outcome.ran = true;
        outcome.failed |= failed;
        outcome.time = if job.phony { None } else { modified(&target) };
        self.done.insert(target, outcome);
    }

    fn find_vpath(&self, target: &[u8]) -> Option<(Option<Vec<u8>>, SystemTime)> {
        if let Some(time) = modified(target) {
            return Some((None, time));
        }
        self.database
            .vpath_directories(target)
            .into_iter()
            .chain(self.vpath_dirs.iter().cloned())
            .find_map(|directory| {
                let path = Path::new(os(&directory)).join(os(target));
                let name = path.as_os_str().as_bytes();
                modified(name).map(|time| (Some(name.to_vec()), time))
            })
    }

    fn resolve_implicit(&mut self, target: &[u8]) {
        let existing = self.database.rules.get(target).cloned();
        if existing
            .as_ref()
            .is_some_and(|rule| rule.recipes.is_some() || rule.double_colon.is_some())
        {
            return;
        }
        let mut budget = 100_000;
        let mut plan = self.implicit_plan(target, &mut Vec::new(), false, &mut budget);
        if plan.is_none() {
            budget = 100_000;
            plan = self.implicit_plan(target, &mut Vec::new(), true, &mut budget);
        }
        if let Some(plan) = plan {
            for (name, mut rule) in plan {
                if let Some(explicit) = self.database.rules.get(&name) {
                    rule.dependencies.extend(explicit.dependencies.clone());
                    rule.deferred |= explicit.deferred;
                }
                if name != target
                    && !self.database.explicit_names.contains(&name)
                    && !self.options.goals.contains(&name)
                {
                    self.intermediates.insert(name.clone());
                }
                self.database.rules.insert(name, rule);
            }
        }
    }

    /// Search is transactional: a rejected branch never installs rules or marks
    /// files intermediate. Rule identities, rather than names alone, prevent
    /// unbounded chains such as %.x: %.x.x.
    fn implicit_plan(
        &self,
        target: &[u8],
        active: &mut Vec<usize>,
        compatibility: bool,
        budget: &mut usize,
    ) -> Option<Vec<(Vec<u8>, Rule)>> {
        if active.len() >= 128 || *budget == 0 {
            return None;
        }
        let candidates = self.database.implicit_candidates_excluding(target, active);
        for chain in [false, true] {
            for candidate in &candidates {
                if *budget == 0 {
                    return None;
                }
                *budget -= 1;
                if active.contains(&candidate.identity)
                    || (chain && candidate.rule.terminal)
                    || (!active.is_empty() && candidate.match_anything && !candidate.rule.terminal)
                {
                    continue;
                }
                let mut plan = Vec::new();
                let mut valid = true;
                active.push(candidate.identity);
                for dependency in &candidate.rule.dependencies {
                    let name = &dependency.name;
                    if name == b".WAIT" {
                        continue;
                    }
                    let explicit = self.database.rules.contains_key(name)
                        || self.database.rules.get(target).is_some_and(|rule| {
                            rule.dependencies.iter().any(|dep| dep.name == *name)
                        });
                    if explicit
                        || self.find_vpath(name).is_some()
                        || (compatibility && self.database.explicit_names.contains(name))
                    {
                        continue;
                    }
                    if chain
                        && let Some(child) = self.implicit_plan(name, active, compatibility, budget)
                    {
                        plan.extend(child);
                    } else {
                        valid = false;
                        break;
                    }
                }
                active.pop();
                if valid {
                    plan.push((target.to_vec(), candidate.rule.clone()));
                    return Some(plan);
                }
            }
        }
        None
    }

    fn expand_dependencies(&mut self, target: &[u8], rule: &mut Rule) -> Result<(), ExpandError> {
        if !rule.deferred {
            return Ok(());
        }
        let mut variables = self.target_variables(target)?;
        let mut dependencies: Vec<Dependency> = Vec::new();
        let cut = rule.recipe_dependency_count.min(rule.dependencies.len());
        let mut prior_count = 0;
        for (index, occurrence) in rule.dependencies[cut..]
            .iter()
            .chain(&rule.dependencies[..cut])
            .enumerate()
        {
            if index == rule.dependencies.len() - cut {
                prior_count = dependencies.len();
            }
            let normal: Vec<_> = dependencies
                .iter()
                .filter(|d| !d.order_only && d.name != b".WAIT")
                .map(|d| d.name.clone())
                .collect();
            let order: Vec<_> = dependencies
                .iter()
                .filter(|d| d.order_only && d.name != b".WAIT")
                .map(|d| d.name.clone())
                .collect();
            for (name, value) in [
                (b"@".as_slice(), target.to_vec()),
                (b"<", normal.first().cloned().unwrap_or_default()),
                (b"^", joined(unique(&normal))),
                (b"+", joined(&normal)),
                (b"|", joined(unique(&order))),
                (b"?", Vec::new()),
                (b"*", rule.stem.clone().unwrap_or_default()),
            ] {
                variables.define_literal(name, &value, Origin::Automatic)?;
            }
            let value = self.expand(
                &mut variables,
                &occurrence.name,
                rule.prerequisite_location.as_ref(),
            )?;
            let mut order_only = occurrence.order_only;
            for name in words(&value) {
                if name == b"|" {
                    order_only = true;
                } else {
                    dependencies.push(Dependency {
                        name: name.to_vec(),
                        order_only,
                    });
                }
            }
        }
        if cut > 0 {
            dependencies.rotate_left(prior_count);
        }
        rule.dependencies = dependencies;
        rule.deferred = false;
        Ok(())
    }

    fn resolved_name(&self, name: &[u8]) -> Vec<u8> {
        self.resolved_names
            .get(name)
            .cloned()
            .unwrap_or_else(|| name.to_vec())
    }

    fn build(&mut self, target: &[u8], parent: Option<&[u8]>, depth: usize) -> Outcome {
        self.poll_jobs();
        if let Some(outcome) = self.done.get(target) {
            return *outcome;
        }
        if depth >= 512 {
            self.fatal(None, b"rmake: dependency nesting limit exceeded");
            return Outcome {
                failed: true,
                fatal: true,
                ..Outcome::default()
            };
        }
        let phony = self.special(b".PHONY", target, false);
        if !phony {
            self.resolve_implicit(target);
        }
        let resolved = if phony { None } else { self.find_vpath(target) };
        let before = resolved.as_ref().map(|(_, time)| *time);
        if let Some((Some(resolved), _)) = resolved {
            self.resolved_names.insert(target.to_vec(), resolved);
        }
        let Some(mut rule) = self
            .database
            .rules
            .get(target)
            .cloned()
            .or_else(|| phony.then(Rule::default))
        else {
            if before.is_some() {
                let result = Outcome {
                    time: before,
                    ..Outcome::default()
                };
                self.done.insert(target.to_vec(), result);
                return result;
            }
            let mut message = [b"*** No rule to make target '".as_slice(), target, b"'"].concat();
            if let Some(parent) = parent {
                message.extend_from_slice(&[b", needed by '".as_slice(), parent, b"'"].concat());
            }
            message.extend_from_slice(if self.options.keep_going {
                b"."
            } else {
                b".  Stop."
            });
            self.show_makefile_error();
            self.stderr(&message);
            let result = Outcome {
                failed: true,
                ..Outcome::default()
            };
            self.done.insert(target.to_vec(), result);
            return result;
        };
        if let Some(branches) = rule.double_colon.take() {
            let mut combined = Outcome {
                time: before,
                ..Outcome::default()
            };
            for branch in branches {
                let mut result =
                    self.build_rule(target, parent, depth, before, phony, branch, true);
                self.wait_target(target);
                if let Some(completed) = self.done.get(target) {
                    result = *completed;
                }
                combined.time = result.time;
                combined.ran |= result.ran;
                combined.failed |= result.failed;
                combined.fatal |= result.fatal;
                combined.blocked |= result.blocked;
                combined.question_outdated |= result.question_outdated;
                self.done.remove(target);
                if result.fatal || (result.failed && !self.options.keep_going) {
                    break;
                }
            }
            self.done.insert(target.to_vec(), combined);
            return combined;
        }
        self.build_rule(target, parent, depth, before, phony, rule, false)
    }

    #[allow(clippy::too_many_arguments)]
    fn build_rule(
        &mut self,
        target: &[u8],
        parent: Option<&[u8]>,
        depth: usize,
        before: Option<SystemTime>,
        phony: bool,
        mut rule: Rule,
        double_colon: bool,
    ) -> Outcome {
        // Scope activation is lazy: an up-to-date or empty-recipe target must
        // not evaluate deferred pattern assignments. Record first ownership so
        // a descendant's recipe can later initialize the ancestor contexts.
        self.scope_parents
            .entry(target.to_vec())
            .or_insert_with(|| parent.map(Vec::from));
        if let Err(error) = self.expand_dependencies(target, &mut rule) {
            self.expansion_error(rule.prerequisite_location.as_ref(), &error);
            let result = Outcome {
                failed: true,
                fatal: true,
                ..Outcome::default()
            };
            self.done.insert(target.to_vec(), result);
            return result;
        }
        self.visiting.insert(target.to_vec());
        let canonical = rule.canonical_dependencies();
        let normal: Vec<_> = canonical
            .iter()
            .filter(|d| !d.order_only)
            .map(|d| d.name.clone())
            .collect();
        let order: Vec<_> = canonical
            .iter()
            .filter(|d| d.order_only)
            .map(|d| d.name.clone())
            .collect();
        let normal_names: HashSet<_> = normal.iter().map(Vec::as_slice).collect();
        let mut newer = Vec::new();
        let mut dropped = HashSet::new();
        let mut deferred_intermediates = Vec::new();
        let mut stale = before.is_none()
            || phony
            || self.options.always
            || (double_colon && rule.dependencies.is_empty());
        let mut outcome = Outcome {
            time: before,
            ..Outcome::default()
        };
        // Visit occurrences in the merged declaration order. Completed nodes
        // are memoized, but every circular occurrence must still be diagnosed.
        for (index, occurrence) in rule.dependencies.iter().enumerate() {
            let dependency = &occurrence.name;
            if dependency == b".WAIT" {
                for prior in &rule.dependencies[..index] {
                    self.wait_target(&prior.name);
                    if let Some(child) = self.done.get(&prior.name) {
                        outcome.ran |= child.ran;
                        outcome.failed |= child.failed;
                        outcome.fatal |= child.fatal;
                        outcome.blocked |= child.failed;
                        outcome.question_outdated |= child.question_outdated;
                    }
                }
                if outcome.fatal || (outcome.failed && !self.options.keep_going) {
                    break;
                }
                continue;
            }
            if self.visiting.contains(dependency) {
                self.stderr(
                    &[
                        b"Circular ".as_slice(),
                        target,
                        b" <- ",
                        dependency,
                        b" dependency dropped.",
                    ]
                    .concat(),
                );
                dropped.insert(dependency.clone());
                continue;
            }
            let mut child =
                if !stale && self.intermediate(dependency) && modified(dependency).is_none() {
                    let child =
                        self.check_intermediate(dependency, target, before.unwrap(), depth + 1);
                    if !self.done.contains_key(dependency) {
                        deferred_intermediates.push(dependency.clone());
                    }
                    if !occurrence.order_only && (child.time.is_none() || child.time > before) {
                        stale = true;
                    }
                    child
                } else {
                    self.build(dependency, Some(target), depth + 1)
                };
            if self.special(b".NOTPARALLEL", target, false) {
                self.wait_target(dependency);
                if let Some(completed) = self.done.get(dependency) {
                    child = *completed;
                }
            }
            outcome.ran |= child.ran;
            outcome.failed |= child.failed;
            outcome.fatal |= child.fatal;
            outcome.blocked |= child.failed;
            outcome.question_outdated |= child.question_outdated;
            if child.fatal
                || ((child.failed || child.question_outdated) && !self.options.keep_going)
            {
                break;
            }
        }
        for occurrence in &rule.dependencies {
            let dependency = &occurrence.name;
            if dependency == b".WAIT" || dropped.contains(dependency) {
                continue;
            }
            self.wait_target(dependency);
            let Some(child) = self.done.get(dependency).copied() else {
                continue;
            };
            outcome.ran |= child.ran;
            outcome.failed |= child.failed;
            outcome.fatal |= child.fatal;
            outcome.blocked |= child.failed;
            outcome.question_outdated |= child.question_outdated;
            let order_only = !normal_names.contains(dependency.as_slice());
            if !order_only
                && (self.options.always
                    || before.is_none()
                    || child.time.is_none()
                    || child.time > before
                    || (self.options.dry_run && child.ran))
            {
                stale = true;
                newer.push(dependency.clone());
            }
        }
        if stale && !outcome.failed && !outcome.question_outdated {
            for dependency in deferred_intermediates {
                let child = self.build(&dependency, Some(target), depth + 1);
                self.wait_target(&dependency);
                let child = self.done.get(&dependency).copied().unwrap_or(child);
                outcome.ran |= child.ran;
                outcome.failed |= child.failed;
                outcome.fatal |= child.fatal;
                outcome.blocked |= child.failed;
                outcome.question_outdated |= child.question_outdated;
            }
        }
        if stale && !outcome.failed && !outcome.question_outdated {
            if let Some(recipes) = &rule.recipes {
                let normal: Vec<_> = normal
                    .iter()
                    .filter(|name| !dropped.contains(*name))
                    .cloned()
                    .collect();
                let order: Vec<_> = order
                    .iter()
                    .filter(|name| !dropped.contains(*name))
                    .cloned()
                    .collect();
                let repeated: Vec<_> = rule
                    .normal_prerequisites()
                    .filter(|name| !dropped.contains(*name))
                    .map(Vec::from)
                    .collect();
                self.resolved_names.remove(target);
                let normal: Vec<_> = normal.iter().map(|name| self.resolved_name(name)).collect();
                let order: Vec<_> = order.iter().map(|name| self.resolved_name(name)).collect();
                let repeated: Vec<_> = repeated
                    .iter()
                    .map(|name| self.resolved_name(name))
                    .collect();
                let newer: Vec<_> = newer.iter().map(|name| self.resolved_name(name)).collect();
                let autos = [
                    (b"@".as_slice(), target.to_vec()),
                    (b"<", repeated.first().cloned().unwrap_or_default()),
                    (b"^", joined(&normal)),
                    (b"+", joined(&repeated)),
                    (b"?", joined(unique(&newer))),
                    (b"|", joined(&order)),
                    (
                        b"*",
                        rule.stem
                            .clone()
                            .unwrap_or_else(|| self.database.explicit_stem(target).to_vec()),
                    ),
                    (b"%", Vec::new()),
                ];
                let execution = self.execute(target, recipes, &autos, phony, before);
                outcome.ran |= execution.ran;
                outcome.failed |= matches!(execution.status, Status::Failure | Status::Fatal);
                outcome.fatal |= execution.status == Status::Fatal;
                outcome.question_outdated |= execution.status == Status::Question;
                if execution.ran && self.intermediate(target) && before.is_none() {
                    self.created_intermediates.push(target.to_vec());
                }
                self.cleanup_target(target, before, phony, execution.cleanup);
            }
            outcome.time = if phony {
                None
            } else if rule.recipes.is_some() {
                modified(target)
            } else {
                before
            };
        }
        self.visiting.remove(target);
        self.done.insert(target.to_vec(), outcome);
        outcome
    }

    /// Initialize each target once, from its first parent outwards. Pattern
    /// conditionals can mutate globals, so cached contexts refresh that layer
    /// before reuse without replaying deferred assignment effects.
    fn target_variables(&mut self, target: &[u8]) -> Result<VariableStore, ExpandError> {
        if let Some(variables) = self.target_contexts.get(target) {
            let mut variables = variables.clone();
            variables.refresh_globals(&self.database.variables);
            return Ok(variables);
        }
        let parent = self.scope_parents.get(target).cloned().flatten();
        let inherited = if let Some(parent) = parent {
            self.target_variables(&parent)?
        } else {
            self.database.variables.clone()
        };
        let variables = self.database.variables_for_target_with(
            target,
            &inherited,
            ExpansionContext::new(None, &mut |event| emit_output(&self.program, event))
                .with_filesystem(&mut |effect| self.filesystem.borrow_mut().execute(effect))
                .with_shell(&mut |request| {
                    let result = crate::shell::run(request, &self.program, self.makelevel);
                    self.filesystem.borrow_mut().child_completed();
                    result
                }),
        )?;
        self.target_contexts
            .insert(target.to_vec(), variables.clone());
        Ok(variables)
    }

    fn execute(
        &mut self,
        target: &[u8],
        recipes: &[Recipe],
        automatic: &[(&[u8], Vec<u8>)],
        phony: bool,
        before: Option<SystemTime>,
    ) -> Execution {
        let one_shell = self.database.rules.contains_key(b".ONESHELL".as_slice());
        let combined;
        let (recipes, has_commands, recursive) = if one_shell {
            let Some(first) = recipes.first() else {
                return Execution::success(false);
            };
            let script =
                crate::oneshell::collect(recipes.iter().map(|recipe| recipe.bytes.as_slice()));
            combined = [Recipe {
                bytes: script.bytes,
                location: first.location.clone(),
                prefix: first.prefix,
            }];
            (combined.as_slice(), script.has_commands, script.recursive)
        } else {
            let has_commands = recipes.iter().any(|recipe| {
                recipe
                    .bytes
                    .iter()
                    .any(|byte| !matches!(byte, b' ' | b'\t' | b'\n' | b'@' | b'-' | b'+'))
            });
            let recursive = recipes.iter().any(|recipe| {
                has_force_prefix(&recipe.bytes)
                    || recipe.bytes.windows(7).any(|s| s == b"$(MAKE)")
                    || recipe.bytes.windows(7).any(|s| s == b"${MAKE}")
            });
            (recipes, has_commands, recursive)
        };
        let flow = Flow::new(
            recipes
                .iter()
                .map(|recipe| {
                    has_force_prefix(&recipe.bytes)
                        || recipe
                            .bytes
                            .windows(7)
                            .any(|part| part == b"$(MAKE)" || part == b"${MAKE}")
                })
                .collect(),
        );
        if self.options.touch && !recursive {
            return if phony {
                Execution::success(false)
            } else {
                self.touch_target(target)
            };
        }
        if !has_commands {
            return if self.options.touch && !phony && flow.has_ordinary() {
                self.touch_target(target)
            } else {
                Execution::success(false)
            };
        }
        let parallel = self.job_slots() > 1
            && !self.options.dry_run
            && !self.options.question
            && !self.options.touch;
        if parallel {
            while self.parallel_jobs.len() >= self.job_slots() {
                self.reap_job();
            }
        } else {
            self.wait_all_jobs();
        }
        if self.parallel_failed && !self.options.keep_going {
            return Execution::failure(false, Cleanup::None);
        }
        let permit = if parallel {
            match self.reserve_job() {
                Ok(permit) => permit,
                Err(error) => {
                    self.fatal(None, format!("jobserver: {}", os_error(&error)).as_bytes());
                    return Execution::fatal(false);
                }
            }
        } else {
            None
        };
        if self.parallel_failed && !self.options.keep_going {
            return Execution::failure(false, Cleanup::None);
        }
        let mut variables = match self.target_variables(target) {
            Ok(variables) => variables,
            Err(error) => {
                self.expansion_error(None, &error);
                return Execution::fatal(false);
            }
        };
        for (name, value) in automatic {
            if let Err(error) = variables.define_literal(name, value, Origin::Automatic) {
                self.expansion_error(None, &error);
                return Execution::fatal(false);
            }
        }
        let mut expanded = std::collections::VecDeque::new();
        for (index, recipe) in recipes.iter().enumerate() {
            match self.expand(&mut variables, &recipe.bytes, Some(&recipe.location)) {
                Ok(bytes) => expanded.push_back((index, bytes, recipe.clone())),
                Err(err) => {
                    self.expansion_error(Some(&recipe.location), &err);
                    return Execution::fatal(false);
                }
            }
        }
        self.continue_recipe(
            target,
            RecipeState {
                expanded,
                variables,
                flow,
                ran: false,
                one_shell,
                stdin_owner: !self
                    .parallel_jobs
                    .values()
                    .any(|job| job.continuation.stdin_owner),
                environment: None,
                permit,
            },
            phony,
            before,
            parallel,
        )
    }

    fn continue_recipe(
        &mut self,
        target: &[u8],
        state: RecipeState,
        phony: bool,
        before: Option<SystemTime>,
        parallel: bool,
    ) -> Execution {
        let RecipeState {
            mut expanded,
            mut variables,
            mut flow,
            mut ran,
            one_shell,
            stdin_owner,
            mut environment,
            permit,
        } = state;
        let variables = &mut variables;
        while let Some((index, bytes, recipe)) = expanded.pop_front() {
            if bytes.is_empty() {
                continue;
            }
            let mut command = trim_leading(&bytes);
            let raw_flags = crate::oneshell::prefixes(&recipe.bytes).1;
            let mut quiet = self.globally_silent()
                || self.special(b".SILENT", target, false)
                || raw_flags.quiet;
            let mut ignore = self.options.ignore_errors
                || self.special(b".IGNORE", target, true)
                || raw_flags.ignore_errors;
            let mut force = flow.recursive(index);
            while let Some(prefix) = command.first() {
                match prefix {
                    b'@' => quiet = true,
                    b'-' => ignore = true,
                    b'+' => force = true,
                    _ => break,
                }
                command = trim_leading(&command[1..]);
            }
            flow.visit(index, force);
            // GNU expands configuration before a dry-run/query decision, after
            // all recipe bodies have expanded. A nonempty expansion consisting
            // only of blanks/modifiers still reaches configuration; a zero-byte
            // expansion does not. Only the unforced touch shortcut skips expansion.
            let shell = match self.expand(variables, b"$(SHELL)", Some(&recipe.location)) {
                Ok(shell) => shell,
                Err(err) => {
                    self.expansion_error(Some(&recipe.location), &err);
                    return Execution::fatal(ran);
                }
            };
            let shell = shell.as_slice();
            // GNU rescans the raw flag text even for a simple binding and
            // attributes direct errors to the recipe, not .SHELLFLAGS itself.
            let flag_text = variables
                .get(b".SHELLFLAGS")
                .map_or_else(Vec::new, |variable| variable.raw_value().to_vec());
            let flags = match self.expand(variables, &flag_text, Some(&recipe.location)) {
                Ok(flags) => flags,
                Err(err) => {
                    self.expansion_error(Some(&recipe.location), &err);
                    return Execution::fatal(ran);
                }
            };
            let ifs = if variables.get(b"IFS").is_some() {
                match self.expand(variables, b"$(IFS)", Some(&recipe.location)) {
                    Ok(value) => Some(value),
                    Err(err) => {
                        self.expansion_error(Some(&recipe.location), &err);
                        return Execution::fatal(ran);
                    }
                }
            } else {
                None
            };
            if command.is_empty() {
                continue;
            }
            let transformed;
            let ordinary_invocation;
            if one_shell {
                transformed = crate::oneshell::transform(command, shell, recipe.prefix);
                command = &transformed;
                ordinary_invocation = None;
            } else {
                let line = crate::process::prepare_line(command, shell, &flags, ifs.as_deref());
                if let Some(next) = line.next {
                    expanded.push_front((index, command[next..].to_vec(), recipe.clone()));
                }
                command = &command[..line.end];
                ordinary_invocation = Some(line.invocation);
                if command.is_empty() {
                    continue;
                }
            }
            if self.options.question && !force {
                self.would_build = true;
                return Execution::question(ran, flow.continuation_cleanup());
            }
            if self.options.touch && !force {
                continue;
            }
            if self.options.dry_run || !quiet {
                self.stdout(command);
            }
            ran = true;
            if self.options.dry_run && !force {
                continue;
            }
            let invocation = if one_shell {
                match crate::oneshell::prepare(command, shell, &flags, ifs.as_deref()) {
                    Ok(invocation) => invocation,
                    Err(crate::oneshell::PrepareError::ShellFlagSyntax) => {
                        self.fatal(
                            Some(&recipe.location),
                            b"rmake: shell syntax in .ONESHELL .SHELLFLAGS is not implemented yet",
                        );
                        return Execution::failure(ran, Cleanup::None);
                    }
                    Err(crate::oneshell::PrepareError::QuotedMultilineDirect) => {
                        self.fatal(
                            Some(&recipe.location),
                            b"rmake: .ONESHELL direct commands with quoted newlines are not implemented yet",
                        );
                        return Execution::failure(ran, Cleanup::None);
                    }
                }
            } else {
                ordinary_invocation.expect("ordinary commands are prepared with their boundary")
            };
            if crate::recipe_flow::optimized_noop(&invocation) {
                continue;
            }
            let (program, arguments) = match invocation {
                crate::process::Invocation::Direct { program, args }
                | crate::process::Invocation::Shell { program, args } => (program, args),
            };
            let mut child = Command::new(os(&program));
            child.args(arguments.iter().map(|arg| os(arg)));
            if let Some(values) = &environment {
                for (name, value) in values {
                    if let Some(value) = value {
                        child.env(name, value);
                    } else {
                        child.env_remove(name);
                    }
                }
            } else {
                // Environment selection skips private pattern bindings, but inner
                // references still expand using the complete target context.
                let environment_variables = variables.for_environment();
                let mut exports: BTreeSet<Vec<u8>> = self
                    .inherited_exports
                    .iter()
                    .chain(self.database.exports.iter())
                    .chain(self.database.unexports.iter())
                    .cloned()
                    .collect();
                let export_all = self.database.export_all
                    || self
                        .database
                        .rules
                        .contains_key(b".EXPORT_ALL_VARIABLES".as_slice());
                for (name, variable) in environment_variables.definitions() {
                    if environment_variables.resolve_export_policy(name).is_some()
                        || (export_all
                            && name != b"SHELL"
                            && !matches!(variable.origin(), Origin::Default | Origin::Automatic)
                            && automatic_export_name(name))
                    {
                        exports.insert(name.to_vec());
                    }
                }
                for name in &exports {
                    if !environment_variables
                        .resolve_export_policy(name)
                        .unwrap_or_else(|| {
                            let default_export = || {
                                automatic_export_name(name)
                                    && environment_variables.get(name).is_some_and(|variable| {
                                        !matches!(
                                            variable.origin(),
                                            Origin::Default | Origin::Automatic
                                        ) && (export_all
                                            || matches!(
                                                variable.origin(),
                                                Origin::Environment
                                                    | Origin::EnvironmentOverride
                                                    | Origin::CommandLine
                                            ))
                                    })
                            };
                            if self
                                .database
                                .variables
                                .get(name)
                                .is_some_and(|variable| variable.export_default_reset())
                            {
                                // A pattern conditional can explicitly reset an
                                // imported name's policy. Re-evaluate default
                                // eligibility from the selected binding instead
                                // of retaining the startup name-list decision.
                                return default_export();
                            }
                            if self
                                .database
                                .variables
                                .get(name)
                                .is_some_and(|variable| variable.is_private())
                            {
                                // A suppressed private conditional can mark the
                                // global binding private. Its export policy is then
                                // hidden too; export-all still applies independently
                                // to an unrelated visible target binding.
                                export_all
                                    && automatic_export_name(name)
                                    && environment_variables.get(name).is_some_and(|variable| {
                                        !matches!(
                                            variable.origin(),
                                            Origin::Default | Origin::Automatic
                                        )
                                    })
                            } else {
                                !self.database.unexports.contains(name) && default_export()
                            }
                        })
                    {
                        child.env_remove(os(name));
                        continue;
                    }
                    if let Some(variable) = environment_variables.get(name) {
                        // Imported environment values are recursive when referenced
                        // by make syntax, but are passed back to children verbatim.
                        if matches!(
                            variable.origin(),
                            Origin::Environment | Origin::EnvironmentOverride
                        ) {
                            child.env(os(name), OsString::from_vec(variable.raw_value().to_vec()));
                            continue;
                        }
                        match self.database.expand_environment_in(
                            variables,
                            name,
                            ExpansionContext::new(variable.definition(), &mut |event| {
                                emit_output(&self.program, event)
                            })
                            .with_filesystem(&mut |effect| {
                                self.filesystem.borrow_mut().execute(effect)
                            })
                            .with_shell(&mut |request| {
                                let result =
                                    crate::shell::run(request, &self.program, self.makelevel);
                                self.filesystem.borrow_mut().child_completed();
                                result
                            })
                            .with_includes(&mut read_include),
                        ) {
                            Ok(value) => {
                                child.env(os(name), OsString::from_vec(value));
                            }
                            Err(err) => {
                                self.expansion_error(variable.definition(), &err);
                                return Execution::fatal(ran);
                            }
                        }
                    } else {
                        child.env_remove(os(name));
                    }
                }
                if !environment_variables
                    .resolve_export_policy(b"MAKEFLAGS")
                    .unwrap_or_else(|| !self.database.unexports.contains(b"MAKEFLAGS".as_slice()))
                {
                    child.env_remove("MAKEFLAGS");
                } else if !child
                    .get_envs()
                    .any(|(name, value)| name == "MAKEFLAGS" && value.is_some())
                {
                    match self.expand(variables, b"$(MAKEFLAGS)", Some(&recipe.location)) {
                        Ok(flags) => {
                            child.env("MAKEFLAGS", OsString::from_vec(flags));
                        }
                        Err(error) => {
                            self.expansion_error(Some(&recipe.location), &error);
                            return Execution::fatal(ran);
                        }
                    }
                }
                // Ordinary export expansion above still has its normal effects.
                // The child's numeric depth always comes from the startup snapshot,
                // even if the visible binding was changed, removed, or unexported.
                child.env("MAKELEVEL", self.makelevel.wrapping_add(1).to_string());
                environment = Some(
                    child
                        .get_envs()
                        .map(|(name, value)| (name.to_owned(), value.map(OsStr::to_owned)))
                        .collect(),
                );
            }
            let _ = io::stdout().flush();
            if parallel {
                let sender = self.job_sender.clone();
                let name = target.to_vec();
                let executable = program.clone();
                let first = stdin_owner;
                let jobserver = self.jobserver.clone();
                let worker =
                    std::thread::Builder::new()
                        .name("rmake-job".into())
                        .spawn(move || {
                            let result = launch_child(&executable, &child, |mut command| {
                                if let Some(server) = &jobserver {
                                    server.configure_child(&mut command, force);
                                }
                                if !first {
                                    command.stdin(std::process::Stdio::null());
                                }
                                command.status()
                            });
                            // Completion must imply that the worker has released
                            // its jobserver owner, so process exit cannot race
                            // the last FIFO unlink against thread teardown.
                            drop(jobserver);
                            let _ = sender.send((name, result));
                        });
                if let Err(error) = worker {
                    self.stderr(os_error(&error).as_bytes());
                    return Execution::failure(ran, Cleanup::None);
                }
                self.parallel_jobs.insert(
                    target.to_vec(),
                    ParallelJob {
                        program,
                        location: recipe.location.clone(),
                        ignore,
                        before,
                        phony,
                        continuation: RecipeState {
                            expanded,
                            variables: variables.clone(),
                            flow,
                            ran,
                            one_shell,
                            stdin_owner,
                            environment,
                            permit,
                        },
                    },
                );
                return Execution::success(true);
            }
            self.wait_all_jobs();
            let result = launch_child(&program, &child, |mut command| {
                if let Some(server) = &self.jobserver {
                    server.configure_child(&mut command, force);
                }
                command.status()
            });
            self.filesystem.borrow_mut().child_completed();
            let launched = result.is_ok();
            flow.attempted(launched);
            let failure_cleanup = if launched {
                Cleanup::ChangedOnError
            } else {
                Cleanup::None
            };
            let error = match result {
                Ok(status) if status.success() => None,
                // Recursive make uses exit 1 to report an out-of-date build in
                // query mode. GNU applies that convention to every forced
                // command; ignored errors retain their ordinary handling.
                Ok(status) if self.options.question && !ignore && status.code() == Some(1) => {
                    self.would_build = true;
                    return Execution::question(ran, failure_cleanup);
                }
                Ok(status) => Some(status.code().map_or_else(
                    || format!("Signal {}", status.signal().unwrap_or(0)),
                    |code| format!("Error {code}"),
                )),
                Err(err) => {
                    let text = err.to_string();
                    let suffix = err
                        .raw_os_error()
                        .map(|code| format!(" (os error {code})"))
                        .unwrap_or_default();
                    let message = text.strip_suffix(&suffix).unwrap_or(&text);
                    self.stderr(&[program.as_slice(), b": ", message.as_bytes()].concat());
                    Some("Error 127".to_owned())
                }
            };
            if let Some(error) = error {
                if !ignore {
                    self.show_makefile_error();
                }
                if !ignore || !self.globally_silent() {
                    let mut message = if ignore { Vec::new() } else { b"*** ".to_vec() };
                    message.push(b'[');
                    message.extend_from_slice(&location_prefix(&recipe.location));
                    message.extend_from_slice(target);
                    message.extend_from_slice(
                        format!("] {error}{}", if ignore { " (ignored)" } else { "" }).as_bytes(),
                    );
                    self.stderr(&message);
                }
                if !ignore {
                    return Execution::failure(ran, failure_cleanup);
                }
            }
        }
        if !self.options.touch || phony || !flow.has_ordinary() {
            return Execution::success(ran);
        }
        let passes = if one_shell {
            1
        } else {
            flow.completion_passes()
        };
        for pass in 0..passes {
            let mut touched = self.touch_target(target);
            ran |= touched.ran;
            if touched.status == Status::Failure {
                // Only dispatch completion can fail before a continuation owner
                // performs cleanup. The owner's final completion has no such step.
                if pass == 0 && passes > 1 {
                    touched.cleanup = flow.continuation_cleanup();
                }
                touched.ran |= ran;
                return touched;
            }
        }
        Execution::success(ran)
    }

    fn touch_target(&self, target: &[u8]) -> Execution {
        if !self.globally_silent() {
            self.stdout(&[b"touch ".as_slice(), target].concat());
        }
        if self.options.dry_run {
            return Execution::success(true);
        }
        let file = match OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(os(target))
        {
            Ok(file) => file,
            Err(error) => {
                self.stderr(
                    &[
                        b"touch: open: ".as_slice(),
                        target,
                        b": ",
                        os_error(&error).as_bytes(),
                    ]
                    .concat(),
                );
                return Execution::failure(true, Cleanup::None);
            }
        };
        if let Err(error) = file.set_times(FileTimes::new().set_modified(SystemTime::now())) {
            self.stderr(
                &[
                    b"touch: ".as_slice(),
                    target,
                    b": ",
                    os_error(&error).as_bytes(),
                ]
                .concat(),
            );
            return Execution::failure(true, Cleanup::None);
        }
        Execution::success(true)
    }

    fn cleanup_target(
        &self,
        target: &[u8],
        before: Option<SystemTime>,
        phony: bool,
        cleanup: Cleanup,
    ) {
        if cleanup == Cleanup::None
            || phony
            || self.database.matches_special_pattern(b".PRECIOUS", target)
        {
            return;
        }
        if cleanup == Cleanup::ChangedOnError
            && !self
                .database
                .rules
                .contains_key(b".DELETE_ON_ERROR".as_slice())
        {
            return;
        }
        let Ok(metadata) = fs::metadata(os(target)) else {
            return;
        };
        if !metadata.is_file() {
            return;
        }
        if cleanup == Cleanup::ChangedOnError && metadata.modified().ok() == before {
            return;
        }
        self.stderr(&[b"*** Deleting file '".as_slice(), target, b"'"].concat());
        if let Err(error) = fs::remove_file(os(target))
            && error.kind() != io::ErrorKind::NotFound
        {
            self.stderr(
                &[
                    b"unlink: ".as_slice(),
                    target,
                    b": ",
                    os_error(&error).as_bytes(),
                ]
                .concat(),
            );
        }
    }
}

fn read_include(name: &[u8]) -> Result<IncludedFile, Vec<u8>> {
    fs::read(os(name))
        .map(|bytes| IncludedFile {
            name: name.to_vec(),
            bytes,
        })
        .map_err(|error| os_error(&error).into_bytes())
}

pub(crate) fn os_error(error: &io::Error) -> String {
    let text = error.to_string();
    let suffix = error
        .raw_os_error()
        .map(|code| format!(" (os error {code})"))
        .unwrap_or_default();
    text.strip_suffix(&suffix).unwrap_or(&text).to_owned()
}

fn trim_leading(bytes: &[u8]) -> &[u8] {
    &bytes[bytes
        .iter()
        .position(|b| !matches!(b, b' ' | b'\t'))
        .unwrap_or(bytes.len())..]
}

/// Prefix markers can appear in any order, interleaved with leading blanks.
fn has_force_prefix(bytes: &[u8]) -> bool {
    let mut bytes = trim_leading(bytes);
    while let Some(prefix) = bytes.first() {
        match prefix {
            b'+' => return true,
            b'@' | b'-' => bytes = trim_leading(&bytes[1..]),
            _ => break,
        }
    }
    false
}

fn copy_environment(source: &Command, destination: &mut Command) {
    for (name, value) in source.get_envs() {
        if let Some(value) = value {
            destination.env(name, value);
        } else {
            destination.env_remove(name);
        }
    }
}

/// Keep the exact executable that returned ENOEXEC. Re-searching using file mode
/// bits is incorrect: an earlier PATH entry can have an execute bit yet deny the
/// current user execution. Only the kernel's actual exec result decides that.
pub(crate) fn launch_child<T>(
    program: &[u8],
    template: &Command,
    mut run: impl FnMut(Command) -> io::Result<T>,
) -> io::Result<T> {
    let paths = if program.contains(&b'/') {
        vec![std::path::PathBuf::from(os(program))]
    } else {
        let path = template
            .get_envs()
            .find(|(key, _)| *key == "PATH")
            .map(|(_, value)| value.map(OsStr::to_owned))
            .unwrap_or_else(|| std::env::var_os("PATH"))
            .unwrap_or_else(|| "/bin:/usr/bin".into());
        std::env::split_paths(&path)
            .map(|directory| {
                let directory = if directory.as_os_str().is_empty() {
                    std::path::PathBuf::from(".")
                } else {
                    directory
                };
                directory.join(os(program))
            })
            .collect()
    };
    let mut denied = None;
    let mut missing = io::Error::from_raw_os_error(2);
    for path in paths {
        let mut command = Command::new(&path);
        command.arg0(os(program)).args(template.get_args());
        copy_environment(template, &mut command);
        match run(command) {
            // Linux ENOEXEC: interpret the attempted file through /bin/sh,
            // preserving args/environment but not the recipe's -e/-c flags.
            Err(error) if error.raw_os_error() == Some(8) => {
                let mut shell = Command::new("/bin/sh");
                shell.arg(&path).args(template.get_args());
                copy_environment(template, &mut shell);
                return run(shell);
            }
            Err(error)
                if !program.contains(&b'/') && error.kind() == io::ErrorKind::PermissionDenied =>
            {
                denied = Some(error)
            }
            Err(error)
                if !program.contains(&b'/') && matches!(error.raw_os_error(), Some(2 | 20)) =>
            {
                missing = error
            }
            result => return result,
        }
    }
    Err(denied.unwrap_or(missing))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmake_core::variables::Flavor;

    #[test]
    fn makelevel_uses_raw_decimal_prefix_with_a_distinct_first_byte_minus_guard() {
        for (input, expected) in [
            (b"".as_slice(), 0),
            (b"007", 7),
            (b"+7", 7),
            (b"-7", 0),
            (b" -7", 4_294_967_289),
            (b" \t\n\x0b\x0c\r+7tail", 7),
            (b"7\xff", 7),
            (b"\xff7", 0),
            (b"0x10", 0),
            (b"1.2", 1),
            (b"+-7", 0),
            (b" -", 0),
            (b"$(intcmp 3,3)", 0),
        ] {
            assert_eq!(startup_makelevel(input), expected, "{input:?}");
        }
    }

    #[test]
    fn makelevel_saturates_at_unsigned_long_limit_then_keeps_low_unsigned_int_bits() {
        for (input, expected) in [
            (b"4294967295".as_slice(), u32::MAX),
            (b"4294967296", 0),
            (b"4294967297", 1),
            (b"9223372036854775808", 0),
            (b"9223372036854775809", 1),
            (b"18446744073709551615", u32::MAX),
            (b"18446744073709551616", u32::MAX),
            (b"999999999999999999999999999999999", u32::MAX),
            (b" -999999999999999999999999999999999", u32::MAX),
            (b"-999999999999999999999999999999999", 0),
        ] {
            assert_eq!(startup_makelevel(input), expected, "{input:?}");
        }
        assert_eq!(startup_makelevel(b" -1").wrapping_add(1), 0);
    }

    fn automatic_store() -> VariableStore {
        let mut variables = VariableStore::new();
        define_automatic_variants(&mut variables).unwrap();
        variables
    }

    #[test]
    fn automatic_variants_have_observable_recursive_definitions() {
        let variables = automatic_store();
        for base in b"@<^+?*%" {
            for suffix in b"DF" {
                let name = [*base, *suffix];
                let variable = variables.get(&name).unwrap();
                assert_eq!(variable.origin(), Origin::Automatic);
                assert_eq!(variable.flavor(), Flavor::Recursive);
                let expected = if *suffix == b'D' {
                    [b"$(patsubst %/,%,$(dir $".as_slice(), &[*base], b"))"].concat()
                } else {
                    [b"$(notdir $".as_slice(), &[*base], b")"].concat()
                };
                assert_eq!(variable.raw_value(), expected);
            }
        }
    }

    #[test]
    fn variants_exist_before_base_values_and_order_only_has_no_variants() {
        let variables = automatic_store();
        assert_eq!(variables.definitions().count(), 14);
        for base in b"@<^+?*%|" {
            assert!(variables.get(&[*base]).is_none());
        }
        assert!(variables.get(b"|D").is_none());
        assert!(variables.get(b"|F").is_none());
        assert_eq!(
            variables
                .expand(b"$(@D)$(@F)$(<D)$(<F)$(^D)$(^F)$(+D)$(+F)$(?D)$(?F)$(*D)$(*F)$(%D)$(%F)")
                .unwrap(),
            b""
        );
    }

    #[test]
    fn variants_replace_inherited_definitions_and_ignore_lower_origins() {
        for origin in [
            Origin::Default,
            Origin::Environment,
            Origin::File,
            Origin::EnvironmentOverride,
            Origin::CommandLine,
            Origin::Override,
        ] {
            let mut variables = VariableStore::new();
            variables
                .define_literal(b"@D", b"inherited", origin)
                .unwrap();
            define_automatic_variants(&mut variables).unwrap();
            for assignment in [
                Assignment::Recursive,
                Assignment::Simple,
                Assignment::Append,
                Assignment::Conditional,
            ] {
                variables
                    .define(b"@D", b"replacement", assignment, origin)
                    .unwrap();
                assert_eq!(
                    variables.get(b"@D").unwrap().raw_value(),
                    b"$(patsubst %/,%,$(dir $@))"
                );
            }
        }
    }

    #[test]
    fn parsing_cannot_override_or_undefine_automatic_variants() {
        let mut database = Database::default();
        database.variables = automatic_store();
        database
            .read(
                b"@D = ignored\n@F := ignored\noverride <D = ignored\noverride <F += ignored\nundefine ^D\noverride undefine ^F\n",
                b"Makefile",
            )
            .unwrap();
        assert_eq!(database.variables.definitions().count(), 14);
        for name in [b"@D", b"@F", b"<D", b"<F", b"^D", b"^F"] {
            let variable = database.variables.get(name).unwrap();
            assert_eq!(variable.origin(), Origin::Automatic);
            assert_eq!(variable.flavor(), Flavor::Recursive);
            assert!(
                !variable
                    .raw_value()
                    .windows(7)
                    .any(|part| part == b"ignored")
            );
        }
    }

    #[test]
    fn user_base_value_is_visible_during_parsing_then_recipe_value_wins() {
        let mut database = Database::default();
        database.variables = automatic_store();
        database
            .read(
                b"override @ := before/file\nPARSE := $(@D)|$(@F)\n",
                b"Makefile",
            )
            .unwrap();
        database
            .variables
            .define_literal(b"@", b"after/target", Origin::Automatic)
            .unwrap();
        assert_eq!(
            database.variables.expand(b"$(PARSE)|$(@D)|$(@F)").unwrap(),
            b"before|file|after|target"
        );
    }

    #[test]
    fn variants_use_make_path_functions_without_path_normalization() {
        let mut variables = automatic_store();
        for (base, directory, file) in [
            (b"bare".as_slice(), b".".as_slice(), b"bare".as_slice()),
            (b"dir/file", b"dir", b"file"),
            (b"dir//file", b"dir/", b"file"),
            (b"./file", b".", b"file"),
            (b"/file", b"", b"file"),
            (b"/", b"", b""),
            (b"dir/", b"dir", b""),
            (b"one$two/file", b"one$two", b"file"),
            (b"dir\xff/file\xfe", b"dir\xff", b"file\xfe"),
        ] {
            variables
                .define_literal(b"@", base, Origin::Automatic)
                .unwrap();
            assert_eq!(variables.expand(b"$(@D)").unwrap(), directory);
            assert_eq!(variables.expand(b"$(@F)").unwrap(), file);
        }
    }

    #[test]
    fn list_variants_preserve_repetition_and_function_word_semantics() {
        let mut variables = automatic_store();
        for base in b"@<^+?*%" {
            variables
                .define_literal(
                    &[*base],
                    b" bare  dir/one\tdir/two bare ",
                    Origin::Automatic,
                )
                .unwrap();
            let directory = [b"$(".as_slice(), &[*base], b"D)"].concat();
            let file = [b"$(".as_slice(), &[*base], b"F)"].concat();
            assert_eq!(variables.expand(&directory).unwrap(), b". dir dir .");
            assert_eq!(variables.expand(&file).unwrap(), b"bare one two bare");
        }
    }

    #[test]
    fn engine_installs_variants_even_when_builtin_variables_disabled() {
        for no_variables in [false, true] {
            let engine = Engine::new(
                Options {
                    no_variables,
                    assignments: vec![b"@D=ignored".to_vec()],
                    ..Options::default()
                },
                b"rmake".to_vec(),
                OsStr::new("rmake"),
            )
            .unwrap();
            let variable = engine.database.variables.get(b"@D").unwrap();
            assert_eq!(variable.origin(), Origin::Automatic);
            assert_eq!(variable.flavor(), Flavor::Recursive);
            assert_eq!(variable.raw_value(), b"$(patsubst %/,%,$(dir $@))");
        }
    }
}
