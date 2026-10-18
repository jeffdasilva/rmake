// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Linux filesystem and serial process boundary for the initial explicit-rule slice.
//! No persistent cache and no delegation to GNU Make are involved.

use crate::cli::Options;
use crate::recipe_flow::{Cleanup, Execution, Flow, Status};
use rmake_core::parser::{Database, Location, Recipe, Rule, assignment, trim, words};
use rmake_core::variables::{Assignment, ExpandError, Origin, VariableStore};
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
    question_outdated: bool,
    blocked: bool,
}

pub struct Engine {
    options: Options,
    database: Database,
    program: Vec<u8>,
    inherited_exports: HashSet<Vec<u8>>,
    done: HashMap<Vec<u8>, Outcome>,
    visiting: HashSet<Vec<u8>>,
    would_build: bool,
    initial_flags: Vec<u8>,
    initial_gnu_flags: Vec<u8>,
}

impl Engine {
    pub fn new(options: Options, program: Vec<u8>, invocation: &OsStr) -> Result<Self, String> {
        for dir in &options.directories {
            std::env::set_current_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        let mut database = Database::default();
        let mut inherited_exports = HashSet::new();
        for (name, value) in std::env::vars_os() {
            let name = name.as_bytes();
            if name == b"SHELL" {
                continue;
            }
            let origin = if options.environment_override {
                Origin::EnvironmentOverride
            } else {
                Origin::Environment
            };
            database
                .variables
                .define(name, value.as_bytes(), Assignment::Recursive, origin)
                .map_err(|e| e.to_string())?;
            inherited_exports.insert(name.to_vec());
        }
        database
            .initialize_suffixes(!options.no_rules)
            .map_err(|error| error.to_string())?;
        define_automatic_variants(&mut database.variables).map_err(|error| error.to_string())?;
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
                .map_err(|e| e.to_string())?;
        }
        let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
        database
            .variables
            .define_literal(b"CURDIR", cwd.as_os_str().as_bytes(), Origin::File)
            .map_err(|e| e.to_string())?;
        database
            .variables
            .define_literal(b"MAKECMDGOALS", &joined(&options.goals), Origin::Default)
            .map_err(|e| e.to_string())?;
        database
            .variables
            .set_working_directory(cwd.as_os_str().as_bytes());
        database.variables.remove(b"MAKEFLAGS");
        database
            .variables
            .define_literal(b"MAKEFLAGS", &options.flags(), Origin::File)
            .map_err(|e| e.to_string())?;
        database
            .variables
            .define_literal(b"MAKELEVEL", b"0", Origin::Default)
            .map_err(|e| e.to_string())?;
        for arg in &options.assignments {
            let spec = assignment(arg).expect("CLI identified assignment");
            if [b"MAKEFLAGS".as_slice(), b"GNUMAKEFLAGS"].contains(&trim(spec.name)) {
                return Err("assigning make flags is not implemented yet".into());
            }
            let name = database
                .variables
                .expand(spec.name)
                .map_err(|e| e.to_string())?;
            database
                .variables
                .define(&name, spec.value, spec.kind, Origin::CommandLine)
                .map_err(|e| e.to_string())?;
            inherited_exports.insert(name);
        }
        database.variables.remove(b"GNUMAKEFLAGS");
        database
            .variables
            .define_literal(b"GNUMAKEFLAGS", b"", Origin::Override)
            .map_err(|e| e.to_string())?;
        let initial_flags = options.flags();
        Ok(Self {
            options,
            database,
            program,
            inherited_exports,
            done: HashMap::new(),
            visiting: HashSet::new(),
            would_build: false,
            initial_flags,
            initial_gnu_flags: Vec::new(),
        })
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
            let _ = out.write_all(&location.file);
            let _ = write!(out, ":{}: *** ", location.line);
            let _ = out.write_all(message);
            let _ = out.write_all(b".  Stop.\n");
        } else {
            self.stderr(&[b"*** ".as_slice(), message, b".  Stop."].concat());
        }
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
        for file in &files {
            let mut bytes = Vec::new();
            let read = if file == Path::new("-") {
                io::stdin().read_to_end(&mut bytes)
            } else {
                fs::read(file).map(|content| {
                    bytes = content;
                    bytes.len()
                })
            };
            if let Err(err) = read {
                self.fatal(None, format!("{}: {err}", file.display()).as_bytes());
                return 2;
            }
            let mut list = self
                .database
                .variables
                .expand(b"$(MAKEFILE_LIST)")
                .unwrap_or_default();
            if !list.is_empty() {
                list.push(b' ');
            }
            list.extend_from_slice(file.as_os_str().as_bytes());
            if let Err(err) =
                self.database
                    .variables
                    .define_literal(b"MAKEFILE_LIST", &list, Origin::File)
            {
                self.fatal(None, err.to_string().as_bytes());
                return 2;
            }
            if let Err(err) = self.database.read(&bytes, file.as_os_str().as_bytes()) {
                self.fatal(Some(&err.location), err.message.as_bytes());
                return 2;
            }
        }
        for (name, initial) in [
            (b"MAKEFLAGS".as_slice(), &self.initial_flags),
            (b"GNUMAKEFLAGS", &self.initial_gnu_flags),
        ] {
            match self
                .database
                .variables
                .expand(&[b"$(".as_slice(), name, b")"].concat())
            {
                Ok(current) if &current == initial => {}
                _ => {
                    self.fatal(
                        None,
                        b"rmake: changing make flags in a Makefile is not implemented yet",
                    );
                    return 2;
                }
            }
        }
        for (location, message) in &self.database.warnings {
            let mut out = io::stderr().lock();
            let _ = out.write_all(&location.file);
            let _ = writeln!(out, ":{}: {message}", location.line);
        }
        // Reject special execution modes until their semantics are implemented,
        // rather than accidentally passing cases under serial defaults.
        for name in [
            b".SECONDEXPANSION".as_slice(),
            b".POSIX",
            b".WAIT",
            b".NOTPARALLEL",
            b".INTERMEDIATE",
            b".SECONDARY",
            b".LOW_RESOLUTION_TIME",
        ] {
            if self.database.rules.contains_key(name) {
                self.fatal(
                    None,
                    &[b"rmake: unsupported special target ".as_slice(), name].concat(),
                );
                return 2;
            }
        }
        let goals = if self.options.goals.is_empty() {
            match self.database.variables.expand(b"$(.DEFAULT_GOAL)") {
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
                    self.fatal(None, err.to_string().as_bytes());
                    return 2;
                }
            }
        } else {
            self.options.goals.clone()
        };
        let mut failed = false;
        for goal in goals {
            let already_done = self.done.contains_key(&goal);
            let outcome = self.build(&goal, None, 0);
            failed |= outcome.failed;
            if outcome.failed && !self.options.keep_going {
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
                let has_recipe = self
                    .database
                    .rules
                    .get(&goal)
                    .is_some_and(|rule| rule.recipes.is_some())
                    && !self.special(b".PHONY", &goal, false);
                let body = if has_recipe {
                    [b"'".as_slice(), &goal, b"' is up to date."].concat()
                } else {
                    [b"Nothing to be done for '".as_slice(), &goal, b"'."].concat()
                };
                self.stdout(&[self.program.as_slice(), b": ", &body].concat());
            }
        }
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

    fn build(&mut self, target: &[u8], parent: Option<&[u8]>, depth: usize) -> Outcome {
        if let Some(outcome) = self.done.get(target) {
            return *outcome;
        }
        if depth >= 512 {
            self.fatal(None, b"rmake: dependency nesting limit exceeded");
            return Outcome {
                failed: true,
                ..Outcome::default()
            };
        }
        let before = modified(target);
        let phony = self.special(b".PHONY", target, false);
        let Some(rule) = self
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
            self.stderr(&message);
            let result = Outcome {
                failed: true,
                ..Outcome::default()
            };
            self.done.insert(target.to_vec(), result);
            return result;
        };
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
        let mut stale = before.is_none() || phony || self.options.always;
        let mut outcome = Outcome {
            time: before,
            ..Outcome::default()
        };
        // Visit occurrences in the merged declaration order. Completed nodes
        // are memoized, but every circular occurrence must still be diagnosed.
        for occurrence in &rule.dependencies {
            let dependency = &occurrence.name;
            let order_only = !normal_names.contains(dependency.as_slice());
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
            let child = self.build(dependency, Some(target), depth + 1);
            outcome.ran |= child.ran;
            outcome.failed |= child.failed;
            outcome.blocked |= child.failed;
            outcome.question_outdated |= child.question_outdated;
            if (child.failed || child.question_outdated) && !self.options.keep_going {
                break;
            }
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
            if let Some(recipes) = &rule.recipes {
                let mut variables = self.database.variables.clone();
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
                let autos = [
                    (b"@".as_slice(), target.to_vec()),
                    (b"<", repeated.first().cloned().unwrap_or_default()),
                    (b"^", joined(&normal)),
                    (b"+", joined(&repeated)),
                    (b"?", joined(unique(&newer))),
                    (b"|", joined(&order)),
                    (b"*", self.database.explicit_stem(target).to_vec()),
                    (b"%", Vec::new()),
                ];
                for (name, value) in autos {
                    if let Err(err) = variables.define_literal(name, &value, Origin::Automatic) {
                        self.fatal(None, err.to_string().as_bytes());
                        outcome.failed = true;
                        break;
                    }
                }
                if !outcome.failed {
                    let execution = self.execute(target, recipes, &variables, phony);
                    outcome.ran |= execution.ran;
                    outcome.failed |= execution.status == Status::Failure;
                    outcome.question_outdated |= execution.status == Status::Question;
                    self.cleanup_target(target, before, phony, execution.cleanup);
                }
            }
            outcome.time = if phony { None } else { modified(target) };
        }
        self.visiting.remove(target);
        self.done.insert(target.to_vec(), outcome);
        outcome
    }

    fn execute(
        &mut self,
        target: &[u8],
        recipes: &[Recipe],
        variables: &VariableStore,
        phony: bool,
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
        let mut flow = Flow::new(
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
        let mut expanded = Vec::new();
        for recipe in recipes {
            match variables.expand(&recipe.bytes) {
                Ok(bytes) => expanded.push((bytes, recipe)),
                Err(err) => {
                    self.fatal(Some(&recipe.location), err.to_string().as_bytes());
                    return Execution::failure(false, Cleanup::None);
                }
            }
        }
        let mut ran = false;
        for (index, (bytes, recipe)) in expanded.into_iter().enumerate() {
            if bytes.is_empty() {
                continue;
            }
            let mut command = trim_leading(&bytes);
            let mut quiet = self.globally_silent() || self.special(b".SILENT", target, false);
            let mut ignore = self.options.ignore_errors || self.special(b".IGNORE", target, true);
            let mut force = recipe
                .bytes
                .windows(7)
                .any(|s| s == b"$(MAKE)" || s == b"${MAKE}");
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
            let shell = match variables.expand(b"$(SHELL)") {
                Ok(shell) => shell,
                Err(err) => {
                    self.fatal(Some(&recipe.location), err.to_string().as_bytes());
                    return Execution::failure(ran, Cleanup::None);
                }
            };
            let shell = if shell.is_empty() {
                b"/bin/sh".as_slice()
            } else {
                &shell
            };
            let flags = match variables.expand(b"$(.SHELLFLAGS)") {
                Ok(flags) => flags,
                Err(err) => {
                    self.fatal(Some(&recipe.location), err.to_string().as_bytes());
                    return Execution::failure(ran, Cleanup::None);
                }
            };
            let ifs = if variables.get(b"IFS").is_some() {
                match variables.expand(b"$(IFS)") {
                    Ok(value) => Some(value),
                    Err(err) => {
                        self.fatal(Some(&recipe.location), err.to_string().as_bytes());
                        return Execution::failure(ran, Cleanup::None);
                    }
                }
            } else {
                None
            };
            if command.is_empty() {
                continue;
            }
            let transformed;
            if one_shell {
                transformed = crate::oneshell::transform(command, shell, recipe.prefix);
                command = &transformed;
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
                crate::process::prepare(command, shell, &flags, ifs.as_deref())
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
            let mut exports: BTreeSet<Vec<u8>> = self
                .inherited_exports
                .iter()
                .chain(self.database.exports.iter())
                .cloned()
                .collect();
            if self.database.export_all
                || self
                    .database
                    .rules
                    .contains_key(b".EXPORT_ALL_VARIABLES".as_slice())
            {
                for (name, variable) in variables.definitions() {
                    if name != b"SHELL"
                        && !matches!(variable.origin(), Origin::Default | Origin::Automatic)
                        && name.iter().all(|b| b.is_ascii_alphanumeric() || *b == b'_')
                    {
                        exports.insert(name.to_vec());
                    }
                }
            }
            for name in &exports {
                if self.database.unexports.contains(name) {
                    child.env_remove(os(name));
                    continue;
                }
                if let Some(var) = variables.get(name) {
                    let value = if var.flavor() == rmake_core::variables::Flavor::Simple {
                        Ok(var.raw_value().to_vec())
                    } else {
                        variables.expand(var.raw_value())
                    };
                    match value {
                        Ok(value) => {
                            child.env(os(name), OsString::from_vec(value));
                        }
                        Err(err) => {
                            self.fatal(Some(&recipe.location), err.to_string().as_bytes());
                            return Execution::failure(ran, Cleanup::None);
                        }
                    }
                } else {
                    child.env_remove(os(name));
                }
            }
            for name in &self.database.unexports {
                child.env_remove(os(name));
            }
            if self.database.unexports.contains(b"MAKEFLAGS".as_slice()) {
                child.env_remove("MAKEFLAGS");
            } else {
                child.env("MAKEFLAGS", OsString::from_vec(self.options.flags()));
            }
            let level = variables
                .expand(b"$(MAKELEVEL)")
                .ok()
                .and_then(|v| String::from_utf8(v).ok())
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(0);
            child.env("MAKELEVEL", level.saturating_add(1).to_string());
            let _ = io::stdout().flush();
            let result = launch_status(&program, &child);
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
                if !ignore || !self.globally_silent() {
                    let mut message = if ignore { Vec::new() } else { b"*** ".to_vec() };
                    message.push(b'[');
                    message.extend_from_slice(&recipe.location.file);
                    message.extend_from_slice(format!(":{}: ", recipe.location.line).as_bytes());
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
        if cleanup == Cleanup::None || phony || self.special(b".PRECIOUS", target, false) {
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

fn os_error(error: &io::Error) -> String {
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
fn launch_status(program: &[u8], template: &Command) -> io::Result<std::process::ExitStatus> {
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
        match command.status() {
            // Linux ENOEXEC: interpret the attempted file through /bin/sh,
            // preserving args/environment but not the recipe's -e/-c flags.
            Err(error) if error.raw_os_error() == Some(8) => {
                let mut shell = Command::new("/bin/sh");
                shell.arg(&path).args(template.get_args());
                copy_environment(template, &mut shell);
                return shell.status();
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
        let mut database = Database {
            variables: automatic_store(),
            ..Database::default()
        };
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
        let mut database = Database {
            variables: automatic_store(),
            ..Database::default()
        };
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
