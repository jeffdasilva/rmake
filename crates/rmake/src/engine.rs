// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Linux filesystem and serial process boundary for the initial explicit-rule slice.
//! No persistent cache and no delegation to GNU Make are involved.

use crate::cli::Options;
use rmake_core::parser::{Database, Location, Recipe, Rule, assignment, trim, words};
use rmake_core::variables::{Assignment, Origin, VariableStore};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::ffi::{OsStr, OsString};
use std::fs::{self, FileTimes, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::process::ExitStatusExt;
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
            b".ONESHELL".as_slice(),
            b".SECONDEXPANSION",
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
            if outcome.failed && self.options.keep_going && outcome.blocked {
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
                && !self.options.silent
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
            (empty_means_all && rule.prerequisites.is_empty())
                || rule.prerequisites.iter().any(|name| name == target)
        })
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
            return Outcome {
                failed: true,
                ..Outcome::default()
            };
        };
        self.visiting.insert(target.to_vec());
        let normal = unique(&rule.prerequisites);
        let order: Vec<_> = unique(&rule.order_only)
            .into_iter()
            .filter(|name| !normal.contains(name))
            .collect();
        let mut newer = Vec::new();
        let mut dropped = HashSet::new();
        let mut stale = before.is_none() || phony || self.options.always;
        let mut outcome = Outcome {
            time: before,
            ..Outcome::default()
        };
        for (dependency, order_only) in normal
            .iter()
            .map(|n| (n, false))
            .chain(order.iter().map(|n| (n, true)))
        {
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
                    .prerequisites
                    .iter()
                    .filter(|name| !dropped.contains(*name))
                    .cloned()
                    .collect();
                let autos = [
                    (b"@".as_slice(), target.to_vec()),
                    (b"<", normal.first().cloned().unwrap_or_default()),
                    (b"^", joined(&normal)),
                    (b"+", joined(&repeated)),
                    (b"?", joined(&newer)),
                    (b"|", joined(&order)),
                    (b"*", Vec::new()),
                ];
                for (name, value) in autos {
                    if let Err(err) = variables.define_literal(name, &value, Origin::Automatic) {
                        self.fatal(None, err.to_string().as_bytes());
                        outcome.failed = true;
                        break;
                    }
                }
                if !outcome.failed {
                    let (ran, failed, question_outdated) =
                        self.execute(target, recipes, &variables, phony);
                    outcome.ran |= ran;
                    outcome.failed |= failed;
                    outcome.question_outdated |= question_outdated;
                    if failed
                        && self
                            .database
                            .rules
                            .contains_key(b".DELETE_ON_ERROR".as_slice())
                        && !self.special(b".PRECIOUS", target, false)
                        && !phony
                        && modified(target).is_some_and(|now| Some(now) != before)
                        && fs::remove_file(os(target)).is_ok()
                    {
                        self.stderr(&[b"*** Deleting file '".as_slice(), target, b"'"].concat());
                    }
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
    ) -> (bool, bool, bool) {
        let recursive = recipes.iter().any(|recipe| {
            has_force_prefix(&recipe.bytes)
                || recipe.bytes.windows(7).any(|s| s == b"$(MAKE)")
                || recipe.bytes.windows(7).any(|s| s == b"${MAKE}")
        });
        if self.options.touch && !self.options.dry_run && !self.options.question && recursive {
            self.fatal(
                None,
                b"rmake: forced commands with touch mode are not implemented yet",
            );
            return (false, true, false);
        }
        if self.options.touch && !self.options.dry_run && !self.options.question && !recursive {
            if phony {
                return (false, false, false);
            }
            if !self.options.silent {
                self.stdout(&[b"touch ".as_slice(), target].concat());
            }
            let result = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(false)
                .open(os(target))
                .and_then(|file| file.set_times(FileTimes::new().set_modified(SystemTime::now())));
            if let Err(err) = result {
                self.stderr(format!("touch: {err}").as_bytes());
                return (true, true, false);
            }
            return (true, false, false);
        }
        let mut expanded = Vec::new();
        for recipe in recipes {
            match variables.expand(&recipe.bytes) {
                Ok(bytes) => expanded.push((bytes, recipe)),
                Err(err) => {
                    self.fatal(Some(&recipe.location), err.to_string().as_bytes());
                    return (false, true, false);
                }
            }
        }
        let mut ran = false;
        for (bytes, recipe) in expanded {
            let mut command = trim_leading(&bytes);
            let mut quiet = self.options.silent || self.special(b".SILENT", target, true);
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
            if command.is_empty() {
                continue;
            }
            if self.options.question && !force {
                self.would_build = true;
                return (ran, false, true);
            }
            if self.options.touch && !self.options.dry_run && !force {
                continue;
            }
            if self.options.dry_run || !quiet {
                self.stdout(command);
            }
            ran = true;
            if self.options.dry_run && !force {
                continue;
            }
            let shell = match variables.expand(b"$(SHELL)") {
                Ok(shell) => shell,
                Err(err) => {
                    self.fatal(Some(&recipe.location), err.to_string().as_bytes());
                    return (ran, true, false);
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
                    return (ran, true, false);
                }
            };
            let mut child = Command::new(os(shell));
            child.args(words(&flags).map(os)).arg(os(command));
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
                            return (ran, true, false);
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
            let status = match child.status() {
                Ok(status) => status,
                Err(err) => {
                    self.stderr(format!("{}: {err}", String::from_utf8_lossy(shell)).as_bytes());
                    return (ran, true, false);
                }
            };
            if !status.success() {
                let error = status.code().map_or_else(
                    || format!("Signal {}", status.signal().unwrap_or(0)),
                    |code| format!("Error {code}"),
                );
                if !ignore || !self.options.silent {
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
                    return (ran, true, false);
                }
            }
        }
        (ran, false, false)
    }
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
