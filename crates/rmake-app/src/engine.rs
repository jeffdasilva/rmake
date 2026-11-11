// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Linux filesystem, dependency traversal and process execution boundary.
//! No persistent cache and no delegation to GNU Make are involved.

mod assumptions;
mod database_output;
mod initial;
mod runtime_options;
mod startup;
mod startup_exit;
pub use startup_exit::{Failure, NativeExit};

use crate::cli::Options;
use crate::file_aliases::Aliases;
use crate::makefiles::{IncludeSearch, source_open_exhausted};
use crate::recipe_flow::{Cleanup, Execution, Flow, Shutdown, Status};
use rmake_core::diagnostic::{
    Diagnostic, ExpansionContext, IncludeError, IncludeFailure, Output, OutputReceiver, ScopeParent,
};
use rmake_core::messages::Message;
use rmake_core::parser::{Database, Dependency, Location, Makefile, Recipe, Rule, trim, words};
use rmake_core::variables::{
    Assignment, ExpandError, Flavor, Origin, Pattern, TargetScope, Variable, VariableStore,
};
use rmake_linux::jobserver::{Jobserver, Permit};
use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::ffi::{OsStr, OsString};
use std::fs::{self, FileTimes, OpenOptions};
use std::io::{self, Write};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::Path;
use std::process::Command;
use std::rc::Rc;
use std::time::{Duration, SystemTime};

type FilesystemRuntime = rmake_linux::filesystem::Runtime<crate::output::Controller>;

#[cfg(not(feature = "guile"))]
const FEATURES: &[u8] = b"target-specific order-only second-expansion else-if shortest-stem undefine oneshell nocomment grouped-target extra-prereqs notintermediate shell-export archives jobserver jobserver-fifo output-sync check-symlink load";
#[cfg(feature = "guile")]
const FEATURES: &[u8] = b"target-specific order-only second-expansion else-if shortest-stem undefine oneshell nocomment grouped-target extra-prereqs notintermediate shell-export archives jobserver jobserver-fifo output-sync check-symlink guile load";

fn os(bytes: &[u8]) -> &OsStr {
    OsStr::from_bytes(bytes)
}

/// GNU 4.4.1's directory-search spelling includes a historical trailing-slash
/// quirk: with no filename directory prefix, restoring the separator replaces
/// the first basename byte. Test existence using that spelling as GNU does.
fn searched_name(directory: &[u8], target: &[u8]) -> Vec<u8> {
    if target.starts_with(b"/") {
        return target.to_vec();
    }
    let directory = if directory.len() > 1 && directory.ends_with(b"/") {
        &directory[..directory.len() - 1]
    } else {
        directory
    };
    let slash = target
        .iter()
        .rposition(|&byte| byte == b'/')
        .filter(|&index| index > 0);
    let (prefix, basename) = slash.map_or((b"".as_slice(), target), |index| {
        (&target[..index], &target[index + 1..])
    });
    let separator = prefix.last().or_else(|| directory.last()) != Some(&b'/');
    let mut name = Vec::with_capacity(directory.len() + target.len() + usize::from(separator));
    name.extend_from_slice(directory);
    if !prefix.is_empty() {
        name.push(b'/');
        name.extend_from_slice(prefix);
    }
    if separator {
        name.push(b'/');
        name.extend_from_slice(basename);
    } else {
        let start = name.len();
        name.extend_from_slice(basename);
        if let Some(first) = name.get_mut(start) {
            *first = b'/';
        }
    }
    name
}

/// Most searches use one spelling. Repeated trailing separators keep the
/// original basename for record/cache lookup, then clobber it for stat/output.
fn search_probe_names<'a>(
    directory: &[u8],
    target: &[u8],
    searched: &'a [u8],
) -> (std::borrow::Cow<'a, [u8]>, std::borrow::Cow<'a, [u8]>) {
    use std::borrow::Cow;
    let directory = if directory.len() > 1 && directory.ends_with(b"/") {
        &directory[..directory.len() - 1]
    } else {
        directory
    };
    if directory.ends_with(b"/")
        && !target.starts_with(b"/")
        && !target.contains(&b'/')
        && !target.is_empty()
    {
        let mut lookup = searched.to_vec();
        lookup[directory.len()] = target[0];
        let membership = [directory, b"/", target].concat();
        (Cow::Owned(lookup), Cow::Owned(membership))
    } else {
        (Cow::Borrowed(searched), Cow::Borrowed(searched))
    }
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
    if let Some((archive, member)) = rmake_core::filenames::archive_parts(name) {
        rmake_linux::archive::modified(archive, member)
            .ok()
            .flatten()
    } else {
        fs::metadata(os(name)).and_then(|m| m.modified()).ok()
    }
}

fn prerequisite_variable_name(name: &[u8]) -> &[u8] {
    rmake_core::filenames::archive_parts(name).map_or(name, |(_, member)| member)
}

fn target_variable_names(name: &[u8]) -> (&[u8], &[u8]) {
    rmake_core::filenames::archive_parts(name).unwrap_or((name, b""))
}

/// A normal occurrence upgrades the first occurrence and later order-only
/// duplicates. Order-only duplicates already passed before the first normal
/// occurrence retain their roles in the recorded chain.
fn promote_order_only_duplicates(dependencies: &mut [Dependency], ready: impl Fn(usize) -> bool) {
    let mut first = HashMap::new();
    let mut promotions = Vec::new();
    for (index, dependency) in dependencies.iter().enumerate() {
        if !ready(index) || dependency.hidden || dependency.name == b".WAIT" {
            continue;
        }
        let (first_index, order_only) = first
            .entry(dependency.name.as_slice())
            .or_insert((index, dependency.order_only));
        if *order_only != dependency.order_only {
            promotions.extend([*first_index, index]);
            *order_only = false;
        }
    }
    for index in promotions {
        dependencies[index].order_only = false;
    }
}

fn secondary_automatic(
    target: &[u8],
    dependencies: &mut [Dependency],
    ready: &[bool],
    stem: Vec<u8>,
    always: bool,
) -> [(&'static [u8], Vec<u8>); 8] {
    let normal = dependencies
        .iter()
        .zip(ready)
        .filter(|(d, ready)| **ready && !d.order_only && !d.hidden && d.name != b".WAIT")
        .map(|(d, _)| d.name.as_slice());
    let first_normal = normal.clone().next().unwrap_or_default().to_vec();
    let plus = joined(normal.map(prerequisite_variable_name));
    // GNU computes < and + before promoting duplicate order-only
    // occurrences. Promotion survives into later clause expansions.
    promote_order_only_duplicates(dependencies, |index| {
        ready.get(index).copied().unwrap_or(false)
    });
    let mut seen = HashSet::new();
    let canonical: Vec<_> = dependencies
        .iter()
        .zip(ready)
        .filter(|(d, ready)| {
            **ready && !d.hidden && d.name != b".WAIT" && seen.insert(d.name.as_slice())
        })
        .map(|(d, _)| d)
        .collect();
    [
        (b"@".as_slice(), target_variable_names(target).0.to_vec()),
        (b"<", first_normal),
        (
            b"^",
            joined(
                canonical
                    .iter()
                    .filter(|d| !d.order_only)
                    .map(|d| prerequisite_variable_name(&d.name)),
            ),
        ),
        (b"+", plus),
        (
            b"|",
            joined(
                canonical
                    .iter()
                    .filter(|d| d.order_only)
                    .map(|d| prerequisite_variable_name(&d.name)),
            ),
        ),
        (
            b"?",
            if always {
                joined(
                    canonical
                        .iter()
                        .filter(|d| !d.order_only)
                        .map(|d| prerequisite_variable_name(&d.name)),
                )
            } else {
                Vec::new()
            },
        ),
        (b"%", target_variable_names(target).1.to_vec()),
        (b"*", stem),
    ]
}

fn location_prefix(location: &Location) -> Vec<u8> {
    let mut bytes = if location.file.is_null() {
        return b"<builtin>: ".to_vec();
    } else {
        location.file.to_vec()
    };
    bytes.extend_from_slice(format!(":{}", location.line).as_bytes());
    bytes.extend_from_slice(b": ");
    bytes
}

fn one_logical_recipe_slot(bytes: &[u8]) -> bool {
    bytes
        .split_inclusive(|&byte| byte == b'\n')
        .filter_map(|line| line.strip_suffix(b"\n"))
        .all(|line| line.iter().rev().take_while(|&&byte| byte == b'\\').count() % 2 == 1)
}

/// GNU exposes these recursive definitions even while reading makefiles and
/// with -R. Their automatic origin also protects them from override/undefine.
/// Keep the formulas, rather than precomputing paths: origin/flavor/value and
/// references to user-defined base variables during parsing are observable.
fn define_automatic_variants(variables: &mut VariableStore) -> Result<(), ExpandError> {
    for (suffix, prefix, ending) in [
        (
            b'D',
            b"$(patsubst %/,%,$(dir $".as_slice(),
            b"))".as_slice(),
        ),
        (b'F', b"$(notdir $".as_slice(), b")".as_slice()),
    ] {
        for base in b"@%*<?^+" {
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

/// Only completion changes this policy: initial cached timestamps stay distinct
/// from simulated updates. A real double-colon branch dominates assumed-new.
#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
enum CompletionTime {
    #[default]
    Unchanged,
    Preserve,
    AssumeNew,
    AssumeNewPhony,
    Refresh,
    ArchiveUnknown,
}

impl CompletionTime {
    fn preserves(self) -> bool {
        matches!(
            self,
            Self::Preserve | Self::AssumeNew | Self::AssumeNewPhony
        )
    }
}

/// Diagnostic result provenance is separate from the graph's completion policy.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum DatabaseResult {
    #[default]
    Outcome,
    Simulated,
    ObservedGroup,
    ObservedDoubleColon,
    RecipeUnstarted,
}

#[derive(Clone, Copy)]
enum GroupPeerTime {
    Unchecked,
    Observed(Option<SystemTime>),
    Outcome,
}

#[derive(Clone, Copy, Default)]
struct Outcome {
    time: Option<SystemTime>,
    ran: bool,
    failed: bool,
    fatal: bool,
    question_outdated: bool,
    // Direct goals observe completed double-colon branches separately.
    first_branch_verdict: Option<Status>,
    blocked: bool,
    // A failed dispatch in an optional parallel remake can precede more visits.
    deferred_spawn_failure: bool,
    // An optional grouped recipe already supplied this peer's failure.
    optional_group_peer_failure: bool,
    // The cached declared head can remain successful after the tail fails.
    late_input_failed: bool,
    // A real recipe invalidates its timestamp; a parent may search again.
    search_after_update: bool,
    completion: CompletionTime,
    database_result: DatabaseResult,
    debug_time_changed: bool,
    // A serial peer completed with its owner before its own diagnostic visit.
    debug_serial_group_peer: bool,
}

impl Outcome {
    fn terminal_verdict(self) -> Option<Status> {
        if self.failed {
            Some(Status::Failure)
        } else {
            self.question_outdated.then_some(Status::Question)
        }
    }

    fn goal_verdict(self) -> Option<Status> {
        self.first_branch_verdict
            .or_else(|| self.terminal_verdict())
    }

    fn retain_branch_verdict(&mut self, branch: Self) {
        self.first_branch_verdict = self
            .first_branch_verdict
            .or_else(|| branch.terminal_verdict());
    }
}

#[derive(Default)]
struct OrdinaryQueries {
    first_verdict: Option<Status>,
    pass: u64,
    deferred_tails: HashMap<Vec<u8>, DeferredQueryTail>,
}

impl OrdinaryQueries {
    fn record(&mut self, verdict: Option<Status>) {
        self.first_verdict = self.first_verdict.or(verdict);
    }
}

struct DeferredQueryTail {
    pass: u64,
    queued: bool,
    state: DoubleColonState,
}

struct RequestedGoal {
    name: Vec<u8>,
    already_done: bool,
    initial: Outcome,
    awaiting_recipe: bool,
    awaiting_group_peer: bool,
    started_command: bool,
}

type SharedIncludeError = Rc<Cell<Option<PendingIncludeError>>>;
type MakefileVisits<'a> = HashMap<&'a [u8], (Option<SystemTime>, Option<SharedIncludeError>)>;

/// State owned by one makefile update. Intermediate inspection can suppress
/// the optional policy temporarily without discarding its retained error.
#[derive(Clone)]
struct MakefileRemake {
    optional: bool,
    retain_failure: bool,
    stopped: Option<Rc<Cell<bool>>>,
    open_error: Option<SharedIncludeError>,
}

impl MakefileRemake {
    fn stopped(&self) -> bool {
        self.stopped.as_deref().is_some_and(Cell::get)
    }

    fn take_open_error(&self) -> Option<PendingIncludeError> {
        self.open_error.as_ref()?.take()
    }
}

#[derive(Clone, Copy, Default)]
struct MakefileModes {
    dry_run: bool,
    question: bool,
    touch: bool,
}

impl MakefileModes {
    fn from_options(options: &Options) -> Self {
        Self {
            dry_run: options.dry_run,
            question: options.question,
            touch: options.touch,
        }
    }

    fn apply(self, options: &mut Options) {
        options.dry_run = self.dry_run;
        options.question = self.question;
        options.touch = self.touch;
    }

    fn active(self) -> bool {
        self.dry_run || self.question || self.touch
    }

    fn check_update(self, outcome: Outcome) -> bool {
        !self.dry_run
            && !self.question
            && (matches!(
                outcome.completion,
                CompletionTime::Refresh | CompletionTime::ArchiveUnknown
            ) || (self.touch && outcome.ran))
    }
}

struct PendingMakefileRemake<'a> {
    modes: MakefileModes,
    file: &'a Makefile,
    context: MakefileRemake,
    duplicate: bool,
    active: bool,
    initial: Outcome,
}

struct OptionalMakefileFailure {
    open_error: Option<PendingIncludeError>,
    before: Option<SystemTime>,
    refresh_timestamp: bool,
}

struct FailedPrerequisite {
    target: Vec<u8>,
    parent: Vec<u8>,
}

struct PendingIncludeError {
    location: Option<Location>,
    name: Vec<u8>,
    detail: IncludeError,
}

impl PendingIncludeError {
    fn bytes(&self) -> std::borrow::Cow<'_, [u8]> {
        match &self.detail {
            IncludeError::Errno(code) => std::borrow::Cow::Owned(
                rmake_linux::filesystem::source_error_detail(&io::Error::from_raw_os_error(*code)),
            ),
            IncludeError::Bytes(detail) => std::borrow::Cow::Borrowed(detail),
        }
    }
}

/// Freshness is a per-parent decision, independent of the intermediate's final
/// timestamp: a later branch can deliberately restore an older date.
struct Inspection {
    outcome: Outcome,
    pending: bool,
    needs_rebuild: bool,
}

fn merge_child(outcome: &mut Outcome, child: Outcome) {
    if child.failed {
        outcome.deferred_spawn_failure =
            child.deferred_spawn_failure && (!outcome.failed || outcome.deferred_spawn_failure);
    }
    outcome.ran |= child.ran;
    outcome.failed |= child.failed;
    outcome.fatal |= child.fatal;
    outcome.blocked |= child.failed;
    outcome.late_input_failed |= child.late_input_failed;
    outcome.question_outdated |= child.question_outdated;
}

fn export_selected(
    environment: &VariableStore,
    globals: &VariableStore,
    unexports: &HashSet<Vec<u8>>,
    name: &[u8],
    export_all: bool,
) -> bool {
    environment.resolve_export_policy(name).unwrap_or_else(|| {
        let default_export = || {
            automatic_export_name(name)
                && environment.get(name).is_some_and(|variable| {
                    !matches!(variable.origin(), Origin::Default | Origin::Automatic)
                        && (export_all
                            || matches!(
                                variable.origin(),
                                Origin::Environment
                                    | Origin::EnvironmentOverride
                                    | Origin::CommandLine
                            ))
                })
        };
        // A reset or private global no longer supplies export policy.
        // Evaluate default eligibility from the visible target binding.
        let ignore_global_policy = globals
            .get(name)
            .is_some_and(|variable| variable.export_default_reset() || variable.is_private());
        (ignore_global_policy || !unexports.contains(name)) && default_export()
    })
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
    startup_unsigned(value)
}

/// Imported restart markers affect directory reporting; the public binding
/// excludes that marker while its original environment bytes remain owned.
fn startup_restart_value(value: &[u8]) -> &[u8] {
    value.strip_prefix(b"-").unwrap_or(value)
}

fn startup_unsigned(value: &[u8]) -> u32 {
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
fn runtime_context<'a>(
    location: Option<&'a Location>,
    output: &'a mut dyn OutputReceiver,
) -> ExpansionContext<'a> {
    ExpansionContext::with_receiver(location, output)
        .with_original_environment(crate::shell::original_environment)
        .with_native_runtime(crate::native::current())
        .with_evaluation_stack(rmake_linux::evaluation::adapter())
        .with_cancellation(rmake_linux::signals::cancellation())
}

fn emit_output(controller: &crate::output::Controller, program: &[u8], output: Output<'_>) {
    if let Output::SystemWarning { location, message } = output {
        let message = crate::messages::system(message.message());
        return emit_output(
            controller,
            program,
            Output::Warning {
                location,
                message: &message,
            },
        );
    }
    if matches!(output, Output::Info(_))
        && controller.current().is_none()
        && rmake_linux::native::stdout_closed()
    {
        return;
    }
    if crate::native::has_modules() && controller.current().is_none() {
        let _ = rmake_linux::native::flush_stream(matches!(output, Output::Warning { .. }));
    }
    match output {
        Output::Info(message) => controller.write_parts(false, &[message, b"\n"]),
        Output::SystemWarning { .. } => unreachable!("typed warning was formatted above"),
        Output::Warning { location, message } => {
            if let Some(location) = location.filter(|location| !location.file.is_null()) {
                controller.write_diagnostic_parts(&[&location_prefix(location), message, b"\n"]);
            } else {
                controller.write_diagnostic_parts(&[program, b": ", message, b"\n"]);
            }
        }
    }
}

fn emit_getcwd_error(output: &crate::output::Controller, program: &[u8], error: &io::Error) {
    let detail = os_error(error);
    let message = crate::messages::system(Message::OsError {
        action: b"getcwd",
        path: b"",
        detail: detail.as_bytes(),
    });
    emit_output(
        output,
        program,
        Output::Warning {
            location: None,
            message: &message,
        },
    );
}

struct TargetReads<'a> {
    parents: &'a HashMap<Vec<u8>, Option<ScopeParent<Vec<u8>>>>,
    retained_parents: &'a HashMap<Vec<u8>, ScopeParent<Vec<u8>>>,
    aliases: &'a Aliases,
    base: &'a HashMap<Vec<u8>, VariableStore>,
    recipes: &'a mut HashMap<Vec<u8>, VariableStore>,
    branches: &'a HashMap<(Vec<u8>, usize), VariableStore>,
}

impl TargetReads<'_> {
    fn resolve_parent<'a>(&'a self, parent: &'a ScopeParent<Vec<u8>>) -> ScopeParent<&'a [u8]> {
        ScopeParent {
            target: self.aliases.resolve(&parent.target),
            branch: parent.branch,
        }
    }
}

impl rmake_core::diagnostic::TargetReadReceiver for TargetReads<'_> {
    fn variables(&self, target: &[u8]) -> Option<&VariableStore> {
        self.recipes.get(target).or_else(|| self.base.get(target))
    }

    fn parent(&self, target: &[u8]) -> Option<Option<ScopeParent<&[u8]>>> {
        self.parents
            .get(target)
            .map(|parent| parent.as_ref().map(|parent| self.resolve_parent(parent)))
    }

    fn has_parent_changes(&self) -> bool {
        !self.retained_parents.is_empty()
    }

    fn changed_parent(&self, target: &[u8]) -> Option<ScopeParent<&[u8]>> {
        self.retained_parents
            .get(target)
            .map(|parent| self.resolve_parent(parent))
    }

    fn parent_variables(&self, parent: ScopeParent<&[u8]>) -> Option<&VariableStore> {
        if let Some(ordinal) = parent.branch {
            let mut key = (parent.target.to_vec(), ordinal);
            if let Some(scope) = self.branches.get(&key) {
                return Some(scope);
            }
            // A branch without its own table sees the head's table.
            if ordinal != 0 {
                key.1 = 0;
                if let Some(head) = self.branches.get(&key) {
                    return Some(head);
                }
            }
        }
        self.variables(parent.target)
    }

    fn retain_locals(&mut self, target: &[u8], variables: VariableStore) {
        if let Some(recipe) = self.recipes.get_mut(target) {
            *recipe = variables;
        } else {
            self.recipes.insert(target.to_vec(), variables);
        }
    }
}

/// All reader/expansion contexts observe the same live diagnostic policy.
struct RuntimeOutput<'a> {
    controller: &'a crate::output::Controller,
    program: &'a [u8],
    warnings: &'a Cell<bool>,
}
impl<'a> RuntimeOutput<'a> {
    fn new(
        controller: &'a crate::output::Controller,
        program: &'a [u8],
        warnings: &'a Cell<bool>,
    ) -> Self {
        Self {
            controller,
            program,
            warnings,
        }
    }
}
impl OutputReceiver for RuntimeOutput<'_> {
    fn loader_event(&mut self, event: rmake_core::native::LoaderEvent<'_>) {
        crate::debug::loader_event(self.controller, event);
    }

    fn guile_callback(&mut self, callback: rmake_core::native::GuileCallback<'_>) {
        crate::debug::guile_callback(self.controller, callback);
    }

    fn format_system_message(
        &mut self,
        message: rmake_core::messages::MessageRef<'_>,
    ) -> Option<Result<Vec<u8>, Vec<u8>>> {
        Some(crate::messages::try_format(message))
    }
    fn emit(&mut self, event: Output<'_>) {
        emit_output(self.controller, self.program, event);
    }
    fn warns_undefined(&self) -> bool {
        self.warnings.get()
    }
    fn set_undefined_warnings(&mut self, enabled: bool) {
        self.warnings.set(enabled);
    }
    fn reader_event(&mut self, event: rmake_core::diagnostic::ReaderEvent<'_>) {
        crate::debug::reader(self.controller, event);
    }

    fn recursive_shell_export(&mut self, definition: &Location, name: &[u8]) {
        crate::debug::recursive_shell_export(self.controller, definition, name);
    }
}

struct SerialWait {
    status: std::process::ExitStatus,
    cleaned: bool,
    #[cfg(feature = "guile")]
    interpreter_failed: bool,
}

struct ParallelJob {
    identity: u64,
    program: Vec<u8>,
    location: Location,
    ignore: bool,
    recursive: bool,
    before: Option<SystemTime>,
    phony: bool,
    continuation: RecipeState,
    stdin_owner: bool,
    waiting_load: bool,
    child: Option<rmake_linux::child::Child>,
    signal_cleaned: bool,
}

/// A rename can merge a newly visited file into one with a running recipe.
/// Keep the common single-job case inline in the primary map; overlapping
/// workers retain independent completion identities and continuations.
#[derive(Default)]
struct ParallelJobs {
    primary: HashMap<Vec<u8>, ParallelJob>,
    overlapping: HashMap<(Vec<u8>, u64), ParallelJob>,
}

impl ParallelJobs {
    fn insert(&mut self, name: Vec<u8>, job: ParallelJob) {
        match self.primary.entry(name) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(job);
            }
            std::collections::hash_map::Entry::Occupied(entry) => {
                self.overlapping
                    .insert((entry.key().clone(), job.identity), job);
            }
        }
    }

    fn remove(&mut self, name: &[u8], identity: u64) -> Option<ParallelJob> {
        if let Some(job) = self.primary.remove(name) {
            if job.identity == identity {
                return Some(job);
            }
            self.primary.insert(name.to_vec(), job);
        }
        self.overlapping.remove(&(name.to_vec(), identity))
    }

    fn get(&self, name: &[u8], identity: u64) -> Option<&ParallelJob> {
        self.primary
            .get(name)
            .filter(|job| job.identity == identity)
            .or_else(|| self.overlapping.get(&(name.to_vec(), identity)))
    }

    fn contains_key(&self, name: &[u8]) -> bool {
        self.primary.contains_key(name) || self.overlapping.keys().any(|(target, _)| target == name)
    }

    fn len(&self) -> usize {
        self.primary.len() + self.overlapping.len()
    }
    fn is_empty(&self) -> bool {
        self.primary.is_empty() && self.overlapping.is_empty()
    }
    fn entries_mut(&mut self) -> impl Iterator<Item = (&[u8], &mut ParallelJob)> {
        self.primary
            .iter_mut()
            .map(|(name, job)| (name.as_slice(), job))
            .chain(
                self.overlapping
                    .iter_mut()
                    .map(|((name, _), job)| (name.as_slice(), job)),
            )
    }

    fn values(&self) -> impl Iterator<Item = &ParallelJob> {
        self.primary.values().chain(self.overlapping.values())
    }
}

/// Rule observations borrowed until recipe dispatch.
struct RecipeInputs<'a> {
    phony: bool,
    before: Option<SystemTime>,
    branch: Option<usize>,
    implicit_group: bool,
    location: Option<&'a Location>,
    dependencies: &'a [Dependency],
    dropped: &'a HashSet<Vec<u8>>,
    normal_names: &'a HashSet<&'a [u8]>,
}

/// Interpreter-owned continuation. Workers receive only one prepared Command;
/// subsequent command configuration and exports are evaluated after completion.
struct RecipeState {
    branch: Option<usize>,
    // Snapshot the effective policy while sharing the root's one-shot error.
    remake: Option<MakefileRemake>,
    output: Option<std::rc::Rc<crate::output::Buffer>>,
    expanded: std::collections::VecDeque<(usize, Vec<u8>, Recipe)>,
    variables: VariableStore,
    flow: Flow,
    ran: bool,
    one_shell: bool,
    environment: Option<RecipeEnvironment>,
    permit: Option<Permit>,
    debug_job: Option<crate::debug::JobHandle>,
}

struct RecipeOwner {
    permit: Option<Permit>,
    debug_job: Option<crate::debug::JobHandle>,
}

struct RecipeEnvironment {
    values: Vec<(OsString, Option<OsString>)>,
    order: Vec<Vec<u8>>,
}

/// A dependency traversal can yield without holding a recipe slot. Only the
/// interpreter owns these frames; effects and secondary expansion run once.
struct RuleState {
    remake: Option<MakefileRemake>,
    branch: Option<usize>,
    rule: Rule,
    group: Option<Box<GroupState>>,
    comparison: Option<SystemTime>,
    last_epoch: Option<u64>,
    depth: usize,
    before: Option<SystemTime>,
    phony: bool,
    next: usize,
    order: Vec<usize>,
    inspections: HashMap<usize, (u64, Outcome)>,
    // Rebuild evidence is rare; keep ordinary suspended graph frames compact.
    #[allow(clippy::box_collection)]
    intermediate_stale: Option<Box<HashSet<usize>>>,
    #[allow(clippy::box_collection)]
    debug_changed_edges: Option<Box<HashSet<usize>>>,
    waited_for_prerequisites: bool,
    replay_cached_after_recipe: bool,
    last_debug_pass: u64,
    stopped: bool,
    stale: bool,
    outcome: Outcome,
    dropped: HashSet<Vec<u8>>,
    // A shuffled circular visit removes the current declaration slot while
    // suppressing the visited edge. Keep those two identities owned and separate.
    #[allow(clippy::box_collection)]
    presentation_drops: Option<Box<HashSet<usize>>>,
    deferred_intermediates: Vec<Vec<u8>>,
}

impl RuleState {
    // Ordinary branches own their prerequisite edges. A group can combine
    // prerequisites from several file records in the same traversal.
    fn scope_parent<'a>(&self, target: &'a [u8]) -> ScopeParent<&'a [u8]> {
        ScopeParent {
            target,
            branch: self.branch.filter(|_| self.group.is_none()),
        }
    }

    fn visited_prerequisites(&self) -> bool {
        self.group.is_none() && self.next == self.rule.dependencies.len()
    }

    fn visited_group_prerequisites(&self) -> bool {
        self.next == self.rule.dependencies.len()
            && self
                .group
                .as_ref()
                .is_some_and(|group| group.peer_targets.is_empty())
    }

    fn waiting_at_barrier(&self) -> bool {
        let index = self.order.get(self.next).copied().unwrap_or(self.next);
        self.group.as_ref().is_none_or(|group| group.run.implicit)
            && self
                .rule
                .dependencies
                .get(index)
                .is_some_and(|dependency| dependency.name == b".WAIT")
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BuildVisit {
    Ordinary,
    GoalPassStart,
    RevisitPrerequisite,
    PrerequisiteAfterRecipe,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PrerequisiteVisit {
    First,
    Resume,
    DebugReplay,
}

/// Double-colon branches remain sequential for one target, while their
/// unfinished dependencies yield to unrelated targets in the enclosing graph.
struct DoubleColonState {
    remake: Option<MakefileRemake>,
    synthetic_tail: Option<Box<SyntheticDoubleColonTail>>,
    depth: usize,
    before: Option<SystemTime>,
    phony: bool,
    branches: std::collections::VecDeque<(usize, Option<Rule>)>,
    active: Option<usize>,
    last_epoch: Option<u64>,
    combined: Outcome,
    awaiting_recipe: bool,
}

struct SyntheticDoubleColonTail {
    assumed_time: Option<SystemTime>,
    head: Outcome,
}

impl DoubleColonState {
    fn single_active_branch(&self) -> bool {
        self.active.is_none_or(|ordinal| ordinal == 0) && self.branches.is_empty()
    }
}

/// Only grouped rules need a second recipe view and peer traversal cursor.
struct GroupState {
    recipe_rule: Rule,
    peer_segments: Vec<GroupSegment>,
    peer_targets: std::collections::VecDeque<(Vec<u8>, usize)>,
    // Dependency-order ranges skipped at blocked peer barriers.
    wait_frontiers: Vec<std::ops::Range<usize>>,
    frontiers_enabled: bool,
    run: GroupRun,
}

struct GroupSegment {
    start: usize,
    target: Vec<u8>,
    ordinal: usize,
}

struct GroupRun {
    // The declaration identity and the recipe owner's branch index.
    double_colon: Option<(usize, usize)>,
    ordinary: bool,
    implicit: bool,
    location: Option<Location>,
    before: Option<SystemTime>,
    members: Vec<(Vec<u8>, Option<SystemTime>, bool)>,
}

type JobResult = (Vec<u8>, u64, Option<io::Result<std::process::ExitStatus>>);

#[derive(Default)]
struct ArchiveVpathReads {
    members: HashSet<Vec<u8>>,
    archives: HashSet<Vec<u8>>,
    paths: HashMap<Vec<u8>, Vec<u8>>,
}

#[derive(Clone, Copy)]
struct ArchiveCompletion {
    time: Option<SystemTime>,
    known: bool,
}

struct ArchiveLocation {
    refresh: bool,
    member_time: Option<SystemTime>,
    container_time: Option<SystemTime>,
    path: Option<Vec<u8>>,
}

enum ImplicitPresence {
    Local,
    Search(Vec<u8>),
    Absent,
}

struct ImplicitSelection {
    name: Vec<u8>,
    rule: Rule,
    prepared_scope: Option<Box<ImplicitScope>>,
    explicit_names: Vec<Vec<u8>>,
    intermediate_names: Vec<Vec<u8>>,
}

struct ImplicitScope {
    pattern: Option<TargetScope>,
    shell_status: Option<Variable>,
}

struct ImplicitSearch {
    compatibility: bool,
    archive: bool,
    intermediate_scope: Option<ImplicitScope>,
}

// A selected Makefile record can name a file that does not exist yet.
// Keep the search identity separate from its optional filesystem timestamp.
type PathObservation = Option<(Option<Vec<u8>>, Option<SystemTime>)>;

#[derive(Clone, Copy)]
enum ClockSkew {
    Unchecked,
    Through(SystemTime),
    Detected,
}

struct FailedOwner {
    target: Vec<u8>,
    location: Location,
    ignore: bool,
    output: Option<std::rc::Rc<crate::output::Buffer>>,
}

pub struct Engine {
    options: Options,
    clock_skew: Cell<ClockSkew>,
    delay_clock_checks: bool,
    deferred_clock_checks: RefCell<Vec<(Vec<u8>, SystemTime)>>,
    assumptions: Option<Box<assumptions::Files>>,
    initial_plan: Option<Box<initial::Plan>>,
    startup_directory: Option<std::path::PathBuf>,
    output: crate::output::Controller,
    reader_output: Option<std::rc::Rc<crate::output::Buffer>>,
    dispatch_output: Option<std::rc::Rc<crate::output::Buffer>>,
    fatal_outputs: Vec<std::rc::Rc<crate::output::Buffer>>,
    filesystem: RefCell<FilesystemRuntime>,
    include_search: std::rc::Rc<RefCell<IncludeSearch>>,
    flag_state: runtime_options::State,
    flag_entry_jobs: Option<usize>,
    database: Database,
    database_dumped: Cell<bool>,
    database_implicit: HashSet<Vec<u8>>,
    terminal_prerequisites: HashSet<Vec<u8>>,
    database_contexts: HashMap<Vec<u8>, VariableStore>,
    database_branch_contexts: HashMap<(Vec<u8>, usize), VariableStore>,
    database_expanded_prerequisites: Option<Box<database_output::ExpandedPrerequisites>>,
    database_branch_outcomes: HashMap<(Vec<u8>, usize), Outcome>,
    database_branch_secondary: BTreeSet<(Vec<u8>, usize)>,
    database_group_peer_times: HashMap<Vec<u8>, Vec<GroupPeerTime>>,
    database_unchecked_times: RefCell<HashSet<Vec<u8>>>,
    database_delete_policy: Cell<bool>,
    database_times: RefCell<HashMap<Vec<u8>, Option<SystemTime>>>,
    database_secondary: HashSet<Vec<u8>>,
    database_remake_started: HashSet<Vec<u8>>,
    program: Vec<u8>,
    makelevel: u32,
    inherited_exports: HashSet<Vec<u8>>,
    command_replay: bool,
    done: HashMap<Vec<u8>, Outcome>,
    #[allow(clippy::box_collection)]
    debug_considered: Option<Box<HashSet<Vec<u8>>>>,
    recipe_reaps: u64,
    commands_started: u64,
    debug_pass: u64,
    visiting: HashSet<Vec<u8>>,
    scope_parents: HashMap<Vec<u8>, Option<ScopeParent<Vec<u8>>>>,
    // A later encounter affects declarations that revisit a completed file.
    // Its first traversal context still owns the recipe and diagnostics.
    retained_scope_parents: HashMap<Vec<u8>, ScopeParent<Vec<u8>>>,
    target_contexts: HashMap<Vec<u8>, VariableStore>,
    implicit_scopes: HashMap<Vec<u8>, ImplicitScope>,
    impossible_prerequisites: HashSet<Vec<u8>>,
    impossible_generation: u64,
    would_build: bool,
    forced_query_stop: bool,
    ordinary_queries: OrdinaryQueries,
    parallel_jobs: ParallelJobs,
    aborted_recipe_permit: Option<Permit>,
    job_debug: crate::debug::Jobs,
    load_waiting: Vec<(Vec<u8>, u64)>,
    load: crate::load::Monitor,
    next_job: u64,
    pending_rules: HashMap<Vec<u8>, RuleState>,
    pending_double_colon: HashMap<Vec<u8>, DoubleColonState>,
    active_peers: HashMap<Vec<u8>, Vec<u8>>,
    running_groups: HashMap<Vec<u8>, GroupRun>,
    completed_group_branches: HashMap<usize, Outcome>,
    active_group_branches: HashSet<usize>,
    double_colon_before: HashMap<Vec<u8>, Option<SystemTime>>,
    inferred_peer_flags: HashMap<Vec<u8>, (bool, bool)>,
    graph_epoch: u64,
    graph_abort: Option<Status>,
    interrupt: Option<i32>,
    parallel_failed: bool,
    waiting_reported: Cell<bool>,
    shutdown: Cell<Shutdown>,
    shutdown_launch_failure: RefCell<Option<FailedOwner>>,
    remaking: bool,
    parallel_makefile_remakes: bool,
    defer_stopped_remake_load: bool,
    active_makefile_remake: Option<MakefileRemake>,
    // A child owns its failure policy, but source-open diagnostics follow the
    // last root visited by the remake traversal.
    remake_error_cursor: Option<SharedIncludeError>,
    // A missing cause here marks a direct, suppressed failure.
    optional_remake_failures: HashMap<Vec<u8>, Option<Rc<FailedPrerequisite>>>,
    vpath_dirs: Vec<Vec<u8>>,
    gpath_dirs: HashSet<Vec<u8>>,
    makefile_names: HashSet<Vec<u8>>,
    aliases: Aliases,
    path_observations: HashMap<Vec<u8>, PathObservation>,
    resolved_names: HashMap<Vec<u8>, Vec<u8>>,
    ignored_vpath: HashSet<Vec<u8>>,
    archive_vpath_reads: RefCell<Option<Box<ArchiveVpathReads>>>,
    archive_completion_times: HashMap<Vec<u8>, ArchiveCompletion>,
    initial_flags: Vec<u8>,
    initial_gnu_flags: Vec<u8>,
    intermediate_policy: crate::intermediates::Policy,
    #[allow(clippy::box_collection)]
    phony_names: Option<Box<HashSet<Vec<u8>>>>,
    intermediates: HashSet<Vec<u8>>,
    cleanup_candidates: Vec<Vec<u8>>,
    jobserver: Option<Jobserver>,
    unbounded_auth_jobs: bool,
    shuffle: crate::shuffle::Planner,
}

/// Startup errors can originate inside real files read by a CLI eval. Keep the
/// source identity through constructor unwinding and buffered-output flushing.
pub struct StartupError {
    pub message: Diagnostic,
    pub location: Option<Location>,
    pub version: bool,
    pub buffered: Option<std::rc::Rc<crate::output::Buffer>>,
    diagnostic_program: Option<Vec<u8>>,
}

impl std::fmt::Debug for StartupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StartupError")
            .field("message", &self.message)
            .field("location", &self.location)
            .field("version", &self.version)
            .finish_non_exhaustive()
    }
}

impl From<Diagnostic> for StartupError {
    fn from(message: Diagnostic) -> Self {
        Self {
            message,
            location: None,
            version: false,
            buffered: None,
            diagnostic_program: None,
        }
    }
}

impl StartupError {
    fn expansion(error: rmake_core::variables::ExpandError) -> Self {
        Self {
            location: error
                .diagnostic_location(None)
                .filter(|location| !location.file.is_null())
                .cloned(),
            message: error.diagnostic(),
            version: false,
            buffered: None,
            diagnostic_program: None,
        }
    }

    fn with_program(mut self, program: &[u8]) -> Self {
        self.diagnostic_program = Some(program.to_vec());
        self
    }

    pub fn prefix(&self, program: &[u8]) -> Vec<u8> {
        self.location.as_ref().map_or_else(
            || [self.diagnostic_program.as_deref().unwrap_or(program), b": "].concat(),
            location_prefix,
        )
    }
}

pub fn emit_invocation_error(
    output: &crate::output::Controller,
    program: &[u8],
    error: InvocationError,
) {
    initial::emit_error(output, program, error);
}

pub struct Invocation {
    pub options: Options,
    initial: Box<initial::Prepared>,
}
impl Invocation {
    pub fn into_native_exit(self, program: Vec<u8>) -> Option<Box<NativeExit>> {
        let mut prepared = *self.initial;
        if !crate::native::has_modules() && !prepared.output.database_requested() {
            return prepared
                .plan
                .stdin_source
                .take()
                .map(|source| NativeExit::source(source, prepared.output.clone(), program));
        }
        Some(Box::new(NativeExit::Startup(Box::new(
            startup_exit::Host {
                stdin_source: prepared.plan.stdin_source.take(),
                database: prepared.database,
                filesystem: prepared.filesystem,
                output: prepared.output,
                reader_output: prepared.reader_output,
                construction: Some(prepared.construction),
                warnings: prepared.warnings,
                program,
                level: 0,
                include_search: None,
                restore_directory: None,
                database_dumped: Cell::new(false),
            },
        ))))
    }
}

#[derive(Debug)]
pub enum InvocationError {
    Options(String),
    Usage { version: bool },
    Expansion(StartupError),
}

impl Engine {
    #[cfg(test)]
    pub fn invocation(
        args: impl IntoIterator<Item = OsString>,
        program: &[u8],
        invocation: &OsStr,
        makeflags: Option<OsString>,
        gnumakeflags: Option<OsString>,
    ) -> Result<Invocation, Failure<InvocationError>> {
        Self::invocation_with_output(args, program, invocation, makeflags, gnumakeflags, None)
    }
    pub fn invocation_with_output(
        args: impl IntoIterator<Item = OsString>,
        program: &[u8],
        invocation: &OsStr,
        makeflags: Option<OsString>,
        gnumakeflags: Option<OsString>,
        output: Option<crate::output::Controller>,
    ) -> Result<Invocation, Failure<InvocationError>> {
        let args: Vec<_> = args.into_iter().collect();
        let plan = initial::Plan::new(args, program, invocation, makeflags, gnumakeflags, output);
        Box::new(plan).decode()
    }
}

impl Engine {
    pub fn from_invocation(
        launch: Invocation,
        mut program: Vec<u8>,
        invocation: &OsStr,
    ) -> Result<Self, Failure<StartupError>> {
        let Invocation { options, initial } = launch;
        let initial::Prepared {
            mut database,
            filesystem,
            output,
            mut reader_output,
            mut construction,
            exports: mut inherited_exports,
            seed,
            mut plan,
            initial_directory,
            shuffle,
            warnings: startup_warnings,
        } = *initial;
        let mut exit_level = 0;
        let mut exit_includes = None;
        let mut exit_directory = None;
        macro_rules! retain {
            ($operation:expr) => {
                match $operation {
                    Ok(value) => value,
                    Err(error) => {
                        let error: StartupError = error.into();
                        let exit = if crate::native::has_modules() || output.database_requested() {
                            drop(construction);
                            Some(Box::new(NativeExit::Startup(Box::new(
                                startup_exit::Host {
                                    stdin_source: plan.stdin_source.take(),
                                    database,
                                    filesystem,
                                    output,
                                    reader_output,
                                    construction: None,
                                    warnings: startup_warnings,
                                    program,
                                    level: exit_level,
                                    include_search: exit_includes,
                                    restore_directory: exit_directory,
                                    database_dumped: Cell::new(false),
                                },
                            ))))
                        } else {
                            plan.stdin_source.take().map(|source| {
                                NativeExit::source(source, output.clone(), program.clone())
                            })
                        };
                        return Err(Failure { error, exit });
                    }
                }
            };
        }
        let mode = retain!(options.output_sync.as_deref().map_or(
            Ok(crate::output::Mode::None),
            |raw| {
                crate::output::Mode::parse(raw)
                    .ok_or_else(|| Diagnostic::from(crate::usage::output_sync_mode(raw)))
            }
        ));
        if options.debug_mask.has(crate::debug::Mask::BASIC) {
            output.print_compatibility_header(options.print_database);
        }
        rmake_linux::native::line_buffer_stdout();
        output.set_database_requested(options.print_database);
        output.set_debug_mask(options.debug_mask);
        // Explicit switches select the active context only after every CLI
        // effect and early exit. Closing an inherited spool still uses level 0.
        output.set_mode(mode);
        if !mode.root_buffered() {
            output.select(None);
            if let Some(buffer) = reader_output.take() {
                buffer.dump();
            }
        } else if reader_output.is_none() {
            reader_output = output.reader_buffer();
        }
        output.select(reader_output.as_ref());
        construction.replace_buffer(reader_output.clone());
        let makelevel = startup_makelevel(
            database
                .variables
                .database_lookup(b"MAKELEVEL")
                .map_or(b"".as_slice(), |variable| variable.raw_value()),
        );
        exit_level = makelevel;
        rmake_linux::output::set_stdout_level(makelevel);
        output.set_recursive(makelevel != 0);
        let program_base_length = program.len();
        if makelevel != 0 {
            program.extend_from_slice(format!("[{makelevel}]").as_bytes());
        }
        output.set_program(&program, program_base_length, makelevel);
        let relative_command =
            !Path::new(invocation).is_absolute() && invocation.as_bytes().contains(&b'/');
        let startup_directory = if !options.directories.is_empty() || relative_command {
            initial_directory.clone()
        } else {
            None
        };
        let make_command = if relative_command {
            initial_directory.as_ref().map_or_else(
                || invocation.to_owned(),
                |cwd| cwd.join(invocation).into_os_string(),
            )
        } else {
            invocation.to_owned()
        };
        output.set_imported_directory(Some(
            initial_directory
                .as_ref()
                .map_or(b"".as_slice(), |path| path.as_os_str().as_bytes()),
        ));
        if !options.directories.is_empty() {
            exit_directory = startup_directory.clone();
        }
        for directory in &options.directories {
            if let Some(history) = database.database_history() {
                history.intern(directory.as_os_str().as_bytes());
            }
            retain!(std::env::set_current_dir(directory).map_err(|error| {
                StartupError::from(Diagnostic::from(
                    [
                        directory.as_os_str().as_bytes(),
                        b": ",
                        os_error(&error).as_bytes(),
                    ]
                    .concat(),
                ))
                .with_program(&program)
            }));
        }
        // GNU retains the initial logical directory without -C. An initial
        // getcwd failure leaves an empty named path. After -C, a failed refresh
        // keeps the prior CURDIR bytes and publishes an unknown reporting path.
        let cwd = if options.directories.is_empty() {
            initial_directory.unwrap_or_default()
        } else {
            match std::env::current_dir() {
                Ok(cwd) => {
                    output.set_imported_directory(Some(cwd.as_os_str().as_bytes()));
                    cwd
                }
                Err(error) => {
                    emit_getcwd_error(&output, &program, &error);
                    output.set_imported_directory(None);
                    initial_directory.unwrap_or_default()
                }
            }
        };
        for goal in &options.goals {
            // Ordered argv preparation has already entered ordinary goals.
            // Retain support for callers constructing an invocation directly.
            if !database.file_records.contains(goal) {
                database.file_records.insert(goal);
            }
        }
        database
            .variables
            .set_environment_override(options.environment_override);
        retain!(
            database
                .variables
                .define_literal(b"CURDIR", cwd.as_os_str().as_bytes(), Origin::File)
                .map_err(|error| StartupError::expansion(error).with_program(&program))
        );
        database
            .variables
            .set_working_directory(cwd.as_os_str().as_bytes());
        for (name, descriptor) in [
            (b"MAKE_TERMOUT".as_slice(), 1),
            (b"MAKE_TERMERR".as_slice(), 2),
        ] {
            if let Some(value) = rmake_linux::output::terminal(descriptor)
                && database.variables.database_lookup(name).is_none()
            {
                retain!(
                    database
                        .variables
                        .define_literal(name, &value, Origin::Default)
                        .map_err(StartupError::expansion)
                );
                database.variables.set_export_policy(name, Some(true));
            }
        }
        startup_warnings.set(options.warn_undefined);
        let include_search = retain!(startup::include_metadata(
            &mut database,
            &options,
            startup::Runtime {
                program: &program,
                level: makelevel,
                output: &output,
                filesystem: &filesystem,
                warnings: &startup_warnings,
            },
            &mut exit_includes,
        ));
        retain!(output.activate_debug_signal().map_err(|error| {
            StartupError::from(Diagnostic::from(error.to_string())).with_program(&program)
        }));
        retain!(
            database
                .variables
                .define_literal(b"MAKE_COMMAND", make_command.as_bytes(), Origin::Default)
                .map_err(StartupError::expansion)
        );
        retain!(
            database
                .variables
                .define(
                    b"MAKE",
                    b"$(MAKE_COMMAND)",
                    Assignment::Recursive,
                    Origin::Default,
                )
                .map_err(StartupError::expansion)
        );
        let command_replay = retain!(
            crate::command_variables::prepare(&mut database.variables, &seed.commands)
                .map_err(StartupError::expansion)
        );
        retain!(
            database
                .initialize_suffixes(!options.no_rules)
                .map_err(StartupError::expansion)
        );
        database
            .variables
            .reserve_environment_layout(&[b"MFLAGS"], true);
        if !options.no_rules {
            retain!(
                database
                    .initialize_builtin_rules()
                    .map_err(|error| error.message)
            );
        }
        database
            .variables
            .reserve_environment_layout(&[b"MAKE_HOST", b".DEFAULT_GOAL"], false);
        database
            .variables
            .reserve_environment_layout(&[b"MFLAGS"], true);
        retain!(
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
                .map_err(StartupError::expansion)
        );
        retain!(
            database
                .variables
                .define_literal(b"MAKE_VERSION", b"4.4.1", Origin::Default)
                .map_err(StartupError::expansion)
        );
        retain!(
            database
                .variables
                .define_literal(b"MAKE_HOST", b"x86_64-pc-linux-gnu", Origin::Default)
                .map_err(StartupError::expansion)
        );
        retain!(
            database
                .variables
                .initialize_shell_default(b"/bin/sh")
                .map_err(StartupError::expansion)
        );
        retain!(
            database
                .variables
                .define_literal(b"MAKEFILES", b"", Origin::Default)
                .map_err(StartupError::expansion)
        );
        database.variables.set_export_if_set(b"MAKEFILES");
        retain!(
            define_automatic_variants(&mut database.variables).map_err(StartupError::expansion)
        );
        retain!(
            database
                .variables
                .update_mflags(&options.recursive_flags(), options.environment_override)
                .map_err(StartupError::expansion)
        );
        inherited_exports.insert(b"MFLAGS".to_vec());
        let mut initial_flags = options.makeflags();
        database
            .variables
            .database_lookup(database.command_overrides_name());
        if command_replay {
            initial_flags.extend_from_slice(b" -- $(MAKEOVERRIDES)");
        }
        retain!(
            database
                .define_canonical_makeflags(
                    &initial_flags,
                    runtime_options::origin(options.environment_override),
                )
                .map_err(StartupError::expansion)
        );
        database
            .variables
            .set_export_policy(b"MAKEFLAGS", Some(true));
        if !options.no_variables {
            retain!(
                database
                    .initialize_builtin_variables()
                    .map_err(StartupError::expansion)
            );
            retain!(
                database
                    .variables
                    .define(b"GNUMAKEFLAGS", b"", Assignment::Recursive, Origin::Default)
                    .map_err(StartupError::expansion)
            );
        }
        if let Some(history) = database.database_history() {
            history.intern(b".DEFAULT");
        }
        database.file_records.insert(b".DEFAULT");
        retain!(
            database
                .variables
                .define_literal(b".DEFAULT_GOAL", b"", Origin::File)
                .map_err(StartupError::expansion)
        );
        let flag_state =
            runtime_options::State::new(&options, output.clone(), include_search.clone());
        let flag_entry_jobs = options.jobs;
        let assumptions = retain!(
            assumptions::Files::from_options(&options)
                .map_err(|message| StartupError::from(Diagnostic::from(message.to_vec())))
        );
        let initial_plan = Some(plan);
        let mut engine = Self {
            initial_plan,
            assumptions,
            clock_skew: Cell::new(ClockSkew::Unchecked),
            delay_clock_checks: false,
            deferred_clock_checks: RefCell::new(Vec::new()),
            startup_directory,
            output,
            reader_output,
            dispatch_output: None,
            fatal_outputs: Vec::new(),
            options,
            filesystem,
            include_search,
            flag_state,
            flag_entry_jobs,
            database,
            database_dumped: Cell::new(false),
            database_implicit: HashSet::new(),
            terminal_prerequisites: HashSet::new(),
            database_contexts: HashMap::new(),
            database_branch_contexts: HashMap::new(),
            database_expanded_prerequisites: None,
            database_branch_outcomes: HashMap::new(),
            database_branch_secondary: BTreeSet::new(),
            database_group_peer_times: HashMap::new(),
            database_unchecked_times: RefCell::new(HashSet::new()),
            database_delete_policy: Cell::new(false),
            database_times: RefCell::new(HashMap::new()),
            database_secondary: HashSet::new(),
            database_remake_started: HashSet::new(),
            program,
            makelevel,
            inherited_exports,
            command_replay,
            done: HashMap::new(),
            debug_considered: None,
            recipe_reaps: 0,
            commands_started: 0,
            debug_pass: 0,
            visiting: HashSet::new(),
            scope_parents: HashMap::new(),
            retained_scope_parents: HashMap::new(),
            target_contexts: HashMap::new(),
            implicit_scopes: HashMap::new(),
            impossible_prerequisites: HashSet::new(),
            impossible_generation: 0,
            would_build: false,
            forced_query_stop: false,
            ordinary_queries: OrdinaryQueries::default(),
            parallel_jobs: ParallelJobs::default(),
            aborted_recipe_permit: None,
            job_debug: crate::debug::Jobs::default(),
            load_waiting: Vec::new(),
            load: crate::load::Monitor::default(),
            next_job: 0,
            pending_rules: HashMap::new(),
            pending_double_colon: HashMap::new(),
            active_peers: HashMap::new(),
            running_groups: HashMap::new(),
            completed_group_branches: HashMap::new(),
            active_group_branches: HashSet::new(),
            double_colon_before: HashMap::new(),
            inferred_peer_flags: HashMap::new(),
            graph_epoch: 0,
            graph_abort: None,
            interrupt: None,
            parallel_failed: false,
            waiting_reported: Cell::new(false),
            shutdown: Cell::new(Shutdown::Running),
            shutdown_launch_failure: RefCell::new(None),
            remaking: false,
            parallel_makefile_remakes: false,
            defer_stopped_remake_load: false,
            active_makefile_remake: None,
            remake_error_cursor: None,
            optional_remake_failures: HashMap::new(),
            vpath_dirs: Vec::new(),
            gpath_dirs: HashSet::new(),
            makefile_names: HashSet::new(),
            aliases: Aliases::default(),
            path_observations: HashMap::new(),
            resolved_names: HashMap::new(),
            ignored_vpath: HashSet::new(),
            archive_vpath_reads: RefCell::new(None),
            archive_completion_times: HashMap::new(),
            initial_flags,
            initial_gnu_flags: Vec::new(),
            intermediate_policy: crate::intermediates::Policy::default(),
            phony_names: None,
            intermediates: HashSet::new(),
            cleanup_candidates: Vec::new(),
            jobserver: None,
            unbounded_auth_jobs: false,
            shuffle: crate::shuffle::Planner::new(shuffle),
        };
        // Inherited pipe descriptors must become close-on-exec before reader
        // shell functions run. Creating a new owned pool waits until after read.
        let completed = (|| -> Result<(), StartupError> {
            engine.setup_jobserver(false)?;
            let mut flags = engine.options.makeflags();
            if engine.command_replay {
                flags.extend_from_slice(b" -- $(MAKEOVERRIDES)");
            }
            if flags != engine.initial_flags {
                engine
                    .database
                    .variables
                    .update_mflags(
                        &engine.options.recursive_flags(),
                        engine.options.environment_override,
                    )
                    .map_err(|error| error.diagnostic())?;
                engine
                    .database
                    .define_canonical_makeflags(
                        &flags,
                        runtime_options::origin(engine.options.environment_override),
                    )
                    .map_err(|error| error.diagnostic())?;
                engine.initial_flags = flags;
            }
            engine.flag_state.refresh(&engine.options);
            Ok(())
        })();
        if let Err(error) = completed {
            let exit = if crate::native::has_modules() || engine.output.database_requested() {
                drop(construction);
                Some(Box::new(NativeExit::Engine(Box::new(engine))))
            } else {
                None
            };
            return Err(Failure { error, exit });
        }
        construction.disarm();
        Ok(engine)
    }

    fn expand(
        &mut self,
        variables: &mut VariableStore,
        bytes: &[u8],
        location: Option<&Location>,
    ) -> Result<Vec<u8>, ExpandError> {
        let mut target_reads = TargetReads {
            parents: &self.scope_parents,
            retained_parents: &self.retained_scope_parents,
            aliases: &self.aliases,
            base: &self.target_contexts,
            recipes: &mut self.database_contexts,
            branches: &self.database_branch_contexts,
        };
        let result = self.database.expand_in(
            variables,
            bytes,
            runtime_context(
                location,
                &mut RuntimeOutput::new(
                    &self.output,
                    &self.program,
                    &self.flag_state.warn_undefined,
                ),
            )
            .with_makeflags(&mut self.flag_state.receiver())
            .with_target_reads(&mut target_reads)
            .with_filesystem(&mut |effect| self.filesystem.borrow_mut().execute(effect))
            .with_shell(&mut |request| {
                crate::shell::run(
                    request,
                    &self.program,
                    self.makelevel,
                    &self.output,
                    self.jobserver.as_ref(),
                    || self.filesystem.borrow_mut().child_completed(),
                )
            })
            .with_includes(&mut |name| self.include_search.borrow().read(name)),
        );
        if let Some(options) = self.flag_state.take() {
            self.options = options;
        }
        result
    }

    fn expand_global(
        &mut self,
        bytes: &[u8],
        location: Option<&Location>,
    ) -> Result<Vec<u8>, ExpandError> {
        let result = self.database.expand_with(
            bytes,
            runtime_context(
                location,
                &mut RuntimeOutput::new(
                    &self.output,
                    &self.program,
                    &self.flag_state.warn_undefined,
                ),
            )
            .with_makeflags(&mut self.flag_state.receiver())
            .with_filesystem(&mut |effect| self.filesystem.borrow_mut().execute(effect))
            .with_shell(&mut |request| {
                crate::shell::run(
                    request,
                    &self.program,
                    self.makelevel,
                    &self.output,
                    self.jobserver.as_ref(),
                    || self.filesystem.borrow_mut().child_completed(),
                )
            })
            .with_includes(&mut |name| self.include_search.borrow().read(name)),
        );
        if let Some(options) = self.flag_state.take() {
            self.options = options;
        }
        result
    }

    fn native_module_operation(
        &mut self,
        name: &[u8],
        location: Option<&Location>,
        reload: bool,
    ) -> Result<bool, ExpandError> {
        let result = {
            let mut output =
                RuntimeOutput::new(&self.output, &self.program, &self.flag_state.warn_undefined);
            let mut flags = self.flag_state.receiver();
            let mut filesystem = |effect: rmake_core::diagnostic::FilesystemEffect<'_>| {
                self.filesystem.borrow_mut().execute(effect)
            };
            let mut shell = |request: rmake_core::diagnostic::ShellRequest<'_>| {
                crate::shell::run(
                    request,
                    &self.program,
                    self.makelevel,
                    &self.output,
                    self.jobserver.as_ref(),
                    || self.filesystem.borrow_mut().child_completed(),
                )
            };
            let mut include = |name: &[u8]| self.include_search.borrow().read(name);
            let mut context = runtime_context(location, &mut output)
                .with_makeflags(&mut flags)
                .with_filesystem(&mut filesystem)
                .with_shell(&mut shell)
                .with_includes(&mut include);
            if reload {
                self.database
                    .load_native_with(
                        name,
                        false,
                        location.expect("module load location"),
                        &mut context,
                    )
                    .map(|_| true)
            } else {
                self.database.unload_native_with(name, &mut context)
            }
        };
        if let Some(options) = self.flag_state.take() {
            self.options = options;
        }
        result
    }

    pub fn requires_retained_exit(&self) -> bool {
        crate::native::has_modules()
    }

    pub fn prepare_native_exit(&mut self) {
        // Keep semantic state alive for libc finalizers, while releasing the
        // scheduler resources normally released when the engine is dropped.
        self.wait_all_jobs();
        self.parallel_jobs = ParallelJobs::default();
        self.load_waiting.clear();
        self.jobserver.take();
    }

    pub fn native_exit(&mut self, mut status: u8) -> u8 {
        self.output.process_lifetime().finish();
        if !self.options.directories.is_empty()
            && let Some(directory) = &self.startup_directory
        {
            let _ = std::env::set_current_dir(directory);
        }
        if !crate::native::has_modules() {
            return status;
        }
        loop {
            let result = self.terminate_native(rmake_linux::signals::ExitAction::Code(status));
            match result {
                Ok(()) => return status,
                Err(error) => {
                    self.expansion_error(None, &error);
                    status = 2;
                    if !crate::native::can_retry_exit() {
                        return status;
                    }
                }
            }
        }
    }

    fn with_retained_native_context<T>(
        &mut self,
        operation: impl FnOnce(&mut Database, &mut ExpansionContext<'_>) -> Result<T, ExpandError>,
    ) -> Result<T, ExpandError> {
        let mut output =
            RuntimeOutput::new(&self.output, &self.program, &self.flag_state.warn_undefined);
        let mut flags = self.flag_state.receiver();
        let mut filesystem = |effect: rmake_core::diagnostic::FilesystemEffect<'_>| {
            self.filesystem.borrow_mut().execute(effect)
        };
        let mut shell = |request: rmake_core::diagnostic::ShellRequest<'_>| {
            crate::shell::run(
                request,
                &self.program,
                self.makelevel,
                &self.output,
                self.jobserver.as_ref(),
                || self.filesystem.borrow_mut().child_completed(),
            )
        };
        let mut include = |name: &[u8]| self.include_search.borrow().read(name);
        let mut context = runtime_context(None, &mut output)
            .with_makeflags(&mut flags)
            .with_filesystem(&mut filesystem)
            .with_shell(&mut shell)
            .with_includes(&mut include);
        operation(&mut self.database, &mut context)
    }

    fn terminate_native(
        &mut self,
        action: rmake_linux::signals::ExitAction,
    ) -> Result<(), ExpandError> {
        self.with_retained_native_context(|database, context| match action {
            rmake_linux::signals::ExitAction::Code(status) => {
                database.terminate_native_with(status, context)
            }
            rmake_linux::signals::ExitAction::Signal(number) => {
                database.terminate_native_signal_with(number, context)
            }
        })
    }

    #[cfg(feature = "guile")]
    fn service_waiting_workers(&mut self) -> Result<(), ExpandError> {
        let result =
            self.with_retained_native_context(Database::service_waiting_native_workers_with);
        if let Some(options) = self.flag_state.take() {
            self.options = options;
        }
        result
    }

    #[cfg(feature = "guile")]
    fn service_parallel_workers(&mut self) -> bool {
        match self.service_waiting_workers() {
            Ok(()) => {
                self.observe_interrupt();
                self.interrupt.is_none()
            }
            Err(ExpandError::Cancelled) => {
                self.observe_interrupt();
                false
            }
            Err(error) => {
                self.expansion_error(None, &error);
                self.parallel_failed = true;
                self.record_graph_failure(Outcome {
                    failed: true,
                    fatal: true,
                    ..Outcome::default()
                });
                false
            }
        }
    }

    #[cfg(feature = "guile")]
    pub fn native_signal_exit(&mut self, number: i32) -> u8 {
        self.output.process_lifetime().finish();
        if !self.options.directories.is_empty()
            && let Some(directory) = &self.startup_directory
        {
            let _ = std::env::set_current_dir(directory);
        }
        if let Err(error) = self.terminate_native(rmake_linux::signals::ExitAction::Signal(number))
        {
            self.expansion_error(None, &error);
        }
        // A valid platform signal operation never returns. The bootstrap
        // contract guard retains this host and fail-stops if it does.
        2
    }

    fn with_makefile_remake<T>(
        &mut self,
        remake: Option<MakefileRemake>,
        visit: impl FnOnce(&mut Self) -> T,
    ) -> T {
        if remake.is_none() && self.active_makefile_remake.is_none() {
            return visit(self);
        }
        let previous = std::mem::replace(&mut self.active_makefile_remake, remake);
        let result = visit(self);
        self.active_makefile_remake = previous;
        result
    }

    fn remake_stop(&self) -> Option<&Rc<Cell<bool>>> {
        self.active_makefile_remake.as_ref()?.stopped.as_ref()
    }

    fn remake_stopped(&self) -> bool {
        self.remake_stop().is_some_and(|stopped| stopped.get())
    }

    fn optional_remake(&self) -> bool {
        self.active_makefile_remake
            .as_ref()
            .is_some_and(|remake| remake.optional)
    }

    fn retain_remake_failure(&self) -> bool {
        self.active_makefile_remake
            .as_ref()
            .is_some_and(|remake| remake.retain_failure)
    }

    fn has_optional_remake_failure(&self, name: &[u8]) -> bool {
        self.optional_remake_failures
            .contains_key(self.aliases.resolve(name))
    }

    fn cached_optional_group_peer_failure(&self, name: &[u8]) -> bool {
        let identity = self.aliases.resolve(name);
        self.done
            .get(identity)
            .is_some_and(|outcome| outcome.optional_group_peer_failure)
            && !self.optional_remake_failures.contains_key(identity)
    }

    fn cached_recipe_outcome(&self, mut outcome: Outcome) -> Outcome {
        if !self.remaking && self.active_makefile_remake.is_some() {
            // A recipe left running by a stopped optional remake can finish
            // during ordinary goals without becoming ordinary recipe activity.
            outcome.ran = false;
        }
        outcome
    }

    fn show_makefile_error(&self) {
        self.show_remake_open_error(self.active_makefile_remake.as_ref());
    }

    fn show_remake_open_error(&self, remake: Option<&MakefileRemake>) {
        if let Some(error) = remake.and_then(MakefileRemake::take_open_error) {
            self.emit_makefile_error(&error);
        }
    }

    fn show_recipe_makefile_error(&self, remake: Option<&MakefileRemake>) {
        if self.parallel_makefile_remakes {
            if let Some(error) = self
                .remake_error_cursor
                .as_ref()
                .and_then(|pending| pending.take())
            {
                self.emit_makefile_error(&error);
            }
        } else {
            self.show_remake_open_error(remake);
        }
    }

    fn emit_makefile_error(&self, error: &PendingIncludeError) {
        // The ordinary remake complaint has fixed punctuation. Only the
        // errno detail comes from the active libc messages locale.
        let detail = error.bytes();
        emit_output(
            &self.output,
            &self.program,
            Output::Warning {
                location: error.location.as_ref(),
                message: &[error.name.as_slice(), b": ", &detail].concat(),
            },
        );
    }

    fn stderr(&self, body: &[u8]) {
        self.output
            .write_parts(true, &[&self.program, b": ", body, b"\n"]);
    }

    fn clock_candidate(&self, time: Option<SystemTime>) -> Option<SystemTime> {
        let time = time?;
        match self.clock_skew.get() {
            ClockSkew::Detected => None,
            ClockSkew::Through(limit) if time <= limit => None,
            _ => Some(time),
        }
    }

    fn check_clock_skew(&self, name: &[u8], time: Option<SystemTime>) {
        let Some(time) = self.clock_candidate(time) else {
            return;
        };
        if self
            .assumptions
            .as_ref()
            .is_some_and(|files| files.is_new_time(Some(time)) || files.is_old_time(Some(time)))
        {
            return;
        }
        if self.delay_clock_checks {
            self.deferred_clock_checks
                .borrow_mut()
                .push((name.to_vec(), time));
            return;
        }
        let Some((now, resolution)) = rmake_linux::clock::realtime() else {
            return;
        };
        let limit = now
            .checked_add(resolution - Duration::from_nanos(1))
            .unwrap_or(now);
        self.clock_skew.set(ClockSkew::Through(limit));
        if time > limit
            && let Some(seconds) = rmake_linux::clock::future_offset(time, now)
        {
            self.stderr(&crate::messages::system(
                rmake_core::messages::Message::FutureFileTime {
                    name,
                    seconds: &seconds,
                },
            ));
            self.clock_skew.set(ClockSkew::Detected);
        }
    }

    fn check_considered_goal_clock(&mut self) {
        if !self.deferred_clock_checks.get_mut().is_empty() {
            for (name, time) in std::mem::take(self.deferred_clock_checks.get_mut()) {
                self.check_clock_skew(&name, Some(time));
            }
        }
    }

    fn warn_circular_dependency(&self, target: &[u8], prerequisite: &[u8]) {
        self.stderr(&crate::messages::system(
            rmake_core::messages::Message::CircularDependency {
                target,
                prerequisite,
            },
        ));
    }

    fn stdout(&self, body: &[u8]) {
        self.output.write_parts(false, &[body, b"\n"]);
    }

    fn fatal(&self, location: Option<&Location>, message: &[u8]) {
        self.fatal_with_owner(location, message, false);
    }

    fn fatal_system(&self, message: rmake_core::messages::Message<'_>) {
        self.fatal(None, &crate::messages::system(message));
    }

    fn fatal_with_owner(&self, location: Option<&Location>, message: &[u8], owner: bool) {
        self.fatal_parts(location, message, &crate::messages::stop(), owner);
    }

    fn fatal_diagnostic(&self, location: Option<&Location>, message: &Diagnostic, owner: bool) {
        self.fatal_parts(
            location,
            &crate::messages::diagnostic(message),
            &crate::messages::stop_for(message),
            owner,
        );
    }

    fn fatal_parts(&self, location: Option<&Location>, message: &[u8], stop: &[u8], owner: bool) {
        if rmake_linux::signals::pending().is_some() {
            return;
        }
        let mut shutdown = self.shutdown.get();
        let wait = shutdown.report(owner);
        self.shutdown.set(shutdown);
        if let Some(location) = location.filter(|location| !location.file.is_null()) {
            self.output
                .write_parts(true, &[&location_prefix(location), b"*** ", message, stop]);
        } else {
            self.output
                .write_parts(true, &[&self.program, b": *** ", message, stop]);
        }
        self.unlink_stdin();
        if wait {
            self.notice_waiting_for_jobs();
        }
    }

    fn recipe_expansion_error(
        &self,
        target: &[u8],
        location: &Location,
        error: &ExpandError,
        flow: &Flow,
        ignore: bool,
        output: Option<&std::rc::Rc<crate::output::Buffer>>,
    ) {
        let first = self.shutdown.get() == Shutdown::Running;
        self.fatal_diagnostic(
            error.diagnostic_location(Some(location)),
            &error.diagnostic(),
            flow.retained_owner(),
        );
        if first && flow.retained_owner() && flow.failed_launch() {
            *self.shutdown_launch_failure.borrow_mut() = Some(FailedOwner {
                target: target.to_vec(),
                location: location.clone(),
                ignore,
                output: output.cloned(),
            });
        }
    }

    fn expansion_error(&self, fallback: Option<&Location>, error: &ExpandError) {
        self.fatal_diagnostic(
            error.diagnostic_location(fallback),
            &error.diagnostic(),
            false,
        );
    }

    fn enter_preload_file<'a>(&mut self, name: &'a [u8]) -> &'a [u8] {
        // An existing normalized record takes precedence over a new spelling.
        let lookup = rmake_core::filenames::without_dot_prefix(name);
        if lookup.len() < name.len() && self.database.file_records.contains(lookup) {
            return lookup;
        }
        self.database.file_records.insert(name);
        name
    }

    fn read_startup_makefiles(&mut self) -> Result<bool, ()> {
        if self
            .database
            .variables
            .database_lookup(b"MAKEFILES")
            .is_none_or(|value| value.raw_value().is_empty())
        {
            return Ok(false);
        }
        let names = match self.expand_global(b"$(MAKEFILES)", None) {
            Ok(names) => names,
            Err(error) => {
                self.expansion_error(None, &error);
                return Err(());
            }
        };
        let mut found = false;
        // MAKEFILES uses ordinary words: no globbing or backslash filename list
        // decoding. Freeze the expanded list before any preload changes it.
        for spelling in words(&names) {
            crate::debug::reader(
                &self.output,
                rmake_core::diagnostic::ReaderEvent::Reading {
                    name: spelling,
                    search: true,
                    optional: true,
                    no_default_goal: true,
                    no_tilde: false,
                },
            );
            let path = match crate::makefiles::expand_preload_name(
                spelling,
                &mut self.database.variables,
                runtime_context(
                    None,
                    &mut RuntimeOutput::new(
                        &self.output,
                        &self.program,
                        &self.flag_state.warn_undefined,
                    ),
                )
                .with_makeflags(&mut self.flag_state.receiver())
                .with_filesystem(&mut |effect| self.filesystem.borrow_mut().execute(effect))
                .with_shell(&mut |request| {
                    crate::shell::run(
                        request,
                        &self.program,
                        self.makelevel,
                        &self.output,
                        self.jobserver.as_ref(),
                        || self.filesystem.borrow_mut().child_completed(),
                    )
                }),
            ) {
                Ok(path) => path,
                Err(error) => {
                    self.expansion_error(None, &error);
                    return Err(());
                }
            };
            let name = path.as_slice();
            let input = self.include_search.borrow().read(name);
            let file = match input {
                Ok(file) => file,
                Err(IncludeFailure::Deferred {
                    resolved_name,
                    detail,
                }) => {
                    let filename =
                        self.enter_preload_file(resolved_name.as_deref().unwrap_or(name));
                    self.database.makefiles.push(Makefile {
                        name: filename.to_vec(),
                        optional: true,
                        missing: true,
                        location: None,
                        read_error: Some(detail),
                    });
                    continue;
                }
                Err(IncludeFailure::Fatal(detail)) => {
                    self.fatal(None, &detail);
                    return Err(());
                }
            };
            let filename = self.enter_preload_file(&file.name);
            found = true;
            self.database.makefiles.push(Makefile {
                name: filename.to_vec(),
                optional: true,
                missing: false,
                location: None,
                read_error: None,
            });
            if let Err(error) = self.database.variables.append_literal_at(
                b"MAKEFILE_LIST",
                filename,
                Origin::File,
                &Location {
                    file: spelling.into(),
                    line: 1,
                },
            ) {
                self.expansion_error(None, &error);
                return Err(());
            }
            if let Err(error) = self.database.read_preload_input(
                file.input,
                spelling,
                runtime_context(
                    None,
                    &mut RuntimeOutput::new(
                        &self.output,
                        &self.program,
                        &self.flag_state.warn_undefined,
                    ),
                )
                .with_makeflags(&mut self.flag_state.receiver())
                .with_filesystem(&mut |effect| self.filesystem.borrow_mut().execute(effect))
                .with_shell(&mut |request| {
                    crate::shell::run(
                        request,
                        &self.program,
                        self.makelevel,
                        &self.output,
                        self.jobserver.as_ref(),
                        || self.filesystem.borrow_mut().child_completed(),
                    )
                })
                .with_includes(&mut |name| self.include_search.borrow().read(name)),
            ) {
                self.fatal_diagnostic(error.source_location(), &error.message, false);
                return Err(());
            }
        }
        Ok(found)
    }

    pub fn run_retained(&mut self) -> u8 {
        let mut status = self.run_inner();
        self.unlink_stdin();
        if self.interrupt.is_none() && rmake_linux::signals::pending().is_some() {
            self.observe_interrupt();
            self.wait_all_jobs();
            self.remove_intermediates();
        }
        if let Some(owner) = self.output.take_interrupted_owner() {
            self.output.select(None);
            let number = rmake_linux::signals::pending().expect("interrupted output lock");
            if rmake_linux::signals::deletes_targets(number) {
                self.cleanup_target(
                    &owner.target,
                    owner.before,
                    owner.phony,
                    Cleanup::ChangedOnSignal,
                );
                self.stderr(b"*** wait: No child processes.  Stop.");
                self.notice_waiting_for_jobs();
            } else {
                self.notice_waiting_for_jobs();
                self.stderr(b"*** wait: No child processes.  Stop.");
            }
            self.stderr(b"*** wait: No child processes.  Stop.");
            self.shutdown.set(Shutdown::Reentered);
            rmake_linux::signals::terminal_status(2);
            status = 2;
        }
        if self.interrupt.is_none() && self.shutdown.get() != Shutdown::Reentered {
            if let Some(buffer) = self.output.current() {
                buffer.dump();
            }
            if let Some(buffer) = self.reader_output.take() {
                buffer.dump();
            }
        }
        self.output.select(None);
        self.job_debug.drain(
            self.output.debug_mask(),
            self.jobserver.as_ref().and_then(Jobserver::owned_slots),
        );
        if self.interrupt.is_none() || self.shutdown.get() == Shutdown::Reentered {
            self.print_database();
        }
        let aborted_permit = self.aborted_recipe_permit.take();
        self.release_completed_permit(aborted_permit);
        status
    }

    fn read_command_evals(&mut self) -> Result<(), ()> {
        if self.options.evals.is_empty() {
            return Ok(());
        }
        let mut index = 0;
        while let Some(bytes) = self.options.evals.get(index).cloned() {
            index += 1;
            let result = self.database.read_evaluated(
                &bytes,
                None,
                runtime_context(
                    None,
                    &mut RuntimeOutput::new(
                        &self.output,
                        &self.program,
                        &self.flag_state.warn_undefined,
                    ),
                )
                .with_makeflags(&mut self.flag_state.receiver())
                .with_filesystem(&mut |effect| self.filesystem.borrow_mut().execute(effect))
                .with_shell(&mut |request| {
                    crate::shell::run(
                        request,
                        &self.program,
                        self.makelevel,
                        &self.output,
                        self.jobserver.as_ref(),
                        || self.filesystem.borrow_mut().child_completed(),
                    )
                })
                .with_includes(&mut |name| self.include_search.borrow().read(name)),
            );
            if let Err(error) = result {
                self.fatal_diagnostic(error.source_location(), &error.message, false);
                return Err(());
            }
            if let Some(options) = self.flag_state.take() {
                self.options = options;
            }
        }
        let mut encoded = Vec::new();
        for bytes in &self.options.evals {
            if !encoded.is_empty() {
                encoded.push(b' ');
            }
            encoded.extend_from_slice(b"--eval=");
            if let Err(error) = crate::command_variables::quote(&mut encoded, bytes) {
                self.expansion_error(None, &error);
                return Err(());
            }
        }
        if let Err(error) =
            self.database
                .variables
                .define_literal(b"-*-eval-flags-*-", &encoded, Origin::Automatic)
        {
            self.expansion_error(None, &error);
            return Err(());
        }
        Ok(())
    }

    fn initialize_makefile_list(&mut self) -> Result<(), ()> {
        if let Err(error) =
            self.database
                .variables
                .define_literal(b"MAKEFILE_LIST", b"", Origin::File)
        {
            self.expansion_error(None, &error);
            return Err(());
        }
        Ok(())
    }

    fn unlink_stdin(&self) {
        crate::stdin_source::unlink(
            self.initial_plan
                .as_ref()
                .and_then(|plan| plan.stdin_source.as_ref()),
            &self.output,
            &self.program,
        );
    }

    fn prepare_stdin(&mut self) -> Result<(), ()> {
        for index in 0..self.options.files.len() {
            if self.options.files[index] != Path::new("-") {
                continue;
            }
            if self.options.temp_stdin_file.is_some() {
                self.fatal(None, b"Makefile from standard input specified twice");
                return Err(());
            }
            let source = match rmake_linux::stdin_source::Source::copy_stdin() {
                Ok(source) => source,
                Err(_) if rmake_linux::signals::pending().is_some() => return Err(()),
                Err(rmake_linux::stdin_source::CopyError::Write { path, error }) => {
                    self.fatal(
                        None,
                        &[
                            b"fwrite: temporary file ".as_slice(),
                            path.as_os_str().as_bytes(),
                            b": ",
                            os_error(&error).as_bytes(),
                        ]
                        .concat(),
                    );
                    return Err(());
                }
                Err(rmake_linux::stdin_source::CopyError::Storage) => {
                    self.fatal(
                        None,
                        b"cannot store makefile from stdin to a temporary file",
                    );
                    return Err(());
                }
            };
            self.options.files[index] = source.path().to_owned();
            self.options.temp_stdin_file = Some(index);
            self.flag_state.publish_stdin(&self.options, index);
            self.initial_plan
                .as_mut()
                .expect("initial invocation")
                .stdin_source = Some(source);
        }
        if let Some(index) = self.options.temp_stdin_file {
            let name = self.options.files[index].as_os_str().as_bytes();
            self.database.file_records.insert(name);
            self.done.insert(
                name.to_vec(),
                Outcome {
                    time: modified(name),
                    completion: CompletionTime::Preserve,
                    ..Outcome::default()
                },
            );
        }
        Ok(())
    }

    fn restart_after_makefiles(&mut self, restarts: u32) -> u8 {
        self.print_database();
        let plan = self.initial_plan.take().expect("initial invocation");
        let retired_slots = self.jobserver.as_ref().and_then(Jobserver::owned_slots);
        let inherited_jobserver = self
            .jobserver
            .as_ref()
            .filter(|server| server.owned_slots().is_none())
            .cloned();
        self.jobserver.take();
        self.output.select(None);
        self.output.clear();
        if !self.options.directories.is_empty() {
            let restored = if let Some(directory) = &self.startup_directory {
                match std::env::set_current_dir(directory) {
                    Ok(()) => true,
                    Err(error) => {
                        self.stderr(
                            [b"chdir: ".as_slice(), os_error(&error).as_bytes()]
                                .concat()
                                .as_slice(),
                        );
                        false
                    }
                }
            } else {
                false
            };
            if !restored {
                self.fatal(None, b"Couldn't change back to original directory");
                if let Some(slots) = retired_slots {
                    self.stderr(format!("INTERNAL: Exiting with 1 jobserver tokens available; should be {slots}!").as_bytes());
                }
                return 2;
            }
        }
        if self.output.debug_mask().has(crate::debug::Mask::BASIC) {
            plan.debug_restart(restarts);
        }
        // A makefile restart is a new program image. Replaying only
        // Rust construction misses process-entry hooks, temporary
        // cache validation and replacement of the invoked program.
        let error = plan.exec_restart(restarts, self.makelevel, inherited_jobserver.as_ref());
        self.stderr(
            &[
                plan.restart_program().as_bytes(),
                b": ",
                os_error(&error).as_bytes(),
            ]
            .concat(),
        );
        if let Some(server) = &inherited_jobserver {
            server.finish_recursive_exec();
        }
        crate::stdin_source::unlink(plan.stdin_source.as_ref(), &self.output, &self.program);
        rmake_linux::process::exit_immediately(127);
    }

    fn run_inner(&mut self) -> u8 {
        if self.prepare_stdin().is_err() {
            return 2;
        }
        if let Err(error) = rmake_linux::signals::enter_reader_mask() {
            let message = format!("sigprocmask(SIG_SETMASK, SIGCHLD): {}", os_error(&error));
            self.fatal(None, message.as_bytes());
            return 2;
        }
        self.observe_interrupt();
        if self.interrupt.is_some() {
            return 2;
        }
        let mut found_makefile;
        self.database.activate_runtime_metadata();
        // Empty command eval initializes this binding before ordinary reading;
        // nonempty eval retains its separate reader boundary below.
        if self.options.evals.is_empty() && self.initialize_makefile_list().is_err() {
            return 2;
        }
        let restarts = self
            .initial_plan
            .as_ref()
            .map_or(0, |plan| plan.startup_restarts);
        let published_load_limit = self.options.load_limit.is_some_and(|limit| limit != -1.0);
        let restart_pending = 'makefiles: {
            if !self.options.evals.is_empty() {
                let eval_entry_no_rules = self.options.no_rules;
                if self.read_command_evals().is_err() {
                    return 2;
                }
                if !eval_entry_no_rules && self.options.no_rules {
                    self.database.remove_builtin_recipes();
                }
                if self.options.command_jobs.is_some() {
                    self.options.command_jobs = self.options.jobs;
                }
                // Nonempty buffers precede MAKEFILE_LIST initialization on
                // every pass; higher-priority CLI/-e bindings still prevail.
                if self.initialize_makefile_list().is_err() {
                    return 2;
                }
            }

            crate::debug::emit(&self.output, crate::debug::Event::Reading);
            let entry_no_rules = self.options.no_rules;
            let entry_no_variables = self.options.no_variables;
            self.flag_entry_jobs = self.options.jobs;
            self.flag_state.refresh(&self.options);
            let startup_found = match self.read_startup_makefiles() {
                Ok(found) => found,
                Err(()) => return 2,
            };
            let files = self.options.files.clone();
            let mut default_file = None;
            if files.is_empty() {
                for name in ["GNUmakefile", "makefile", "Makefile"] {
                    match self.directory_member(name.as_bytes()) {
                        Ok(true) => {
                            default_file = Some(Path::new(name));
                            break;
                        }
                        Ok(false) => {}
                        Err(error) => {
                            self.expansion_error(None, &error);
                            return 2;
                        }
                    }
                }
                if default_file.is_none() {
                    // Preloaded rules may generate a default main makefile.
                    for name in [b"GNUmakefile".as_slice(), b"makefile", b"Makefile"] {
                        self.database.file_records.insert(name);
                        self.database.makefiles.push(Makefile {
                            name: name.to_vec(),
                            optional: true,
                            missing: true,
                            location: None,
                            read_error: None,
                        });
                    }
                }
            }
            let files = files
                .iter()
                .map(std::path::PathBuf::as_path)
                .chain(default_file);
            found_makefile = startup_found
                || files
                    .clone()
                    .enumerate()
                    .any(|(index, _)| self.options.temp_stdin_file != Some(index));
            for file in files {
                if let Some(history) = self.database.database_history() {
                    history.intern(file.as_os_str().as_bytes());
                }
                let name = file.as_os_str().as_bytes();
                if let Some(history) = self.database.database_history() {
                    history.record_file_lookup(name);
                }
                if !self.database.file_records.contains(name) {
                    self.database.file_records.insert(name);
                }
                crate::debug::reader(
                    &self.output,
                    rmake_core::diagnostic::ReaderEvent::Reading {
                        name: file.as_os_str().as_bytes(),
                        search: false,
                        optional: false,
                        no_default_goal: false,
                        no_tilde: false,
                    },
                );
                let input = rmake_linux::filesystem::open_source(file.as_os_str().as_bytes());
                let input = match input {
                    Ok(input) => input,
                    Err(err) => {
                        if rmake_linux::signals::pending().is_some() {
                            return 2;
                        }
                        if source_open_exhausted(&err) {
                            self.fatal(None, os_error(&err).as_bytes());
                            return 2;
                        }
                        let detail = rmake_linux::filesystem::source_error_detail(&err);
                        emit_output(
                            &self.output,
                            &self.program,
                            Output::Warning {
                                location: None,
                                message: &crate::messages::system(Message::OsError {
                                    action: b"",
                                    path: file.as_os_str().as_bytes(),
                                    detail: &detail,
                                }),
                            },
                        );
                        self.database.makefiles.push(Makefile {
                            name: file.as_os_str().as_bytes().to_vec(),
                            optional: false,
                            missing: true,
                            location: None,
                            read_error: None,
                        });
                        continue;
                    }
                };
                found_makefile = true;
                if file != Path::new("-") {
                    self.database.makefiles.push(Makefile {
                        name: file.as_os_str().as_bytes().to_vec(),
                        optional: false,
                        missing: false,
                        location: None,
                        read_error: None,
                    });
                }
                if let Err(err) = self.database.variables.append_literal_at(
                    b"MAKEFILE_LIST",
                    file.as_os_str().as_bytes(),
                    Origin::File,
                    &Location {
                        file: file.as_os_str().as_bytes().into(),
                        line: 1,
                    },
                ) {
                    self.expansion_error(None, &err);
                    return 2;
                }
                if let Err(err) = self.database.read_input(
                    input,
                    file.as_os_str().as_bytes(),
                    runtime_context(
                        None,
                        &mut RuntimeOutput::new(
                            &self.output,
                            &self.program,
                            &self.flag_state.warn_undefined,
                        ),
                    )
                    .with_makeflags(&mut self.flag_state.receiver())
                    .with_filesystem(&mut |effect| self.filesystem.borrow_mut().execute(effect))
                    .with_shell(&mut |request| {
                        crate::shell::run(
                            request,
                            &self.program,
                            self.makelevel,
                            &self.output,
                            self.jobserver.as_ref(),
                            || self.filesystem.borrow_mut().child_completed(),
                        )
                    })
                    .with_includes(&mut |name| self.include_search.borrow().read(name)),
                ) {
                    self.fatal_diagnostic(err.source_location(), &err.message, false);
                    return 2;
                }
            }
            if self
                .read_runtime_flags(restarts, published_load_limit)
                .is_err()
            {
                return 2;
            }
            // GNU applies -R's implied -r after the post-read option decode,
            // independently of the snapshots taken after initial eval buffers.
            if self.options.no_variables && !self.options.no_rules {
                self.options.no_rules = true;
                self.flag_state.refresh(&self.options);
            }
            if self.options.no_rules
                && !entry_no_rules
                && let Err(error) = self.database.disable_builtin_rules()
            {
                self.expansion_error(None, &error);
                return 2;
            }
            if self.options.no_variables && !entry_no_variables {
                self.database.disable_builtin_variables();
            }
            match crate::command_variables::replay(
                &mut self.database.variables,
                self.command_replay,
                if self.options.environment_override {
                    Origin::EnvironmentOverride
                } else {
                    Origin::Environment
                },
                runtime_context(
                    None,
                    &mut RuntimeOutput::new(
                        &self.output,
                        &self.program,
                        &self.flag_state.warn_undefined,
                    ),
                )
                .with_makeflags(&mut self.flag_state.receiver())
                .with_filesystem(&mut |effect| self.filesystem.borrow_mut().execute(effect))
                .with_shell(&mut |request| {
                    crate::shell::run(
                        request,
                        &self.program,
                        self.makelevel,
                        &self.output,
                        self.jobserver.as_ref(),
                        || self.filesystem.borrow_mut().child_completed(),
                    )
                }),
            ) {
                Ok(names) => self.inherited_exports.extend(names),
                Err(error) => {
                    self.expansion_error(None, &error);
                    return 2;
                }
            }
            if let Err(error) = self.setup_jobserver(true) {
                self.fatal(None, error.as_bytes());
                return 2;
            }
            if self.jobserver.is_none()
                && !self.unbounded_auth_jobs
                && self.options.jobs.unwrap_or(1) == 1
                && self.output.mode().root_buffered()
            {
                self.output.select(None);
                if let Some(buffer) = self.reader_output.take() {
                    buffer.dump();
                }
                self.output.set_mode(crate::output::Mode::None);
            } else {
                match self.output.setup(self.options.sync_mutex.as_deref()) {
                    Ok(authentication) => self.options.sync_mutex = authentication,
                    Err(error) => {
                        let message = match self.options.sync_mutex.as_deref() {
                            Some(auth) => crate::usage::sync_mutex_open(auth, &error),
                            None => format!("cannot open output sync mutex: {}", os_error(&error))
                                .into_bytes(),
                        };
                        self.fatal(None, &message);
                        return 2;
                    }
                }
            }

            if let Some(auth) = &self.options.jobserver_auth {
                crate::debug::Jobs::controller(self.output.debug_mask(), auth);
            }
            if let Some(authentication) = self.options.sync_mutex.as_deref() {
                crate::debug::output_sync_mutex(self.output.debug_mask(), authentication);
            }

            if let Err(error) = self.database.variables.update_mflags(
                &self.options.recursive_flags(),
                self.options.environment_override,
            ) {
                self.expansion_error(None, &error);
                return 2;
            }
            let mut runtime_flags = self.options.makeflags();
            if self
                .database
                .variables
                .database_lookup(self.database.command_overrides_name())
                .is_some_and(|v| !v.raw_value().is_empty())
            {
                runtime_flags.extend_from_slice(self.database.command_overrides_reference());
            }
            if let Err(error) = self.database.define_canonical_makeflags(
                &runtime_flags,
                runtime_options::origin(self.options.environment_override),
            ) {
                self.expansion_error(None, &error);
                return 2;
            }
            self.flag_state.refresh(&self.options);
            self.initial_flags = runtime_flags.clone();
            self.record_database_file_policy();
            self.intermediate_policy = match crate::intermediates::Policy::read(&self.database) {
                Ok(policy) => policy,
                Err(message) => {
                    self.fatal(None, &message);
                    return 2;
                }
            };
            self.database.freeze_rules();
            let extra_records = match self.database.snap_extra_prerequisites(
                self.intermediate_policy.all_secondary,
                &mut runtime_context(
                    None,
                    &mut RuntimeOutput::new(
                        &self.output,
                        &self.program,
                        &self.flag_state.warn_undefined,
                    ),
                )
                .with_makeflags(&mut self.flag_state.receiver())
                .with_filesystem(&mut |effect| self.filesystem.borrow_mut().execute(effect))
                .with_shell(&mut |request| {
                    crate::shell::run(
                        request,
                        &self.program,
                        self.makelevel,
                        &self.output,
                        self.jobserver.as_ref(),
                        || self.filesystem.borrow_mut().child_completed(),
                    )
                })
                .with_includes(&mut |name| self.include_search.borrow().read(name)),
            ) {
                Ok(records) => records,
                Err(error) => {
                    self.expansion_error(None, &error);
                    return 2;
                }
            };
            self.intermediate_policy
                .capture_extra_records(extra_records);
            if !self.options.no_rules {
                self.database.record_builtin_suffixes();
            }
            if let Err(error) = self.database.finalize_implicit_rules(
                !self.options.no_rules,
                &mut runtime_context(
                    None,
                    &mut RuntimeOutput::new(
                        &self.output,
                        &self.program,
                        &self.flag_state.warn_undefined,
                    ),
                )
                .with_makeflags(&mut self.flag_state.receiver()),
            ) {
                self.fatal_diagnostic(error.source_location(), &error.message, false);
                return 2;
            }
            if let Err(error) = self.database.snap_implicit_extra_prerequisites(
                &mut runtime_context(
                    None,
                    &mut RuntimeOutput::new(
                        &self.output,
                        &self.program,
                        &self.flag_state.warn_undefined,
                    ),
                )
                .with_makeflags(&mut self.flag_state.receiver())
                .with_filesystem(&mut |effect| self.filesystem.borrow_mut().execute(effect))
                .with_shell(&mut |request| {
                    crate::shell::run(
                        request,
                        &self.program,
                        self.makelevel,
                        &self.output,
                        self.jobserver.as_ref(),
                        || self.filesystem.borrow_mut().child_completed(),
                    )
                })
                .with_includes(&mut |name| self.include_search.borrow().read(name)),
            ) {
                self.expansion_error(None, &error);
                return 2;
            }
            let vpath = match self.expand_global(b"$(VPATH)", None) {
                Ok(value) => value,
                Err(error) => {
                    self.expansion_error(None, &error);
                    return 2;
                }
            };
            self.vpath_dirs = match self.general_search_directories(&vpath, true) {
                Ok(directories) => directories,
                Err(error) => {
                    self.expansion_error(None, &error);
                    return 2;
                }
            };
            let gpath = if self.database.variables.get(b"GPATH").is_some() {
                match self.expand_global(b"$(strip $(GPATH))", None) {
                    Ok(value) => value,
                    Err(error) => {
                        self.expansion_error(None, &error);
                        return 2;
                    }
                }
            } else {
                self.database.variables.database_lookup(b"GPATH");
                Vec::new()
            };
            self.gpath_dirs = match self.general_search_directories(&gpath, false) {
                Ok(directories) => directories.into_iter().collect(),
                Err(error) => {
                    self.expansion_error(None, &error);
                    return 2;
                }
            };
            self.makefile_names = self
                .database
                .makefiles
                .iter()
                .map(|file| file.name.clone())
                .collect();
            if let Some(files) = &mut self.assumptions {
                files.bind(&mut self.database, restarts == 0);
            }
            self.output.select(None);
            if let Some(buffer) = self.reader_output.take() {
                buffer.dump();
            }
            self.observe_interrupt();
            if self.interrupt.is_some() {
                return 2;
            }
            self.capture_phony_names();
            crate::debug::emit(&self.output, crate::debug::Event::UpdatingMakefiles);
            // The published stdin input survives restarts without a remake goal.
            let stdin_name = self.options.temp_stdin_file.map(|index| {
                rmake_core::filenames::without_dot_prefix(
                    self.options.files[index].as_os_str().as_bytes(),
                )
            });
            // Classify all read makefiles before entering the remake debug mask.
            let read_files: Vec<_> = self
                .database
                .makefiles
                .iter()
                .filter(|makefile| stdin_name != Some(makefile.name.as_slice()))
                .map(|makefile| {
                    let skip = self.special(b".PHONY", &makefile.name, false)
                        || self
                            .database
                            .rules
                            .get(&makefile.name)
                            .and_then(|rule| rule.double_colon.as_ref())
                            .is_some_and(|branches| {
                                branches.iter().any(|rule| {
                                    rule.dependencies.is_empty() && rule.recipes.is_some()
                                })
                            });
                    if skip {
                        crate::debug::emit(
                            &self.output,
                            crate::debug::Event::SkippingMakefileLoop(&makefile.name),
                        );
                    }
                    (makefile.clone(), skip)
                })
                .collect();
            let grouped_peer_times = self.grouped_remake_comparison_times(&read_files);
            if let Err(error) = self.publish_phase_makeflags(true) {
                self.expansion_error(None, &error);
                return 2;
            }
            // A makefile named on the command line uses the requested build mode.
            // All other makefile visits can run recipes even under -n/-q/-t.
            let modes = MakefileModes::from_options(&self.options);
            let always = self.options.always;
            MakefileModes::default().apply(&mut self.options);
            if restarts > 0 {
                self.options.always = false;
            }
            let debug_before_remake = self.output.debug_mask();
            if !debug_before_remake.has(crate::debug::Mask::MAKEFILES) {
                self.output.set_debug_mask(crate::debug::Mask::NONE);
            }
            // A failed reader open installs a missing time for that file.
            // A later successful read of the same file replaces that state.
            let mut failed_opens = HashSet::new();
            for (file, _) in &read_files {
                if file.missing && file.location.is_some() && file.read_error.is_none() {
                    // A failed native load still takes its timestamp from the filesystem.
                    continue;
                }
                if file.missing && self.assumed_time(&file.name).is_none() {
                    failed_opens.insert(file.name.as_slice());
                } else {
                    failed_opens.remove(file.name.as_slice());
                }
            }
            for &name in &failed_opens {
                self.path_observations.insert(name.to_vec(), None);
            }
            use crate::debug::Mask;
            let remake_debug_bits = Mask::BASIC.bits() | Mask::MAKEFILES.bits();
            let qualified_remake_debug = debug_before_remake.bits() == remake_debug_bits
                || (debug_before_remake.bits() == (remake_debug_bits | Mask::VERBOSE.bits())
                    && self
                        .jobserver
                        .as_ref()
                        .is_some_and(|server| !server.auth().starts_with(b"fifo:")));
            let parallel_remake_debug = !debug_before_remake.has(Mask::MAKEFILES)
                || (self.options.load_limit.is_none() && qualified_remake_debug);
            // Required makefiles settle before the ordinary mode resumes.
            // An explicit goal can also name a makefile and use that mode.
            let ordinary_mode = modes.active();
            let named_sources = if ordinary_mode {
                self.index_named_remake_sources(&read_files)
            } else {
                None
            };
            let verbose_mode_remakes = ordinary_mode
                && qualified_remake_debug
                && debug_before_remake.has(Mask::VERBOSE)
                && read_files
                    .iter()
                    .all(|(file, skip)| *skip || !file.optional)
                && read_files.iter().any(|(file, skip)| {
                    let name = rmake_core::filenames::without_dot_prefix(&file.name);
                    !*skip
                        && file.location.is_none()
                        && self.is_makefile_goal(name, named_sources.as_ref())
                        && self
                            .database
                            .rules
                            .get(name)
                            .is_some_and(|rule| !rule.dependencies.is_empty())
                });
            let remakes_can_overlap = !ordinary_mode
                || ((!debug_before_remake.has(Mask::MAKEFILES)
                    || ((debug_before_remake.bits() == remake_debug_bits
                        || verbose_mode_remakes)
                        && self
                            .qualified_debug_remake_sources(&read_files, named_sources.as_ref())))
                    && self.ordinary_mode_remakes_settle(
                        &read_files,
                        named_sources.as_ref(),
                        modes,
                    ));
            self.parallel_makefile_remakes =
                self.job_slots() > 1 && remakes_can_overlap && parallel_remake_debug;
            self.remaking = true;
            self.defer_stopped_remake_load = ordinary_mode;
            let mut changed = false;
            let mut ran_remake_recipe = false;
            let mut failed = false;
            let mut stopped = false;
            let mut pending = Vec::new();
            let initial_reaps = self.recipe_reaps;
            let mut pass_reaps = initial_reaps;
            let mut skipped_errors = Vec::new();
            let mut failed_remakes = HashMap::new();
            let mut optional_failures: HashMap<&[u8], OptionalMakefileFailure> = HashMap::new();
            let mut visited = MakefileVisits::new();
            let mut optional_remake_failed = false;
            let mut remake_question = false;
            let required_names: HashSet<_> = read_files
                .iter()
                .filter(|(file, _)| !file.optional)
                .map(|(file, _)| file.name.as_slice())
                .collect();
            for (source_index, (makefile, skip)) in read_files.iter().enumerate() {
                self.database_remake_started.insert(makefile.name.clone());
                if *skip {
                    if !makefile.optional
                        && let Some(detail) = &makefile.read_error
                    {
                        skipped_errors.push(PendingIncludeError {
                            location: makefile.location.clone(),
                            name: makefile.name.clone(),
                            detail: detail.clone(),
                        });
                    }
                    continue;
                }
                let name = makefile.name.as_slice();
                let named_source = ordinary_mode
                    && self.is_makefile_goal(
                        rmake_core::filenames::without_dot_prefix(name),
                        named_sources.as_ref(),
                    );
                let visit_modes = if named_source {
                    modes
                } else {
                    MakefileModes::default()
                };
                visit_modes.apply(&mut self.options);
                let visit = match visited.entry(name) {
                    std::collections::hash_map::Entry::Vacant(visit) => visit,
                    std::collections::hash_map::Entry::Occupied(first) => {
                        let context = self.parallel_makefile_remakes.then(|| MakefileRemake {
                            optional: makefile.optional,
                            retain_failure: false,
                            stopped: None,
                            // The diagnostic uses the first read's saved error,
                            // selected by the current occurrence's policy.
                            open_error: if makefile.optional {
                                None
                            } else {
                                first.get().1.clone()
                            },
                        });
                        if let Some(context) = &context {
                            self.remake_error_cursor = context.open_error.clone();
                            if self.pending(name) {
                                pending.push(PendingMakefileRemake {
                                    modes: visit_modes,
                                    file: makefile,
                                    context: context.clone(),
                                    duplicate: true,
                                    active: true,
                                    initial: Outcome::default(),
                                });
                                continue;
                            }
                        }
                        if !makefile.optional
                            && let Some(failure) = optional_failures.get_mut(name)
                        {
                            let replayed = if let Some(context) = context {
                                if self.has_optional_remake_failure(name) {
                                    self.graph_epoch = self.graph_epoch.wrapping_add(1);
                                    self.with_makefile_remake(Some(context), |engine| {
                                        engine.build_recording_initial_time(
                                            name,
                                            None,
                                            0,
                                            None,
                                            BuildVisit::Ordinary,
                                            None,
                                        );
                                    });
                                    true
                                } else {
                                    false
                                }
                            } else {
                                let replayed =
                                    self.replay_makefile_failure(name, &mut failure.open_error);
                                if replayed
                                    && self.options.keep_going
                                    && let Some(declarations) =
                                        self.serial_double_colon_declarations(name)
                                {
                                    for _ in 1..declarations {
                                        self.replay_makefile_failure(name, &mut failure.open_error);
                                    }
                                }
                                replayed
                            };
                            if replayed && !self.options.keep_going {
                                stopped = true;
                                break;
                            }
                            failed = true;
                            if self.options.keep_going {
                                failed_remakes
                                    .entry(name)
                                    .or_insert((failure.before, failure.refresh_timestamp));
                            }
                        } else if context.is_none() && self.output.debug_mask().has(Mask::VERBOSE) {
                            let source = self.aliases.resolve(name);
                            let before = first.get().0;
                            let unchanged = before.is_some()
                                && !visit_modes.active()
                                && self.done.get(source).is_some_and(|outcome| {
                                    !outcome.failed
                                        && !outcome.fatal
                                        && !outcome.question_outdated
                                        && outcome.time == before
                                });
                            if !makefile.optional || unchanged {
                                // An unchanged source revisits every cached
                                // declaration, including any grouped entries.
                                let declarations = if unchanged {
                                    self.database
                                        .rules
                                        .get(source)
                                        .and_then(|rule| rule.double_colon.as_ref())
                                        .map_or(1, Vec::len)
                                } else {
                                    self.serial_double_colon_declarations(name).unwrap_or(1)
                                };
                                for _ in 0..declarations {
                                    crate::debug::emit(
                                        &self.output,
                                        crate::debug::Event::Pruning(source, 1),
                                    );
                                }
                            }
                        }
                        continue;
                    }
                };
                let optional = makefile.optional;
                let may_replay = optional && required_names.contains(name);
                let deferred_peer_failure = !optional
                    && !self.options.keep_going
                    && self.cached_optional_group_peer_failure(name);
                let pending_peer = self.parallel_makefile_remakes && self.pending(name);
                // Keep the existing snapshot for recorded rules. A makefile
                // without a rule uses the timestamp of its first graph visit.
                let mut before = if failed_opens.contains(name) {
                    Some(None)
                } else {
                    self.database
                        .rules
                        .contains_key(name)
                        .then(|| self.modified(name))
                };
                let pending_open_error = || {
                    makefile
                        .read_error
                        .as_ref()
                        .map(|detail| PendingIncludeError {
                            location: makefile.location.clone(),
                            name: makefile.name.clone(),
                            detail: detail.clone(),
                        })
                };
                let first_open_error =
                    if !optional || (self.parallel_makefile_remakes && may_replay) {
                        pending_open_error().map(|error| Rc::new(Cell::new(Some(error))))
                    } else {
                        None
                    };
                self.active_makefile_remake = Some(MakefileRemake {
                    optional,
                    retain_failure: optional,
                    stopped: (optional && self.parallel_makefile_remakes)
                        .then(|| Rc::new(Cell::new(false))),
                    open_error: if optional {
                        None
                    } else {
                        first_open_error.clone()
                    },
                });
                let initial_time = before.is_none().then_some(&mut before);
                if self.parallel_makefile_remakes {
                    self.remake_error_cursor = self
                        .active_makefile_remake
                        .as_ref()
                        .and_then(|remake| remake.open_error.clone());
                    self.graph_epoch = self.graph_epoch.wrapping_add(1);
                }
                let mut result = self.build_recording_initial_time(
                    name,
                    None,
                    0,
                    initial_time,
                    BuildVisit::Ordinary,
                    None,
                );
                let serial_declarations = if result.failed && !result.fatal {
                    self.serial_double_colon_declarations(name)
                } else {
                    None
                };
                if let Some(declarations) = serial_declarations
                    && let Some(&head) = self.database_branch_outcomes.get(&(name.to_vec(), 0))
                {
                    if optional && head.failed {
                        use crate::debug::{Event, emit};

                        let cause = self.optional_remake_failures.get(name).cloned().flatten();
                        // A failed prerequisite can be reported again when
                        // each remaining declaration revisits the head.
                        let repeats = if self.options.keep_going && cause.is_some() {
                            declarations - 1
                        } else {
                            1
                        };
                        for _ in 0..repeats {
                            emit(&self.output, Event::Considering(name, 1));
                            emit(&self.output, Event::RecentlyFailed(name, 1));
                            self.show_retained_optional_failure(name, None, cause.clone());
                            if self.options.keep_going {
                                self.debug_completed_double_colon_declarations(
                                    name,
                                    1..declarations,
                                    1,
                                );
                            }
                        }
                        if self.options.keep_going && cause.is_none() {
                            // A directly failed head has one full revisit.
                            // The remaining goal entries prune the chain.
                            for _ in 2..declarations {
                                emit(&self.output, Event::Pruning(name, 1));
                            }
                        }
                        if cause.is_none() || !self.options.keep_going {
                            self.optional_remake_failures.remove(name);
                        }
                    } else if optional {
                        // A later declaration already reported its own error.
                        self.optional_remake_failures.remove(name);
                    }
                    if self.options.keep_going {
                        // Inspect the head status and the physical time of
                        // the shared makefile after the declarations settle.
                        result.failed = head.failed;
                        result.blocked = head.blocked;
                        result.completion = CompletionTime::Refresh;
                        result.time = self.modified(name);
                    } else if optional {
                        result.fatal = true;
                    }
                }
                let before = before.unwrap_or(result.time);
                // A required chain under -k can revisit its cached head
                // after a failed declaration and report the prune.
                let settled_chain = !result.failed
                    || (!optional && self.options.keep_going && serial_declarations.is_some());
                if !self.parallel_makefile_remakes
                    && settled_chain
                    && (!optional || before == result.time)
                {
                    self.debug_double_colon_goal_prunes(name, result);
                }
                visit.insert((before, first_open_error));
                ran_remake_recipe |= result.ran;
                let context = self
                    .active_makefile_remake
                    .take()
                    .expect("active makefile remake");
                if self.forced_query_stop {
                    stopped = true;
                    break;
                }
                if self.parallel_makefile_remakes && self.pending(name) {
                    pending.push(PendingMakefileRemake {
                        modes: visit_modes,
                        file: makefile,
                        context,
                        duplicate: false,
                        active: true,
                        initial: result,
                    });
                    continue;
                }
                let deferred_peer_failure = deferred_peer_failure
                    || (pending_peer
                        && result.optional_group_peer_failure
                        && self.cached_optional_group_peer_failure(name));
                if result.question_outdated {
                    if !failed && !optional_remake_failed {
                        remake_question = true;
                    } else if !remake_question && !optional {
                        // An earlier failure selects the remake error pass,
                        // which also reports required out-of-date sources.
                        failed_remakes.entry(name).or_insert((before, false));
                    }
                }
                failed |= result.failed && !optional;
                optional_remake_failed |= result.failed && optional;
                if result.failed && !result.fatal {
                    // A serial declaration may change the file after a
                    // blocked head. Other roots use their own recipe result.
                    let refresh_timestamp =
                        serial_declarations.is_some() || (result.ran && !result.blocked);
                    if may_replay {
                        optional_failures.insert(
                            name,
                            OptionalMakefileFailure {
                                open_error: pending_open_error(),
                                before,
                                refresh_timestamp,
                            },
                        );
                    } else if !optional && (self.options.keep_going || deferred_peer_failure) {
                        failed_remakes.insert(name, (before, refresh_timestamp));
                    }
                } else if !result.failed {
                    self.database_unchecked_times
                        .borrow_mut()
                        .remove(&makefile.name);
                    // A completed recipe can invalidate the makefile's time.
                    // Expansion effects alone leave its cached observation intact.
                    let physically_changed =
                        visit_modes.check_update(result) && self.modified(&makefile.name) != before;
                    changed |= physically_changed;
                    let simulated_change =
                        visit_modes.dry_run && result.completion == CompletionTime::AssumeNew;
                    if self.parallel_makefile_remakes
                        && optional
                        && visit_modes.active()
                        && !result.question_outdated
                        && (physically_changed || simulated_change)
                        && !pending.is_empty()
                        && read_files[source_index + 1..]
                            .iter()
                            .any(|(_, skip)| !*skip)
                    {
                        // An immediate optional update ends this visit pass.
                        // A previous worker can complete under the named mode
                        // before the next unvisited source starts.
                        self.poll_jobs();
                        if self.recipe_reaps == pass_reaps && !self.parallel_jobs.is_empty() {
                            let previous_jobs = self.parallel_jobs.len();
                            while self.parallel_jobs.len() >= previous_jobs
                                && self.shutdown.get() == Shutdown::Running
                                && self.graph_abort.is_none()
                            {
                                self.reap_job();
                            }
                            self.poll_jobs();
                        }
                        pass_reaps = self.recipe_reaps;
                        self.advance_debug_pass();
                    }
                }
                if self.forced_query_stop
                    || result.fatal
                    || (result.failed
                        && !optional
                        && !self.options.keep_going
                        && !deferred_peer_failure)
                {
                    // A stopping remake failure wins over any earlier update.
                    stopped = true;
                    break;
                }
            }
            if !stopped && !pending.is_empty() {
                // Reaping before the next source visit retains the last
                // named mode while that source still has a pending recipe.
                if MakefileModes::from_options(&self.options).active()
                    && !pending.last().is_some_and(|remake| remake.modes.active())
                {
                    // Pending sources resume with their own build mode. The
                    // last pending source supplies the open error.
                    self.remake_error_cursor = pending
                        .last()
                        .and_then(|remake| remake.context.open_error.clone());
                    MakefileModes::default().apply(&mut self.options);
                }
                let last_reaps = if verbose_mode_remakes {
                    None
                } else {
                    Some(pass_reaps)
                };
                stopped = !self.wait_makefile_remakes(&mut pending, &visited, last_reaps);
                if !stopped {
                    for remake in pending {
                        let name = remake.file.name.as_slice();
                        let before = visited[name].0;
                        let result = self
                            .done
                            .get(self.aliases.resolve(name))
                            .copied()
                            .unwrap_or(remake.initial);
                        if remake.duplicate && (remake.file.optional || !result.failed) {
                            continue;
                        }
                        ran_remake_recipe |= result.ran;
                        if result.question_outdated {
                            if !failed && !optional_remake_failed {
                                remake_question = true;
                            } else if !remake_question && !remake.file.optional {
                                failed_remakes.entry(name).or_insert((before, false));
                            }
                        }
                        optional_remake_failed |= result.failed && remake.file.optional;
                        if result.failed {
                            if !remake.file.optional {
                                failed = true;
                                if !result.fatal
                                    && (self.options.keep_going
                                        || self.cached_optional_group_peer_failure(name))
                                {
                                    failed_remakes
                                        .insert(name, (before, result.ran && !result.blocked));
                                } else {
                                    stopped = true;
                                }
                            } else {
                                stopped |= result.fatal;
                            }
                        } else {
                            self.database_unchecked_times.borrow_mut().remove(name);
                            // A different named source can have supplied the
                            // mode when this recipe completed. Check the real
                            // write when revisiting the source in normal mode.
                            let completed_in_other_mode = !remake.modes.active()
                                && result.ran
                                && result.completion == CompletionTime::AssumeNew;
                            if remake.modes.check_update(result) || completed_in_other_mode {
                                changed |= self.modified(name) != before;
                            }
                        }
                    }
                }
            }
            // Only a continuing ordinary build may overlap an already
            // running sibling from a stopped optional remake.
            self.defer_stopped_remake_load = false;
            if stopped || changed || failed || self.shutdown.get() != Shutdown::Running {
                self.wait_all_jobs();
            }
            modes.apply(&mut self.options);
            self.options.always = always;
            self.output.set_debug_mask(debug_before_remake);
            self.remaking = false;
            self.parallel_makefile_remakes = false;
            self.remake_error_cursor = None;
            if stopped || self.shutdown.get() != Shutdown::Running {
                self.remove_intermediates();
                return if self.forced_query_stop && self.shutdown.get() == Shutdown::Running {
                    1
                } else {
                    2
                };
            }
            // A question verdict leaves the loaded makefiles in use and
            // prevents later keep-going failures from deciding the phase.
            if remake_question {
                self.parallel_failed = false;
                failed = false;
            }
            failed |= !skipped_errors.is_empty();
            // These are reported after the remake chain, using the locale
            // and gettext domain that its recipes left in effect.
            for error in skipped_errors.iter().rev() {
                let detail = error.bytes();
                emit_output(
                    &self.output,
                    &self.program,
                    Output::Warning {
                        location: error.location.as_ref(),
                        message: &crate::messages::system(Message::SourceError {
                            path: &error.name,
                            detail: &detail,
                        }),
                    },
                );
            }
            // Use the settled peer outcomes and the original visit timestamps
            // for the required read reports and the restart decision.
            if !remake_question && (!failed_remakes.is_empty() || optional_remake_failed) {
                for (makefile, skip) in read_files.iter().filter(|(file, _)| !file.optional) {
                    let name = makefile.name.as_slice();
                    // A later optional group can replace an earlier required
                    // peer's successful outcome before the remake phase ends.
                    if optional_remake_failed
                        && !*skip
                        && !failed_remakes.contains_key(name)
                        && let Some(&(before, _)) = visited.get(name)
                        && let Some(outcome) = self.done.get(self.aliases.resolve(name))
                        && outcome.optional_group_peer_failure
                    {
                        failed = true;
                        failed_remakes.insert(name, (before, outcome.ran && !outcome.blocked));
                    }
                    let Some((before, refresh_timestamp)) = failed_remakes.get_mut(name) else {
                        continue;
                    };
                    emit_output(
                        &self.output,
                        &self.program,
                        Output::Warning {
                            location: makefile.location.as_ref(),
                            message: &crate::messages::system(Message::FailedMakefileRemake(
                                &makefile.name,
                            )),
                        },
                    );
                    // Later remake recipes can still change this file.
                    if *refresh_timestamp {
                        let before = grouped_peer_times
                            .get(name)
                            .copied()
                            .filter(|_| {
                                self.done
                                    .get(name)
                                    .is_some_and(|outcome| outcome.optional_group_peer_failure)
                            })
                            .or(*before);
                        let after = self.modified(&makefile.name);
                        changed |= after.is_some() && after != before;
                        *refresh_timestamp = false;
                    }
                }
            }
            if changed && !remake_question {
                self.wait_all_jobs();
                self.remove_intermediates();
                break 'makefiles true;
            }
            if failed {
                self.wait_all_jobs();
                self.remove_intermediates();
                return 2;
            }
            if ran_remake_recipe || self.recipe_reaps != initial_reaps {
                // Keep the cached outcomes and timestamps, but report ordinary
                // goal activity only for recipes started in that phase.
                for outcome in self.done.values_mut() {
                    outcome.ran = false;
                }
            }
            for module in crate::native::unloaded() {
                let location = self.database.loaded_modules.get(&module).cloned();
                if let Err(error) = self.native_module_operation(&module, location.as_ref(), true) {
                    self.expansion_error(location.as_ref(), &error);
                    return 2;
                }
            }
            false
        };
        if restart_pending {
            return self.restart_after_makefiles(restarts.wrapping_add(1));
        }
        if let Err(error) = self.publish_phase_makeflags(false) {
            self.expansion_error(None, &error);
            return 2;
        }
        if restarts != 0 {
            self.publish_restarted_assumptions();
        }
        self.unlink_stdin();
        let mut goals = if self.options.goals.is_empty() {
            // GNU expands a recursive default goal's raw value here, without
            // entering the .DEFAULT_GOAL binding's definition context. Nested
            // references still carry their own definition locations.
            let goal = match self.database.variables.get(b".DEFAULT_GOAL") {
                None => Ok(Vec::new()),
                Some(variable) => {
                    // Expansion can replace the binding, so keep its raw bytes.
                    let value = variable.raw_value().to_vec();
                    if variable.flavor() == Flavor::Simple {
                        Ok(value)
                    } else {
                        self.expand_global(&value, None)
                    }
                }
            };
            match goal {
                Ok(mut goal) if !trim(&goal).is_empty() => {
                    // GNU first looks up the complete spelling. An interned
                    // filename from a glob can contain blanks without naming
                    // multiple default goals.
                    let lookup = rmake_core::filenames::without_dot_prefix(&goal);
                    if let Some(history) = self.database.database_history() {
                        history.record_file_lookup(lookup);
                    }
                    if self.database.rules.contains_key(lookup)
                        || self.database.explicit_names.contains(lookup)
                    {
                        if goal.starts_with(b"./") {
                            goal = lookup.to_vec();
                        }
                        vec![goal]
                    } else {
                        let names = self.database.rule_filenames(
                            &goal,
                            &mut runtime_context(
                                None,
                                &mut RuntimeOutput::new(
                                    &self.output,
                                    &self.program,
                                    &self.flag_state.warn_undefined,
                                ),
                            )
                            .with_makeflags(&mut self.flag_state.receiver())
                            .with_filesystem(&mut |effect| {
                                self.filesystem.borrow_mut().execute(effect)
                            })
                            .with_shell(&mut |request| {
                                crate::shell::run(
                                    request,
                                    &self.program,
                                    self.makelevel,
                                    &self.output,
                                    self.jobserver.as_ref(),
                                    || self.filesystem.borrow_mut().child_completed(),
                                )
                            })
                            .with_includes(&mut |name| self.include_search.borrow().read(name)),
                        );
                        let goals: Vec<_> = match names {
                            Ok(names) => names.map(std::borrow::Cow::into_owned).collect(),
                            Err(error) => {
                                self.expansion_error(None, &error);
                                return 2;
                            }
                        };
                        if goals.len() != 1 {
                            self.fatal_system(rmake_core::messages::Message::MultipleDefaultGoals);
                            return 2;
                        }
                        goals
                    }
                }
                Ok(_) => {
                    self.fatal_system(rmake_core::messages::Message::NoTargets {
                        makefile_found: found_makefile,
                    });
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
        // Remake and ordinary goals are separate diagnostic traversals.
        self.debug_considered = None;
        self.debug_pass = self.debug_progress();
        crate::debug::emit(&self.output, crate::debug::Event::UpdatingGoals);
        if self.shuffle.enabled() {
            let disabled = self.database.globally_serial();
            self.shuffle.start(&mut goals, &self.database, disabled);
        }
        let mut failed = false;
        let mut fatal = false;
        let mut stopped_on_failure = false;
        let mut requested = Vec::new();
        let initial_goal_generation = self.filesystem.borrow().generation();
        let goal_count = goals.len();
        for (index, goal) in goals.into_iter().enumerate() {
            self.graph_epoch = self.graph_epoch.wrapping_add(1);
            let name = self.aliases.resolve(&goal);
            let already_done = (self.done.contains_key(name)
                && !self.cached_double_colon_input_due(name))
                || self.pending(&goal);
            if already_done && !self.parallel_graph() {
                self.wait_target(&goal);
            }
            let mut started_command = false;
            let outcome = self.build_recording_initial_time(
                &goal,
                None,
                0,
                None,
                BuildVisit::Ordinary,
                Some(&mut started_command),
            );
            if self.forced_query_stop {
                failed |= outcome.failed;
                fatal |= outcome.fatal;
                break;
            }
            self.debug_double_colon_goal_prunes(&goal, outcome);
            let deferred_query = self.queued_query_tail(&goal);
            let pending = self.pending(&goal) || deferred_query;
            let outcome = if pending {
                outcome
            } else {
                self.observe_updated(&goal, outcome)
            };
            let mut completed_report = None;
            if self.options.keep_going && !outcome.fatal {
                let name = self.aliases.resolve(&goal);
                if self.visited_cached_double_colon_input(name) {
                    // The goal summary uses the declared head, while the
                    // first visit still reports the trailing input's failure.
                    completed_report = self.done.get(name).copied();
                } else if outcome.late_input_failed && !pending {
                    completed_report = Some(outcome);
                }
            }
            if completed_report.is_none() && (deferred_query || (self.parallel_graph() && pending))
            {
                if self.options.question {
                    // An earlier double-colon branch can have reported a
                    // verdict before a later branch finishes its recipe.
                    self.ordinary_queries.record(outcome.first_branch_verdict);
                }
                let awaiting_recipe = !already_done && self.goal_recipe_waiting(&goal);
                let awaiting_group_peer = self.output.debug_mask().has(crate::debug::Mask::VERBOSE)
                    && self.ordinary_group_peer_pending(&goal);
                requested.push(RequestedGoal {
                    name: goal,
                    already_done,
                    initial: outcome,
                    awaiting_recipe,
                    awaiting_group_peer,
                    started_command,
                });
            } else {
                failed |= outcome.failed;
                let verdict = outcome.goal_verdict();
                let first_question = self.ordinary_queries.first_verdict.is_none()
                    && verdict == Some(Status::Question);
                self.ordinary_queries.record(verdict);
                self.report_goal(&goal, started_command, completed_report.unwrap_or(outcome));
                if first_question
                    && !self.options.keep_going
                    && self.parallel_graph()
                    && (index + 1 < goal_count
                        || requested.iter().any(|goal| self.pending(&goal.name)))
                {
                    // A first query verdict ends the pass. If goals remain
                    // and no child or file effect advanced it, wait before
                    // visiting them. Otherwise only the final job drain runs.
                    if self.filesystem.borrow().generation() == initial_goal_generation
                        && !self.parallel_jobs.is_empty()
                    {
                        self.reap_job();
                    }
                    self.poll_jobs();
                    if self.forced_query_stop || self.graph_abort.is_some() {
                        stopped_on_failure = self.graph_abort.is_some();
                        break;
                    }
                    for prior in &requested {
                        self.debug_cached_prerequisite_visit(&prior.name, 0, prior.initial);
                    }
                }
            }
            fatal |= outcome.fatal;
            if outcome.fatal || (outcome.failed && !self.options.keep_going) {
                stopped_on_failure = true;
                break;
            }
        }
        let mut last_goal_pass = self.debug_pass;
        'goal_passes: while !self.forced_query_stop
            && requested.iter().any(|goal| {
                self.pending(&goal.name)
                    || (!stopped_on_failure
                        && !fatal
                        && self.graph_abort.is_none()
                        && (self.options.keep_going || !self.parallel_jobs.is_empty())
                        && self.queued_query_tail(&goal.name))
            })
        {
            self.ordinary_queries.pass = self.ordinary_queries.pass.wrapping_add(1);
            self.advance_debug_pass();
            self.graph_epoch = self.graph_epoch.wrapping_add(1);
            let pass_epoch = self.graph_epoch;
            let mut first_goal = true;
            for goal in &mut requested {
                let deferred = !stopped_on_failure
                    && !fatal
                    && self.graph_abort.is_none()
                    && self.queued_query_tail(&goal.name);
                if self.graph_pending(&goal.name) || deferred {
                    let visit = if first_goal {
                        BuildVisit::GoalPassStart
                    } else {
                        BuildVisit::Ordinary
                    };
                    let outcome = self.build_recording_initial_time(
                        &goal.name,
                        None,
                        0,
                        None,
                        visit,
                        Some(&mut goal.started_command),
                    );
                    if self.forced_query_stop {
                        failed |= outcome.failed;
                        fatal |= outcome.fatal;
                        break 'goal_passes;
                    }
                    if self.options.question {
                        self.ordinary_queries.record(outcome.first_branch_verdict);
                    }
                    if !self.pending(&goal.name)
                        && !self.queued_query_tail(&goal.name)
                        && (outcome.late_input_failed || self.options.question)
                    {
                        failed |= outcome.failed;
                        self.ordinary_queries.record(outcome.goal_verdict());
                    }
                    if self.debug_pass != last_goal_pass {
                        self.debug_double_colon_goal_prunes(&goal.name, outcome);
                    }
                    first_goal = false;
                    if !goal.already_done && !goal.awaiting_recipe {
                        goal.awaiting_recipe = self.goal_recipe_waiting(&goal.name);
                    }
                } else if goal.awaiting_recipe && !self.pending(&goal.name) {
                    first_goal = false;
                    self.debug_completed_goal_visit(&goal.name, false);
                    goal.awaiting_recipe = false;
                    if self.options.question
                        && let Some(outcome) = self.done.get(self.aliases.resolve(&goal.name))
                    {
                        self.ordinary_queries.record(outcome.goal_verdict());
                    }
                } else if self.debug_pass != last_goal_pass
                    && self.graph_abort.is_none()
                    && self.shutdown.get() == Shutdown::Running
                    && (goal.awaiting_recipe
                        || (goal.awaiting_group_peer
                            && self.ordinary_group_peer_pending(&goal.name)))
                {
                    // Progress can trigger a new visit while the goal or its group runs.
                    self.debug_cached_prerequisite_visit(&goal.name, 0, goal.initial);
                }
            }
            last_goal_pass = self.debug_pass;
            // A completion or a file write can make more work observable in
            // the next pass. Rescan before blocking on the remaining children.
            if self.graph_epoch == pass_epoch
                && self.debug_progress() == self.debug_pass
                && !self.parallel_jobs.is_empty()
                && requested
                    .iter()
                    .any(|goal| self.pending(&goal.name) || self.queued_query_tail(&goal.name))
            {
                self.reap_job();
            }
        }
        self.advance_debug_pass();
        for goal in requested {
            if self.forced_query_stop {
                break;
            }
            // Reaping the last job can end the pending-work loop before the
            // next scan. Keep each grouped peer visit beside its own summary.
            if goal.awaiting_recipe || goal.awaiting_group_peer {
                self.debug_completed_goal_visit(&goal.name, goal.awaiting_group_peer);
            }
            let outcome = self
                .done
                .get(self.aliases.resolve(&goal.name))
                .copied()
                .unwrap_or(goal.initial);
            let outcome = self.observe_updated(&goal.name, outcome);
            failed |= outcome.failed;
            fatal |= outcome.fatal;
            self.ordinary_queries.record(outcome.goal_verdict());
            self.report_goal(&goal.name, goal.started_command, outcome);
        }
        if self.options.question
            && self.would_build
            && !self.options.keep_going
            && self
                .parallel_jobs
                .values()
                .any(|job| job.continuation.remake.is_some() && !job.waiting_load)
        {
            self.notice_waiting_for_jobs();
        }
        self.wait_all_jobs();
        failed |= self.parallel_failed;
        fatal |= self.graph_abort == Some(Status::Fatal);
        let jobserver_error = self.jobserver.as_ref().and_then(Jobserver::take_error);
        if matches!(self.clock_skew.get(), ClockSkew::Detected)
            && self.interrupt.is_none()
            && rmake_linux::signals::pending().is_none()
            && jobserver_error.is_none()
            && !fatal
            && !self.forced_query_stop
            && self.graph_abort != Some(Status::Fatal)
            && (!failed || self.options.keep_going)
        {
            self.stderr(&crate::messages::system(
                rmake_core::messages::Message::ClockSkew,
            ));
        }
        self.remove_intermediates();
        if let Some(error) = jobserver_error {
            self.fatal(
                None,
                format!("jobserver token return: {}", os_error(&error)).as_bytes(),
            );
            return 2;
        }
        // Keep-going retains the first observed query or failure while still
        // visiting other goals. An invocation-wide stop takes precedence.
        let query_first = self.ordinary_queries.first_verdict == Some(Status::Question)
            && !stopped_on_failure
            && !fatal
            && self.graph_abort.is_none();
        if query_first
            && !failed
            && !self.forced_query_stop
            && self.shutdown.get() == Shutdown::Running
            && self.options.question
            && !self.options.keep_going
            && self.has_unfinished_query_tail()
        {
            self.expansion_error(
                None,
                &ExpandError::Safety(rmake_core::safety::SafetyError::StalledDoubleColonQuery),
            );
            return 2;
        }
        if failed && (!self.forced_query_stop || fatal) && !query_first {
            2
        } else if self.would_build || query_first {
            1
        } else {
            0
        }
    }

    fn index_named_remake_sources<'files>(
        &self,
        read_files: &'files [(Makefile, bool)],
    ) -> Option<HashSet<&'files [u8]>> {
        // A short list is cheap to scan, especially the single-goal case.
        if read_files.len() < 16 || self.options.goals.len() < 32 {
            return None;
        }
        let goals: HashSet<_> = self.options.goals.iter().map(Vec::as_slice).collect();
        // Keep only source-name borrows so runtime option updates can still
        // take ownership of the original goal vector while remakes run.
        Some(
            read_files
                .iter()
                .filter(|(_, skip)| !skip)
                .map(|(file, _)| rmake_core::filenames::without_dot_prefix(&file.name))
                .filter(|name| goals.contains(name))
                .collect(),
        )
    }

    fn is_makefile_goal(&self, name: &[u8], indexed: Option<&HashSet<&[u8]>>) -> bool {
        if let Some(indexed) = indexed {
            indexed.contains(name)
        } else {
            self.options
                .goals
                .iter()
                .any(|goal| goal.as_slice() == name)
        }
    }

    fn ordinary_mode_remakes_settle(
        &self,
        read_files: &[(Makefile, bool)],
        named_sources: Option<&HashSet<&[u8]>>,
        modes: MakefileModes,
    ) -> bool {
        use rmake_core::filenames::{archive_parts, without_dot_prefix};

        let goals = &self.options.goals;
        let mut has_subdirectory_goal = false;
        // CLI decoding strips leading ./ before ordinary mode admission.
        // Other relative aliases and expanded identities stay guarded.
        if !goals.is_empty()
            && (self.database.has_vpaths()
                || !self.vpath_dirs.is_empty()
                || !self.gpath_dirs.is_empty()
                || goals.iter().any(|goal| {
                    if goal.starts_with(b"~")
                        || goal.starts_with(b"-l")
                        || archive_parts(goal).is_some()
                    {
                        return true;
                    }
                    if !goal.contains(&b'/') {
                        return false;
                    }
                    has_subdirectory_goal = true;
                    goal.split(|&byte| byte == b'/')
                        .any(|component| matches!(component, b"" | b"." | b".."))
                }))
        {
            return false;
        }
        if read_files.iter().any(|(file, skip)| file.optional && !skip)
            && !self.optional_mode_sources_settle(read_files)
        {
            return false;
        }
        let mut named = HashMap::new();
        read_files.iter().enumerate().all(|(index, (file, skip))| {
            let name = without_dot_prefix(&file.name);
            // A plain subdirectory goal cannot name a leaf makefile.
            if has_subdirectory_goal && name.contains(&b'/') {
                return false;
            }
            *skip
                || goals.is_empty()
                || (!name.starts_with(b"-l")
                    && archive_parts(name).is_none()
                    && (!self.is_makefile_goal(name, named_sources) || {
                        let include_policy = file.location.as_ref().map(|_| file.optional);
                        let compatible_read =
                            named
                                .insert(name, include_policy)
                                .is_none_or(|earlier_policy| {
                                    include_policy.is_some() && include_policy == earlier_policy
                                });
                        compatible_read
                            && self.simple_named_remake(
                                file,
                                name,
                                &read_files[..index],
                                !modes.touch && (modes.dry_run ^ modes.question),
                            )
                    }))
        })
    }

    fn qualified_debug_remake_sources(
        &self,
        read_files: &[(Makefile, bool)],
        named_sources: Option<&HashSet<&[u8]>>,
    ) -> bool {
        use rmake_core::filenames::without_dot_prefix;

        read_files.iter().all(|(file, skip)| {
            let name = without_dot_prefix(&file.name);
            *skip
                || self.database.rules.get(name).is_none_or(|rule| {
                    rule.secondary.is_none()
                        && rule.group.is_none()
                        && rule.double_colon.is_none()
                        && (rule.dependencies.is_empty()
                            || (self.is_makefile_goal(name, named_sources)
                                && self.simple_named_remake(file, name, &[], false)))
                })
        })
    }

    fn simple_named_remake(
        &self,
        file: &Makefile,
        name: &[u8],
        earlier_sources: &[(Makefile, bool)],
        grouped_simulation: bool,
    ) -> bool {
        let primary = file.location.is_none();
        if (primary && (file.optional || file.missing))
            || self.database.loaded_modules.contains_key(name)
            || self.assumptions.is_some()
            || self.options.load_limit.is_some()
        {
            return false;
        }
        let no_default_recipe = !self.database.rules.contains_key(b".DEFAULT".as_slice());
        let earlier_sources = if primary { earlier_sources } else { &[] };
        self.database
            .rules
            .get(name)
            .map_or(primary && no_default_recipe, |rule| {
                if rule.secondary.is_some()
                    && (!primary || !no_default_recipe || rule.group.is_some())
                {
                    return false;
                }
                let mut deferred = rule
                    .secondary
                    .iter()
                    .flat_map(|clauses| clauses.iter())
                    .peekable();
                // Dry-run and question can simulate the grouped remake without
                // touching a primary or running a recursive recipe.
                let group_can_simulate = rule.group.as_ref().is_none_or(|group| {
                    primary
                        && grouped_simulation
                        && group.double_id.is_none()
                        && group.targets.len() == 2
                        && group.targets.iter().any(|target| target == name)
                        && group.targets.iter().any(|peer| {
                            peer != name
                                && !peer.contains(&b'/')
                                && !peer.starts_with(b".")
                                && !peer.starts_with(b"~")
                                && !peer.starts_with(b"-l")
                                && rmake_core::filenames::archive_parts(peer).is_none()
                                && !self.makefile_names.contains(peer)
                                && !self.special(b".PHONY", peer, false)
                        })
                        && rule.recipes.as_ref().is_some_and(|recipes| {
                            recipes.iter().all(|recipe| !recipe_is_recursive(recipe))
                        })
                });
                group_can_simulate
                    && rule.double_colon.is_none()
                    && (rule.dependencies.is_empty()
                        || (no_default_recipe
                            && rule.dependencies.iter().enumerate().all(|(index, dep)| {
                                // The remake visit owns expansion of this slot.
                                deferred.next_if(|clause| clause.index == index).is_some()
                                    || self.simple_remake_prerequisite(dep, earlier_sources)
                            })))
                    && rule.recipes.is_some()
            })
    }

    fn simple_remake_prerequisite(
        &self,
        dependency: &Dependency,
        earlier_sources: &[(Makefile, bool)],
    ) -> bool {
        use rmake_core::filenames::{archive_parts, without_dot_prefix};

        let name = dependency.name.as_slice();
        if dependency.hidden
            || name == b".WAIT"
            || name.contains(&b'/')
            || name.starts_with(b"~")
            || name.starts_with(b"-l")
            || archive_parts(name).is_some()
            || self.special(b".PHONY", name, false)
        {
            return false;
        }
        let Some(rule) = self.database.rules.get(name) else {
            return fs::metadata(os(name)).is_ok_and(|metadata| metadata.is_file());
        };
        // A direct earlier include can already be running when the named
        // primary reaches it as a prerequisite.
        earlier_sources.iter().any(|(source, skip)| {
            !*skip
                && !source.optional
                && source.location.is_some()
                && without_dot_prefix(&source.name) == name
        }) && !self.database.loaded_modules.contains_key(name)
            && rule.dependencies.is_empty()
            && rule.secondary.is_none()
            && rule.group.is_none()
            && rule.double_colon.is_none()
            && rule
                .recipes
                .as_ref()
                .is_some_and(|recipes| recipes.iter().all(|recipe| !recipe_is_recursive(recipe)))
    }

    fn no_remake_recipe_fallback(&self) -> bool {
        self.options.no_rules
            && self.database.database_implicit_rules().is_empty()
            && !self.database.rules.contains_key(b".DEFAULT".as_slice())
    }

    fn optional_mode_sources_settle(&self, read_files: &[(Makefile, bool)]) -> bool {
        use rmake_core::filenames::{archive_parts, without_dot_prefix};

        // Use simple file identities and single-colon optional roots. A
        // grouped root qualifies when its recipe has no prerequisites
        // that could start another recipe before it.
        if self.options.check_symlink_times
            || self.assumptions.is_some()
            || !self.options.include_directories.is_empty()
            || self.database.has_vpaths()
            || !self.vpath_dirs.is_empty()
            || !self.gpath_dirs.is_empty()
        {
            return false;
        }
        let no_recipe_fallback = self.no_remake_recipe_fallback();
        if self.options.load_limit.is_some() {
            let ruled_sources = || {
                read_files.iter().filter_map(|(file, skip)| {
                    (!skip)
                        .then(|| self.database.rules.get(without_dot_prefix(&file.name)))
                        .flatten()
                        .map(|rule| (file, rule))
                })
            };
            if ruled_sources().any(|(file, rule)| file.optional && !rule.dependencies.is_empty())
                && ruled_sources().take(2).count() > 1
            {
                // A different root can still change the restart decision
                // before a stopped optional owner's queue settles.
                return false;
            }
        }
        let mut names = HashMap::new();
        read_files.iter().all(|(file, skip)| {
            let name = without_dot_prefix(&file.name);
            *skip
                || (!name.contains(&b'/')
                    && !name.starts_with(b"~")
                    && !name.starts_with(b"-l")
                    && archive_parts(name).is_none()
                    && {
                        let include_policy = file.location.as_ref().map(|_| file.optional);
                        names
                            .insert(name, include_policy)
                            .is_none_or(|earlier_policy| {
                                include_policy.is_some()
                                    && include_policy == earlier_policy
                                    && self.options.load_limit.is_none()
                                    && !self.database.loaded_modules.contains_key(name)
                                    && self.database.rules.get(name).is_some_and(|rule| {
                                        rule.dependencies.is_empty()
                                            && rule.secondary.is_none()
                                            && rule.group.is_none()
                                            && rule.double_colon.is_none()
                                            && rule.recipes.is_some()
                                    })
                            })
                    }
                    && (!file.optional
                        || self
                            .database
                            .rules
                            .get(name)
                            .map_or(no_recipe_fallback, |rule| {
                                rule.secondary.is_none()
                                    && rule.double_colon.is_none()
                                    && (if rule.dependencies.is_empty() {
                                        rule.recipes.is_some() || no_recipe_fallback
                                    } else {
                                        no_recipe_fallback
                                    })
                                    && (rule.group.is_none()
                                        || (self.options.load_limit.is_none()
                                            && rule.dependencies.iter().all(|dependency| {
                                                self.simple_remake_prerequisite(dependency, &[])
                                            })))
                            })))
        })
    }

    fn grouped_remake_comparison_times<'a>(
        &self,
        read_files: &'a [(Makefile, bool)],
    ) -> HashMap<&'a [u8], SystemTime> {
        let mut times = HashMap::new();
        if self.options.check_symlink_times
            || !self.options.old_files.is_empty()
            || !self.options.new_files.is_empty()
        {
            return times;
        }
        let mut earlier_optional_peers = HashSet::new();
        for (file, skip) in read_files {
            if *skip {
                continue;
            }
            let name = file.name.as_slice();
            if file.optional {
                if let Some(group) = self
                    .database
                    .rules
                    .get(name)
                    .and_then(|rule| rule.group.as_ref())
                {
                    earlier_optional_peers.extend(group.targets.iter().map(Vec::as_slice));
                }
            } else if !file.missing
                && earlier_optional_peers.contains(name)
                && rmake_core::filenames::archive_parts(name).is_none()
                && let Ok(metadata) = fs::metadata(os(name))
                && metadata.is_file()
                && let Ok(time) = metadata.modified()
            {
                // An earlier remake can rewrite this peer before its own visit.
                // Keep the comparison separate from graph and database observations.
                times.entry(name).or_insert(time);
            }
        }
        times
    }

    fn debug_progress(&self) -> u64 {
        self.recipe_reaps
            .wrapping_add(self.filesystem.borrow().file_writes())
    }

    /// A child reap or a make-function file write advances the goal diagnostics.
    /// Speculative scheduler rescans leave this progress unchanged.
    fn advance_debug_pass(&mut self) {
        let progress = self.debug_progress();
        if self.debug_pass != progress {
            self.debug_pass = progress;
            if let Some(considered) = &mut self.debug_considered {
                considered.clear();
            }
        }
    }

    fn debug_consider(&mut self, target: &[u8], name: &[u8], depth: usize) {
        use crate::debug::{Event, Mask, emit};

        if self.output.debug_mask().has(Mask::VERBOSE) {
            self.debug_considered
                .get_or_insert_default()
                .insert(target.to_vec());
        }
        emit(&self.output, Event::Considering(name, depth));
    }

    fn debug_cached_prerequisite_visit(&mut self, target: &[u8], depth: usize, outcome: Outcome) {
        use crate::debug::{Event, Mask, emit};

        if !self.output.debug_mask().has(Mask::VERBOSE)
            || outcome.fatal
            || outcome.question_outdated
        {
            return;
        }
        let name = self.aliases.resolve(target);
        let considered = self.debug_considered.get_or_insert_default();
        if considered.contains(name) {
            emit(&self.output, Event::Pruning(name, depth));
            return;
        }
        considered.insert(name.to_vec());
        emit(&self.output, Event::Considering(name, depth));
        emit(
            &self.output,
            if self.pending(name) {
                Event::StillUpdating(name, depth)
            } else if outcome.failed {
                Event::RecentlyFailed(name, depth)
            } else {
                Event::Already(name, depth)
            },
        );
    }

    fn debug_double_colon_goal_prunes(&self, goal: &[u8], outcome: Outcome) {
        use crate::debug::{Event, Mask, emit};

        if !self.output.debug_mask().has(Mask::VERBOSE)
            || self.graph_abort.is_some()
            || self.shutdown.get() != Shutdown::Running
            || outcome.fatal
            || (!self.options.keep_going && (outcome.failed || outcome.question_outdated))
        {
            return;
        }
        let name = self.aliases.resolve(goal);
        if let Some(branches) = self
            .database
            .rules
            .get(name)
            .and_then(|rule| rule.double_colon.as_ref())
        {
            // Each extra goal entry returns to the same considered chain head.
            let count = branches.len() + usize::from(self.visited_cached_double_colon_input(name));
            for _ in 1..count {
                emit(
                    &self.output,
                    Event::Pruning(name, usize::from(self.remaking)),
                );
            }
        }
    }

    fn debug_completed_double_colon_declarations(
        &self,
        target: &[u8],
        declarations: std::ops::Range<usize>,
        depth: usize,
    ) {
        use crate::debug::{Event, Mask, emit};

        if declarations.is_empty() || !self.output.debug_mask().has(Mask::VERBOSE) {
            return;
        }
        let mut key = (target.to_vec(), 0);
        for prior in declarations {
            key.1 = prior;
            if let Some(outcome) = self.database_branch_outcomes.get(&key) {
                // Every finished declaration has its own diagnostic visit.
                emit(&self.output, Event::Considering(target, depth));
                emit(
                    &self.output,
                    if outcome.failed || outcome.question_outdated {
                        Event::RecentlyFailed(target, depth)
                    } else {
                        Event::Already(target, depth)
                    },
                );
            }
        }
    }

    fn goal_recipe_waiting(&self, goal: &[u8]) -> bool {
        let name = self.aliases.resolve(goal);
        self.parallel_jobs.contains_key(name)
            && self
                .pending_double_colon
                .get(name)
                .is_none_or(DoubleColonState::single_active_branch)
    }

    fn ordinary_group_peer_pending(&self, peer: &[u8]) -> bool {
        self.active_peers
            .get(self.aliases.resolve(peer))
            .is_some_and(|owner| self.ordinary_group_recipe_pending(owner))
    }

    fn ordinary_group_recipe_pending(&self, owner: &[u8]) -> bool {
        self.parallel_jobs.contains_key(owner)
            && self
                .running_groups
                .get(owner)
                .is_some_and(|group| group.ordinary)
    }

    fn debug_completed_goal_visit(&mut self, goal: &[u8], grouped_peer: bool) {
        use crate::debug::{Event, Mask, emit};

        if !self.output.debug_mask().has(Mask::VERBOSE)
            || self.graph_abort.is_some()
            || self.shutdown.get() != Shutdown::Running
        {
            return;
        }
        let name = self.aliases.resolve(goal);
        let Some(outcome) = self.done.get(name).copied() else {
            return;
        };
        if outcome.fatal
            || outcome.question_outdated
            || (outcome.failed && !self.options.keep_going)
        {
            return;
        }
        // The goal chain reads the cached completion here. No rule, timestamp,
        // or prerequisite expansion is evaluated for this diagnostic visit.
        if grouped_peer {
            self.debug_cached_prerequisite_visit(goal, 0, outcome);
            return;
        }
        emit(&self.output, Event::Considering(name, 0));
        emit(
            &self.output,
            if outcome.failed {
                Event::RecentlyFailed(name, 0)
            } else {
                Event::Already(name, 0)
            },
        );
    }

    fn report_blocked_goal(&self, goal: &[u8]) {
        if self.options.keep_going && !self.options.question && !self.options.dry_run {
            self.stderr(&crate::messages::system(
                rmake_core::messages::Message::TargetNotRemade(goal),
            ));
        }
    }

    fn report_goal(&self, goal: &[u8], started_command: bool, outcome: Outcome) {
        if outcome.fatal {
            return;
        }
        if outcome.failed && outcome.blocked {
            let identity = self.aliases.resolve(goal);
            if self
                .database
                .rules
                .get(identity)
                .is_none_or(|rule| rule.double_colon.is_none())
            {
                self.report_blocked_goal(goal);
            }
        } else if !outcome.failed
            && !started_command
            && !self.options.question
            && !self.globally_silent()
        {
            let identity = self.aliases.resolve(goal);
            let has_recipe = self.database.rules.get(identity).is_some_and(|rule| {
                rule.recipes.is_some()
                    || rule
                        .double_colon
                        .as_ref()
                        .is_some_and(|branches| branches.iter().any(|rule| rule.recipes.is_some()))
            }) && !self.special(b".PHONY", goal, false);
            let reported_goal = self
                .resolved_names
                .get(identity)
                .map_or(identity, Vec::as_slice);
            // Rehashing into a file already entered by a declaration or CLI
            // goal merges GNU's goal record; the old record has no summary.
            if reported_goal != goal
                && (if self.aliases.contains(goal) {
                    self.aliases.merged(goal)
                } else {
                    self.database.explicit_names.contains(reported_goal)
                        || self.options.goals.iter().any(|name| name == reported_goal)
                })
            {
                return;
            }
            let goal = reported_goal;
            let message = if has_recipe {
                rmake_core::messages::Message::UpToDate(goal)
            } else {
                rmake_core::messages::Message::NothingToBeDone(goal)
            };
            let body = crate::messages::system(message);
            self.stdout(&[self.program.as_slice(), b": ", &body].concat());
        }
    }

    // Reader headers are frozen and hidden extras have been snapped. The
    // live rule remains authoritative after a runtime update or alias.
    fn capture_phony_names(&mut self) {
        const MIN_NAMES: usize = 64;
        self.phony_names = None;
        let Some(rule) = self
            .database
            .rules
            .get(b".PHONY".as_slice())
            .filter(|rule| rule.dependencies.len() >= MIN_NAMES)
        else {
            return;
        };
        let names = rule.dependencies.iter().filter(|dep| !dep.hidden);
        let count = names.clone().count();
        if count >= MIN_NAMES {
            let mut indexed = HashSet::with_capacity(count);
            indexed.extend(names.map(|dep| dep.name.clone()));
            self.phony_names = Some(Box::new(indexed));
        }
    }

    fn invalidate_phony_names(&mut self, target: &[u8]) {
        if target == b".PHONY" {
            self.phony_names = None;
        }
    }

    fn special(&self, special: &[u8], target: &[u8], empty_means_all: bool) -> bool {
        if let Some(names) = &self.phony_names
            && special == b".PHONY"
            && !empty_means_all
            && self.aliases.is_empty()
        {
            return names.contains(target);
        }
        if let Some(names) = self.intermediate_policy.named(special) {
            return (empty_means_all
                && special == b".SECONDARY"
                && self.intermediate_policy.all_secondary)
                || names.contains(target)
                || (!self.aliases.is_empty()
                    && names
                        .iter()
                        .any(|name| self.aliases.resolve(name) == target));
        }
        self.database.rules.get(special).is_some_and(|rule| {
            (empty_means_all && rule.dependencies.iter().all(|dep| dep.hidden))
                || rule.dependencies.iter().any(|dependency| {
                    !dependency.hidden && self.aliases.resolve(&dependency.name) == target
                })
        })
    }

    fn intermediate(&self, target: &[u8]) -> bool {
        let target = self.aliases.resolve(target);
        let eligible = || {
            self.special(b".INTERMEDIATE", target, false)
                || self.special(b".SECONDARY", target, false)
                || (!self.intermediate_policy.no_intermediates
                    && (self.intermediate_policy.global_intermediate(target)
                        || self.intermediates.contains(target))
                    && !self.special_for_rule(b".NOTINTERMEDIATE", target))
        };
        let explicitly_requested = || {
            self.options
                .goals
                .iter()
                .any(|goal| self.aliases.resolve(goal) == target)
        };
        if self.intermediate_policy.all_secondary || !self.aliases.is_empty() {
            // Broad eligibility and live aliases can make the policy check
            // more expensive than finding a requested goal.
            !explicitly_requested() && eligible()
        } else {
            eligible() && !explicitly_requested()
        }
    }

    fn special_for_rule(&self, special: &[u8], target: &[u8]) -> bool {
        self.inferred_peer_flags
            .get(target)
            .is_some_and(|&(precious, notintermediate)| {
                (special == b".PRECIOUS" && precious)
                    || (special == b".NOTINTERMEDIATE" && notintermediate)
            })
            || self.special(special, target, false)
            || self
                .database
                .rules
                .get(target)
                .and_then(|rule| rule.implicit_pattern.as_deref())
                .is_some_and(|pattern| self.special(special, pattern, false))
    }

    fn remove_intermediates(&mut self) {
        match self.shutdown.get() {
            Shutdown::RetainedOwner => {
                if let Some(FailedOwner {
                    target,
                    location,
                    ignore,
                    output,
                }) = self.shutdown_launch_failure.borrow_mut().take()
                {
                    self.output
                        .select(output.as_ref().filter(|buffer| buffer.enabled()));
                    self.stderr(
                        &[
                            if ignore { b"".as_slice() } else { b"*** " },
                            b"[",
                            &location_prefix(&location),
                            &target,
                            if ignore {
                                b"] Error 127 (ignored)".as_slice()
                            } else {
                                b"] Error 127"
                            },
                            self.shuffle.suffix(),
                        ]
                        .concat(),
                    );
                    self.output.select(None);
                    if let Some(buffer) = output {
                        buffer.dump();
                    }
                } else {
                    self.fatal(None, b"wait: No child processes");
                }
                return;
            }
            Shutdown::Reentered => return,
            Shutdown::Running | Shutdown::Draining => {}
        }
        if self.interrupt.is_some() && self.options.dry_run {
            return;
        }
        if self.cleanup_candidates.is_empty() {
            return;
        }
        if self.intermediate_policy.all_secondary || self.intermediate_policy.no_intermediates {
            self.cleanup_candidates.clear();
            return;
        }
        let created: HashSet<_> = std::mem::take(&mut self.cleanup_candidates)
            .into_iter()
            .filter(|target| self.intermediate(target))
            .collect();
        if created.is_empty() {
            return;
        }
        let ordered: Vec<_> = self
            .database
            .file_records
            .names()
            .filter(|name| created.contains(*name))
            .map(Vec::from)
            .collect();
        let mut line = Vec::new();
        for target in ordered {
            if self.special(b".SECONDARY", &target, true)
                || self.database_secondary.contains(&target)
                || self.special_for_rule(b".PRECIOUS", &target)
                || self.special_for_rule(b".NOTINTERMEDIATE", &target)
            {
                continue;
            }
            if self.options.question || self.options.touch {
                continue;
            }
            let result = if self.options.dry_run {
                Ok(())
            } else {
                fs::remove_file(os(&target))
            };
            if result
                .as_ref()
                .is_err_and(|error| error.kind() == io::ErrorKind::NotFound)
            {
                continue;
            }
            if self.interrupt.is_some() {
                self.stderr(
                    &[
                        b"*** Deleting intermediate file '".as_slice(),
                        &target,
                        b"'",
                    ]
                    .concat(),
                );
                if let Err(error) = result {
                    self.stderr(
                        &[
                            b"\nunlink: ".as_slice(),
                            &target,
                            b": ",
                            os_error(&error).as_bytes(),
                        ]
                        .concat(),
                    );
                }
                continue;
            }
            // GNU emits the debug heading whenever it starts a printed rm
            // list; under -s the list stays empty, so each removal logs it.
            if line.is_empty() {
                crate::debug::emit(&self.output, crate::debug::Event::RemovingIntermediates);
            }
            if !self.globally_silent() {
                if line.is_empty() {
                    line.extend_from_slice(b"rm ");
                } else {
                    line.push(b' ');
                }
                line.extend_from_slice(&target);
            }
            if let Err(error) = result {
                // GNU flushes the partial command before the unlink diagnostic
                // and begins a new command for any later removable record.
                crate::output::stdout_parts(&[&line]);
                line.clear();
                self.stderr(
                    &[
                        b"\nunlink: ".as_slice(),
                        &target,
                        b": ",
                        os_error(&error).as_bytes(),
                    ]
                    .concat(),
                );
            }
        }
        if !line.is_empty() {
            self.stdout(&line);
        }
    }

    fn intermediate_time(&mut self, target: &[u8]) -> Result<Option<SystemTime>, ()> {
        if rmake_core::filenames::archive_parts(target).is_none() {
            let time = self.modified(target);
            if self.clock_candidate(time).is_some()
                && !self.done.contains_key(target)
                && !self.special(b".PHONY", target, false)
            {
                self.check_clock_skew(target, time);
            }
            return Ok(time);
        }
        if self.special(b".PHONY", target, false) {
            return Ok(None);
        }
        let prepared = self.prepare_search_name(target)?;
        let target = prepared.as_deref().unwrap_or(target);
        if let Some(outcome) = self.done.get(target) {
            return Ok(outcome.time);
        }
        if !self.path_observations.contains_key(target) && self.archive_metadata_error(target, true)
        {
            return Err(());
        }
        Ok(self
            .path_observations
            .get(target)
            .and_then(|observation| observation.as_ref())
            .and_then(|(_, time)| *time))
    }

    /// Missing intermediate files do not by themselves make a parent stale.
    /// Update their real inputs first, and materialize the chain only if an
    /// input is newer than the parent (or another prerequisite requires it).
    fn check_intermediate(
        &mut self,
        target: &[u8],
        parent: ScopeParent<&[u8]>,
        time: Option<SystemTime>,
        depth: usize,
    ) -> Inspection {
        let prepared = if rmake_core::filenames::archive_parts(target).is_some() {
            match self.prepare_search_name(target) {
                Ok(prepared) => prepared,
                Err(()) => {
                    return Inspection {
                        outcome: Outcome {
                            failed: true,
                            fatal: true,
                            ..Outcome::default()
                        },
                        pending: false,
                        needs_rebuild: false,
                    };
                }
            }
        } else {
            None
        };
        let target = prepared.as_deref().unwrap_or(target);
        let direct = depth >= 512
            || self.assumed_time(target).is_some()
            || self.done.contains_key(target)
            || self.pending(target)
            || !self.intermediate(target)
            || self.special(b".PHONY", target, false);
        let present = if direct {
            false
        } else {
            match self.intermediate_time(target) {
                Ok(time) => time.is_some(),
                Err(()) => {
                    return Inspection {
                        outcome: Outcome {
                            failed: true,
                            fatal: true,
                            ..Outcome::default()
                        },
                        pending: false,
                        needs_rebuild: false,
                    };
                }
            }
        };
        if direct || present {
            let mut outcome = self.build(target, Some(parent), depth);
            if !self.pending(target) {
                outcome = self.observe_updated(target, outcome);
            }
            return Inspection {
                needs_rebuild: self.prerequisite_stale(time, outcome),
                outcome,
                pending: self.pending(target),
            };
        }
        self.scope_parents
            .entry(target.to_vec())
            .or_insert_with(|| Some(parent.into_owned()));
        if !self.resolve_implicit_for_intermediate(target, depth) {
            return Inspection {
                outcome: Outcome {
                    failed: true,
                    fatal: true,
                    ..Outcome::default()
                },
                pending: false,
                needs_rebuild: false,
            };
        }
        let Some(stored) = self.database.rules.get(target) else {
            // A missing intermediate without a recipe can remain absent while
            // its existing parent is current. Materialization checks it again
            // if another prerequisite makes the parent stale.
            return Inspection {
                outcome: Outcome {
                    time,
                    ..Outcome::default()
                },
                pending: false,
                needs_rebuild: false,
            };
        };
        // GNU inspects the first file record of a double-colon chain. Other
        // branches and grouped peers are visited only if a build is required.
        let double_colon = stored.double_colon.is_some();
        let mut rule = stored
            .double_colon
            .as_ref()
            .and_then(|branches| branches.first())
            .unwrap_or(stored)
            .clone();
        if rule.secondary.is_some() {
            if let Err(error) =
                self.expand_dependencies(target, &mut rule, double_colon.then_some(0))
            {
                self.expansion_error(rule.prerequisite_location.as_ref(), &error);
                return Inspection {
                    outcome: Outcome {
                        failed: true,
                        fatal: true,
                        ..Outcome::default()
                    },
                    pending: false,
                    needs_rebuild: false,
                };
            }
            self.store_inspected_rule(target, double_colon, rule.clone());
        }
        let child_parent = ScopeParent {
            target,
            branch: double_colon
                .then_some(0)
                .filter(|_| rule.grouped_double_id().is_none()),
        };
        let mut result = Outcome {
            time,
            ..Outcome::default()
        };
        let mut stale = self.options.always;
        let mut waiting = false;
        self.visiting.insert(target.to_vec());
        // Freshness follows declaration order, even when later builds are
        // shuffled.
        let mut next = 0;
        let mut dropped = false;
        while next < rule.dependencies.len() {
            let dep = &rule.dependencies[next];
            // A freshness inspection visits inputs across prerequisite
            // barriers. The actual rebuild still uses the normal scheduler.
            if dep.name == b".WAIT" {
                next += 1;
                continue;
            }
            if self.visiting.contains(&dep.name) {
                self.warn_circular_dependency(target, &dep.name);
                rule.dependencies.remove(next);
                self.shuffle.discard(target, next);
                dropped = true;
                continue;
            }
            next += 1;
            // Inspection of a missing intermediate does not carry the
            // enclosing makefile's optional policy into its prerequisites.
            let optional = self
                .active_makefile_remake
                .as_mut()
                .is_some_and(|remake| std::mem::take(&mut remake.optional));
            let Inspection {
                outcome: mut child,
                pending,
                needs_rebuild,
            } = self.check_intermediate(&dep.name, child_parent, time, depth + 1);
            if let Some(remake) = &mut self.active_makefile_remake {
                remake.optional = optional;
            }
            if optional && child.failed && !self.options.keep_going {
                let mut shutdown = self.shutdown.get();
                shutdown.drain();
                self.shutdown.set(shutdown);
                child.fatal = true;
            }
            merge_child(&mut result, child);
            waiting |= pending;
            // A running child has no final timestamp yet. Never materialize an
            // intermediate just because that provisional timestamp is absent.
            if !pending {
                stale |= !dep.order_only && needs_rebuild;
            }
            if child.fatal || (child.failed && !self.options.keep_going) {
                break;
            }
        }
        if self.retain_remake_failure() && result.blocked && !result.fatal && !waiting {
            self.remember_optional_prerequisite(target, &rule, None);
        }
        if dropped {
            self.store_inspected_rule(target, double_colon, rule);
        }
        self.visiting.remove(target);
        if waiting {
            Inspection {
                outcome: result,
                pending: true,
                needs_rebuild: stale,
            }
        } else {
            // Inspection never executes an intermediate recipe. The owning
            // frame materializes required middles after all prerequisite and
            // grouped-peer inspection phases have completed.
            Inspection {
                outcome: result,
                pending: false,
                needs_rebuild: stale,
            }
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
                .is_some_and(|rule| rule.dependencies.iter().all(|dep| dep.hidden))
    }

    fn job_slots(&self) -> usize {
        if (self.remaking && !self.parallel_makefile_remakes)
            || self.options.inherited_jobs
            || self.database.globally_serial()
        {
            1
        } else if self.jobserver.is_some() || self.unbounded_auth_jobs {
            usize::MAX
        } else {
            self.options.jobs.unwrap_or(1)
        }
    }

    fn reserve_job(
        &mut self,
        debug_job: Option<&crate::debug::JobHandle>,
    ) -> io::Result<Option<Permit>> {
        let Some(server) = self.jobserver.clone() else {
            return Ok(None);
        };
        self.job_debug.need(self.output.debug_mask());
        self.job_debug.live(self.output.debug_mask());
        loop {
            self.start_load_waiting();
            if !self.parallel_jobs.values().any(|job| {
                job.continuation
                    .permit
                    .as_ref()
                    .is_some_and(Permit::is_implicit)
            }) {
                return Ok(Some(Permit::implicit()));
            }
            if let Some(permit) = server.try_acquire()? {
                crate::debug::Jobs::obtained(self.output.debug_mask(), debug_job);
                return Ok(Some(permit));
            }
            // Wait for this pool's reader, owned child exits, or a fatal signal.
            // No polling delay is added to tokens returned by another process.
            self.child_events(true, Some(server.readiness()?))?;
            self.observe_interrupt();
            if self.graph_abort.is_some()
                || self.forced_query_stop
                || self.remake_stopped()
                || (self.parallel_failed && !self.options.keep_going)
            {
                return Ok(None);
            }
        }
    }

    fn has_unfinished_query_tail(&self) -> bool {
        // Once a query verdict is established, a later goal cannot remove its
        // unfinished tail from the pass. A grouped recipe may have completed
        // one of those branches through a different goal.
        self.ordinary_queries
            .deferred_tails
            .iter()
            .any(|(target, tail)| {
                if !tail.queued {
                    return false;
                }
                let mut branch_key = (target.clone(), 0);
                tail.state.branches.iter().any(|(ordinal, rule)| {
                    branch_key.1 = *ordinal;
                    rule.is_some() && !self.database_branch_outcomes.contains_key(&branch_key)
                })
            })
    }

    fn queued_query_tail(&self, target: &[u8]) -> bool {
        !self.ordinary_queries.deferred_tails.is_empty()
            && self
                .ordinary_queries
                .deferred_tails
                .get(self.aliases.resolve(target))
                .is_some_and(|tail| tail.queued)
    }

    fn graph_pending(&self, target: &[u8]) -> bool {
        let target = self.aliases.resolve(target);
        self.pending_rules.contains_key(target) || self.pending_double_colon.contains_key(target)
    }

    fn pending(&self, target: &[u8]) -> bool {
        let target = self.aliases.resolve(target);
        self.graph_pending(target)
            || self.parallel_jobs.contains_key(target)
            || self
                .active_peers
                .get(target)
                .is_some_and(|owner| self.parallel_jobs.contains_key(owner))
    }

    fn parallel_graph(&self) -> bool {
        self.job_slots() > 1
            && (!self.remaking
                || self.parallel_makefile_remakes
                || !(self.options.dry_run || self.options.question || self.options.touch))
    }

    fn wait_makefile_remakes(
        &mut self,
        requests: &mut [PendingMakefileRemake<'_>],
        visited: &MakefileVisits<'_>,
        mut last_reaps: Option<u64>,
    ) -> bool {
        let wait_for_completed_job = last_reaps.is_none();
        'passes: while requests.iter().any(|request| request.active) {
            if self.shutdown.get() != Shutdown::Running {
                return false;
            }
            // Reap against the previous pass's cursor. A pass with no newly
            // completed commands waits before changing which root is current.
            let previous_jobs = self.parallel_jobs.len();
            let wait = Some(self.recipe_reaps) == last_reaps;
            self.poll_jobs();
            if wait
                && (if wait_for_completed_job {
                    self.parallel_jobs.len() >= previous_jobs
                } else {
                    Some(self.recipe_reaps) == last_reaps
                })
                && self.parallel_jobs.values().any(|job| !job.waiting_load)
            {
                self.reap_job();
                while wait_for_completed_job
                    && self.parallel_jobs.len() >= previous_jobs
                    && self.shutdown.get() == Shutdown::Running
                    && self.graph_abort.is_none()
                    && !self.forced_query_stop
                {
                    self.reap_job();
                }
                self.poll_jobs();
            }
            if self.forced_query_stop || self.shutdown.get() != Shutdown::Running {
                return false;
            }
            last_reaps = Some(self.recipe_reaps);
            self.advance_debug_pass();
            self.graph_epoch = self.graph_epoch.wrapping_add(1);
            let pass_epoch = self.graph_epoch;
            for request in requests.iter_mut().filter(|request| request.active) {
                let name = request.file.name.as_slice();
                if request.duplicate
                    && request.file.optional
                    && request.modes.active()
                    && !self.pending(name)
                {
                    // A settled optional duplicate cannot select the mode
                    // used by an earlier source's pending continuation.
                    request.active = false;
                    continue;
                }
                request.modes.apply(&mut self.options);
                self.remake_error_cursor = request.context.open_error.clone();
                let needs_visit = if request.duplicate {
                    !request.file.optional
                        && !self.pending(name)
                        && self.has_optional_remake_failure(name)
                } else {
                    self.graph_pending(name)
                };
                if needs_visit {
                    request.initial =
                        self.with_makefile_remake(Some(request.context.clone()), |engine| {
                            engine.build_recording_initial_time(
                                name,
                                None,
                                0,
                                None,
                                BuildVisit::Ordinary,
                                None,
                            )
                        });
                }
                request.active = self.pending(name);
                if self.shutdown.get() != Shutdown::Running {
                    return false;
                }
                let outcome = self
                    .done
                    .get(self.aliases.resolve(name))
                    .copied()
                    .unwrap_or(request.initial);
                if !needs_visit {
                    if wait_for_completed_job && outcome.question_outdated && !request.active {
                        self.debug_consider(name, name, 1);
                        crate::debug::emit(
                            &self.output,
                            crate::debug::Event::RecentlyFailed(name, 1),
                        );
                    } else {
                        self.debug_cached_prerequisite_visit(name, 1, outcome);
                    }
                }
                if !request.active
                    && request.file.optional
                    && !outcome.failed
                    && !outcome.question_outdated
                    && matches!(
                        outcome.completion,
                        CompletionTime::Refresh | CompletionTime::ArchiveUnknown
                    )
                    && outcome.time != visited[name].0
                {
                    // An updated optional root ends this pass. Remaining roots
                    // are revisited after the next child event.
                    continue 'passes;
                }
            }
            if self.graph_epoch == pass_epoch
                && (self.parallel_jobs.is_empty()
                    || (self.defer_stopped_remake_load
                        && self.parallel_jobs.values().all(|job| job.waiting_load)))
                && requests.iter().any(|request| request.active)
            {
                self.fatal(None, b"cannot advance pending makefile remakes");
                return false;
            }
        }
        true
    }

    fn wait_target(&mut self, target: &[u8]) {
        while self.pending(target) {
            self.graph_epoch = self.graph_epoch.wrapping_add(1);
            let pass_epoch = self.graph_epoch;
            if self.graph_pending(target) {
                self.build(target, None, 0);
            }
            if self.graph_epoch == pass_epoch
                && self.pending(target)
                && !self.parallel_jobs.is_empty()
            {
                self.reap_job();
            }
        }
    }

    fn notice_waiting_for_jobs(&self) {
        if !self.waiting_reported.replace(true) {
            self.stderr(b"*** Waiting for unfinished jobs....");
        }
    }

    fn wait_all_jobs(&mut self) {
        while !self.parallel_jobs.is_empty() {
            self.reap_job();
        }
    }

    fn poll_jobs(&mut self) {
        self.observe_interrupt();
        self.start_load_waiting();
        loop {
            match self.child_events(false, None) {
                Ok(true) => self.start_load_waiting(),
                Ok(false) => break,
                Err(error) => {
                    self.fatal(None, os_error(&error).as_bytes());
                    break;
                }
            }
        }
    }

    fn parallel_child_handles(&self) -> Vec<(u64, &rmake_linux::child::Child)> {
        self.parallel_jobs
            .values()
            .filter_map(|job| job.child.as_ref().map(|child| (job.identity, child)))
            .collect()
    }

    fn child_events(
        &mut self,
        block: bool,
        additional: Option<std::os::fd::RawFd>,
    ) -> io::Result<bool> {
        let children = self.parallel_child_handles();
        if children.is_empty() && additional.is_none() {
            return Ok(false);
        }
        if block {
            self.job_debug.live(self.output.debug_mask());
        }
        // GNU retries load-delayed recipes after a one-second jobserver wait.
        // Ordinary child waits remain event-driven; token readiness still wakes
        // immediately instead of paying this retry interval.
        let load_retry = (additional.is_some() && !self.load_waiting.is_empty())
            .then_some(Duration::from_secs(1));
        #[cfg(feature = "guile")]
        let (ready, observed_completion) = {
            let mut workers =
                if block && !children.is_empty() && self.shutdown.get() == Shutdown::Running {
                    // Optional readiness must not prevent reaping owned children.
                    rmake_linux::guile::WorkerWait::begin()
                        .ok()
                        .filter(rmake_linux::guile::WorkerWait::is_active)
                } else {
                    None
                };
            let poll_interval = workers
                .as_ref()
                .and_then(rmake_linux::guile::WorkerWait::poll_interval);
            let retry_deadline = workers
                .as_ref()
                .and(load_retry)
                .map(|retry| std::time::Instant::now() + retry);
            let mut maximum_wait = load_retry;
            let mut children = children;
            loop {
                let events = rmake_linux::child::wait_with_notification(
                    &children,
                    block,
                    true,
                    additional,
                    workers.as_ref().and_then(|wait| wait.descriptor()),
                    maximum_wait.into_iter().chain(poll_interval).min(),
                )?;
                let Some(wait) = &mut workers else {
                    break (events.children, None);
                };
                drop(children);
                self.observe_interrupt();
                if self.interrupt.is_some() {
                    break (events.children, None);
                }
                let worker_tick = poll_interval.is_some()
                    && events.timed_out
                    && retry_deadline.is_none_or(|deadline| std::time::Instant::now() < deadline);
                // PID-only children are candidates for reaping on every poll.
                // Keep admission through a timer tick when none has exited.
                let completion = worker_tick
                    .then(|| self.ready_parallel_job(&events.children))
                    .flatten();
                if events.notification_only || (worker_tick && completion.is_none()) {
                    // A callback did not finish the scheduler wait. Retain
                    // admission across a worker's next synchronous request.
                    if !wait.pending() || self.service_parallel_workers() {
                        maximum_wait = retry_deadline.map(|deadline| {
                            deadline.saturating_duration_since(std::time::Instant::now())
                        });
                        children = self.parallel_child_handles();
                        continue;
                    }
                } else {
                    // A child or token can wake the poll together with a
                    // worker. Service accepted requests before closing it.
                    while !wait.finish_if_idle() && self.service_parallel_workers() {}
                }
                break (events.children, completion);
            }
        };
        #[cfg(not(feature = "guile"))]
        let (ready, observed_completion) = (
            rmake_linux::child::wait(&children, block, true, additional, load_retry)?,
            None,
        );
        // Idle nonblocking polls are an internal graph refresh, rather than a
        // visible reaping pass. A ready completion still reports its live scan.
        if !block && !ready.is_empty() {
            self.job_debug.live(self.output.debug_mask());
        }
        self.observe_interrupt();
        if let Some(completion) = observed_completion.or_else(|| self.ready_parallel_job(&ready)) {
            self.complete_job(completion, block && additional.is_none());
            return Ok(true);
        }
        Ok(false)
    }

    fn ready_parallel_job(&mut self, ready: &[u64]) -> Option<JobResult> {
        for &identity in ready {
            let (target, job) = self
                .parallel_jobs
                .entries_mut()
                .find(|(_, job)| job.identity == identity)
                .expect("live child identity");
            let result = match job.child.as_mut().expect("live child").try_wait() {
                Ok(Some(status)) => Ok(status),
                Ok(None) => continue,
                Err(error) => Err(error),
            };
            return Some((target.to_vec(), identity, Some(result)));
        }
        None
    }

    fn observe_interrupt(&mut self) {
        if rmake_linux::signals::pending().is_none() {
            return;
        }
        rmake_linux::signals::drain();
        let number = rmake_linux::signals::pending().expect("a pending signal is retained");
        if self.interrupt == Some(number) {
            return;
        }
        self.interrupt = Some(number);
        self.unlink_stdin();
        self.graph_abort = Some(Status::Fatal);
        self.shutdown.set(Shutdown::Draining);
        self.parallel_failed = true;
        if let Some(server) = &self.jobserver {
            server.unlink_owned();
        }
        let mut cleanup = Vec::new();
        for (name, job) in self.parallel_jobs.entries_mut() {
            let Some(child) = &mut job.child else {
                continue;
            };
            if number == 15 {
                let _ = child.send_signal(number);
            }
            if rmake_linux::signals::deletes_targets(number) && !job.signal_cleaned {
                cleanup.push((job.identity, name.to_vec(), job.before, job.phony));
                job.signal_cleaned = true;
            }
        }
        cleanup.sort_unstable_by_key(|entry| std::cmp::Reverse(entry.0));
        for (_, target, before, phony) in cleanup {
            self.cleanup_target(&target, before, phony, Cleanup::ChangedOnSignal);
        }
        if !rmake_linux::signals::deletes_targets(number)
            && self.parallel_jobs.values().any(|job| job.child.is_some())
        {
            self.notice_waiting_for_jobs();
        }
    }

    fn wait_serial_child(
        &mut self,
        child: &mut rmake_linux::child::Child,
        target: &[u8],
        before: Option<SystemTime>,
        phony: bool,
        debug_job: Option<&crate::debug::JobHandle>,
    ) -> io::Result<SerialWait> {
        self.job_debug.live(self.output.debug_mask());
        let mut observed = None;
        let mut cleaned = false;
        #[cfg(feature = "guile")]
        let mut workers = rmake_linux::guile::WorkerWait::begin()?;
        #[cfg(feature = "guile")]
        let mut interpreter_failed = false;
        loop {
            self.observe_interrupt();
            if let Some(number) = self.interrupt
                && observed != Some(number)
            {
                observed = Some(number);
                #[cfg(feature = "guile")]
                workers.cancel();
                if number == 15 {
                    child.send_signal(number)?;
                }
                if rmake_linux::signals::deletes_targets(number) {
                    if !cleaned {
                        self.cleanup_target(target, before, phony, Cleanup::ChangedOnSignal);
                        cleaned = true;
                    }
                } else {
                    self.notice_waiting_for_jobs();
                }
            }
            let status = child.try_wait()?;
            #[cfg(feature = "guile")]
            let worker_pending = if status.is_some() {
                !workers.finish_if_idle()
            } else {
                workers.pending()
            };
            #[cfg(feature = "guile")]
            if worker_pending {
                if let Err(error) = self.service_waiting_workers() {
                    workers.cancel();
                    if !matches!(error, ExpandError::Cancelled) {
                        self.expansion_error(None, &error);
                        if status.is_none() {
                            self.notice_waiting_for_jobs();
                        }
                        interpreter_failed = true;
                    }
                }
                continue;
            }
            if let Some(status) = status {
                crate::debug::Jobs::reaped(self.output.debug_mask(), debug_job, status.success());
                self.load.reaped();
                return Ok(SerialWait {
                    status,
                    cleaned,
                    #[cfg(feature = "guile")]
                    interpreter_failed,
                });
            }
            #[cfg(feature = "guile")]
            let (worker_wake, retry) = (workers.descriptor(), workers.poll_interval());
            #[cfg(not(feature = "guile"))]
            let (worker_wake, retry) = (None::<std::os::fd::BorrowedFd<'_>>, None);
            rmake_linux::child::wait(
                &[(0, child)],
                true,
                true,
                worker_wake.map(|fd| std::os::fd::AsRawFd::as_raw_fd(&fd)),
                retry,
            )?;
        }
    }

    fn load_high(&mut self) -> bool {
        let Some(limit) = self.options.load_limit else {
            return false;
        };
        let debug = self.output.debug_mask().has(crate::debug::Mask::JOBS);
        let running = if debug {
            self.parallel_jobs
                .values()
                .filter(|job| job.child.is_some())
                .count()
        } else {
            0
        };
        let output = &self.output;
        let program = &self.program;
        self.load.high(limit, |event| {
            if let crate::load::Observation::AverageFailure(error) = event {
                let body = if error.raw_os_error() == Some(0) {
                    "cannot enforce load limits on this operating system".to_owned()
                } else {
                    format!("cannot enforce load limit: getloadavg: {}", os_error(error))
                };
                output.write_parts(true, &[program, b": ", body.as_bytes(), b"\n"]);
            } else if debug {
                crate::debug::load(event, limit, running);
            }
        })
    }

    fn start_load_waiting(&mut self) {
        if self.graph_abort.is_some()
            || self.forced_query_stop
            || self.shutdown.get() != Shutdown::Running
        {
            while let Some((target, identity)) = self.load_waiting.pop() {
                let job = self
                    .parallel_jobs
                    .remove(&target, identity)
                    .expect("reserved load waiter");
                let mut outcome = self.done.get(&target).copied().unwrap_or_default();
                outcome.failed = true;
                outcome.fatal |= self.graph_abort == Some(Status::Fatal);
                outcome.ran = false;
                self.done.insert(target, outcome);
                drop(job); // Return the queued recipe's permit without launching it.
            }
            return;
        }
        while let Some(last) = self.load_waiting.len().checked_sub(1) {
            if self.parallel_jobs.values().any(|job| !job.waiting_load) && self.load_high() {
                break;
            }
            let next = if self.defer_stopped_remake_load
                && self
                    .active_makefile_remake
                    .as_ref()
                    .is_none_or(MakefileRemake::stopped)
            {
                // A queued sibling of a stopped optional remake keeps its
                // reservation and sees the ordinary build mode on dispatch.
                self.load_waiting.iter().rposition(|(name, identity)| {
                    !self
                        .parallel_jobs
                        .get(name, *identity)
                        .and_then(|job| job.continuation.remake.as_ref())
                        .is_some_and(MakefileRemake::stopped)
                })
            } else {
                Some(last)
            };
            let Some(index) = next else { break };
            let (target, identity) = self.load_waiting.remove(index);
            self.complete_job((target, identity, None), false);
        }
    }

    fn reap_job(&mut self) {
        loop {
            self.observe_interrupt();
            self.start_load_waiting();
            if self.parallel_jobs.is_empty() {
                return;
            }
            match self.child_events(true, None) {
                Ok(true) => return,
                Ok(false) => {}
                Err(error) => {
                    self.fatal(None, os_error(&error).as_bytes());
                    return;
                }
            }
        }
    }

    fn forced_query_completed(&mut self) {
        self.would_build = true;
        // A required child can stop a parallel remake pass with status one.
        if (!self.remaking || self.parallel_makefile_remakes)
            && !self.options.keep_going
            && !self.optional_remake()
            && self.graph_abort.is_none()
        {
            self.forced_query_stop = true;
            if self.parallel_jobs.values().any(|job| !job.waiting_load) {
                self.notice_waiting_for_jobs();
            }
        }
    }

    fn release_completed_permit(&self, permit: Option<Permit>) {
        if let Some(permit) = permit
            && !permit.is_implicit()
            && self.interrupt.is_none()
            && rmake_linux::signals::pending().is_none()
        {
            permit.release_completed();
        }
    }

    /// A user error during command configuration leaves the already reserved
    /// logical slot outstanding while the real children drain.
    fn retain_configuration_abort(&mut self, permit: &mut Option<Permit>, error: &ExpandError) {
        if error.is_user_error()
            && self.aborted_recipe_permit.is_none()
            && self.interrupt.is_none()
            && rmake_linux::signals::pending().is_none()
        {
            self.aborted_recipe_permit = permit.take();
        }
    }

    /// Move one outstanding paid lease into the newly freed implicit slot.
    /// A failed dispatch can still own a logical slot during ordinary drain.
    fn reuse_implicit_job_slot(&mut self, ordinary_completion: bool) {
        if self.jobserver.is_none()
            || self
                .aborted_recipe_permit
                .as_ref()
                .is_some_and(Permit::is_implicit)
            || self.parallel_jobs.values().any(|job| {
                job.continuation
                    .permit
                    .as_ref()
                    .is_some_and(Permit::is_implicit)
            })
        {
            return;
        }
        let paid = if let Some((_, job)) = self
            .parallel_jobs
            .entries_mut()
            .filter(|(_, job)| {
                job.continuation
                    .permit
                    .as_ref()
                    .is_some_and(|permit| !permit.is_implicit())
            })
            .min_by_key(|(_, job)| job.identity)
        {
            job.continuation.permit.replace(Permit::implicit())
        } else if self.aborted_recipe_permit.is_some() {
            self.aborted_recipe_permit.replace(Permit::implicit())
        } else {
            None
        };
        if ordinary_completion {
            self.release_completed_permit(paid);
        }
    }

    fn complete_job(&mut self, (target, identity, result): JobResult, blocking_reap: bool) {
        self.graph_epoch = self.graph_epoch.wrapping_add(1);
        let job = self
            .parallel_jobs
            .remove(&target, identity)
            .expect("reserved parallel target");
        self.with_makefile_remake(job.continuation.remake.clone(), |engine| {
            engine.finish_parallel_job(target, job, result, blocking_reap);
        });
    }

    fn finish_parallel_job(
        &mut self,
        target: Vec<u8>,
        mut job: ParallelJob,
        result: Option<io::Result<std::process::ExitStatus>>,
        blocking_reap: bool,
    ) {
        let output = job.continuation.output.clone();
        let debug_job = job.continuation.debug_job.clone();
        if let Some(result) = &result {
            crate::debug::Jobs::reaped(
                self.output.debug_mask(),
                debug_job.as_ref(),
                result.as_ref().is_ok_and(|status| status.success()),
            );
        }
        if result.as_ref().is_some_and(|status| status.is_ok()) {
            self.recipe_reaps = self.recipe_reaps.wrapping_add(1);
            self.load.reaped();
            if let Some(buffer) = &output {
                buffer.child_reaped(true);
            }
        }
        let report_error = result
            .as_ref()
            .is_some_and(|r| !r.as_ref().is_ok_and(|status| status.success()));
        if report_error {
            self.output
                .select(output.as_ref().filter(|buffer| buffer.enabled()));
        }
        let launched = result.as_ref().is_some_and(Result::is_ok);
        if result.is_some() {
            self.filesystem.borrow_mut().child_completed();
        }
        let signaled = result
            .as_ref()
            .and_then(|status| status.as_ref().ok())
            .is_some_and(|status| status.signal().is_some());
        let error_code = match result.as_ref() {
            Some(Ok(status)) => status.code().filter(|code| *code != 0),
            Some(Err(_)) => Some(127),
            None => None,
        };
        let command_question =
            self.options.question && job.recursive && !job.ignore && error_code == Some(1);
        let error = match result {
            None => None,
            Some(Ok(status)) if status.success() || command_question => None,
            Some(Ok(status)) => Some(
                status
                    .code()
                    .map_or_else(|| signal_status(status), |code| format!("Error {code}")),
            ),
            Some(Err(error)) => {
                self.stderr(&[job.program.as_slice(), b": ", os_error(&error).as_bytes()].concat());
                Some("Error 127".to_string())
            }
        };
        if !job.waiting_load {
            job.continuation.flow.attempted(launched);
        }
        let command_failed = error.is_some() && !job.ignore;
        if command_failed {
            self.note_group_recipe_failure(&target, &job.location);
        }
        let mut failed = command_failed;
        let remake = job.continuation.remake.as_ref();
        if let Some(error) = error
            && (job.ignore || !remake.is_some_and(|remake| remake.optional))
            && (!job.ignore || !self.globally_silent())
        {
            self.show_recipe_makefile_error(remake);
            let source = location_prefix(&job.location);
            let ignored = if job.ignore {
                crate::messages::system(rmake_core::messages::Message::Ignored)
            } else {
                Vec::new()
            };
            let message = if let Some((code, source)) = error_code.zip(source.strip_suffix(b": ")) {
                crate::messages::system(rmake_core::messages::Message::RecipeError {
                    prefix: if job.ignore { b"" } else { b"*** " },
                    source,
                    target: &target,
                    code,
                    ignored: &ignored,
                    suffix: self.shuffle.suffix(),
                })
            } else {
                [
                    if job.ignore { b"".as_slice() } else { b"*** " },
                    b"[",
                    &location_prefix(&job.location),
                    &target,
                    format!("] {error}").as_bytes(),
                    &ignored,
                    self.shuffle.suffix(),
                ]
                .concat()
            };
            self.stderr(&message);
        }
        if report_error {
            self.output.select(None);
        }
        let mut cleanup_fatal = false;
        if (failed || command_question) && launched && !job.signal_cleaned {
            cleanup_fatal = self.cleanup_target(
                &target,
                job.before,
                job.phony,
                if signaled {
                    Cleanup::ChangedOnSignal
                } else {
                    Cleanup::ChangedOnError
                },
            );
        }
        if self.interrupt.is_some() {
            if !job.signal_cleaned
                && !failed
                && job
                    .continuation
                    .expanded
                    .iter()
                    .any(|(_, bytes, _)| !bytes.is_empty())
            {
                cleanup_fatal |=
                    self.cleanup_target(&target, job.before, job.phony, Cleanup::ChangedOnSignal);
            }
            failed = true;
        }
        if (failed || self.output.mode() == crate::output::Mode::Line)
            && let Some(buffer) = &output
        {
            buffer.dump();
        }
        if failed && self.parallel_jobs.contains_key(&target) {
            self.notice_waiting_for_jobs();
        }
        let mut outcome = self.done.get(&target).copied().unwrap_or_default();
        outcome.fatal |= cleanup_fatal || self.interrupt.is_some();
        let mut all_recursive = !job.continuation.flow.has_ordinary();
        let (completed_permit, continued) = if command_question {
            outcome.question_outdated = true;
            failed |= cleanup_fatal;
            (job.continuation.permit.take(), false)
        } else if !failed {
            let (execution, permit) =
                self.continue_recipe(&target, job.continuation, job.phony, job.before, true);
            failed |= matches!(execution.status, Status::Failure | Status::Fatal);
            outcome.fatal |= execution.status == Status::Fatal;
            outcome.question_outdated |= execution.status == Status::Question;
            all_recursive = execution.all_recursive;
            if self.cleanup_target(&target, job.before, job.phony, execution.cleanup) {
                failed = true;
                outcome.fatal = true;
            }
            (permit, execution.pending)
        } else {
            (job.continuation.permit.take(), false)
        };
        if let Some(buffer) = output {
            if outcome.fatal && self.interrupt.is_none() {
                self.fatal_outputs.push(buffer);
            } else if !self.parallel_jobs.values().any(|job| {
                job.continuation
                    .output
                    .as_ref()
                    .is_some_and(|active| std::rc::Rc::ptr_eq(active, &buffer))
            }) {
                self.output.clear_buffer(&buffer);
                buffer.dump();
            }
        }
        let pending = self.parallel_jobs.contains_key(&target);
        let mode = self.options.dry_run || self.options.question || self.options.touch;
        if mode && !pending {
            outcome.completion = if all_recursive {
                CompletionTime::Refresh
            } else if job.phony {
                CompletionTime::AssumeNewPhony
            } else {
                CompletionTime::AssumeNew
            };
            if self.options.dry_run
                && !self.options.question
                && !self.options.touch
                && !failed
                && outcome.completion == CompletionTime::AssumeNew
                && self
                    .running_groups
                    .get(&target)
                    .is_none_or(|group| group.implicit)
            {
                outcome.database_result = DatabaseResult::Simulated;
            }
        }
        // Inspect archive metadata only after the last command exits. A child
        // may temporarily leave a partial header while updating its archive.
        if !failed
            && (!mode || outcome.completion == CompletionTime::Refresh)
            && !pending
            && self.archive_metadata_error(&target, false)
        {
            failed = true;
            outcome.fatal = true;
        }
        if self
            .archive_completion_times
            .get(&target)
            .is_some_and(|stamp| !stamp.known)
        {
            outcome.completion = CompletionTime::ArchiveUnknown;
        }
        let optional_failure = self.optional_remake() && !outcome.fatal;
        if !optional_failure {
            self.parallel_failed |= failed;
        }
        outcome.ran = true;
        outcome.failed |= failed;
        if optional_failure && failed {
            if !self.options.keep_going
                && self.parallel_graph()
                && let Some(stopped) = self.remake_stop()
            {
                // A reaped job can stop its optional owner's traversal early.
                stopped.set(true);
            }
            self.optional_remake_failures
                .entry(target.clone())
                .or_default();
        }
        self.record_graph_failure(outcome);
        if !pending && !outcome.completion.preserves() {
            self.release_assumed_new(&target);
        }
        outcome.time = if job.phony {
            None
        } else if pending || outcome.completion.preserves() {
            outcome.time
        } else {
            self.modified(&target)
        };
        outcome.search_after_update = self.update_search_pending(&target, job.phony, all_recursive);
        if !job.phony && !pending {
            self.database_unchecked_times
                .borrow_mut()
                .insert(target.clone());
        }
        let cached = self.cached_recipe_outcome(outcome);
        self.done.insert(target.clone(), cached);
        if !pending {
            self.finish_group(&target, outcome);
        }
        if !outcome.fatal {
            self.release_completed_permit(completed_permit);
        }
        self.finish_debug_job(debug_job.as_ref());
        self.reuse_implicit_job_slot(!outcome.fatal);
        // A blocking reap scans a continued command at its next wait. A
        // completed recipe or a token or poll wake still reports its scan here.
        if !blocking_reap || !continued {
            self.job_debug.live(self.output.debug_mask());
        }
        if command_question {
            self.forced_query_completed();
        }
    }

    fn gpath_match(&self, original: &[u8], resolved: &[u8]) -> bool {
        resolved
            .len()
            .checked_sub(original.len() + 1)
            .is_some_and(|length| self.gpath_dirs.contains(&resolved[..length]))
    }

    /// Observe a searched file before rule or scope lookup. GPATH keeps a new
    /// name permanently; any search can merge into an existing file record.
    /// The observation is consumed by build_inner, so alias preparation never
    /// repeats the search after expansion effects have run.
    fn prepare_search_name(&mut self, target: &[u8]) -> Result<Option<Vec<u8>>, ()> {
        if self.aliases.contains(target) {
            return Ok(Some(self.aliases.resolve(target).to_vec()));
        }
        if (!self.database.has_vpaths() && self.vpath_dirs.is_empty() && !target.starts_with(b"-l"))
            || self.makefile_names.contains(target)
            || self.path_observations.contains_key(target)
            || self.assumed_time(target).is_some()
            || self.done.contains_key(target)
            || self.pending(target)
            || self.ordinary_queries.deferred_tails.contains_key(target)
            || self.visiting.contains(target)
            || self.special(b".PHONY", target, false)
        {
            return Ok(None);
        }
        if let Some((archive, member)) = rmake_core::filenames::archive_parts(target) {
            let location = self
                .archive_location_for(target, false)
                .map_err(|error| self.expansion_error(None, &error))?;
            if let Some(path) = &location.path
                && self.has_file_record(archive)
                && self.database.file_records.logical_name(path) == Some(archive)
            {
                // enter_file recreated the original container key. Merging
                // it back into its own tentative record keeps its local name.
                self.database.file_records.rename(archive, path);
            }
            if let Some(path) = &location.path
                && (self.gpath_match(archive, path) || self.other_file_record(path, archive))
            {
                self.rename_search_record(archive, path)?;
                self.path_observations
                    .entry(path.clone())
                    .or_insert(Some((None, location.container_time)));
                let name = [path.as_slice(), b"(", member, b")"].concat();
                self.rename_search_record(target, &name)?;
                let observation = self
                    .archive_observation_at(target, &location, Some(&name))
                    .map_err(|error| self.expansion_error(None, &error))?;
                if location.container_time.is_some() {
                    self.path_observations
                        .insert(name.clone(), observation.map(|(_, time)| (None, time)));
                }
                return Ok(Some(name));
            }
            if let Some(path) = &location.path {
                if !self.has_file_record(path) {
                    self.database.file_records.rehash(archive, path);
                }
                let name = [path.as_slice(), b"(", member, b")"].concat();
                if self.other_file_record(&name, target) {
                    self.rename_search_record(target, &name)?;
                    let observation = self
                        .archive_observation_at(target, &location, Some(&name))
                        .map_err(|error| self.expansion_error(None, &error))?;
                    if location.container_time.is_some() {
                        self.path_observations
                            .insert(name.clone(), observation.map(|(_, time)| (None, time)));
                    }
                    return Ok(Some(name));
                }
                if !self.has_file_record(&name) {
                    self.database.file_records.rehash(target, &name);
                }
            }
            let observation = self
                .archive_observation_at(target, &location, Some(target))
                .map_err(|error| self.expansion_error(None, &error))?;
            if location.container_time.is_some() {
                self.path_observations.insert(target.to_vec(), observation);
            }
            return Ok(None);
        }
        let observation = match self
            .find_vpath(target, true)
            .map_err(|error| self.expansion_error(None, &error))?
        {
            Some((Some(name), time)) => {
                if self.gpath_match(target, &name) || self.other_file_record(&name, target) {
                    self.rename_search_record(target, &name)?;
                    self.path_observations
                        .insert(name.clone(), Some((None, time)));
                    return Ok(Some(name));
                }
                self.database.file_records.rehash(target, &name);
                Some((Some(name), time))
            }
            observation => observation,
        };
        self.path_observations.insert(target.to_vec(), observation);
        Ok(None)
    }

    fn rename_search_record(&mut self, from: &[u8], to: &[u8]) -> Result<(), ()> {
        if from == to {
            return Ok(());
        }
        if self.database.file_records.contains(from)
            && self.database.file_records.logical_name(to) == Some(from)
        {
            // A fresh lookup of the original archive spelling can meet its
            // older rehashed record. They share the logical runtime data;
            // remove the new hash entry without renaming that runtime name.
            self.database.file_records.rename(from, to);
            return Ok(());
        }
        let completion = self.archive_completion_times.remove(from);
        self.archive_completion_times.remove(to);
        if let Some(completion) = completion {
            self.archive_completion_times
                .insert(to.to_vec(), completion);
        }
        self.database.file_records.rename(from, to);
        let merged = self.database.explicit_names.contains(to)
            || self.database.target_scopes.contains_key(to)
            || self.options.goals.iter().any(|goal| goal == to);
        self.invalidate_phony_names(from);
        self.invalidate_phony_names(to);
        let source = self.database.rules.remove(from);
        if let Some(mut source) = source {
            if let Some(destination) = self.database.rules.get_mut(to) {
                let source_commands = source
                    .double_colon
                    .as_ref()
                    .and_then(|branches| branches.first())
                    .unwrap_or(&source);
                let destination_commands = destination
                    .double_colon
                    .as_ref()
                    .and_then(|branches| branches.first())
                    .unwrap_or(destination);
                if source_commands.recipes.is_some()
                    && destination_commands.recipes.is_some()
                    && source_commands.recipe_location != destination_commands.recipe_location
                {
                    let location = source_commands.recipe_location.as_ref();
                    let first = if let Some(location) = location {
                        [
                            b"Recipe was specified for file '".as_slice(),
                            from,
                            b"' at ",
                            location.file.as_ref(),
                            b":",
                            location.line.to_string().as_bytes(),
                            b",",
                        ]
                        .concat()
                    } else {
                        [
                            b"Recipe for file '".as_slice(),
                            from,
                            b"' was found by implicit rule search,",
                        ]
                        .concat()
                    };
                    for message in [
                        first,
                        [
                            b"but '".as_slice(),
                            from,
                            b"' is now considered the same file as '",
                            to,
                            b"'.",
                        ]
                        .concat(),
                        [
                            b"Recipe for '".as_slice(),
                            from,
                            b"' will be ignored in favor of the one for '",
                            to,
                            b"'.",
                        ]
                        .concat(),
                    ] {
                        emit_output(
                            &self.output,
                            &self.program,
                            Output::Warning {
                                location,
                                message: &message,
                            },
                        );
                    }
                }
                if destination.record_only
                    && destination.double_colon.is_none()
                    && source.double_colon.is_some()
                {
                    // The destination adopts double-colon identity, but the
                    // merge joins only the heads. Source later branches are
                    // not transferred through GNU's renamed head record.
                    destination.double_colon = Some(vec![std::mem::take(destination)]);
                }
                if destination.double_colon.is_some() != source.double_colon.is_some()
                    && !source.record_only
                {
                    let message = [
                        b"can't rename ".as_slice(),
                        if source.double_colon.is_some() {
                            b"double-colon '"
                        } else {
                            b"single-colon '"
                        },
                        from,
                        b"' to ",
                        if destination.double_colon.is_some() {
                            b"double-colon '"
                        } else {
                            b"single-colon '"
                        },
                        to,
                        b"'",
                    ]
                    .concat();
                    self.fatal(None, &message);
                    return Err(());
                }
                // File-record merging joins the head records, not the two
                // double-colon chains. The destination retains its later
                // branches; the source's later branches are not transferred.
                let destination = match &mut destination.double_colon {
                    Some(branches) => branches.first_mut().expect("double-colon head"),
                    None => destination,
                };
                let source = match &mut source.double_colon {
                    Some(branches) => branches.first_mut().expect("double-colon head"),
                    None => &mut source,
                };
                if destination.recipes.is_none() {
                    destination.recipes = source.recipes.take();
                    destination.recipe_location = source.recipe_location.take();
                    destination.default_recipe = source.default_recipe;
                }
                destination.record_only &= source.record_only;
                destination.append_prerequisites(source);
            } else {
                // An existing filename record with no rule does not carry the
                // source file's recipe stem through GNU's record merge.
                if merged {
                    source.stem = None;
                }
                self.database.rules.insert(to.to_vec(), source);
            }
        }
        self.database.merge_target_scope(from, to);
        if let Some(parent) = self.scope_parents.remove(from) {
            self.scope_parents.entry(to.to_vec()).or_insert(parent);
        }
        if let Some(parent) = self.retained_scope_parents.remove(from) {
            self.retained_scope_parents
                .entry(to.to_vec())
                .or_insert(parent);
        }
        if let Some(context) = self.target_contexts.remove(from) {
            self.target_contexts.entry(to.to_vec()).or_insert(context);
        }
        if let Some(context) = self.database_contexts.remove(from) {
            self.database_contexts.entry(to.to_vec()).or_insert(context);
        }
        let branch_keys: Vec<_> = self
            .database_branch_contexts
            .keys()
            .filter(|(name, _)| name.as_slice() == from)
            .cloned()
            .collect();
        for key in branch_keys {
            if let Some(context) = self.database_branch_contexts.remove(&key) {
                self.database_branch_contexts
                    .entry((to.to_vec(), key.1))
                    .or_insert(context);
            }
        }
        let outcome_keys: Vec<_> = self
            .database_branch_outcomes
            .keys()
            .filter(|(name, _)| name.as_slice() == from)
            .cloned()
            .collect();
        for key in outcome_keys {
            if let Some(outcome) = self.database_branch_outcomes.remove(&key) {
                self.database_branch_outcomes
                    .entry((to.to_vec(), key.1))
                    .or_insert(outcome);
            }
        }
        let secondary_keys: Vec<_> = self
            .database_branch_secondary
            .iter()
            .filter(|(name, _)| name.as_slice() == from)
            .cloned()
            .collect();
        for key in secondary_keys {
            self.database_branch_secondary.remove(&key);
            self.database_branch_secondary.insert((to.to_vec(), key.1));
        }
        if let Some(branches) = self.database_group_peer_times.remove(from) {
            self.database_group_peer_times
                .entry(to.to_vec())
                .or_insert(branches);
        }
        let mut unchecked_times = self.database_unchecked_times.borrow_mut();
        if unchecked_times.remove(from) {
            unchecked_times.insert(to.to_vec());
        }
        drop(unchecked_times);
        {
            let times = self.database_times.get_mut();
            if let Some(time) = times.remove(from) {
                times.entry(to.to_vec()).or_insert(time);
            }
        }
        for flags in [
            &mut self.database_implicit,
            &mut self.terminal_prerequisites,
            &mut self.database_secondary,
            &mut self.database_remake_started,
            &mut self.ignored_vpath,
        ] {
            if flags.remove(from) {
                flags.insert(to.to_vec());
            }
        }
        if let Some(scope) = self.database.target_scopes.get(to)
            && let Some(context) = self.target_contexts.get_mut(to)
        {
            context.refresh_target_scope(scope);
        }
        if let Some(scope) = self.implicit_scopes.remove(from) {
            self.implicit_scopes.entry(to.to_vec()).or_insert(scope);
        }
        if self.database.explicit_names.contains(from) {
            self.database.explicit_names.insert(to.to_vec());
        }
        if !merged && self.intermediates.remove(from) {
            self.intermediates.insert(to.to_vec());
        }
        self.intermediate_policy.renamed(from, to, merged);
        self.aliases.insert(from, to, merged);
        Ok(())
    }

    // Makefile records remain local even when optional reading failed. Their
    // prerequisites can still use the ordinary VPATH/GPATH search policy.
    fn vpath_searchable(&self, target: &[u8]) -> bool {
        (self.database.has_vpaths() || !self.vpath_dirs.is_empty())
            && !self.makefile_names.contains(target)
    }

    fn has_file_record(&self, name: &[u8]) -> bool {
        if let Some(history) = self.database.database_history() {
            history.record_file_lookup(name);
        }
        self.database.file_records.contains(name)
    }

    fn other_file_record(&self, key: &[u8], target: &[u8]) -> bool {
        // Search already looked up the destination. These checks inspect its
        // retained ownership; moving the key records actual remove/enter events.
        key != target
            && self.database.file_records.contains(key)
            && (self.database.file_records.contains(target)
                || self.database.file_records.logical_name(key).unwrap_or(key) != target)
    }

    fn record_is_target(&self, name: &[u8]) -> bool {
        let name = self
            .database
            .file_records
            .logical_name(name)
            .unwrap_or(name);
        self.database
            .rules
            .get(name)
            .is_some_and(|rule| !rule.record_only)
            || self.special(b".PHONY", name, false)
    }

    /// Record lookup strips leading ./ but the searched hash spelling is
    /// retained. An explicitly declared source requires a declared destination;
    /// a non-target source can use any entered Makefile filename.
    fn search_record_matches(&self, target: &[u8], name: &[u8]) -> bool {
        let record = rmake_core::filenames::without_dot_prefix(name);
        self.has_file_record(record)
            && (!self.record_is_target(target) || self.record_is_target(record))
    }

    fn name_time(&self, name: &[u8]) -> Option<SystemTime> {
        let observed = rmake_linux::timestamp::observe(name, self.options.check_symlink_times);
        for issue in observed.issues {
            self.stderr(
                &[
                    issue.operation,
                    issue.path.as_slice(),
                    b": ",
                    os_error(&issue.error).as_bytes(),
                ]
                .concat(),
            );
        }
        self.retain_database_time(name, observed.time);
        observed.time
    }

    fn archive_time(
        &self,
        archive: &[u8],
        member: &[u8],
        member_time: Option<SystemTime>,
    ) -> Option<SystemTime> {
        self.name_time(archive)?;
        let date = rmake_linux::archive::date(archive, member).ok().flatten()?;
        if member_time
            .and_then(|time| time.duration_since(SystemTime::UNIX_EPOCH).ok())
            .is_some_and(|duration| duration.as_secs() > date)
        {
            return None;
        }
        SystemTime::UNIX_EPOCH.checked_add(Duration::from_secs(date))
    }

    fn retain_database_time(&self, name: &[u8], time: Option<SystemTime>) {
        let mut times = self.database_times.borrow_mut();
        if let Some(previous) = times.get_mut(name) {
            *previous = time;
        } else {
            times.insert(name.to_vec(), time);
        }
    }

    fn modified(&self, name: &[u8]) -> Option<SystemTime> {
        self.observed_modified(name, true)
    }

    fn observed_modified(&self, name: &[u8], retain_result: bool) -> Option<SystemTime> {
        let time = if let Some(time) = self.assumed_time(name) {
            Some(time)
        } else if let Some(cached) = self.archive_completion_times.get(name) {
            cached.time
        } else if let Some((archive, member)) = rmake_core::filenames::archive_parts(name) {
            if self.options.check_symlink_times {
                self.archive_time(archive, member, self.name_time(member))
            } else {
                modified(name)
            }
        } else {
            // A physical file observation already records its database time.
            return self.name_time(name);
        };
        if retain_result {
            self.retain_database_time(name, time);
        }
        time
    }

    fn general_search_directories(
        &self,
        value: &[u8],
        retain_search_spelling: bool,
    ) -> Result<Vec<Vec<u8>>, ExpandError> {
        let mut directories = Vec::new();
        for path in value
            .split(|byte| *byte == b':' || byte.is_ascii_whitespace() || *byte == 0x0b)
            .filter(|path| !path.is_empty())
        {
            let original = path;
            let path = if path.len() > 1 {
                path.strip_suffix(b"/").unwrap_or(path)
            } else {
                path
            };
            if path == b"." {
                continue;
            }
            if let Some(history) = self.database.database_history() {
                history.intern(path);
            }
            let mut name = self
                .filesystem
                .borrow_mut()
                .execute(rmake_core::diagnostic::FilesystemEffect::DirectoryName(
                    path,
                ))
                .map_err(|error| match error {
                    rmake_core::diagnostic::FilesystemError::Caller(message) => {
                        ExpandError::FileError(message)
                    }
                    rmake_core::diagnostic::FilesystemError::Unlocated(message) => {
                        ExpandError::UnlocatedError(message)
                    }
                })?;
            // Search retains the original byte length/spelling. The directory
            // cache and database presentation use the once-trimmed name.
            if retain_search_spelling && original.len() != path.len() {
                name.push(b'/');
            }
            directories.push(name);
        }
        if !directories.is_empty()
            && let Some(history) = self.database.database_history()
        {
            history.intern(b"%");
        }
        Ok(directories)
    }

    fn directory_member(&self, name: &[u8]) -> Result<bool, ExpandError> {
        self.filesystem
            .borrow_mut()
            .file_exists(name)
            .map_err(|error| match error {
                rmake_core::diagnostic::FilesystemError::Caller(message) => {
                    ExpandError::FileError(message)
                }
                rmake_core::diagnostic::FilesystemError::Unlocated(message) => {
                    ExpandError::UnlocatedError(message)
                }
            })
    }

    fn searched_file(&self, target: &[u8]) -> Result<PathObservation, ExpandError> {
        if !self.vpath_searchable(target) {
            return Ok(None);
        }
        if let Some(history) = self.database.database_history() {
            history.record_file_lookup(target);
        }
        for directory in self
            .database
            .vpath_directories(target)
            .chain(self.vpath_dirs.iter().map(Vec::as_slice))
            .filter(|directory| *directory != b".")
        {
            let mut name = searched_name(directory, target);
            let (lookup, membership) = search_probe_names(directory, target, &name);
            let declared = self.search_record_matches(target, &lookup);
            if declared || self.directory_member(&membership)? {
                // A cached name gates the probe, but VPATH must still reject
                // a stale positive when a fresh stat no longer finds it.
                let physical = modified(&name);
                if declared || physical.is_some() {
                    // Only an eligible search record contributes its logical age.
                    // GPATH retains a physical search result and merges it with
                    // the age already on the resulting name.
                    let assumed_name: &[u8] = if declared { &lookup } else { &name };
                    let assumed =
                        self.assumed_time(rmake_core::filenames::without_dot_prefix(assumed_name));
                    let time = if declared && assumed.is_some() {
                        assumed
                    } else if !declared
                        && (self.options.check_symlink_times || assumed.is_some())
                        && self.gpath_match(target, &name)
                    {
                        physical.max(assumed)
                    } else if self.options.check_symlink_times {
                        self.name_time(&name)
                    } else {
                        physical
                    };
                    self.record_vpath_result(directory, target, &mut name)?;
                    return Ok(Some((Some(name), time)));
                }
            }
        }
        Ok(None)
    }

    fn record_vpath_result(
        &self,
        directory: &[u8],
        target: &[u8],
        name: &mut Vec<u8>,
    ) -> Result<(), ExpandError> {
        let Some(history) = self.database.database_history() else {
            return Ok(());
        };
        // The retained span includes the separator even when the historical
        // trailing-slash spelling ends earlier.
        let visible = name.len();
        let directory = if directory.len() > 1 {
            directory.strip_suffix(b"/").unwrap_or(directory)
        } else {
            directory
        };
        let extent = if target.starts_with(b"/") {
            visible
        } else {
            directory
                .len()
                .checked_add(1)
                .and_then(|length| length.checked_add(target.len()))
                .ok_or(ExpandError::ExpansionLimit)?
        };
        if extent < visible {
            return Err(ExpandError::ExpansionLimit);
        }
        name.resize(extent, 0);
        history.intern_buffer(name);
        name.truncate(visible);
        Ok(())
    }

    fn archive_location_for(
        &mut self,
        target: &[u8],
        refresh: bool,
    ) -> Result<ArchiveLocation, ExpandError> {
        let (archive, member) =
            rmake_core::filenames::archive_parts(target).expect("archive reference");
        if member.starts_with(b"(") && member.ends_with(b")") {
            return Err(ExpandError::UnlocatedError(
                [
                    b"attempt to use unsupported feature: '".as_slice(),
                    target,
                    b"'",
                ]
                .concat(),
            ));
        }
        let member_time = self.name_time(member);
        self.database.file_records.insert(archive);
        let container = self
            .database
            .file_records
            .logical_name(archive)
            .unwrap_or(archive)
            .to_vec();
        let local_time = self.name_time(&container);
        let path = if local_time.is_none() {
            self.archive_remote_vpath(&container)?
        } else {
            None
        };
        let container_time = if let Some(path) = &path {
            self.name_time(path)
        } else {
            local_time
        };
        // The archive is a separate file observation even when the member
        // has already finished. Its first physical timestamp may be here.
        if self.clock_candidate(container_time).is_some()
            && !self.done.contains_key(self.aliases.resolve(&container))
        {
            let name = path
                .as_deref()
                .filter(|path| {
                    self.gpath_match(&container, path) || self.other_file_record(path, &container)
                })
                .unwrap_or(&container);
            self.check_clock_skew(name, container_time);
        }
        if let Some(path) = &path {
            let mut reads = self.archive_vpath_reads.borrow_mut();
            let reads = reads.get_or_insert_with(Default::default);
            reads.members.insert(target.to_vec());
            reads.archives.insert(archive.to_vec());
            reads
                .paths
                .entry(target.to_vec())
                .or_insert_with(|| path.clone());
        }
        Ok(ArchiveLocation {
            refresh,
            member_time,
            container_time,
            path,
        })
    }

    fn archive_observation_at(
        &self,
        target: &[u8],
        location: &ArchiveLocation,
        clock_name: Option<&[u8]>,
    ) -> Result<PathObservation, ExpandError> {
        let (archive, member) =
            rmake_core::filenames::archive_parts(target).expect("archive reference");
        let resolved = location
            .path
            .as_ref()
            .map(|path| [path.as_slice(), b"(", member, b")"].concat());
        if location.container_time.is_none() {
            return Ok(resolved.map(|name| (Some(name), None)));
        }
        // A rebuilt archive member retains its prior hash filename. The local
        // container timestamp is refreshed, but its member date can still be
        // read from the archive chosen before the commands ran.
        let previous = location
            .refresh
            .then(|| {
                self.archive_vpath_reads
                    .borrow()
                    .as_ref()
                    .and_then(|reads| reads.paths.get(target).cloned())
            })
            .flatten();
        let path = previous
            .as_deref()
            .or(location.path.as_deref())
            .unwrap_or(archive);
        let date = rmake_linux::archive::date(path, member)
            .map_err(|error| ExpandError::UnlocatedError(error.into_message()))?;
        let time = date
            .filter(|date| {
                location
                    .member_time
                    .and_then(|time| time.duration_since(SystemTime::UNIX_EPOCH).ok())
                    .is_none_or(|duration| duration.as_secs() <= *date)
            })
            .and_then(|date| {
                SystemTime::UNIX_EPOCH.checked_add(std::time::Duration::from_secs(date))
            });
        if let Some(name) = clock_name {
            self.check_clock_skew(name, time);
        }
        Ok(Some((resolved, time)))
    }

    fn archive_observation(
        &mut self,
        target: &[u8],
        check_clock: bool,
    ) -> Result<(PathObservation, bool), ExpandError> {
        let location = self.archive_location_for(target, false)?;
        self.archive_observation_at(target, &location, check_clock.then_some(target))
            .map(|observation| (observation, location.container_time.is_some()))
    }

    fn find_vpath(
        &mut self,
        target: &[u8],
        check_clock: bool,
    ) -> Result<PathObservation, ExpandError> {
        self.database_unchecked_times.borrow_mut().remove(target);
        if let Some(time) = self.assumed_time(target) {
            return Ok(Some((None, Some(time))));
        }
        if rmake_core::filenames::archive_parts(target).is_some() {
            return self
                .archive_observation(target, check_clock)
                .map(|(observation, _)| observation);
        }
        if self
            .archive_vpath_reads
            .borrow()
            .as_ref()
            .is_some_and(|reads| reads.archives.contains(target))
        {
            return Ok(None);
        }
        // Build timestamps use fresh metadata. Only implicit viability's
        // local-name test uses cached directory membership instead.
        if let Some(time) = self.modified(target) {
            if check_clock {
                self.check_clock_skew(target, Some(time));
            }
            return Ok(Some((None, Some(time))));
        }
        if self.ignored_vpath.contains(target) {
            return Ok(None);
        }
        let mut searched = self.searched_file(target)?;
        if searched.is_none() && target.starts_with(b"-l") && !self.makefile_names.contains(target)
        {
            searched = self.library_search(&target[2..])?;
        }
        if check_clock && let Some((Some(name), time)) = &searched {
            self.check_search_clock(target, name, *time);
        }
        Ok(searched)
    }

    fn check_search_clock(&self, target: &[u8], name: &[u8], time: Option<SystemTime>) {
        if self.clock_candidate(time).is_none() {
            return;
        }
        let permanent = self.gpath_match(target, name);
        if permanent {
            // Ordinary GPATH matches can reuse a physical timestamp directly.
            // A declared search record instead goes through the file-time lookup.
            let record = rmake_core::filenames::without_dot_prefix(name);
            if !self.database.file_records.contains(record)
                || (self.record_is_target(target) && !self.record_is_target(record))
            {
                return;
            }
        }
        let spelling = if permanent || self.other_file_record(name, target) {
            name
        } else {
            target
        };
        if !self.done.contains_key(self.aliases.resolve(spelling)) {
            self.check_clock_skew(spelling, time);
        }
    }

    fn library_search(&mut self, library: &[u8]) -> Result<PathObservation, ExpandError> {
        let patterns = self.expand_global(b"$(.LIBPATTERNS)", None)?;
        let history = self.database.database_history();
        let mut selected: Option<((usize, usize), Vec<u8>)> = None;
        for text in words(&patterns) {
            let pattern = Pattern::new(text);
            if !pattern.has_wildcard() {
                self.stderr(
                    &[
                        b".LIBPATTERNS element '".as_slice(),
                        pattern.literal_bytes(),
                        b"' is not a pattern",
                    ]
                    .concat(),
                );
                continue;
            }
            let name = pattern.replace(library)?;
            if self.name_time(&name).is_some() {
                if let Some(history) = history {
                    history.intern(&name);
                }
                selected = Some(((0, 0), name));
                break;
            }
            let paths = self.database.ranked_vpath_directories(&name).chain(
                self.vpath_dirs
                    .iter()
                    .filter(|path| path.as_slice() != b".")
                    .enumerate()
                    .map(|(index, path)| {
                        ((self.database.vpath_group_count(), index), path.as_slice())
                    }),
            );
            let mut observed_group = None;
            for (rank, directory) in paths {
                if let Some(history) = history
                    && observed_group != Some(rank.0)
                {
                    history.record_file_lookup(&name);
                    observed_group = Some(rank.0);
                }
                let mut path = searched_name(directory, &name);
                let (lookup, membership) = search_probe_names(directory, &name, &path);
                let declared = self.search_record_matches(&name, &lookup);
                if declared || (self.directory_member(&membership)? && modified(&path).is_some()) {
                    self.record_vpath_result(directory, &name, &mut path)?;
                    if selected
                        .as_ref()
                        .is_none_or(|(previous, _)| rank < *previous)
                    {
                        selected = Some((rank, path));
                    }
                    break;
                }
            }
            for (index, directory) in [
                b"/lib".as_slice(),
                b"/usr/lib",
                option_env!("RMAKE_LIBDIR")
                    .unwrap_or("/usr/local/lib")
                    .as_bytes(),
            ]
            .into_iter()
            .enumerate()
            {
                // Standard directories concatenate rather than interpreting
                // an absolute pattern as a replacement for the directory.
                let path = [directory, b"/", &name].concat();
                let rank = (usize::MAX - 3 + index, 0);
                if self.name_time(&path).is_some()
                    && selected
                        .as_ref()
                        .is_none_or(|(previous, _)| rank < *previous)
                {
                    if let Some(history) = history {
                        history.intern(&path);
                    }
                    selected = Some((rank, path));
                }
            }
        }
        Ok(selected.map(|(_, name)| {
            let time = self.name_time(&name);
            (Some(name), time)
        }))
    }

    fn archive_remote_vpath(&self, archive: &[u8]) -> Result<Option<Vec<u8>>, ExpandError> {
        if !self.vpath_searchable(archive) {
            return Ok(None);
        }
        for directory in self
            .database
            .vpath_directories(archive)
            .chain(self.vpath_dirs.iter().map(Vec::as_slice))
            .filter(|directory| *directory != b".")
        {
            let path = searched_name(directory, archive);
            let (lookup, membership) = search_probe_names(directory, archive, &path);
            if self.search_record_matches(archive, &lookup)
                || (self.directory_member(&membership)? && fs::metadata(os(&path)).is_ok())
            {
                return Ok(Some(path));
            }
        }
        Ok(None)
    }

    /// Local implicit prerequisites use directory membership without a fresh
    /// stat. VPATH candidates still require its positive-name stat check.
    fn implicit_presence(&mut self, target: &[u8]) -> Result<ImplicitPresence, ExpandError> {
        if rmake_core::filenames::archive_parts(target).is_none() {
            return if self.directory_member(target)? {
                Ok(ImplicitPresence::Local)
            } else {
                Ok(match self.searched_file(target)? {
                    Some((Some(path), _)) => ImplicitPresence::Search(path),
                    Some((None, _)) => ImplicitPresence::Local,
                    None => ImplicitPresence::Absent,
                })
            };
        }
        // Viability uses the archive date alone. Standalone-member freshness
        // belongs to the later build timestamp phase. Existing containers enter
        // the filename table even if the candidate is eventually rejected.
        let (archive, member) =
            rmake_core::filenames::archive_parts(target).expect("archive reference");
        if member.starts_with(b"(") && member.ends_with(b")") {
            return Err(ExpandError::UnlocatedError(
                [
                    b"attempt to use unsupported feature: '".as_slice(),
                    target,
                    b"'",
                ]
                .concat(),
            ));
        }
        if self.database.file_records.contains(archive) || self.directory_member(archive)? {
            self.database.file_records.insert(archive);
            self.name_time(archive);
        }
        if rmake_linux::archive::date(archive, member)
            .map_err(|error| ExpandError::UnlocatedError(error.into_message()))?
            .is_some()
        {
            return Ok(ImplicitPresence::Local);
        }
        if self.vpath_searchable(target) {
            for directory in self
                .database
                .vpath_directories(target)
                .chain(self.vpath_dirs.iter().map(Vec::as_slice))
            {
                let name = searched_name(directory, target);
                let (lookup, membership) = search_probe_names(directory, target, &name);
                if self.search_record_matches(target, &lookup)
                    || (self.directory_member(&membership)? && fs::metadata(os(&name)).is_ok())
                {
                    return Ok(ImplicitPresence::Search(name));
                }
            }
        }
        Ok(ImplicitPresence::Absent)
    }

    fn update_search_pending(&self, target: &[u8], phony: bool, all_recursive: bool) -> bool {
        !phony
            && !self.ignored_vpath.contains(target)
            && (self.vpath_searchable(target) || target.starts_with(b"-l"))
            && (!(self.options.dry_run || self.options.question || self.options.touch)
                || all_recursive)
    }

    fn observe_updated(&mut self, target: &[u8], mut outcome: Outcome) -> Outcome {
        // A completed GPATH rename has already selected the actual file.
        // Consume its timestamp without merging the original goal again.
        let alias = self
            .aliases
            .contains(target)
            .then(|| self.aliases.resolve(target).to_vec());
        let target = alias.as_deref().unwrap_or(target);
        if outcome.failed {
            return outcome;
        }
        if !outcome.search_after_update {
            // This is the successful consumer/goal timestamp phase. Without
            // search paths, use the already-retained local observation; the
            // bookkeeping stat need not be repeated just for diagnostics.
            self.database_unchecked_times.borrow_mut().remove(target);
            return outcome;
        }
        if let Some(stamp) = self.archive_completion_times.get(target) {
            outcome.time = stamp.time;
            if !stamp.known {
                outcome.completion = CompletionTime::ArchiveUnknown;
            }
            outcome.search_after_update = false;
            self.done.insert(target.to_vec(), outcome);
            return outcome;
        }
        outcome.search_after_update = false;
        match self.find_vpath(target, false) {
            Ok(observation) => {
                self.database_unchecked_times.borrow_mut().remove(target);
                outcome.time = observation.as_ref().and_then(|(_, time)| *time);
                if let Some((Some(name), _)) = observation
                    && (self.gpath_match(target, &name) || self.other_file_record(&name, target))
                {
                    if self.rename_search_record(target, &name).is_err() {
                        outcome.failed = true;
                        outcome.fatal = true;
                    }
                    self.done.insert(name, outcome);
                }
            }
            Err(error) => {
                self.expansion_error(None, &error);
                outcome.failed = true;
                outcome.fatal = true;
            }
        }
        self.done.insert(target.to_vec(), outcome);
        self.record_graph_failure(outcome);
        outcome
    }

    fn implicit_known_file(&self, target: &[u8]) -> bool {
        self.database.file_records.contains(target)
    }

    fn refresh_impossible_prerequisites(&mut self) {
        let generation = self.filesystem.borrow().generation();
        if self.impossible_generation != generation {
            self.impossible_prerequisites.clear();
            self.impossible_generation = generation;
        }
    }

    #[must_use = "search exhaustion must stop the build instead of selecting a fallback"]
    fn resolve_implicit_for_intermediate(&mut self, target: &[u8], depth: usize) -> bool {
        // Inspection already receives the edge depth used by search output.
        let debug_depth = depth - 1 + usize::from(self.remaking);
        self.resolve_implicit_at(target, debug_depth, false)
    }
    fn resolve_implicit_at(
        &mut self,
        target: &[u8],
        debug_depth: usize,
        graph_visit: bool,
    ) -> bool {
        let existing = match self.database.rules.get(target) {
            Some(rule) if rule.recipes.is_some() || rule.double_colon.is_some() => return true,
            rule => rule.cloned(),
        };
        let Ok(plan) = self.plan_implicit_update(target, debug_depth, true) else {
            return false;
        };
        if plan.is_none()
            && existing.is_none()
            && self
                .database_times
                .borrow()
                .get(target)
                .is_some_and(Option::is_some)
        {
            self.database_secondary.insert(target.to_vec());
        }
        if let Some(plan) = plan {
            let _ = self.install_implicit_plan(target, plan, false);
        } else if existing.as_ref().is_none_or(|rule| rule.record_only)
            && let Some(mut rule) = self.default_update_rule(target, debug_depth, graph_visit)
        {
            rule.dependencies = existing.map_or_else(Vec::new, |rule| rule.dependencies);
            self.invalidate_phony_names(target);
            self.database.rules.insert(target.to_vec(), rule);
        }
        true
    }

    fn plan_implicit_update(
        &mut self,
        target: &[u8],
        debug_depth: usize,
        record_attempt: bool,
    ) -> Result<Option<Vec<ImplicitSelection>>, ()> {
        if self.terminal_prerequisites.contains(target) {
            return Ok(None);
        }
        if record_attempt {
            self.database_implicit.insert(target.to_vec());
        }
        crate::debug::emit(
            &self.output,
            crate::debug::Event::Looking(target, debug_depth + 1),
        );
        let mut budget = 100_000;
        match self.implicit_plan(target, &mut Vec::new(), false, &mut budget, debug_depth) {
            Ok(None) if budget == 0 => {
                self.fatal(None, b"rmake: implicit rule search resource limit exceeded");
                Err(())
            }
            Ok(plan) => Ok(plan),
            Err(error) => {
                self.expansion_error(None, &error);
                Err(())
            }
        }
    }

    /// The synthetic double-colon input owns its selected root rule. Child
    /// selections still enter the normal graph with their prepared state.
    fn install_implicit_plan(
        &mut self,
        target: &[u8],
        plan: Vec<ImplicitSelection>,
        detach_root: bool,
    ) -> Option<Rule> {
        let mut detached_rule = None;
        // Recursive searches return unentered intermediate records. Commit
        // each accepted child's input list when its parent installs it;
        // the parent's candidate inputs are processed in reverse order.
        let inferred: HashMap<_, _> = plan
            .iter()
            .map(|selection| (selection.name.as_slice(), &selection.rule))
            .collect();
        for selection in &plan {
            let group = selection.rule.group.as_ref();
            for dependency in selection.rule.dependencies.iter().rev() {
                if let Some(child) = inferred.get(dependency.name.as_slice()) {
                    self.database.file_records.insert(&dependency.name);
                    for input in child.dependencies.iter().filter(|dep| dep.name != b".WAIT") {
                        self.database.file_records.insert(&input.name);
                    }
                } else if selection.name == target && dependency.name != b".WAIT" {
                    if group.is_some() {
                        if let Some(history) = self.database.database_history() {
                            history.intern(&dependency.name);
                        }
                        self.database
                            .file_records
                            .enter_prerequisite(&dependency.name);
                    } else {
                        self.database.file_records.insert(&dependency.name);
                    }
                }
            }
            if let Some(group) = group {
                if let Some(history) = self.database.database_history() {
                    if let Some(stem) = &selection.rule.stem {
                        history.intern(stem);
                    }
                    if let Some(pattern) = &selection.rule.implicit_pattern {
                        history.record_file_lookup(pattern);
                    }
                }
                for (peer, pattern) in &group.implicit_peer_patterns {
                    if peer != &selection.name {
                        if let Some(history) = self.database.database_history() {
                            history.intern(peer);
                        }
                        self.database.file_records.insert(peer);
                        if let Some(history) = self.database.database_history() {
                            history.record_file_lookup(pattern);
                        }
                    }
                }
            }
        }
        for selection in &plan {
            self.database
                .explicit_names
                .extend(selection.explicit_names.iter().cloned());
        }
        for selection in plan {
            let ImplicitSelection {
                name,
                mut rule,
                prepared_scope,
                explicit_names: _,
                intermediate_names,
            } = selection;
            if rule.terminal {
                // Accepting a terminal rule bars implicit search when
                // visiting its prerequisites. Explicit recipes and the
                // ordinary default-recipe check still apply.
                for dependency in &rule.dependencies {
                    if dependency.name != b".WAIT" {
                        self.terminal_prerequisites.insert(dependency.name.clone());
                        self.database_implicit.insert(dependency.name.clone());
                    }
                }
            }
            // A selected implicit rule promotes its file to a target,
            // including recipes converted from undeclared builtin records.
            rule.record_only = false;
            let implicit_prefix = rule.dependencies.len();
            let detached = detach_root && name == target;
            if !detached && let Some(scope) = prepared_scope {
                self.implicit_scopes.insert(name.clone(), *scope);
            }
            if let Some(group) = &rule.group {
                for (peer, pattern) in &group.implicit_peer_patterns {
                    let precious = self.special(b".PRECIOUS", pattern, false);
                    let notintermediate = self.special(b".NOTINTERMEDIATE", pattern, false);
                    let flags = self.inferred_peer_flags.entry(peer.clone()).or_default();
                    flags.0 |= precious;
                    flags.1 |= notintermediate;
                }
            }
            if !detached && let Some(explicit) = self.database.rules.get(&name) {
                rule.append_prerequisites(explicit);
            }
            if name != target
                && !self.database.explicit_names.contains(&name)
                && !self.options.goals.contains(&name)
            {
                self.intermediates.insert(name.clone());
            }
            self.intermediates.extend(intermediate_names);
            if self.shuffle.enabled() && (implicit_prefix != 0 || !self.shuffle.has_plan(&name)) {
                let disabled = self.database.globally_serial();
                self.shuffle
                    .regenerate(&name, &rule, &self.database, disabled, implicit_prefix);
            }
            if detached {
                detached_rule = Some(rule);
            } else {
                self.invalidate_phony_names(&name);
                self.database.rules.insert(name, rule);
            }
        }
        detached_rule
    }

    fn default_update_rule(
        &self,
        target: &[u8],
        debug_depth: usize,
        graph_visit: bool,
    ) -> Option<Rule> {
        let default = self.database.rules.get(b".DEFAULT".as_slice())?;
        let recipes = default.recipes.clone()?;
        let event = if graph_visit {
            crate::debug::Event::UsingDefaultRecipe(target, debug_depth + 1)
        } else {
            crate::debug::Event::UsingDefaultCommands(target, debug_depth + 1)
        };
        crate::debug::emit(&self.output, event);
        Some(Rule {
            recipes: Some(recipes),
            recipe_location: default.recipe_location.clone(),
            default_recipe: true,
            ..Rule::default()
        })
    }

    fn synthetic_double_colon_recipe(
        &mut self,
        target: &[u8],
        depth: usize,
    ) -> Result<Option<Rule>, ()> {
        let depth = depth + usize::from(self.remaking);
        Ok(match self.plan_implicit_update(target, depth, false)? {
            Some(plan) => self.install_implicit_plan(target, plan, true),
            None => self.default_update_rule(target, depth, true),
        })
    }

    /// Search is transactional: a rejected branch never installs rules or marks
    /// files intermediate. Rule identities, rather than names alone, prevent
    /// unbounded chains such as %.x: %.x.x.
    fn implicit_plan(
        &mut self,
        target: &[u8],
        active: &mut Vec<usize>,
        compatibility: bool,
        budget: &mut usize,
        debug_depth: usize,
    ) -> Result<Option<Vec<ImplicitSelection>>, ExpandError> {
        if active.len() >= 128 {
            *budget = 0;
        }
        if *budget == 0 {
            return Ok(None);
        }
        let depth = debug_depth + active.len() + 1;
        let archive_name = rmake_core::filenames::archive_parts(target)
            .map(|(archive, _)| &target[archive.len()..]);
        let root = active.is_empty();
        let mut search = ImplicitSearch {
            compatibility,
            archive: false,
            intermediate_scope: None,
        };
        for archive_search in [false, true] {
            if archive_search && archive_name.is_none() {
                break;
            }
            if archive_search && root {
                crate::debug::emit(
                    &self.output,
                    crate::debug::Event::LookingArchive(target, depth),
                );
            }
            search.compatibility = compatibility;
            search.archive = archive_search;
            if let Some(plan) =
                self.implicit_search(target, active, &mut search, budget, debug_depth)?
            {
                return Ok(Some(plan));
            }
            let printed = if archive_search {
                archive_name.unwrap_or(target)
            } else {
                target
            };
            if !archive_search || root {
                crate::debug::emit(&self.output, crate::debug::Event::NoRule(printed, depth));
            }
            if archive_search && root {
                crate::debug::emit(
                    &self.output,
                    crate::debug::Event::NoArchiveRule(target, depth),
                );
            }
        }
        Ok(None)
    }

    fn implicit_search(
        &mut self,
        target: &[u8],
        active: &mut Vec<usize>,
        search: &mut ImplicitSearch,
        budget: &mut usize,
        debug_depth: usize,
    ) -> Result<Option<Vec<ImplicitSelection>>, ExpandError> {
        let compatibility = search.compatibility;
        let archive_search = search.archive;
        let tracing = self.output.debug_mask().has(crate::debug::Mask::IMPLICIT);
        // Both archive search phases match without ignoring any directory.
        let trace_directory =
            tracing && !archive_search && rmake_core::filenames::archive_parts(target).is_none();
        if tracing && !active.is_empty() {
            let mut blocked = active.clone();
            blocked.sort_unstable();
            for identity in blocked {
                let label = crate::debug::rule_label(&self.database, identity);
                crate::debug::emit(
                    &self.output,
                    crate::debug::Event::AvoidingRecursion(&label, debug_depth + active.len() + 2),
                );
            }
        }
        let candidates = if archive_search {
            self.database
                .implicit_archive_candidates_excluding(target, active)
        } else {
            self.database.implicit_candidates_excluding(target, active)
        };
        let mut compatibility_needed = false;
        let mut cancelled = HashSet::new();
        let intermediate = !active.is_empty();
        let mut variables = None;
        for chain in [false, true] {
            if chain && !candidates.is_empty() {
                crate::debug::emit(
                    &self.output,
                    crate::debug::Event::TryingHarder(debug_depth + active.len() + 2),
                );
            }
            for (ordinal, candidate) in candidates.iter().enumerate() {
                if *budget == 0 {
                    return Ok(None);
                }
                *budget -= 1;
                if cancelled.contains(&ordinal)
                    || active.contains(&candidate.identity)
                    || (chain && candidate.rule.terminal)
                    || (intermediate && candidate.match_anything && !candidate.rule.terminal)
                {
                    continue;
                }
                let debug_label =
                    tracing.then(|| crate::debug::rule_label(&self.database, candidate.identity));
                let stem = candidate.rule.stem.as_deref().unwrap_or_default();
                // The candidate stem already includes any ignored directory.
                // GNU's stem buffer holds PATH_MAX bytes plus the terminator.
                let stem_too_long = stem.len() > rmake_linux::filesystem::PATH_MAX;
                if let Some(label) = &debug_label {
                    let directory = if trace_directory
                        && candidate
                            .rule
                            .implicit_pattern
                            .as_ref()
                            .is_some_and(|pattern| !pattern.contains(&b'/'))
                    {
                        rmake_core::implicit::directory_prefix(target)
                    } else {
                        b""
                    };
                    let (directory, printed_stem) = stem
                        .strip_prefix(directory)
                        .map_or((b"".as_slice(), stem), |printed| (directory, printed));
                    crate::debug::emit(
                        &self.output,
                        crate::debug::Event::TryingRule(
                            label,
                            printed_stem,
                            debug_depth + active.len() + 2,
                        ),
                    );
                    if stem_too_long {
                        crate::debug::emit(
                            &self.output,
                            crate::debug::Event::StemTooLong(
                                directory,
                                printed_stem,
                                debug_depth + active.len() + 2,
                            ),
                        );
                    }
                }
                if stem_too_long {
                    continue;
                }
                let mut plan = Vec::new();
                let mut explicit_names = Vec::new();
                let mut intermediate_names = Vec::new();
                let mut dependencies: Vec<Dependency> = Vec::new();
                let mut valid = true;
                let mut order_only = false;
                let mut stem_set = false;
                active.push(candidate.identity);
                'prerequisites: for (index, occurrence) in
                    candidate.rule.dependencies.iter().enumerate()
                {
                    let deferred =
                        candidate.rule.secondary.as_ref().is_some_and(|clauses| {
                            clauses.iter().any(|clause| clause.index == index)
                        });
                    if dependencies
                        .last()
                        .is_some_and(|dependency| dependency.order_only != occurrence.order_only)
                    {
                        Self::trim_implicit_waits(&mut dependencies);
                    }
                    let mut words = rmake_core::implicit::Words::new(&occurrence.name);
                    let mut immediate = Some(occurrence.name.as_slice());
                    while let Some(part) = if deferred {
                        words.next()
                    } else {
                        immediate.take()
                    } {
                        let mut explicit_word = !candidate.stem_dependencies.contains(&index);
                        let names = if deferred {
                            if part == b"|" && !order_only {
                                order_only = true;
                                continue;
                            }
                            if variables.is_none() {
                                variables = Some(self.implicit_variables(
                                    target,
                                    intermediate,
                                    candidate.rule.stem.clone().unwrap_or_default(),
                                    &mut search.intermediate_scope,
                                )?);
                            } else if !stem_set {
                                variables.as_mut().unwrap().define_literal(
                                    b"*",
                                    candidate.rule.stem.as_deref().unwrap_or_default(),
                                    Origin::Automatic,
                                )?;
                                stem_set = true;
                            }
                            let (rewritten, percent) = rmake_core::implicit::stem_references(
                                part,
                                candidate.secondary_directory.is_some(),
                            );
                            explicit_word = !percent;
                            let variables = variables.as_mut().expect("initialized file scope");
                            let value = self.expand(variables, &rewritten, None);
                            if let Some(status) = variables.local_shell_status() {
                                if intermediate {
                                    let scope = search
                                        .intermediate_scope
                                        .as_mut()
                                        .expect("initialized intermediate scope");
                                    if scope.shell_status.as_ref() != Some(status) {
                                        scope.shell_status = Some(status.clone());
                                    }
                                } else if let Some(context) =
                                    self.target_contexts.get_mut(self.aliases.resolve(target))
                                {
                                    // Later candidates and recursive searches can
                                    // read a real file even after a rejection.
                                    context.retain_local_shell_status(status);
                                }
                            }
                            let value = value?;
                            self.implicit_filenames(
                                &value,
                                &mut order_only,
                                if percent {
                                    candidate.secondary_directory.as_deref()
                                } else {
                                    None
                                },
                            )?
                        } else {
                            // ONEWORD parsing preserves blanks and pipes from a
                            // stem, but still expands newly introduced globs.
                            let start = part
                                .iter()
                                .position(|byte| !matches!(byte, b' ' | b'\t'..=b'\r'))
                                .unwrap_or(part.len());
                            if start == part.len() {
                                Vec::new()
                            } else {
                                self.implicit_filename(&part[start..], occurrence.order_only, None)?
                            }
                        };
                        for mut dependency in names {
                            dependency.hidden = occurrence.hidden;
                            let identity = self.aliases.resolve(&dependency.name);
                            if identity != dependency.name {
                                dependency.name = identity.to_vec();
                            }
                            let name = &dependency.name;
                            let event_depth = debug_depth + active.len() + 1;
                            let literal = tracing
                                && self
                                    .database
                                    .implicit_rule_source(candidate.identity)
                                    .and_then(|(_, source)| source.dependencies.get(index))
                                    .is_some_and(|declared| declared.name.as_slice() == name);
                            if candidate.rule.group.is_some() && name != b".WAIT" {
                                let directory = match name.iter().rposition(|&byte| byte == b'/') {
                                    None => b".".as_slice(),
                                    Some(0) => b"/",
                                    Some(index) => &name[..index],
                                };
                                // Checking the impossible-name cache first
                                // registers the directory without reading it.
                                self.filesystem
                                    .borrow_mut()
                                    .execute(
                                        rmake_core::diagnostic::FilesystemEffect::DirectoryName(
                                            directory,
                                        ),
                                    )
                                    .map_err(|error| match error {
                                        rmake_core::diagnostic::FilesystemError::Caller(
                                            message,
                                        ) => ExpandError::FileError(message),
                                        rmake_core::diagnostic::FilesystemError::Unlocated(
                                            message,
                                        ) => ExpandError::UnlocatedError(message),
                                    })?;
                            }
                            self.refresh_impossible_prerequisites();
                            if name != b".WAIT" && self.impossible_prerequisites.contains(name) {
                                if let Some(label) = &debug_label {
                                    crate::debug::emit(
                                        &self.output,
                                        crate::debug::Event::RejectedPrerequisite(
                                            label,
                                            name,
                                            Some(literal),
                                            event_depth,
                                        ),
                                    );
                                }
                                cancelled.insert(ordinal);
                                valid = false;
                                break 'prerequisites;
                            }
                            if explicit_word {
                                explicit_names.push(name.clone());
                            } else if self.database.file_records.contains(name)
                                && !self.database.explicit_names.contains(name)
                            {
                                intermediate_names.push(name.clone());
                            }
                            crate::debug::emit(
                                &self.output,
                                crate::debug::Event::TryingPrerequisite(name, literal, event_depth),
                            );
                            let explicit = (self.has_file_record(name)
                                && self.record_is_target(name))
                                || self.database.rules.get(target).is_some_and(|rule| {
                                    rule.dependencies.iter().enumerate().any(|(index, dep)| {
                                        dep.name == *name
                                            && !rule.secondary.as_ref().is_some_and(|clauses| {
                                                clauses.iter().any(|clause| clause.index == index)
                                            })
                                    })
                                });
                            if explicit {
                                crate::debug::emit(
                                    &self.output,
                                    crate::debug::Event::OughtToExist(
                                        name,
                                        debug_depth + active.len() + 1,
                                    ),
                                );
                            }
                            if name != b".WAIT" && !explicit {
                                let presence = self.implicit_presence(name)?;
                                if matches!(presence, ImplicitPresence::Local) {
                                    crate::debug::emit(
                                        &self.output,
                                        crate::debug::Event::FoundPrerequisite(name, event_depth),
                                    );
                                    dependencies.push(dependency);
                                    continue;
                                }
                                if let ImplicitPresence::Search(path) = presence {
                                    if let Some(label) = &debug_label {
                                        let known = self.implicit_known_file(name);
                                        if known {
                                            crate::debug::emit(
                                                &self.output,
                                                if compatibility {
                                                    crate::debug::Event::UsingCompatibilityRule(
                                                        label,
                                                        name,
                                                        event_depth,
                                                    )
                                                } else {
                                                    crate::debug::Event::NotOughtToExist(
                                                        name,
                                                        label,
                                                        event_depth,
                                                    )
                                                },
                                            );
                                        }
                                        if !known || !compatibility {
                                            crate::debug::emit(
                                                &self.output,
                                                crate::debug::Event::FoundVpath(
                                                    name,
                                                    &path,
                                                    event_depth,
                                                ),
                                            );
                                        }
                                    }
                                    dependencies.push(dependency);
                                    continue;
                                }
                                let known = self.implicit_known_file(name);
                                if let Some(label) = &debug_label
                                    && known
                                {
                                    crate::debug::emit(
                                        &self.output,
                                        if compatibility {
                                            crate::debug::Event::UsingCompatibilityRule(
                                                label,
                                                name,
                                                event_depth,
                                            )
                                        } else {
                                            crate::debug::Event::NotOughtToExist(
                                                name,
                                                label,
                                                event_depth,
                                            )
                                        },
                                    );
                                }
                                if known {
                                    if compatibility {
                                        dependencies.push(dependency);
                                        continue;
                                    }
                                    compatibility_needed = true;
                                }
                                if chain {
                                    let child_explicit = explicit_word
                                        || self.database.explicit_names.contains(name);
                                    crate::debug::emit(
                                        &self.output,
                                        crate::debug::Event::SearchingChildRule(
                                            name,
                                            child_explicit,
                                            event_depth,
                                        ),
                                    );
                                    if let Some(child) = self.implicit_plan(
                                        name,
                                        active,
                                        compatibility,
                                        budget,
                                        debug_depth,
                                    )? {
                                        plan.extend(child);
                                    } else {
                                        if let Some(label) = &debug_label {
                                            crate::debug::emit(
                                                &self.output,
                                                crate::debug::Event::RejectedPrerequisite(
                                                    label,
                                                    name,
                                                    None,
                                                    event_depth,
                                                ),
                                            );
                                        }
                                        if !self.implicit_known_file(name) {
                                            self.refresh_impossible_prerequisites();
                                            if let Some(history) = self.database.database_history()
                                            {
                                                let basename = name
                                                    .rsplit(|byte| *byte == b'/')
                                                    .next()
                                                    .unwrap_or(name);
                                                history.intern(basename);
                                            }
                                            self.impossible_prerequisites.insert(name.clone());
                                        }
                                        valid = false;
                                        break 'prerequisites;
                                    }
                                } else {
                                    crate::debug::emit(
                                        &self.output,
                                        crate::debug::Event::MissingPrerequisite(name, event_depth),
                                    );
                                    valid = false;
                                    break 'prerequisites;
                                }
                            }
                            dependencies.push(dependency);
                        }
                    }
                }
                active.pop();
                if valid {
                    if let Some(label) = &debug_label {
                        let printed = if archive_search {
                            rmake_core::filenames::archive_parts(target)
                                .map_or(target, |(archive, _)| &target[archive.len()..])
                        } else {
                            target
                        };
                        crate::debug::emit(
                            &self.output,
                            crate::debug::Event::FoundRule(
                                label,
                                printed,
                                debug_depth + active.len() + 1,
                            ),
                        );
                    }
                    let mut selected = candidate.rule.clone();
                    // A pattern barrier cannot cross into the target's explicit prerequisites.
                    Self::trim_implicit_waits(&mut dependencies);
                    selected.dependencies = dependencies;
                    selected.secondary = None;
                    plan.push(ImplicitSelection {
                        name: target.to_vec(),
                        rule: selected,
                        explicit_names,
                        intermediate_names,
                        prepared_scope: search.intermediate_scope.take().map(Box::new),
                    });
                    return Ok(Some(plan));
                }
            }
        }
        if compatibility_needed && !compatibility {
            crate::debug::emit(
                &self.output,
                crate::debug::Event::SearchingCompatibility(target, debug_depth + active.len() + 1),
            );
            search.compatibility = true;
            return self.implicit_search(target, active, search, budget, debug_depth);
        }
        Ok(None)
    }

    fn implicit_variables(
        &mut self,
        target: &[u8],
        intermediate: bool,
        stem: Vec<u8>,
        intermediate_scope: &mut Option<ImplicitScope>,
    ) -> Result<VariableStore, ExpandError> {
        if intermediate && intermediate_scope.is_none() {
            let pattern = self.database.prepare_pattern_scope(
                target,
                runtime_context(
                    None,
                    &mut RuntimeOutput::new(
                        &self.output,
                        &self.program,
                        &self.flag_state.warn_undefined,
                    ),
                )
                .with_makeflags(&mut self.flag_state.receiver())
                .with_filesystem(&mut |effect| self.filesystem.borrow_mut().execute(effect))
                .with_shell(&mut |request| {
                    crate::shell::run(
                        request,
                        &self.program,
                        self.makelevel,
                        &self.output,
                        self.jobserver.as_ref(),
                        || self.filesystem.borrow_mut().child_completed(),
                    )
                })
                .with_includes(&mut |name| self.include_search.borrow().read(name)),
            )?;
            *intermediate_scope = Some(ImplicitScope {
                pattern,
                shell_status: None,
            });
        }
        let mut context = if let Some(prepared) = intermediate_scope.as_ref() {
            let mut context = match prepared.pattern.as_ref() {
                Some(scope) => self.database.variables.with_target_scopes(&[scope]),
                None => self.database.variables.with_target_scopes(&[]),
            };
            if let Some(status) = &prepared.shell_status {
                context.retain_local_shell_status(status);
            }
            context
        } else {
            self.target_variables(target)?
        };
        let mut empty = Rule::default();
        let explicit = if intermediate {
            &mut empty
        } else {
            self.database.rules.get_mut(target).unwrap_or(&mut empty)
        };
        let mut ready = vec![true; explicit.dependencies.len()];
        if let Some(clauses) = &explicit.secondary {
            for clause in clauses.iter() {
                ready[clause.index] = false;
            }
        }
        for (name, value) in secondary_automatic(
            target,
            &mut explicit.dependencies,
            &ready,
            stem,
            self.options.always,
        ) {
            context.define_literal(name, &value, Origin::Automatic)?;
        }
        Ok(context)
    }

    fn implicit_filename(
        &mut self,
        name: &[u8],
        order_only: bool,
        directory: Option<&[u8]>,
    ) -> Result<Vec<Dependency>, ExpandError> {
        let name = rmake_core::filenames::without_dot_prefix(name);
        let name = if name.starts_with(b"~") {
            self.database.rule_tilde(
                name,
                &mut runtime_context(
                    None,
                    &mut RuntimeOutput::new(
                        &self.output,
                        &self.program,
                        &self.flag_state.warn_undefined,
                    ),
                )
                .with_makeflags(&mut self.flag_state.receiver())
                .with_filesystem(&mut |effect| self.filesystem.borrow_mut().execute(effect))
                .with_shell(&mut |request| {
                    crate::shell::run(
                        request,
                        &self.program,
                        self.makelevel,
                        &self.output,
                        self.jobserver.as_ref(),
                        || self.filesystem.borrow_mut().child_completed(),
                    )
                })
                .with_includes(&mut |name| self.include_search.borrow().read(name)),
            )?
        } else {
            std::borrow::Cow::Borrowed(name)
        };
        let name = name.as_ref();
        let name = directory.map_or_else(|| name.to_vec(), |dir| [dir, name].concat());
        if !name.iter().any(|byte| matches!(byte, b'*' | b'?' | b'[')) {
            if let Some(history) = self.database.database_history() {
                history.intern(&name);
            }
            return Ok(vec![Dependency {
                name,
                order_only,
                hidden: false,
            }]);
        }
        let bytes = self
            .filesystem
            .borrow_mut()
            .execute(rmake_core::diagnostic::FilesystemEffect::RuleGlob(&name))
            .map_err(|error| match error {
                rmake_core::diagnostic::FilesystemError::Caller(message) => {
                    ExpandError::FileError(message)
                }
                rmake_core::diagnostic::FilesystemError::Unlocated(message) => {
                    ExpandError::UnlocatedError(message)
                }
            })?;
        Ok(bytes
            .split(|&byte| byte == 0)
            .filter(|name| !name.is_empty())
            .map(|name| {
                if let Some(history) = self.database.database_history() {
                    history.intern(name);
                }
                Dependency {
                    name: name.to_vec(),
                    order_only,
                    hidden: false,
                }
            })
            .collect())
    }

    fn implicit_filenames(
        &mut self,
        value: &[u8],
        order_only: &mut bool,
        directory: Option<&[u8]>,
    ) -> Result<Vec<Dependency>, ExpandError> {
        let mut result = Vec::new();
        let (normal, order) = if *order_only {
            (std::borrow::Cow::Borrowed(value), None)
        } else {
            rmake_core::filenames::split_order(value)
        };
        for (text, role) in [
            (normal.as_ref(), *order_only),
            (order.unwrap_or_default(), true),
        ] {
            for name in rmake_core::filenames::sequence(text) {
                result.extend(self.implicit_filename(&name, role, directory)?);
            }
            // Each secondary-expanded word and its order-only partition
            // are parsed separately. A marker cannot escape either.
            Self::trim_implicit_waits(&mut result);
        }
        if order.is_some() {
            *order_only = true;
        }
        Ok(result)
    }

    fn trim_implicit_waits(dependencies: &mut Vec<Dependency>) {
        while dependencies
            .last()
            .is_some_and(|dependency| dependency.name == b".WAIT")
        {
            dependencies.pop();
        }
    }

    fn expand_dependencies(
        &mut self,
        target: &[u8],
        rule: &mut Rule,
        branch: Option<usize>,
    ) -> Result<(), ExpandError> {
        let Some(secondary) = rule.secondary.take() else {
            return Ok(());
        };
        let grouped_double = rule.grouped_double_id().is_some();
        let mut variables = self.rule_variables(target, branch)?;
        // GNU attributes secondary-expansion messages to the selected recipe,
        // including for prerequisite clauses read from other files.
        rule.prerequisite_location = rule.recipe_location.clone();
        // Ready occurrences include every immediate prerequisite, even those
        // after this clause. Deferred placeholders stay invisible until their
        // own expansion. Splicing preserves the parser's recipe-first order.
        let mut ready = vec![true; rule.dependencies.len()];
        for clause in secondary.iter() {
            ready[clause.index] = false;
        }
        let mut ready_count = rule.dependencies.len() - secondary.len();
        let mut original_next = 0;
        let mut expanded_next = 0;
        let mut explicit_stem_prepared = false;
        for clause in *secondary {
            let index = expanded_next + clause.index - original_next;
            // Empty deferred clauses cannot expose a prerequisite. Avoid
            // repeatedly scanning all remaining placeholders in that case.
            let visible = if ready_count == 0 {
                0
            } else {
                rule.dependencies.len()
            };
            let stem = clause.stem.as_deref().or(rule.stem.as_deref());
            if stem.is_none() && !explicit_stem_prepared {
                self.record_explicit_stem_lookup();
                explicit_stem_prepared = true;
            }
            let stem = stem
                .unwrap_or_else(|| {
                    self.database
                        .explicit_stem(prerequisite_variable_name(target))
                })
                .to_vec();
            for (name, value) in secondary_automatic(
                target,
                &mut rule.dependencies[..visible],
                &ready,
                stem,
                self.options.always,
            ) {
                variables.define_literal(name, &value, Origin::Automatic)?;
            }
            let occurrence = &rule.dependencies[index];
            let value = self.expand(
                &mut variables,
                &occurrence.name,
                rule.prerequisite_location.as_ref(),
            )?;
            let order_only = occurrence.order_only;
            let mut filesystem = |effect: rmake_core::diagnostic::FilesystemEffect<'_>| {
                self.filesystem.borrow_mut().execute(effect)
            };
            let mut shell = |request: rmake_core::diagnostic::ShellRequest<'_>| {
                crate::shell::run(
                    request,
                    &self.program,
                    self.makelevel,
                    &self.output,
                    self.jobserver.as_ref(),
                    || self.filesystem.borrow_mut().child_completed(),
                )
            };
            let mut includes = |name: &[u8]| self.include_search.borrow().read(name);
            let mut makeflags = self.flag_state.receiver();
            let mut diagnostic_output =
                RuntimeOutput::new(&self.output, &self.program, &self.flag_state.warn_undefined);
            let mut context = runtime_context(None, &mut diagnostic_output)
                .with_makeflags(&mut makeflags)
                .with_filesystem(&mut filesystem)
                .with_shell(&mut shell)
                .with_includes(&mut includes);
            let mut expanded = Vec::new();
            let mut pending_wait = None;
            let (normal, order) = if order_only {
                (std::borrow::Cow::Borrowed(value.as_slice()), None)
            } else {
                rmake_core::filenames::split_order(&value)
            };
            for (text, role) in [
                (normal.as_ref(), order_only),
                (order.unwrap_or_default(), true),
            ] {
                for name in self
                    .database
                    .secondary_prerequisite_filenames(text, &mut context)?
                {
                    Dependency {
                        name: name.into_owned(),
                        order_only: role,
                        hidden: false,
                    }
                    .append_to_partition(&mut expanded, &mut pending_wait);
                }
            }
            for dependency in expanded.iter().filter(|dep| dep.name != b".WAIT") {
                self.database
                    .file_records
                    .enter_prerequisite(&dependency.name);
            }
            let removed_wait = usize::from(clause.discard_targeted_wait);
            let replaced = index - removed_wait..=index;
            original_next = clause.index + 1;
            expanded_next = index - removed_wait + expanded.len();
            ready_count = ready_count - removed_wait + expanded.len();
            ready.splice(replaced.clone(), std::iter::repeat_n(true, expanded.len()));
            rule.dependencies.splice(replaced, expanded);
        }
        if let Some(context) = self.target_contexts.get_mut(target) {
            let retain_shell_status = !grouped_double && branch.is_none_or(|ordinal| ordinal == 0);
            context.retain_runtime_metadata(&variables, retain_shell_status);
        }
        // The recipe can read the prepared table, and a later branch can
        // read the head's table. Prerequisites use the target context.
        self.retain_database_context(target, branch, &variables);
        // Grouped rules publish their expanded dependency lists directly.
        // Their recipe still needs to know if expansion prepared the stem.
        self.retain_expanded_prerequisites(
            target,
            branch,
            rule.group.is_none().then_some(rule.dependencies.as_slice()),
            explicit_stem_prepared,
        );
        if self.shuffle.enabled() {
            let disabled = self.database.globally_serial();
            self.shuffle
                .regenerate(target, rule, &self.database, disabled, 0);
        }
        Ok(())
    }

    fn resolved_name_ref<'a>(&'a self, name: &'a [u8]) -> &'a [u8] {
        let name = self.aliases.resolve(name);
        self.resolved_names.get(name).map_or(name, Vec::as_slice)
    }

    fn resolved_name(&self, name: &[u8]) -> Vec<u8> {
        self.resolved_name_ref(name).to_vec()
    }

    fn debug_no_remake(&self, target: &[u8], depth: usize) {
        if self.output.debug_mask().has(crate::debug::Mask::VERBOSE) {
            let path = self.resolved_name_ref(target);
            crate::debug::emit(
                &self.output,
                crate::debug::Event::NoNeed(target, (path != target).then_some(path), depth),
            );
        }
    }

    fn record_graph_failure(&mut self, outcome: Outcome) {
        if !(outcome.fatal || (outcome.failed && !self.options.keep_going))
            || !self.parallel_graph()
        {
            return;
        }
        if !outcome.fatal && self.optional_remake() {
            // Synchronous failures settle their prerequisite frame in order.
            // Only a job completion can stop the whole optional owner early.
            return;
        }
        let status = if outcome.fatal {
            Status::Fatal
        } else {
            Status::Failure
        };
        if self.graph_abort.is_none() {
            let mut shutdown = self.shutdown.get();
            shutdown.drain();
            self.shutdown.set(shutdown);
            if self.parallel_jobs.values().any(|job| !job.waiting_load) {
                self.notice_waiting_for_jobs();
            }
            self.graph_abort = Some(status);
            self.graph_epoch = self.graph_epoch.wrapping_add(1);
        } else if status == Status::Fatal {
            self.graph_abort = Some(status);
        }
    }

    fn apply_graph_abort(&self, outcome: &mut Outcome) {
        if self.remake_stopped() {
            outcome.failed = true;
            outcome.blocked = true;
        }
        if let Some(status) = self.graph_abort {
            outcome.failed = true;
            outcome.fatal |= status == Status::Fatal;
        }
    }

    fn remember_optional_prerequisite(
        &mut self,
        target: &[u8],
        rule: &Rule,
        dropped: Option<&HashSet<Vec<u8>>>,
    ) {
        let Some(dependency) = rule.dependencies.iter().find(|dependency| {
            dependency.name != b".WAIT"
                && dropped.is_none_or(|dropped| !dropped.contains(&dependency.name))
                && self.done.get(&dependency.name).map_or_else(
                    || self.optional_remake_failures.contains_key(&dependency.name),
                    |child| child.failed || child.question_outdated,
                )
        }) else {
            return;
        };
        let cause = self
            .optional_remake_failures
            .get(&dependency.name)
            .and_then(Clone::clone)
            .unwrap_or_else(|| {
                Rc::new(FailedPrerequisite {
                    target: dependency.name.clone(),
                    parent: self
                        .scope_parents
                        .get(&dependency.name)
                        .and_then(Option::as_ref)
                        .map_or_else(|| target.to_vec(), |parent| parent.target.clone()),
                })
            });
        self.optional_remake_failures
            .entry(target.to_vec())
            .or_insert(Some(cause));
    }

    fn replay_makefile_failure(
        &mut self,
        name: &[u8],
        open_error: &mut Option<PendingIncludeError>,
    ) -> bool {
        let Some(cause) = self.optional_remake_failures.get(name).cloned() else {
            return false;
        };
        if let Some(error) = open_error.take() {
            self.emit_makefile_error(&error);
        }
        let (target, parent) = cause.as_ref().map_or((name, None), |cause| {
            (cause.target.as_slice(), Some(cause.parent.as_slice()))
        });
        self.report_missing_rule(target, parent);
        // A direct complaint is emitted once. Blocked goals retain their cause
        // and can be revisited by another required read of the same makefile.
        self.optional_remake_failures.remove(target);
        true
    }

    fn complete_cached_double_colon_input(
        &mut self,
        target: &[u8],
        depth: usize,
        mut outcome: Outcome,
        input: assumptions::CachedDoubleColonInput,
    ) -> Outcome {
        use crate::debug::{Event, Mask, emit};
        use assumptions::CachedDoubleColonInput;

        if self.output.debug_mask().has(Mask::VERBOSE) {
            let branches = self
                .database
                .rules
                .get(target)
                .and_then(|rule| rule.double_colon.as_ref())
                .map_or(0, Vec::len);
            self.debug_completed_double_colon_declarations(target, 0..branches, depth);
        }
        self.debug_consider(target, target, depth);
        match input {
            CachedDoubleColonInput::Unvisited(time) => {
                if self.new_double_colon_entry_count(target) > 1 {
                    return self.start_late_double_colon_recipe(target, depth, time, outcome, None);
                }
                match self.synthetic_double_colon_recipe(target, depth) {
                    Ok(Some(rule)) => {
                        return self.start_late_double_colon_recipe(
                            target,
                            depth,
                            time,
                            outcome,
                            Some(rule),
                        );
                    }
                    Err(()) => {
                        outcome.failed = true;
                        outcome.fatal = true;
                        self.done.insert(target.to_vec(), outcome);
                        return outcome;
                    }
                    Ok(None) => {}
                }
                if self.options.keep_going {
                    // Once the tail finishes, dependencies see its logical age.
                    // A same-pass visit returns the declared head's status.
                    let key = target.to_vec();
                    let mut head = self
                        .database_branch_outcomes
                        .get(&(key.clone(), 0))
                        .copied()
                        .unwrap_or(outcome);
                    head.time = Some(time);
                    head.search_after_update = false;
                    self.done.insert(key, head);
                }
                self.fail_double_colon_input(target, depth, &mut outcome);
            }
            CachedDoubleColonInput::Failed => {
                emit(&self.output, Event::RecentlyFailed(target, depth));
                outcome.failed = true;
            }
        }
        outcome.late_input_failed = true;
        if !self.options.keep_going {
            self.done.insert(target.to_vec(), outcome);
        }
        outcome
    }

    fn start_late_double_colon_recipe(
        &mut self,
        target: &[u8],
        depth: usize,
        time: SystemTime,
        head: Outcome,
        first_rule: Option<Rule>,
    ) -> Outcome {
        let ordinal = self
            .database
            .rules
            .get(target)
            .and_then(|rule| rule.double_colon.as_ref())
            .map_or(0, Vec::len);
        let count = self.new_double_colon_entry_count(target);
        let mut branches = std::collections::VecDeque::from([(ordinal, first_rule)]);
        for offset in 1..count {
            branches.push_back((ordinal + offset, None));
        }
        let before = self.modified(target);
        let phony = self.special(b".PHONY", target, false);
        self.done.remove(target);
        self.pending_double_colon.insert(
            target.to_vec(),
            DoubleColonState {
                remake: self.active_makefile_remake.clone(),
                synthetic_tail: Some(Box::new(SyntheticDoubleColonTail {
                    assumed_time: Some(time),
                    head,
                })),
                depth,
                before,
                phony,
                branches,
                active: None,
                last_epoch: None,
                awaiting_recipe: false,
                combined: Outcome {
                    time: Some(time),
                    ran: false,
                    search_after_update: false,
                    completion: CompletionTime::Preserve,
                    ..head
                },
            },
        );
        self.resume_double_colon(target)
    }

    fn fail_double_colon_input(&self, target: &[u8], depth: usize, outcome: &mut Outcome) {
        use crate::debug::{Event, emit};

        emit(&self.output, Event::Finished(target, depth));
        emit(
            &self.output,
            Event::DoubleWithoutPrerequisites(target, depth),
        );
        emit(&self.output, Event::MustRemake(target, depth));
        if !self.optional_remake() {
            self.show_makefile_error();
            self.report_missing_rule(target, None);
        }
        outcome.failed = true;
        if self.options.keep_going {
            emit(&self.output, Event::Failed(target, depth));
        }
    }

    fn show_retained_optional_failure(
        &mut self,
        target: &[u8],
        parent: Option<&[u8]>,
        cause: Option<Rc<FailedPrerequisite>>,
    ) {
        self.show_makefile_error();
        if let Some(cause) = cause {
            self.report_missing_rule(&cause.target, Some(&cause.parent));
            self.optional_remake_failures
                .remove(cause.target.as_slice());
        } else {
            self.report_missing_rule(target, parent);
        }
    }

    fn report_missing_rule(&self, target: &[u8], parent: Option<&[u8]>) {
        let keep = self.options.keep_going;
        let body = crate::messages::system(rmake_core::messages::Message::NoRule {
            prefix: if keep { b"*** " } else { b"" },
            target,
            parent,
            suffix: if keep { b"." } else { b"" },
        });
        if keep {
            self.stderr(&body);
        } else {
            let stop = crate::messages::stop();
            self.output
                .write_parts(true, &[&self.program, b": *** ", &body, &stop]);
        }
    }

    fn archive_metadata_error(&mut self, target: &[u8], search: bool) -> bool {
        let Some((_, member)) = rmake_core::filenames::archive_parts(target) else {
            return false;
        };
        let symbol = member.starts_with(b"(") && member.ends_with(b")");
        if !symbol && self.special(b".PHONY", target, false) {
            return false;
        }
        if search && self.path_observations.contains_key(target) {
            return false;
        }
        if !search && self.archive_completion_times.contains_key(target) {
            return false;
        }
        let location = match self.archive_location_for(target, !search) {
            Ok(location) => location,
            Err(error) => {
                self.expansion_error(None, &error);
                return true;
            }
        };
        match self.archive_observation_at(target, &location, search.then_some(target)) {
            Ok(observation) => {
                if search {
                    if location.container_time.is_some() {
                        self.path_observations.insert(target.to_vec(), observation);
                    }
                } else {
                    self.archive_completion_times.insert(
                        target.to_vec(),
                        ArchiveCompletion {
                            time: observation.as_ref().and_then(|(_, time)| *time),
                            known: location.container_time.is_some(),
                        },
                    );
                }
                false
            }
            Err(error) => {
                if !search {
                    self.archive_completion_times.insert(
                        target.to_vec(),
                        ArchiveCompletion {
                            time: None,
                            known: true,
                        },
                    );
                }
                self.expansion_error(None, &error);
                true
            }
        }
    }

    fn build(
        &mut self,
        target: &[u8],
        parent: Option<ScopeParent<&[u8]>>,
        depth: usize,
    ) -> Outcome {
        self.build_recording_initial_time(target, parent, depth, None, BuildVisit::Ordinary, None)
    }

    fn build_recording_initial_time(
        &mut self,
        target: &[u8],
        parent: Option<ScopeParent<&[u8]>>,
        depth: usize,
        initial_time: Option<&mut Option<Option<SystemTime>>>,
        visit: BuildVisit,
        goal_started_command: Option<&mut bool>,
    ) -> Outcome {
        self.invalidate_phony_names(target);
        let known_alias = self.aliases.contains(target);
        // Goal search preparation can observe a timestamp before the graph
        // visit. Its warning belongs after that goal's consideration line.
        let goal_clock = parent.is_none();
        self.delay_clock_checks = goal_clock;
        let prepared = self.prepare_search_name(target);
        self.delay_clock_checks = false;
        let renamed = match prepared {
            Ok(name) => name,
            Err(()) => {
                self.check_considered_goal_clock();
                return Outcome {
                    failed: true,
                    fatal: true,
                    ..Outcome::default()
                };
            }
        };
        if !known_alias && let Some(name) = &renamed {
            // A goal reaches its timestamp lookup after the already-considered
            // check. A newly merged goal gets a first evaluation even when the
            // destination was completed earlier. Prerequisite/peer lookups run
            // before that check and prepare their aliases before entering here.
            self.done.remove(name);
        }
        // A new goal can announce consideration before a GPATH rename.
        // Prerequisites prepare their lookup before beginning consideration.
        let original_goal = (!known_alias && parent.is_none()).then_some(target);
        let target = renamed.as_deref().unwrap_or(target);
        // The second look at this waiting frame will only read its current
        // pass outcome. Reaping here could settle its child before the outer
        // frame has a chance to yield to the goal loop.
        let cached_wait = visit == BuildVisit::RevisitPrerequisite
            && !self.remaking
            && self.load_waiting.is_empty()
            && self.pending_rules.get(target).is_some_and(|frame| {
                frame.branch.is_none()
                    && frame.last_epoch == Some(self.graph_epoch)
                    && frame.waited_for_prerequisites
                    && frame.visited_prerequisites()
            });
        if cached_wait {
            self.observe_interrupt();
        }
        if !cached_wait || self.shutdown.get() != Shutdown::Running {
            self.poll_jobs();
        }
        if self.forced_query_stop {
            return Outcome {
                question_outdated: true,
                ..Outcome::default()
            };
        }
        if visit == BuildVisit::GoalPassStart {
            // The existing first poll can observe an already waitable child
            // before this pass starts visiting its goals.
            self.advance_debug_pass();
        }
        let deferred_peer_failure =
            self.parallel_makefile_remakes && self.cached_optional_group_peer_failure(target);
        let archive_target = rmake_core::filenames::archive_parts(target).is_some();
        let previously_done = archive_target && self.done.contains_key(target);
        let archive_error = if archive_target
            && !previously_done
            && self.assumed_time(target).is_none()
            && !self.pending(target)
        {
            self.delay_clock_checks = goal_clock;
            let error = self.archive_metadata_error(target, true);
            self.delay_clock_checks = false;
            error
        } else {
            false
        };
        // Count only dispatches after the scheduler's previsit reap and
        // load check, even if that check starts this goal's queued recipe.
        let commands_before = self.commands_started;
        let mut outcome = if archive_error {
            Outcome {
                failed: true,
                fatal: true,
                ..Outcome::default()
            }
        } else {
            self.build_inner(
                target,
                original_goal.unwrap_or(target),
                parent,
                depth,
                initial_time,
                visit,
            )
        };
        if let Some(started) = goal_started_command {
            *started |= self.commands_started != commands_before;
        }
        if goal_clock {
            self.check_considered_goal_clock();
        }
        if archive_target
            && !previously_done
            && !outcome.failed
            && matches!(
                outcome.completion,
                CompletionTime::Refresh | CompletionTime::ArchiveUnknown
            )
            && !self.pending(target)
            && self.archive_metadata_error(target, false)
        {
            outcome.failed = true;
            outcome.fatal = true;
        }
        if self.optional_remake() && (outcome.failed || outcome.question_outdated) && !outcome.fatal
        {
            self.optional_remake_failures
                .entry(target.to_vec())
                .or_default();
        }
        if !deferred_peer_failure || !outcome.optional_group_peer_failure || outcome.fatal {
            self.record_graph_failure(outcome);
        }
        outcome
    }

    fn build_inner(
        &mut self,
        target: &[u8],
        considering_name: &[u8],
        parent: Option<ScopeParent<&[u8]>>,
        depth: usize,
        initial_time: Option<&mut Option<Option<SystemTime>>>,
        visit: BuildVisit,
    ) -> Outcome {
        let mut traced_optional_failure = false;
        if !self.optional_remake()
            && let Some(cause) = self.optional_remake_failures.remove(target)
        {
            // A later required visit exposes the original failure hidden by
            // the optional remake, including the edge to a deeper failure.
            let query = self
                .done
                .get(target)
                .copied()
                .filter(|outcome| outcome.question_outdated);
            if query.is_some() {
                use crate::debug::{Event, emit};

                let debug_depth = depth + usize::from(self.remaking);
                emit(
                    &self.output,
                    Event::Considering(considering_name, debug_depth),
                );
                emit(&self.output, Event::RecentlyFailed(target, debug_depth));
                traced_optional_failure = true;
            } else if !self.remaking
                && self.done.get(target).is_some_and(|outcome| outcome.failed)
                && self.trace_cached_unruled_makefile(considering_name, target, depth, true)
            {
                traced_optional_failure = true;
            }
            if !traced_optional_failure
                && self.remaking
                && !self.parallel_makefile_remakes
                && self.options.keep_going
                && parent.is_some()
                && self
                    .done
                    .get(target)
                    .is_some_and(|outcome| outcome.failed && !outcome.fatal)
            {
                use crate::debug::{Event, emit};

                emit(
                    &self.output,
                    Event::Considering(considering_name, depth + 1),
                );
                emit(&self.output, Event::RecentlyFailed(target, depth + 1));
                traced_optional_failure = true;
            }
            self.show_retained_optional_failure(target, parent.map(|parent| parent.target), cause);
            if let Some(outcome) = query
                && !self.options.keep_going
            {
                return Outcome {
                    failed: true,
                    fatal: true,
                    ..outcome
                };
            }
        }
        if let Some(owner) = self.active_peers.get(target)
            && owner != target
        {
            let mut outcome = self.done.get(owner).copied().unwrap_or_default();
            if !self.remaking
                && !traced_optional_failure
                && self.output.debug_mask().has(crate::debug::Mask::VERBOSE)
                && self.ordinary_group_recipe_pending(owner)
            {
                self.debug_cached_prerequisite_visit(target, depth, outcome);
            }
            outcome.time = self.modified(target);
            return outcome;
        }
        if let Some(tail) = self.ordinary_queries.deferred_tails.get_mut(target) {
            // The goal that first set the verdict was removed from the
            // current pass. A repeated explicit goal can request its tail.
            tail.queued |= parent.is_none();
            if !tail.queued
                || !self.options.keep_going
                || self.graph_abort.is_some()
                || tail.pass == self.ordinary_queries.pass
            {
                return tail.state.combined;
            }
            let tail = self
                .ordinary_queries
                .deferred_tails
                .remove(target)
                .expect("deferred query tail");
            self.pending_double_colon
                .insert(target.to_vec(), tail.state);
        }
        if self.pending_double_colon.contains_key(target) {
            return self.resume_double_colon(target);
        }
        if self.pending_rules.contains_key(target) {
            return self.resume_rule(target);
        }
        if let Some(mut outcome) = self.done.get(target).copied() {
            if let Some(parent) = parent {
                self.remember_retained_scope_parent(target, parent);
            }
            let debug_depth = depth + usize::from(self.remaking);
            if self.graph_abort.is_none()
                && !outcome.fatal
                && (self.options.keep_going || (!outcome.failed && !outcome.question_outdated))
                && let Some(input) = self.visit_cached_double_colon_input(target)
            {
                return self.complete_cached_double_colon_input(
                    target,
                    debug_depth,
                    outcome,
                    input,
                );
            }
            let pruned = self
                .debug_considered
                .as_ref()
                .is_some_and(|files| files.contains(target));
            if !traced_optional_failure {
                if pruned {
                    crate::debug::emit(
                        &self.output,
                        crate::debug::Event::Pruning(target, debug_depth),
                    );
                } else if !self.trace_cached_unruled_makefile(
                    considering_name,
                    target,
                    debug_depth,
                    outcome.failed,
                ) {
                    if outcome.question_outdated && self.makefile_names.contains(target) {
                        use crate::debug::{Event, emit};

                        emit(
                            &self.output,
                            Event::Considering(considering_name, debug_depth),
                        );
                        emit(&self.output, Event::RecentlyFailed(target, debug_depth));
                    } else if visit == BuildVisit::PrerequisiteAfterRecipe
                        || outcome.debug_serial_group_peer
                    {
                        self.debug_cached_prerequisite_visit(target, debug_depth, outcome);
                    } else {
                        let mask = self.output.debug_mask();
                        if depth == 0
                            && !self.remaking
                            && !outcome.failed
                            && mask.has(crate::debug::Mask::MAKEFILES)
                            && mask.has(crate::debug::Mask::VERBOSE)
                            && MakefileModes::from_options(&self.options).active()
                            && self.makefile_names.contains(target)
                            && self.is_makefile_goal(target, None)
                        {
                            crate::debug::emit(
                                &self.output,
                                crate::debug::Event::Considering(considering_name, debug_depth),
                            );
                        }
                        crate::debug::emit(
                            &self.output,
                            crate::debug::Event::Already(target, debug_depth),
                        );
                    }
                }
            }
            if outcome.completion == CompletionTime::ArchiveUnknown
                && rmake_core::filenames::archive_parts(target).is_some()
            {
                match self.archive_observation(target, false) {
                    Ok((observation, known)) => {
                        outcome.time = observation.as_ref().and_then(|(_, time)| *time);
                        outcome.completion = if known {
                            CompletionTime::Refresh
                        } else {
                            CompletionTime::ArchiveUnknown
                        };
                        self.archive_completion_times.insert(
                            target.to_vec(),
                            ArchiveCompletion {
                                time: outcome.time,
                                known,
                            },
                        );
                    }
                    Err(error) => {
                        self.expansion_error(None, &error);
                        outcome.failed = true;
                        outcome.fatal = true;
                    }
                }
                self.done.insert(target.to_vec(), outcome);
            }
            return outcome;
        }
        if self.graph_abort.is_some() || self.remake_stopped() {
            let mut outcome = Outcome::default();
            self.apply_graph_abort(&mut outcome);
            return outcome;
        }
        if self.ignores_file_rules(target) {
            // An old alias still suppresses recipes after the search has
            // retained a physical timestamp for dependents.
            let time = self
                .path_observations
                .remove(target)
                .flatten()
                .and_then(|(_, time)| time)
                .or_else(|| self.assumed_time(target));
            let outcome = Outcome {
                time,
                completion: CompletionTime::Preserve,
                ..Outcome::default()
            };
            self.done.insert(target.to_vec(), outcome);
            return outcome;
        }
        if depth >= 512 {
            self.fatal(None, b"rmake: dependency nesting limit exceeded");
            return Outcome {
                failed: true,
                fatal: true,
                ..Outcome::default()
            };
        }
        let debug_depth = depth + usize::from(self.remaking);
        if parent.is_none() {
            self.debug_consider(target, considering_name, debug_depth);
            self.check_considered_goal_clock();
        }
        // Record first ownership before implicit secondary expansion can ask
        // for this context. Pattern-scope activation itself remains lazy.
        self.scope_parents
            .entry(target.to_vec())
            .or_insert_with(|| parent.map(ScopeParent::into_owned));
        let phony = self.special(b".PHONY", target, false);
        // Search-time expansion can create or remove this target. GNU decides
        // freshness from the observation made before those effects occur.
        let observed = self.path_observations.remove(target);
        let resolved = if phony && observed.is_none() && self.assumed_time(target).is_none() {
            None
        } else {
            match observed
                .map(Ok)
                .unwrap_or_else(|| self.find_vpath(target, true))
            {
                Ok(observation) => observation,
                Err(error) => {
                    self.expansion_error(None, &error);
                    return Outcome {
                        failed: true,
                        fatal: true,
                        ..Outcome::default()
                    };
                }
            }
        };
        // The parent reads a prerequisite's mtime before considering it.
        if parent.is_some() {
            self.debug_consider(target, considering_name, debug_depth);
        }
        let mut before = resolved.as_ref().and_then(|(_, time)| *time);
        if let Some(initial_time) = initial_time {
            *initial_time = Some(before);
        }
        if before.is_none() {
            crate::debug::emit(
                &self.output,
                crate::debug::Event::Missing(target, debug_depth + 1),
            );
        }
        let double_colon = self
            .database
            .rules
            .get(target)
            .is_some_and(|rule| rule.double_colon.is_some());
        let comparison = if double_colon {
            before
        } else {
            self.comparison_time(target, before)
        };
        if !phony && !self.resolve_implicit_at(target, debug_depth, true) {
            return Outcome {
                failed: true,
                fatal: true,
                ..Outcome::default()
            };
        }
        if self
            .database
            .rules
            .get(target)
            .is_some_and(|rule| rule.double_colon.is_some())
        {
            before = *self
                .double_colon_before
                .entry(target.to_vec())
                .or_insert(before);
        }
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
            crate::debug::emit(
                &self.output,
                crate::debug::Event::Finished(target, debug_depth),
            );
            if before.is_some() {
                self.debug_no_remake(target, debug_depth);
                let result = Outcome {
                    time: before,
                    ..Outcome::default()
                };
                if !self.special(b".NOTINTERMEDIATE", target, true) {
                    self.database_secondary.insert(target.to_vec());
                }
                self.done.insert(target.to_vec(), result);
                return result;
            }
            crate::debug::emit(
                &self.output,
                crate::debug::Event::MustRemake(target, debug_depth),
            );
            if !self.optional_remake() {
                self.show_makefile_error();
                self.report_missing_rule(target, parent.map(|parent| parent.target));
            }
            let serial_remake = self.remaking && !self.parallel_makefile_remakes;
            if (self.optional_remake() && (depth == 0 || serial_remake))
                || (self.options.keep_going && (!self.remaking || serial_remake))
            {
                crate::debug::emit(
                    &self.output,
                    crate::debug::Event::Failed(target, debug_depth),
                );
            }
            let result = Outcome {
                failed: true,
                ..Outcome::default()
            };
            self.done.insert(target.to_vec(), result);
            return result;
        };
        if let Some(branches) = rule.double_colon.take() {
            let mut branches: std::collections::VecDeque<_> =
                branches.into_iter().map(Some).enumerate().collect();
            for _ in 0..self.new_double_colon_entry_count(target) {
                branches.push_back((branches.len(), None));
            }
            self.pending_double_colon.insert(
                target.to_vec(),
                DoubleColonState {
                    remake: self.active_makefile_remake.clone(),
                    synthetic_tail: None,
                    depth,
                    before,
                    phony,
                    branches,
                    active: None,
                    last_epoch: None,
                    awaiting_recipe: false,
                    combined: Outcome {
                        time: before,
                        ..Outcome::default()
                    },
                },
            );
            let outcome = self.resume_double_colon(target);
            if !self.parallel_graph() {
                self.wait_target(target);
                if outcome.late_input_failed {
                    return outcome;
                }
                return self.done.get(target).copied().unwrap_or(outcome);
            }
            return outcome;
        }
        self.build_rule(target, depth, before, comparison, phony, rule, None, false)
    }

    fn remember_retained_scope_parent(&mut self, target: &[u8], parent: ScopeParent<&[u8]>) {
        if !self.retained_scope_parents.is_empty()
            && let Some(latest) = self.retained_scope_parents.get_mut(target)
        {
            if latest.target.as_slice() != parent.target {
                latest.target.clear();
                latest.target.extend_from_slice(parent.target);
            }
            latest.branch = parent.branch;
            // Keep the live edge if it returns to its original parent. A
            // retained eager read may have updated this file's local table.
            return;
        }
        if self.scope_parents.get(target).is_some_and(|first| {
            first
                .as_ref()
                .map(|first| (self.aliases.resolve(&first.target), first.branch))
                != Some((parent.target, parent.branch))
        }) {
            self.retained_scope_parents
                .insert(target.to_vec(), parent.into_owned());
        }
    }

    fn trace_cached_unruled_makefile(
        &self,
        considering_name: &[u8],
        target: &[u8],
        depth: usize,
        failed: bool,
    ) -> bool {
        use crate::debug::{Event, Mask, emit};

        if self.remaking
            || !self.output.debug_mask().has(Mask::VERBOSE)
            || !self.makefile_names.contains(target)
            || self.database.rules.contains_key(target)
        {
            return false;
        }
        emit(&self.output, Event::Considering(considering_name, depth));
        emit(
            &self.output,
            if failed {
                Event::RecentlyFailed(target, depth)
            } else {
                Event::Already(target, depth)
            },
        );
        true
    }

    fn resume_double_colon(&mut self, target: &[u8]) -> Outcome {
        if let Some(state) = self.pending_double_colon.get(target)
            && state.last_epoch == Some(self.graph_epoch)
        {
            return state.combined;
        }
        let state = self
            .pending_double_colon
            .remove(target)
            .expect("pending double-colon continuation");
        self.with_makefile_remake(state.remake.clone(), |engine| {
            engine.resume_double_colon_state(target, state)
        })
    }

    fn serial_double_colon_declarations(&self, target: &[u8]) -> Option<usize> {
        if self.parallel_makefile_remakes
            || self.options.dry_run
            || self.options.question
            || self.options.touch
        {
            return None;
        }
        let branches = self.database.rules.get(target)?.double_colon.as_ref()?;
        (branches.len() > 1 && branches.iter().all(|branch| branch.group.is_none()))
            .then_some(branches.len())
    }

    fn resume_double_colon_state(&mut self, target: &[u8], mut state: DoubleColonState) -> Outcome {
        state.last_epoch = Some(self.graph_epoch);
        let mut replay_cached_after_recipe = false;
        let serial_remake_root = self.remaking
            && !self.parallel_makefile_remakes
            && state.depth == 0
            && self.active_makefile_remake.is_some();
        let optional_serial_declarations = serial_remake_root
            && self.optional_remake()
            && self.serial_double_colon_declarations(target).is_some();
        loop {
            if let Some(ordinal) = state.active {
                if self.pending_rules.contains_key(target) {
                    self.resume_rule(target);
                }
                let recipe_waiting = self.parallel_jobs.contains_key(target);
                if self.pending_rules.contains_key(target) || recipe_waiting {
                    state.awaiting_recipe |= recipe_waiting;
                    let mut outcome = state.combined;
                    if let Some(branch) = self.done.get(target).copied() {
                        merge_child(&mut outcome, branch);
                    }
                    self.pending_double_colon.insert(target.to_vec(), state);
                    return outcome;
                }
                let result = self
                    .done
                    .remove(target)
                    .expect("finished double-colon branch");
                // A direct single declaration keeps the ordinary goal-level visit.
                if std::mem::take(&mut state.awaiting_recipe)
                    && (state.depth != 0 || !state.single_active_branch())
                    && !self.remaking
                    && self.graph_abort.is_none()
                    && self.shutdown.get() == Shutdown::Running
                    && !result.fatal
                    && !result.question_outdated
                    && self.output.debug_mask().has(crate::debug::Mask::VERBOSE)
                {
                    use crate::debug::{Event, emit};

                    replay_cached_after_recipe = true;
                    self.debug_completed_double_colon_declarations(target, 0..ordinal, state.depth);
                    emit(&self.output, Event::Considering(target, state.depth));
                    emit(
                        &self.output,
                        if result.failed {
                            Event::RecentlyFailed(target, state.depth)
                        } else {
                            Event::Already(target, state.depth)
                        },
                    );
                }
                // A failed owner can already have a physical observation
                // because it is also a member of the completed group.
                let group_observed = result.database_result != DatabaseResult::RecipeUnstarted
                    && self
                        .database_branch_outcomes
                        .entry((target.to_vec(), ordinal))
                        .or_insert(result)
                        .database_result
                        == DatabaseResult::ObservedGroup;
                let assumed_time = state
                    .synthetic_tail
                    .as_ref()
                    .and_then(|tail| tail.assumed_time);
                state.combined.retain_branch_verdict(result);
                state.combined.time = assumed_time.filter(|_| !result.ran).or(result.time);
                state.combined.ran |= result.ran;
                state.combined.failed |= result.failed;
                state.combined.fatal |= result.fatal;
                state.combined.blocked |= result.blocked;
                state.combined.question_outdated |= result.question_outdated;
                state.combined.late_input_failed |= result.late_input_failed;
                state.combined.search_after_update |= result.search_after_update;
                state.combined.completion = state.combined.completion.max(result.completion);
                if matches!(
                    result.database_result,
                    DatabaseResult::Simulated | DatabaseResult::ObservedDoubleColon
                ) {
                    state.combined.database_result = result.database_result;
                }
                if !group_observed
                    && (result.database_result == DatabaseResult::Simulated
                        || result.database_result == DatabaseResult::ObservedDoubleColon
                        || result.completion >= CompletionTime::Refresh)
                    && let Some(time) = self
                        .database_group_peer_times
                        .get_mut(target)
                        .and_then(|branches| branches.get_mut(ordinal))
                {
                    *time = GroupPeerTime::Outcome;
                }
                self.record_graph_failure(result);
                state.active = None;
                if result.fatal || (result.failed && !self.options.keep_going) {
                    state.branches.clear();
                } else if result.question_outdated && !self.options.keep_going {
                    let earlier_verdict = self.ordinary_queries.first_verdict.is_some();
                    if !self.remaking
                        && self.active_makefile_remake.is_none()
                        && state.depth == 0
                        && (earlier_verdict || self.options.goals.len() > 1)
                        && !state.branches.is_empty()
                    {
                        // A later goal can turn on keep-going. Leave the tail
                        // outside active prerequisite waits until a new pass.
                        let outcome = state.combined;
                        self.ordinary_queries.deferred_tails.insert(
                            target.to_vec(),
                            DeferredQueryTail {
                                pass: self.ordinary_queries.pass,
                                queued: earlier_verdict,
                                state,
                            },
                        );
                        return outcome;
                    }
                    state.branches.clear();
                }
            }
            self.apply_graph_abort(&mut state.combined);
            if self.graph_abort.is_some() || self.remake_stopped() {
                state.branches.clear();
            }
            let Some((ordinal, entry)) = state.branches.pop_front() else {
                // A real recipe refreshes the shared time even if another
                // branch simulated its recipe during the same update.
                if state.combined.completion >= CompletionTime::Refresh
                    || (state.combined.completion != CompletionTime::AssumeNew
                        && state.combined.database_result == DatabaseResult::Simulated)
                {
                    state.combined.database_result = DatabaseResult::Outcome;
                }
                if !state.combined.failed
                    && !state.combined.question_outdated
                    && let Some(branches) = self.database_group_peer_times.remove(target)
                    && branches
                        .iter()
                        .any(|time| matches!(time, GroupPeerTime::Unchecked))
                {
                    // Completion of the chain propagates an unchecked time
                    // to all declarations, so the next observation is physical.
                    if !state.phony && state.combined.completion.preserves() {
                        self.modified(target);
                    }
                    state.combined.database_result = DatabaseResult::ObservedDoubleColon;
                }
                state.combined.time = if state.phony {
                    None
                } else if state.combined.completion.preserves() || state.combined.failed {
                    state.combined.time
                } else {
                    self.modified(target)
                };
                state.combined.debug_time_changed =
                    self.output.debug_mask().has(crate::debug::Mask::BASIC)
                        && state
                            .synthetic_tail
                            .as_ref()
                            .and_then(|tail| tail.assumed_time)
                            .or(state.before)
                            != state.combined.time;
                if let Some(tail) = state.synthetic_tail {
                    if !state.combined.failed {
                        if tail.assumed_time.is_some() {
                            self.forget_cached_double_colon_input(target);
                        }
                    } else if self.options.keep_going && !state.combined.fatal {
                        let mut head = tail.head;
                        head.time = state.combined.time;
                        head.search_after_update = false;
                        self.done.insert(target.to_vec(), head);
                        self.remember_failed_double_colon_input(target);
                        state.combined.late_input_failed = true;
                        return state.combined;
                    }
                }
                self.done.insert(target.to_vec(), state.combined);
                return state.combined;
            };
            let declared = entry.is_some();
            let branch = if let Some(branch) = entry {
                branch
            } else {
                if state.synthetic_tail.is_none()
                    && !self.remaking
                    && self.options.keep_going
                    && !state.combined.failed
                {
                    let head = self
                        .database_branch_outcomes
                        .get(&(target.to_vec(), 0))
                        .copied()
                        .unwrap_or(state.combined);
                    state.synthetic_tail = Some(Box::new(SyntheticDoubleColonTail {
                        assumed_time: None,
                        head,
                    }));
                }
                match self.synthetic_double_colon_recipe(target, state.depth) {
                    Ok(Some(rule)) => rule,
                    Ok(None) => {
                        self.fail_double_colon_input(target, state.depth, &mut state.combined);
                        state
                            .combined
                            .first_branch_verdict
                            .get_or_insert(Status::Failure);
                        self.record_graph_failure(state.combined);
                        if !self.options.keep_going {
                            state.branches.clear();
                        }
                        continue;
                    }
                    Err(()) => {
                        state.combined.failed = true;
                        state.combined.fatal = true;
                        state.branches.clear();
                        continue;
                    }
                }
            };
            if let Some(id) = branch.grouped_double_id() {
                if let Some(mut result) = self.completed_group_branches.get(&id).copied() {
                    // Commands executed for a previous goal must not suppress
                    // this goal's up-to-date diagnostic.
                    result.ran = false;
                    state.combined.retain_branch_verdict(result);
                    merge_child(&mut state.combined, result);
                    state.combined.time = if state.phony {
                        None
                    } else {
                        self.modified(target)
                    };
                    if result.fatal || (result.failed && !self.options.keep_going) {
                        state.branches.clear();
                    }
                    continue;
                }
                if self.active_group_branches.contains(&id) {
                    state.branches.push_front((ordinal, Some(branch)));
                    let outcome = state.combined;
                    self.pending_double_colon.insert(target.to_vec(), state);
                    return outcome;
                }
            }
            // The target visit announced the first declaration.
            let assumed_time = state
                .synthetic_tail
                .as_ref()
                .and_then(|tail| tail.assumed_time);
            let mut branch_before = state.before;
            if serial_remake_root
                && declared
                && ordinal != 0
                && branch_before.is_none()
                && !state.phony
            {
                // A failed source open cached a missing observation for the
                // head alone. Later declarations can see the source created by
                // a preceding recipe. A known initial time remains shared.
                branch_before = self.modified(target);
            }
            if ordinal != 0
                && (!self.remaking || (serial_remake_root && declared))
                && assumed_time.is_none()
            {
                let depth = state.depth + usize::from(self.remaking);
                crate::debug::emit(
                    &self.output,
                    crate::debug::Event::Considering(target, depth),
                );
                if declared && branch_before.is_none() {
                    crate::debug::emit(
                        &self.output,
                        crate::debug::Event::Missing(target, depth + 1),
                    );
                }
            }
            let before = assumed_time.or(branch_before);
            let comparison = self.comparison_time(target, before);
            let visit = |engine: &mut Self| {
                engine.build_rule(
                    target,
                    state.depth,
                    branch_before,
                    comparison,
                    state.phony,
                    branch,
                    Some(ordinal),
                    replay_cached_after_recipe,
                );
            };
            if optional_serial_declarations
                && declared
                && ordinal != 0
                && let Some(mut context) = self.active_makefile_remake.clone()
            {
                // Only the head inherits the optional source policy.
                context.optional = false;
                context.retain_failure = false;
                self.with_makefile_remake(Some(context), visit);
            } else {
                visit(self);
            }
            state.active = Some(ordinal);
        }
    }

    fn drop_group_dependency(
        &mut self,
        owner: &[u8],
        branch: Option<usize>,
        ordinal: usize,
        name: &[u8],
    ) {
        self.invalidate_phony_names(owner);
        if let Some(root) = self.database.rules.get_mut(owner) {
            let rule = if let Some(id) = branch {
                root.double_colon.as_mut().and_then(|branches| {
                    branches.iter_mut().find(|rule| {
                        rule.grouped_double_id() == Some(id)
                            && rule.grouped_double_ordinal() == ordinal
                    })
                })
            } else {
                Some(root)
            };
            if let Some(rule) = rule {
                rule.dependencies
                    .retain(|dependency| dependency.name != name);
            }
        }
    }

    /// An inspection updates just the head record, preserving the later
    /// double-colon branches for their own expansion and command contexts.
    fn store_inspected_rule(&mut self, target: &[u8], double_colon: bool, rule: Rule) {
        if double_colon {
            self.invalidate_phony_names(target);
            if let Some(first) = self
                .database
                .rules
                .get_mut(target)
                .and_then(|root| root.double_colon.as_mut())
                .and_then(|branches| branches.first_mut())
            {
                *first = rule;
            }
        } else {
            self.store_expanded_rule(target, rule);
        }
    }

    fn store_expanded_rule(&mut self, target: &[u8], rule: Rule) {
        self.invalidate_phony_names(target);
        if let Some(id) = rule.grouped_double_id() {
            let ordinal = rule.grouped_double_ordinal();
            if let Some(branch) = self
                .database
                .rules
                .get_mut(target)
                .and_then(|root| root.double_colon.as_mut())
                .and_then(|branches| {
                    branches.iter_mut().find(|branch| {
                        branch.grouped_double_id() == Some(id)
                            && branch.grouped_double_ordinal() == ordinal
                    })
                })
            {
                *branch = rule;
            }
        } else {
            self.database.rules.insert(target.to_vec(), rule);
        }
    }

    /// LOW_RESOLUTION_TIME changes only the target-side comparison. Retain
    /// actual metadata for prerequisite outcomes and changed-file cleanup.
    fn comparison_time(&self, target: &[u8], before: Option<SystemTime>) -> Option<SystemTime> {
        let time = before?;
        if rmake_core::filenames::archive_parts(target).is_none()
            && !self.special(b".LOW_RESOLUTION_TIME", target, false)
        {
            return before;
        }
        let nanos = match time.duration_since(SystemTime::UNIX_EPOCH) {
            Ok(duration) => duration.subsec_nanos(),
            Err(error) => (1_000_000_000 - error.duration().subsec_nanos()) % 1_000_000_000,
        };
        if nanos != 0 {
            self.stderr(&crate::messages::system(
                rmake_core::messages::Message::LowResolutionTime(target),
            ));
        }
        time.checked_add(std::time::Duration::from_nanos(u64::from(
            999_999_999 - nanos,
        )))
        .or(before)
    }

    #[allow(clippy::too_many_arguments)]
    fn build_rule(
        &mut self,
        target: &[u8],
        depth: usize,
        before: Option<SystemTime>,
        mut comparison: Option<SystemTime>,
        phony: bool,
        mut rule: Rule,
        branch: Option<usize>,
        replay_cached_after_recipe: bool,
    ) -> Outcome {
        let double_colon = branch.is_some();
        let mut members = Vec::new();
        let mut peer_targets = std::collections::VecDeque::new();
        let grouped = rule.grouped_targets().is_some();
        if let Some(group) = &rule.group {
            let implicit = rule.implicit_pattern.is_some();
            if !implicit {
                comparison = comparison.min(before);
            }
            let double = rule.grouped_double_id().is_some();
            let mut peers: Vec<_> = group
                .targets
                .iter()
                .map(|name| (Cow::Borrowed(name.as_slice()), None, false))
                .collect();
            let mut compare_peers = before.is_some() && !implicit;
            for (peer, time, observed) in peers.iter_mut().rev() {
                let name = self.aliases.resolve(peer);
                if name != peer.as_ref() {
                    *peer = Cow::Owned(name.to_vec());
                }
                if peer.as_ref() == target {
                    *time = before;
                    *observed = true;
                    continue;
                }
                if !compare_peers {
                    continue;
                }
                match self.prepare_search_name(peer) {
                    Ok(Some(name)) => *peer = Cow::Owned(name),
                    Ok(None) => {}
                    Err(()) => {
                        return Outcome {
                            failed: true,
                            fatal: true,
                            ..Outcome::default()
                        };
                    }
                }
                *time = if self.special(b".PHONY", peer, false) {
                    None
                } else {
                    match self
                        .path_observations
                        .get(peer.as_ref())
                        .cloned()
                        .map(Ok)
                        .unwrap_or_else(|| self.find_vpath(peer, true))
                    {
                        Ok(observation) => observation.and_then(|(_, time)| time),
                        Err(error) => {
                            self.expansion_error(None, &error);
                            return Outcome {
                                failed: true,
                                fatal: true,
                                ..Outcome::default()
                            };
                        }
                    }
                };
                *observed = true;
                comparison = comparison.min(*time);
                compare_peers = time.is_some();
                if !compare_peers {
                    crate::debug::emit(
                        &self.output,
                        crate::debug::Event::GroupedPeerMissing(
                            peer,
                            target,
                            depth + usize::from(self.remaking) + 1,
                        ),
                    );
                }
            }
            // Resolve scheduler peer identities without rewriting declaration
            // names, which the database exposes even after a GPATH rename.
            let mut seen = HashMap::<&[u8], usize>::new();
            for (peer, time, observed) in &peers {
                let peer = peer.as_ref();
                let ordinal = seen.entry(peer).or_default();
                if *ordinal == 0 {
                    let mut before = *time;
                    if double && *observed {
                        before = *self
                            .double_colon_before
                            .entry(peer.to_vec())
                            .or_insert(before);
                    }
                    members.push((peer.to_vec(), before, self.special(b".PHONY", peer, false)));
                }
                if !implicit || peer != target {
                    peer_targets.push_front((peer.to_vec(), if double { *ordinal } else { 0 }));
                }
                *ordinal += 1;
            }
        }
        let had_secondary = rule.secondary.is_some();
        if let Err(error) = self.expand_dependencies(target, &mut rule, branch) {
            self.expansion_error(rule.prerequisite_location.as_ref(), &error);
            let result = Outcome {
                failed: true,
                fatal: true,
                ..Outcome::default()
            };
            self.done.insert(target.to_vec(), result);
            return result;
        }
        if grouped {
            // Expansion is per file, not once for every peer traversal.
            self.store_expanded_rule(target, rule.clone());
        }
        let group = rule.grouped_targets().map(|_| {
            Box::new(GroupState {
                recipe_rule: rule.clone(),
                peer_segments: vec![GroupSegment {
                    start: 0,
                    target: target.to_vec(),
                    ordinal: rule.grouped_double_ordinal(),
                }],
                peer_targets,
                wait_frontiers: Vec::new(),
                frontiers_enabled: rule.grouped_double_id().is_some()
                    || rule
                        .dependencies
                        .iter()
                        .any(|dependency| dependency.name == b".WAIT"),
                run: GroupRun {
                    double_colon: rule
                        .grouped_double_id()
                        .map(|id| (id, branch.expect("grouped double-colon owner branch"))),
                    ordinary: !self.remaking && self.active_makefile_remake.is_none(),
                    implicit: rule.implicit_pattern.is_some(),
                    location: rule.recipe_location.clone(),
                    before,
                    members,
                },
            })
        });
        let stale = comparison.is_none()
            || self.options.always
            || (double_colon && rule.dependencies.is_empty());
        let first_branch = self.shuffle.enabled()
            && double_colon
            && self
                .database
                .rules
                .get(target)
                .and_then(|root| root.double_colon.as_ref())
                .and_then(|branches| branches.first())
                .is_some_and(|first| {
                    first.recipe_location == rule.recipe_location
                        && (first.recipe_location.is_some()
                            || first.prerequisite_location == rule.prerequisite_location)
                });
        let order = if double_colon && !first_branch && !had_secondary {
            Vec::new()
        } else {
            self.shuffle.order(target, rule.dependencies.len())
        };
        let state = RuleState {
            remake: self.active_makefile_remake.clone(),
            branch,
            rule,
            group,
            comparison,
            last_epoch: None,
            depth,
            before,
            phony,
            next: 0,
            order,
            inspections: HashMap::new(),
            intermediate_stale: None,
            debug_changed_edges: None,
            waited_for_prerequisites: false,
            replay_cached_after_recipe,
            last_debug_pass: self.debug_pass,
            stopped: false,
            stale,
            outcome: Outcome {
                time: before,
                ..Outcome::default()
            },
            dropped: HashSet::new(),
            presentation_drops: None,
            deferred_intermediates: Vec::new(),
        };
        // A leaf cannot suspend dependency traversal. Keep the same recipe
        // finalization path, without inserting and removing a graph frame.
        if state.rule.dependencies.is_empty()
            && state
                .group
                .as_ref()
                .is_none_or(|group| group.peer_targets.is_empty())
        {
            return self.finish_rule(target, state);
        }
        self.pending_rules.insert(target.to_vec(), state);
        let outcome = self.resume_rule(target);
        if self.parallel_graph() {
            outcome
        } else {
            self.wait_target(target);
            self.done.get(target).copied().unwrap_or(outcome)
        }
    }

    /// Advance every currently reachable branch until a local barrier or its
    /// unfinished prerequisites require a yield. A suspended node is not on the
    /// DFS ancestry stack: shared pending prerequisites are not circular edges.
    fn resume_rule(&mut self, target: &[u8]) -> Outcome {
        // Shared subgraphs can be reached through many parents in one pass.
        // Reconsider a blocked frame only after a completion or a new outer
        // pass, never once per incoming edge (which can grow exponentially).
        if let Some(state) = self.pending_rules.get(target)
            && state.last_epoch == Some(self.graph_epoch)
        {
            return state.outcome;
        }
        let state = self
            .pending_rules
            .remove(target)
            .expect("pending rule frame");
        self.with_makefile_remake(state.remake.clone(), |engine| {
            engine.resume_rule_state(target, state)
        })
    }

    fn resume_rule_state(&mut self, target: &[u8], mut state: RuleState) -> Outcome {
        // The cached prefix can publish its next goal pass before opening
        // a .WAIT barrier or finishing the dependency traversal.
        let remake_verbose = self.remaking
            && self.parallel_makefile_remakes
            && self.output.debug_mask().has(crate::debug::Mask::VERBOSE);
        let can_replay_debug = (!self.remaking || remake_verbose)
            && state.waited_for_prerequisites
            && (state.visited_prerequisites()
                || state.visited_group_prerequisites()
                || state
                    .group
                    .as_ref()
                    .is_some_and(|group| !group.wait_frontiers.is_empty())
                || state.waiting_at_barrier());
        let replay_debug = can_replay_debug
            && state.last_debug_pass != self.debug_pass
            && self.graph_abort.is_none()
            && self.shutdown.get() == Shutdown::Running;
        state.replay_cached_after_recipe = (state.replay_cached_after_recipe
            && state.last_debug_pass == self.debug_pass)
            || (replay_debug
                && !self.remaking
                && (state.branch.is_some()
                    || state.group.as_ref().is_some_and(|group| group.run.implicit))
                && state.waiting_at_barrier());
        state.last_debug_pass = self.debug_pass;
        let debug_depth = state.depth + usize::from(self.remaking);
        if replay_debug {
            if let Some(ordinal) = state.branch
                && !self.remaking
            {
                self.debug_completed_double_colon_declarations(target, 0..ordinal, debug_depth);
            }
            self.debug_consider(target, target, debug_depth);
        }
        if state.last_epoch.is_some()
            && state.before.is_none()
            && (!can_replay_debug || replay_debug)
        {
            crate::debug::emit(
                &self.output,
                crate::debug::Event::Missing(target, debug_depth + 1),
            );
        }
        state.last_epoch = Some(self.graph_epoch);
        self.apply_graph_abort(&mut state.outcome);
        state.stopped |= self.graph_abort.is_some() || self.remake_stopped();
        self.visiting.insert(target.to_vec());
        let mut waiting = false;
        let previous_next = state.next;
        let previous_frontiers = state
            .group
            .as_mut()
            .map(|group| std::mem::take(&mut group.wait_frontiers))
            .unwrap_or_default();
        let mut group_frontier = !previous_frontiers.is_empty();
        let mut scan_group = group_frontier
            || (!self.remaking
                && self.parallel_graph()
                && state
                    .group
                    .as_ref()
                    .is_some_and(|group| group.frontiers_enabled));
        let mut group_waiting = false;
        let mut segment_index = 0;
        if group_frontier {
            state.next = 0;
            state.replay_cached_after_recipe |= replay_debug && !self.remaking;
        }
        let mut resume_visit = if replay_debug {
            PrerequisiteVisit::DebugReplay
        } else {
            PrerequisiteVisit::Resume
        };
        while !state.stopped && self.graph_abort.is_none() && !self.remake_stopped() {
            if state.next == state.rule.dependencies.len() {
                let Some((peer, ordinal)) = state
                    .group
                    .as_mut()
                    .and_then(|group| group.peer_targets.pop_front())
                else {
                    break;
                };
                let (mut peer_rule, peer_branch) = self
                    .database
                    .rules
                    .get(&peer)
                    .and_then(|root| {
                        if let Some(id) = state.rule.grouped_double_id() {
                            root.double_colon.as_ref()?.iter().enumerate().find_map(
                                |(branch_index, branch)| {
                                    (branch.grouped_double_id() == Some(id)
                                        && branch.grouped_double_ordinal() == ordinal)
                                        .then(|| (branch.clone(), Some(branch_index)))
                                },
                            )
                        } else {
                            Some((root.clone(), None))
                        }
                    })
                    .unwrap_or_default();
                // Expanding a grouped peer uses its own scope. Only the
                // prerequisites visited below inherit the recipe owner's scope.
                if let Err(error) = self.expand_dependencies(&peer, &mut peer_rule, peer_branch) {
                    self.expansion_error(peer_rule.prerequisite_location.as_ref(), &error);
                    state.outcome.failed = true;
                    state.outcome.fatal = true;
                    state.stopped = true;
                    break;
                }
                let group = state
                    .group
                    .as_mut()
                    .expect("peer traversal has group state");
                if !group.frontiers_enabled
                    && peer_rule
                        .dependencies
                        .iter()
                        .any(|dep| dep.name == b".WAIT")
                {
                    group.frontiers_enabled = true;
                    scan_group |= !self.remaking && self.parallel_graph();
                }
                group.peer_segments.push(GroupSegment {
                    start: state.rule.dependencies.len(),
                    target: peer.clone(),
                    ordinal,
                });
                let peer_order = self.shuffle.order(&peer, peer_rule.dependencies.len());
                let offset = state.rule.dependencies.len();
                if !peer_order.is_empty() && state.order.is_empty() {
                    state.order.extend(0..offset);
                }
                if !state.order.is_empty() {
                    if peer_order.is_empty() {
                        state
                            .order
                            .extend(offset..offset + peer_rule.dependencies.len());
                    } else {
                        state
                            .order
                            .extend(peer_order.into_iter().map(|index| offset + index));
                    }
                }
                state
                    .rule
                    .dependencies
                    .extend(peer_rule.dependencies.clone());
                self.store_expanded_rule(&peer, peer_rule);
                continue;
            }
            let index = state.order.get(state.next).copied().unwrap_or(state.next);
            if (self.database.has_vpaths() || !self.vpath_dirs.is_empty())
                && state.rule.dependencies[index].name != b".WAIT"
            {
                match self.prepare_search_name(&state.rule.dependencies[index].name) {
                    Ok(Some(name)) => state.rule.dependencies[index].name = name,
                    Ok(None) => {}
                    Err(()) => {
                        state.outcome.failed = true;
                        state.outcome.fatal = true;
                        state.stopped = true;
                        break;
                    }
                }
            }
            let occurrence = &state.rule.dependencies[index];
            let dependency = &occurrence.name;
            if dependency == b".WAIT"
                && scan_group
                && let Some(group) = state.group.as_mut()
            {
                while group
                    .peer_segments
                    .get(segment_index + 1)
                    .is_some_and(|segment| segment.start <= state.next)
                {
                    segment_index += 1;
                }
                if group_waiting {
                    // Defer this segment's tail while other peers can still
                    // reach their own prerequisite barrier.
                    let end = group
                        .peer_segments
                        .get(segment_index + 1)
                        .map_or(state.rule.dependencies.len(), |segment| segment.start);
                    group.wait_frontiers.push(state.next..end);
                    state.next = end;
                    group_frontier = true;
                } else {
                    state.next += 1;
                }
                continue;
            }
            if dependency == b".WAIT" {
                // Only the first prefix scan is a visit in this goal pass.
                let visit = std::mem::replace(&mut resume_visit, PrerequisiteVisit::Resume);
                for position in 0..state.next {
                    let index = state.order.get(position).copied().unwrap_or(position);
                    let prior = &state.rule.dependencies[index];
                    if prior.name == b".WAIT" || state.dropped.contains(&prior.name) {
                        continue;
                    }
                    let (child, pending) =
                        self.visit_prerequisite(target, &mut state, index, visit);
                    waiting |= pending;
                    merge_child(&mut state.outcome, child);
                }
                if waiting {
                    break;
                }
                state.next += 1;
                if state.outcome.fatal || (state.outcome.failed && !self.options.keep_going) {
                    state.stopped = true;
                }
                continue;
            }
            let visited = !previous_frontiers.is_empty()
                && state.next < previous_next
                && !previous_frontiers
                    .iter()
                    .any(|range| range.contains(&state.next));
            state.next += 1;
            if visited && state.dropped.contains(dependency) {
                continue;
            }
            if !visited && self.visiting.contains(dependency) {
                if self.shuffle.enabled() {
                    state
                        .presentation_drops
                        .get_or_insert_with(Default::default)
                        .insert(state.next - 1);
                }
                self.warn_circular_dependency(target, dependency);
                if let Some(segment) = state.group.as_ref().and_then(|group| {
                    group
                        .peer_segments
                        .iter()
                        .rev()
                        .find(|segment| segment.start < state.next)
                }) {
                    self.drop_group_dependency(
                        &segment.target,
                        state.rule.grouped_double_id(),
                        segment.ordinal,
                        dependency,
                    );
                }
                state.dropped.insert(dependency.clone());
                continue;
            }
            let visit = if visited {
                resume_visit
            } else {
                PrerequisiteVisit::First
            };
            let (child, pending) = self.visit_prerequisite(target, &mut state, index, visit);
            group_waiting |= pending;
            merge_child(&mut state.outcome, child);
            let deferred = child.deferred_spawn_failure && self.optional_remake();
            if child.fatal
                || (((child.failed && !deferred) || child.question_outdated)
                    && !self.options.keep_going)
            {
                state.stopped = true;
            }
        }
        // Revisit suspended children in traversal order without replaying the
        // parent's dependency expansion, circular diagnostics or scope setup.
        if group_frontier {
            waiting |= group_waiting;
        } else {
            for position in 0..state.next {
                let index = state.order.get(position).copied().unwrap_or(position);
                let dependency = &state.rule.dependencies[index].name;
                if dependency == b".WAIT" || state.dropped.contains(dependency) {
                    continue;
                }
                let (child, pending) =
                    self.visit_prerequisite(target, &mut state, index, resume_visit);
                merge_child(&mut state.outcome, child);
                waiting |= pending;
                if self.remaking
                    && resume_visit == PrerequisiteVisit::DebugReplay
                    && child.failed
                    && self.remake_stopped()
                {
                    // A stopped optional parent reports through its first
                    // failed child while later frames still settle internally.
                    resume_visit = PrerequisiteVisit::Resume;
                }
            }
        }
        if !waiting {
            // Staleness only needs an immutable membership test, so repeated
            // normal occurrences need no canonical list or cloned names here.
            state.stale |= state.rule.normal_prerequisites().any(|name| {
                !state.dropped.contains(name)
                    && self
                        .done
                        .get(name)
                        .is_some_and(|child| self.prerequisite_stale(state.comparison, *child))
            });
            self.apply_graph_abort(&mut state.outcome);
            if state.stale && !state.outcome.failed && !state.outcome.question_outdated {
                for dependency in &state.deferred_intermediates {
                    let child = self.build(
                        dependency,
                        Some(state.scope_parent(target)),
                        state.depth + 1,
                    );
                    merge_child(&mut state.outcome, child);
                    waiting |= self.pending(dependency);
                }
            }
        }
        if waiting {
            if !state.waited_for_prerequisites || replay_debug {
                state.waited_for_prerequisites = true;
                crate::debug::emit(
                    &self.output,
                    crate::debug::Event::PrerequisitesRunning(target, debug_depth),
                );
            }
            self.visiting.remove(target);
            let outcome = state.outcome;
            self.pending_rules.insert(target.to_vec(), state);
            return outcome;
        }
        self.finish_rule(target, state)
    }

    /// An inspection is a per-parent timestamp question, not an actual update
    /// of the missing file. Keep it out of `done` so a different parent can ask
    /// against its own timestamp or require the intermediate to materialize.
    fn visit_prerequisite(
        &mut self,
        target: &[u8],
        state: &mut RuleState,
        index: usize,
        visit: PrerequisiteVisit,
    ) -> (Outcome, bool) {
        let first = visit == PrerequisiteVisit::First;
        if let Some((epoch, outcome)) = state.inspections.get(&index)
            && *epoch == self.graph_epoch
        {
            return (*outcome, true);
        }
        let occurrence = &state.rule.dependencies[index];
        let dependency = &occurrence.name;
        let parent = state.scope_parent(target);
        if self.remake_stopped() {
            // Settle the stopped owner's graph while retaining live recipes
            // for the ordinary drain. Shared frames keep their own owner.
            let resolved = self.aliases.resolve(dependency);
            let owns_frame = self.remake_stop().is_some_and(|stop| {
                let same_root = |remake: Option<&MakefileRemake>| {
                    remake
                        .and_then(|remake| remake.stopped.as_ref())
                        .is_some_and(|other| Rc::ptr_eq(stop, other))
                };
                self.pending_rules
                    .get(resolved)
                    .is_some_and(|pending| same_root(pending.remake.as_ref()))
                    || self
                        .pending_double_colon
                        .get(resolved)
                        .is_some_and(|pending| same_root(pending.remake.as_ref()))
            });
            let outcome = if owns_frame {
                self.build(dependency, Some(parent), state.depth + 1)
            } else {
                let outcome = self.done.get(resolved).copied().unwrap_or_default();
                if self.remaking && outcome.failed && visit == PrerequisiteVisit::DebugReplay {
                    self.debug_cached_prerequisite_visit(dependency, state.depth + 2, outcome);
                }
                outcome
            };
            return (outcome, false);
        }
        let missing_middle = if first && self.intermediate(dependency) {
            match self.intermediate_time(dependency) {
                Ok(time) => time.is_none(),
                Err(()) => {
                    return (
                        Outcome {
                            failed: true,
                            fatal: true,
                            ..Outcome::default()
                        },
                        false,
                    );
                }
            }
        } else {
            false
        };
        let inspect = state.inspections.contains_key(&index) || missing_middle;
        if inspect {
            let epoch = self.graph_epoch;
            let Inspection {
                outcome: child,
                pending,
                needs_rebuild,
            } = self.check_intermediate(dependency, parent, state.comparison, state.depth + 1);
            if needs_rebuild {
                state
                    .intermediate_stale
                    .get_or_insert_default()
                    .insert(index);
                state.stale |= !occurrence.order_only;
            }
            if pending {
                state.inspections.insert(index, (epoch, child));
            } else {
                state.inspections.remove(&index);
                if !self.done.contains_key(dependency) {
                    state.deferred_intermediates.push(dependency.clone());
                }
                if !occurrence.order_only && (child.time.is_none() || child.time > state.comparison)
                {
                    state.stale = true;
                }
            }
            (child, pending)
        } else {
            // Only the edge that actually saw an update can report the
            // changed timestamp independently of the final freshness test.
            let observe_change = first
                && self.output.debug_mask().has(crate::debug::Mask::BASIC)
                && !self.done.contains_key(dependency)
                && !self.parallel_graph();
            let unknown_archive = self
                .done
                .get(dependency)
                .is_some_and(|child| child.completion == CompletionTime::ArchiveUnknown)
                && rmake_core::filenames::archive_parts(dependency).is_some();
            let child = if first
                || self.graph_pending(dependency)
                || unknown_archive
                || self.cached_double_colon_input_due(dependency)
            {
                let build_visit = if first && state.replay_cached_after_recipe {
                    BuildVisit::PrerequisiteAfterRecipe
                } else if !first
                    && !unknown_archive
                    && state.branch.is_none()
                    && state.visited_prerequisites()
                {
                    BuildVisit::RevisitPrerequisite
                } else {
                    BuildVisit::Ordinary
                };
                self.build_recording_initial_time(
                    dependency,
                    Some(parent),
                    state.depth + 1,
                    None,
                    build_visit,
                    None,
                )
            } else {
                let outcome = self.done.get(dependency).copied();
                // A suspended required frame can first see a shared child's
                // failed completion here, without entering the builder again.
                if self.parallel_makefile_remakes
                    && !self.optional_remake()
                    && self.graph_abort.is_none()
                    && self.shutdown.get() == Shutdown::Running
                    && let Some(outcome) =
                        outcome.filter(|outcome| outcome.failed && !outcome.fatal)
                    && let Some(cause) = self.optional_remake_failures.remove(dependency)
                {
                    self.show_retained_optional_failure(dependency, Some(parent.target), cause);
                    self.record_graph_failure(outcome);
                }
                if visit == PrerequisiteVisit::DebugReplay
                    && let Some(outcome) = outcome.or_else(|| {
                        // A running peer has no cached outcome until its owner finishes.
                        (!self.remaking && self.ordinary_group_peer_pending(dependency))
                            .then(Outcome::default)
                    })
                {
                    self.debug_cached_prerequisite_visit(
                        dependency,
                        state.depth + usize::from(self.remaking) + 1,
                        outcome,
                    );
                }
                outcome.unwrap_or_default()
            };
            let pending = self.pending(dependency);
            let child = if pending {
                child
            } else {
                self.observe_updated(dependency, child)
            };
            if observe_change && !pending && child.debug_time_changed {
                state
                    .debug_changed_edges
                    .get_or_insert_default()
                    .insert(index);
            }
            if first
                && child.completion == CompletionTime::ArchiveUnknown
                && rmake_core::filenames::archive_parts(dependency).is_some()
            {
                state.stale |= !occurrence.order_only;
            }
            (child, pending)
        }
    }

    fn prerequisite_stale(&self, before: Option<SystemTime>, child: Outcome) -> bool {
        self.options.always || self.prerequisite_timestamp_stale(before, child)
    }

    fn prerequisite_timestamp_stale(&self, before: Option<SystemTime>, child: Outcome) -> bool {
        if child.completion == CompletionTime::AssumeNew
            && self
                .assumptions
                .as_ref()
                .is_some_and(|files| files.is_new_time(before))
        {
            // A non-executed ordinary prerequisite gets the same logical
            // newest time; it cannot be newer than an explicitly newest parent.
            return false;
        }
        matches!(
            child.completion,
            CompletionTime::AssumeNew | CompletionTime::AssumeNewPhony
        ) || before.is_none()
            || child.time.is_none()
            || child.time > before
    }

    fn finish_rule(&mut self, target: &[u8], state: RuleState) -> Outcome {
        if self.parallel_graph()
            && let Some(id) = state.rule.grouped_double_id()
        {
            // Two peers may have suspended on the same prerequisite before
            // either selected a recipe. Recheck ownership after that wait.
            if let Some(mut result) = self.completed_group_branches.get(&id).copied() {
                result.ran = false;
                result.time = if state.phony {
                    None
                } else {
                    self.modified(target)
                };
                self.visiting.remove(target);
                self.done.insert(target.to_vec(), result);
                return result;
            }
            if self.active_group_branches.contains(&id) {
                self.visiting.remove(target);
                let outcome = state.outcome;
                self.pending_rules.insert(target.to_vec(), state);
                return outcome;
            }
        }
        let RuleState {
            branch,
            mut rule,
            mut group,
            comparison,
            before,
            phony,
            mut stale,
            mut outcome,
            dropped,
            presentation_drops,
            intermediate_stale,
            debug_changed_edges,
            deferred_intermediates,
            depth,
            ..
        } = state;
        if !self.aliases.is_empty() {
            for dependency in &mut rule.dependencies {
                let name = self.aliases.resolve(&dependency.name);
                if name != dependency.name {
                    dependency.name = name.to_vec();
                }
            }
            if let Some(group) = &mut group {
                for dependency in &mut group.recipe_rule.dependencies {
                    let name = self.aliases.resolve(&dependency.name);
                    if name != dependency.name {
                        dependency.name = name.to_vec();
                    }
                }
            }
        }
        let recipe_rule = group.as_ref().map_or(&rule, |group| &group.recipe_rule);
        let display_rule = presentation_drops.as_ref().map(|positions| {
            let mut display = recipe_rule.clone();
            let mut ordinal = 0;
            display.dependencies.retain(|_| {
                let retain = !positions.contains(&ordinal);
                ordinal += 1;
                retain
            });
            display
        });
        let recipe_rule = display_rule.as_ref().unwrap_or(recipe_rule);
        if group.is_none()
            && !dropped.is_empty()
            && let Some(captured) = self.expanded_prerequisites_mut(target, branch)
        {
            if let Some(positions) = &presentation_drops {
                let mut index = 0;
                captured.retain(|_| {
                    let retain = !positions.contains(&index);
                    index += 1;
                    retain
                });
            } else {
                captured.retain(|dependency| !dropped.contains(&dependency.name));
            }
        }
        let automatic_drops = display_rule.is_none().then_some(&dropped);
        // Ungrouped rules without a nonempty recipe need only normal-name
        // membership for freshness. Keep grouped canonical views separate from
        // peer edges. Empty recipe vectors never read automatic bindings.
        let capture_automatic = group.is_some()
            || rule
                .recipes
                .as_ref()
                .is_some_and(|recipes| !recipes.is_empty());
        let canonical = if capture_automatic {
            recipe_rule.automatic_dependencies()
        } else {
            Vec::new()
        };
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
        self.apply_graph_abort(&mut outcome);
        let has_hidden = rule.dependencies.iter().any(|dependency| dependency.hidden);
        let visible_normal =
            has_hidden.then(|| normal.iter().map(Vec::as_slice).collect::<HashSet<_>>());
        let normal_names: HashSet<_> = if has_hidden || !capture_automatic {
            rule.normal_prerequisites().collect()
        } else {
            normal.iter().map(Vec::as_slice).collect()
        };
        let mut newer = Vec::new();
        for (index, occurrence) in rule.dependencies.iter().enumerate() {
            let dependency = &occurrence.name;
            if dependency == b".WAIT" || dropped.contains(dependency) {
                continue;
            }
            if !self.remake_stopped() {
                self.wait_target(dependency);
            }
            let Some(child) = self.done.get(dependency).copied() else {
                continue;
            };
            merge_child(&mut outcome, child);
            let order_only = !normal_names.contains(dependency.as_slice());
            let inspected_stale = intermediate_stale
                .as_ref()
                .is_some_and(|edges| edges.contains(&index));
            if !order_only {
                stale |= inspected_stale;
                // GNU's serial path retains the missing-middle changed bit;
                // parallel completion projects $? from the final timestamp.
                if self.prerequisite_stale(comparison, child)
                    || (inspected_stale && !self.parallel_graph())
                {
                    stale = true;
                    if capture_automatic
                        && !occurrence.hidden
                        && visible_normal
                            .as_ref()
                            .is_none_or(|names| names.contains(dependency.as_slice()))
                    {
                        newer.push(dependency.clone());
                    }
                }
            }
        }
        if self.retain_remake_failure()
            && (outcome.blocked || outcome.question_outdated)
            && !outcome.fatal
        {
            self.remember_optional_prerequisite(target, &rule, Some(&dropped));
        }
        let debug_depth = depth + usize::from(self.remaking);
        // A stopped graph still settles its blocked frames internally.
        let stopping_dependency = outcome.blocked
            && (outcome.fatal
                || (!self.options.keep_going
                    && !self.optional_remake()
                    && (self.graph_abort.is_some() || !self.parallel_graph())));
        if !stopping_dependency {
            crate::debug::emit(
                &self.output,
                crate::debug::Event::Finished(target, debug_depth),
            );
        }
        let mask = self.output.debug_mask();
        let mut natural_remake = comparison.is_none();
        let mut dependencies_changed = false;
        if mask.has(crate::debug::Mask::BASIC)
            && before.is_some()
            && !phony
            && !outcome.failed
            && !outcome.question_outdated
        {
            // Peer segments affect the rebuild decision. The trace lists
            // only the selected file's original prerequisite occurrences.
            let owner_end = group.as_ref().map_or(rule.dependencies.len(), |group| {
                group.recipe_rule.dependencies.len()
            });
            // A deferred intermediate can still be missing when its parent
            // is current. Its inspection did not produce a completion record.
            let deferred: HashSet<_> = deferred_intermediates
                .iter()
                .map(|name| self.aliases.resolve(name))
                .collect();
            for (index, occurrence) in rule.dependencies.iter().enumerate() {
                let dependency = &occurrence.name;
                if dependency == b".WAIT" || dropped.contains(dependency) {
                    continue;
                }
                let child = self.done.get(dependency).copied();
                if child.is_none() && !deferred.contains(dependency.as_slice()) {
                    continue;
                }
                let changed = debug_changed_edges
                    .as_ref()
                    .is_some_and(|edges| edges.contains(&index));
                let inspected_stale = intermediate_stale
                    .as_ref()
                    .is_some_and(|edges| edges.contains(&index));
                if let Some(child) = child
                    && !occurrence.order_only
                {
                    dependencies_changed |= changed;
                    if self.options.always {
                        natural_remake |=
                            inspected_stale || self.prerequisite_timestamp_stale(comparison, child);
                    }
                }
                // A missing peer makes the whole group absent for the trace.
                if comparison.is_none() || index >= owner_end {
                    continue;
                }
                use crate::debug::Prerequisite;
                let state = match child {
                    _ if occurrence.order_only => Prerequisite::OrderOnly,
                    Some(child) if child.time.is_some() => {
                        if changed
                            || child.time > comparison
                            || (child.completion == CompletionTime::AssumeNew
                                && self.prerequisite_stale(comparison, child))
                            || inspected_stale
                        {
                            Prerequisite::Newer
                        } else {
                            Prerequisite::Older
                        }
                    }
                    _ => Prerequisite::Missing,
                };
                crate::debug::emit(
                    &self.output,
                    crate::debug::Event::Prerequisite(
                        state,
                        self.resolved_name_ref(dependency),
                        target,
                        debug_depth + 1,
                    ),
                );
            }
        }
        let prerequisite_question = outcome.question_outdated;
        let debug_remake = stale && !outcome.failed && !prerequisite_question;
        let double_without_prerequisites = branch.is_some() && rule.dependencies.is_empty();
        let no_recipe_no_changes = mask.has(crate::debug::Mask::BASIC)
            && !outcome.failed
            && !outcome.question_outdated
            && !double_without_prerequisites
            && before.is_some()
            && !phony
            && !rule.record_only
            && rule.recipes.is_none()
            && !self.options.always
            && !dependencies_changed;
        if debug_remake && double_without_prerequisites {
            crate::debug::emit(
                &self.output,
                crate::debug::Event::DoubleWithoutPrerequisites(target, debug_depth),
            );
        } else if no_recipe_no_changes {
            crate::debug::emit(
                &self.output,
                crate::debug::Event::NoRecipeNoChanges(target, debug_depth),
            );
        } else if debug_remake && self.options.always && rule.recipes.is_some() && !natural_remake {
            crate::debug::emit(
                &self.output,
                crate::debug::Event::AlwaysMake(target, debug_depth),
            );
        }
        if debug_remake {
            if !no_recipe_no_changes {
                crate::debug::emit(
                    &self.output,
                    crate::debug::Event::MustRemake(target, debug_depth),
                );
            }
            if let Some(recipes) = &rule.recipes {
                let normal: Vec<_> = normal
                    .iter()
                    .filter(|name| automatic_drops.is_none_or(|set| !set.contains(*name)))
                    .cloned()
                    .collect();
                let order: Vec<_> = order
                    .iter()
                    .filter(|name| automatic_drops.is_none_or(|set| !set.contains(*name)))
                    .cloned()
                    .collect();
                let repeated: Vec<_> = recipe_rule
                    .dependencies
                    .iter()
                    .filter(|dep| !dep.hidden && !dep.order_only && dep.name != b".WAIT")
                    .map(|dep| dep.name.as_slice())
                    .filter(|name| automatic_drops.is_none_or(|set| !set.contains(*name)))
                    .map(Vec::from)
                    .collect();
                if let Some(discarded) = self.resolved_names.remove(target) {
                    // A rebuild has selected the local name permanently for
                    // this file. A missing result must stay missing.
                    self.ignored_vpath.insert(target.to_vec());
                    crate::debug::emit(
                        &self.output,
                        crate::debug::Event::IgnoringVpath(&discarded),
                    );
                }
                let normal: Vec<_> = normal.iter().map(|name| self.resolved_name(name)).collect();
                let order: Vec<_> = order.iter().map(|name| self.resolved_name(name)).collect();
                let repeated: Vec<_> = repeated
                    .iter()
                    .map(|name| self.resolved_name(name))
                    .collect();
                let newer: Vec<_> = newer.iter().map(|name| self.resolved_name(name)).collect();
                let autos = (!recipes.is_empty()).then(|| {
                    [
                        (b"@".as_slice(), target_variable_names(target).0.to_vec()),
                        (
                            b"<",
                            if rule.default_recipe || target == b".DEFAULT" {
                                target_variable_names(target).0.to_vec()
                            } else {
                                repeated.first().cloned().unwrap_or_default()
                            },
                        ),
                        (
                            b"^",
                            joined(normal.iter().map(|name| prerequisite_variable_name(name))),
                        ),
                        (
                            b"+",
                            joined(repeated.iter().map(|name| prerequisite_variable_name(name))),
                        ),
                        (
                            b"?",
                            joined(
                                unique(&newer)
                                    .iter()
                                    .map(|name| prerequisite_variable_name(name)),
                            ),
                        ),
                        (
                            b"|",
                            joined(order.iter().map(|name| prerequisite_variable_name(name))),
                        ),
                        (
                            b"*",
                            rule.stem.clone().unwrap_or_else(|| {
                                self.database
                                    .explicit_stem(prerequisite_variable_name(target))
                                    .to_vec()
                            }),
                        ),
                        (b"%", target_variable_names(target).1.to_vec()),
                    ]
                });
                let recursive = recipes.iter().any(recipe_is_recursive);
                if (!self.options.touch || recursive)
                    && let Some(group) = group
                {
                    let group = group.run;
                    if let Some((id, _)) = group.double_colon {
                        self.active_group_branches.insert(id);
                    } else {
                        for (peer, _, _) in &group.members {
                            if peer != target {
                                self.active_peers.insert(peer.clone(), target.to_vec());
                            }
                        }
                    }
                    self.running_groups.insert(target.to_vec(), group);
                }
                self.archive_completion_times.remove(target);
                let implicit_group =
                    rule.grouped_targets().is_some() && rule.implicit_pattern.is_some();
                let execution = self.execute(
                    target,
                    recipes,
                    autos
                        .as_ref()
                        .map(|values| values.as_slice())
                        .unwrap_or(&[]),
                    RecipeInputs {
                        phony,
                        before,
                        branch,
                        implicit_group,
                        location: recipes.first().map(|recipe| &recipe.location),
                        dependencies: &rule.dependencies,
                        dropped: &dropped,
                        normal_names: &normal_names,
                    },
                );
                if execution.pending {
                    crate::debug::emit(
                        &self.output,
                        crate::debug::Event::RecipeRunning(target, debug_depth),
                    );
                }
                outcome.ran |= execution.ran;
                // A pending remake takes its completion mode from the reap.
                outcome.completion =
                    if (self.options.dry_run || self.options.question || self.options.touch)
                        && !execution.all_recursive
                        && (!self.remaking || !execution.pending)
                    {
                        if rule.grouped_targets().is_none() || execution.ran {
                            if phony {
                                CompletionTime::AssumeNewPhony
                            } else {
                                CompletionTime::AssumeNew
                            }
                        } else {
                            CompletionTime::Preserve
                        }
                    } else {
                        CompletionTime::Refresh
                    };
                if self.options.dry_run
                    && !self.options.question
                    && !self.options.touch
                    && (rule.grouped_targets().is_none() || implicit_group)
                    && (outcome.completion == CompletionTime::AssumeNew
                        || (implicit_group
                            && !phony
                            && outcome.completion == CompletionTime::Preserve))
                    && execution.status == Status::Success
                {
                    outcome.database_result = DatabaseResult::Simulated;
                }
                if rule.grouped_double_id().is_some()
                    && execution.status == Status::Fatal
                    && !execution.ran
                {
                    outcome.database_result = DatabaseResult::RecipeUnstarted;
                }
                outcome.search_after_update =
                    self.update_search_pending(target, phony, execution.all_recursive);
                outcome.deferred_spawn_failure = execution.launch_failed
                    && self.parallel_makefile_remakes
                    && self.optional_remake();
                outcome.failed |= matches!(execution.status, Status::Failure | Status::Fatal);
                outcome.fatal |= execution.status == Status::Fatal;
                outcome.question_outdated |= execution.status == Status::Question;
                if self.intermediate(target)
                    && before.is_none()
                    && (!implicit_group || !outcome.fatal || outcome.ran)
                {
                    self.cleanup_candidates.push(target.to_vec());
                }
                if self.cleanup_target(target, before, phony, execution.cleanup) {
                    outcome.failed = true;
                    outcome.fatal = true;
                }
            } else if rule.record_only && !phony {
                if !self.optional_remake() {
                    self.show_makefile_error();
                    let parent = self
                        .scope_parents
                        .get(target)
                        .and_then(Option::as_ref)
                        .map(|parent| parent.target.as_slice());
                    self.report_missing_rule(target, parent);
                }
                outcome.failed = true;
            }
            if !outcome.failed
                && outcome.completion == CompletionTime::Refresh
                && !self.pending(target)
                && self.archive_metadata_error(target, false)
            {
                outcome.failed = true;
                outcome.fatal = true;
            }
            if self
                .archive_completion_times
                .get(target)
                .is_some_and(|stamp| !stamp.known)
            {
                outcome.completion = CompletionTime::ArchiveUnknown;
            }
            if outcome.ran
                && !self.parallel_jobs.contains_key(target)
                && !outcome.completion.preserves()
            {
                self.release_assumed_new(target);
            }
            outcome.time = if phony {
                None
            } else if rule.recipes.is_some()
                && !outcome.completion.preserves()
                && !self.parallel_jobs.contains_key(target)
            {
                self.modified(target)
            } else {
                before
            };
            outcome.debug_time_changed = mask.has(crate::debug::Mask::BASIC)
                && !outcome.failed
                && !self.parallel_jobs.contains_key(target)
                && outcome.time != before;
        }
        if !debug_remake && !outcome.failed && !self.special(b".NOTINTERMEDIATE", target, true) {
            if let Some(ordinal) = branch {
                self.database_branch_secondary
                    .insert((target.to_vec(), ordinal));
            } else {
                self.database_secondary.insert(target.to_vec());
            }
        }
        // A suppressed serial makefile failure returns to the updater, so it
        // emits the same completion trace as a keep-going failure.
        let optional_serial_remake = self.remaking
            && !self.parallel_makefile_remakes
            && self.optional_remake()
            && !outcome.fatal;
        let stopping_failure =
            debug_remake && outcome.failed && !self.options.keep_going && !optional_serial_remake;
        if !self.parallel_jobs.contains_key(target)
            && !stopping_failure
            && !stopping_dependency
            && !self.forced_query_stop
        {
            if outcome.blocked || prerequisite_question {
                crate::debug::emit(
                    &self.output,
                    crate::debug::Event::GiveUp(target, debug_depth),
                );
                // A double-colon goal reports each blocked declaration as it finishes.
                if branch.is_some() && debug_depth == 0 && outcome.failed && outcome.blocked {
                    self.report_blocked_goal(target);
                }
            } else if debug_remake && !no_recipe_no_changes {
                crate::debug::emit(
                    &self.output,
                    if outcome.failed {
                        crate::debug::Event::Failed(target, debug_depth)
                    } else if outcome.question_outdated {
                        crate::debug::Event::QuestionOutdated(target, debug_depth)
                    } else {
                        crate::debug::Event::Remade(target, debug_depth)
                    },
                );
            } else {
                self.debug_no_remake(target, debug_depth);
            }
        }
        self.visiting.remove(target);
        self.done.insert(target.to_vec(), outcome);
        if !self.parallel_jobs.contains_key(target) {
            self.finish_group(target, outcome);
        }
        outcome
    }

    fn note_group_recipe_failure(&mut self, target: &[u8], location: &Location) {
        if let Some(group) = self.running_groups.get_mut(target)
            && group.implicit
        {
            // A stopping command keeps its line for the peer warning.
            group.location = Some(location.clone());
        }
    }

    fn finish_group(&mut self, target: &[u8], outcome: Outcome) {
        let Some(group) = self.running_groups.remove(target) else {
            return;
        };
        let cached = self.cached_recipe_outcome(outcome);
        // A query can inspect an expanded recipe without starting it. Only
        // its owner has a verdict in that case, so peers can still be goals.
        let unstarted_query = group.ordinary
            && outcome.question_outdated
            && outcome.completion == CompletionTime::Preserve;
        if let Some((id, owner_branch)) = group.double_colon {
            self.active_group_branches.remove(&id);
            if !unstarted_query {
                self.completed_group_branches.insert(id, cached);
            }
            let database_requested = self.output.database_requested();
            let observe_dry_group = database_requested
                && self.options.dry_run
                && !self.options.question
                && !self.options.touch
                && !outcome.failed
                && outcome.completion == CompletionTime::AssumeNew;
            if observe_dry_group
                && self
                    .database
                    .rules
                    .get(target)
                    .and_then(|rule| rule.double_colon.as_ref())
                    .is_some_and(|branches| branches.iter().any(|rule| rule.group.is_none()))
            {
                // The grouped completion includes the owner in its physical
                // refresh, replacing any preceding standalone dry-run time.
                self.modified(target);
                if let Some(owner) = self.done.get_mut(target) {
                    owner.database_result = DatabaseResult::ObservedDoubleColon;
                }
            }
            if !outcome.question_outdated && !outcome.fatal && (!outcome.failed || outcome.ran) {
                for (peer, _, phony) in group.members {
                    let owner = peer == target;
                    let Some(branches) = self
                        .database
                        .rules
                        .get(&peer)
                        .and_then(|rule| rule.double_colon.as_ref())
                    else {
                        continue;
                    };
                    // A recipe completes every other occurrence in its group,
                    // including repeated occurrences of the owner's filename.
                    let occurrences = branches.iter().enumerate().rev().filter(|(index, rule)| {
                        rule.grouped_double_id() == Some(id)
                            && (!owner || *index != owner_branch || outcome.failed)
                    });
                    let mut occurrences = occurrences.peekable();
                    if occurrences.peek().is_none() {
                        continue;
                    }
                    let mut completed = outcome;
                    completed.database_result = DatabaseResult::ObservedGroup;
                    completed.time = if phony {
                        None
                    } else {
                        self.observed_modified(&peer, false)
                    };
                    let mut peer_times = (database_requested
                        && !phony
                        && if owner {
                            outcome.failed && branches.len() > 1
                        } else {
                            branches.iter().any(|rule| rule.group.is_none())
                        })
                    .then(|| {
                        let initial = self
                            .double_colon_before
                            .get(&peer)
                            .map_or(GroupPeerTime::Unchecked, |time| {
                                GroupPeerTime::Observed(*time)
                            });
                        self.database_group_peer_times
                            .entry(peer.clone())
                            .or_insert_with(|| vec![initial; branches.len()])
                    });
                    for (ordinal, _) in occurrences {
                        self.database_branch_outcomes
                            .insert((peer.clone(), ordinal), completed);
                        if let Some(peer_times) = &mut peer_times {
                            let mut key = (peer.clone(), 0);
                            let head_updated = self.database_branch_outcomes.contains_key(&key);
                            // A physical observation is shared only with
                            // records in the head declaration's update state.
                            for (ordinal, time) in peer_times.iter_mut().enumerate() {
                                key.1 = ordinal;
                                if self.database_branch_outcomes.contains_key(&key) == head_updated
                                {
                                    *time = GroupPeerTime::Observed(completed.time);
                                }
                            }
                        }
                    }
                }
            }
            return;
        }
        let warn_missing = group.implicit
            && !outcome.completion.preserves()
            && self
                .modified(target)
                .is_some_and(|time| Some(time) > group.before);
        let observe_dry_group = self.options.dry_run
            && !self.options.question
            && !self.options.touch
            && !outcome.failed
            && outcome.completion.preserves();
        let debug_serial_group_peer = !self.remaking
            && self.active_makefile_remake.is_none()
            && !self.parallel_graph()
            && !outcome.question_outdated
            && !outcome.fatal;
        for (peer, before, phony) in group.members.into_iter().rev() {
            let database_result =
                if observe_dry_group && !phony && (!group.implicit || peer != target) {
                    // Only explicit groups include the owner in the physical
                    // refresh. Implicit owners keep their simulated timestamp.
                    self.modified(&peer);
                    self.database_unchecked_times.borrow_mut().remove(&peer);
                    DatabaseResult::ObservedGroup
                } else {
                    DatabaseResult::Outcome
                };
            if peer == target {
                if database_result == DatabaseResult::ObservedGroup
                    && let Some(owner) = self.done.get_mut(target)
                {
                    owner.database_result = database_result;
                }
                continue;
            }
            self.active_peers.remove(&peer);
            if unstarted_query {
                continue;
            }
            if group.implicit && (!outcome.failed || outcome.ran) {
                // Later implicit searches can mark an updated peer intermediate.
                self.cleanup_candidates.push(peer.clone());
            }
            self.pending_rules.remove(&peer);
            let mut completed = cached;
            completed.database_result = database_result;
            completed.debug_serial_group_peer = debug_serial_group_peer;
            completed.optional_group_peer_failure =
                self.optional_remake() && outcome.failed && !outcome.fatal;
            completed.search_after_update = false;
            completed.time = if phony {
                None
            } else if outcome.completion.preserves() {
                before
            } else {
                self.modified(&peer)
            };
            if warn_missing && completed.time.is_none() {
                emit_output(
                    &self.output,
                    &self.program,
                    Output::Warning {
                        location: group.location.as_ref(),
                        message: &[
                            b"warning: pattern recipe did not update peer target '".as_slice(),
                            &peer,
                            b"'.",
                        ]
                        .concat(),
                    },
                );
            }
            self.done.insert(peer, completed);
        }
    }

    /// Initialize each target once, from its first parent outwards. Pattern
    /// conditionals can mutate globals, so cached contexts refresh that layer
    /// before reuse without replaying deferred assignment effects.
    fn target_variables(&mut self, target: &[u8]) -> Result<VariableStore, ExpandError> {
        let alias = self
            .aliases
            .contains(target)
            .then(|| self.aliases.resolve(target).to_vec());
        let target = alias.as_deref().unwrap_or(target);
        if let Some(variables) = self.target_contexts.get(target) {
            let mut variables = variables.clone();
            self.database.refresh_scoped_variables(&mut variables);
            return Ok(variables);
        }
        let parent = self.scope_parents.get(target).cloned().flatten();
        let inherited = if let Some(parent) = parent {
            self.rule_variables(&parent.target, parent.branch)?
        } else {
            self.database.variables.clone()
        };
        let variables = if let Some(scope) = self.implicit_scopes.remove(target) {
            let mut variables = self.database.variables_for_target_prepared(
                target,
                &inherited,
                scope.pattern.as_ref(),
            );
            if let Some(status) = &scope.shell_status {
                variables.retain_local_shell_status(status);
            }
            variables
        } else {
            self.database.variables_for_target_with(
                target,
                &inherited,
                runtime_context(
                    None,
                    &mut RuntimeOutput::new(
                        &self.output,
                        &self.program,
                        &self.flag_state.warn_undefined,
                    ),
                )
                .with_makeflags(&mut self.flag_state.receiver())
                .with_filesystem(&mut |effect| self.filesystem.borrow_mut().execute(effect))
                .with_shell(&mut |request| {
                    crate::shell::run(
                        request,
                        &self.program,
                        self.makelevel,
                        &self.output,
                        self.jobserver.as_ref(),
                        || self.filesystem.borrow_mut().child_completed(),
                    )
                })
                .with_includes(&mut |name| self.include_search.borrow().read(name)),
            )?
        };
        self.target_contexts
            .insert(target.to_vec(), variables.clone());
        Ok(variables)
    }

    // Later double-colon branches see the head's table without sharing a
    // writable local table with it or with each other.
    fn rule_variables(
        &mut self,
        target: &[u8],
        branch: Option<usize>,
    ) -> Result<VariableStore, ExpandError> {
        let Some(ordinal) = branch else {
            return self.target_variables(target);
        };
        let key = (self.aliases.resolve(target).to_vec(), ordinal);
        if let Some(scope) = self.database_branch_contexts.get(&key) {
            return Ok(scope.clone());
        }
        if ordinal == 0 {
            return self.target_variables(&key.0);
        }
        let head_key = (key.0, 0);
        if let Some(head) = self.database_branch_contexts.get(&head_key) {
            return Ok(head.fresh_double_colon_scope(ordinal));
        }
        // A later branch can be the first one to expand a recipe.
        let head = self.target_variables(&head_key.0)?;
        let scope = head.fresh_double_colon_scope(ordinal);
        self.database_branch_contexts.insert(head_key, head);
        Ok(scope)
    }

    fn execute(
        &mut self,
        target: &[u8],
        recipes: &[Recipe],
        automatic: &[(&[u8], Vec<u8>)],
        inputs: RecipeInputs<'_>,
    ) -> Execution {
        let phony = inputs.phony;
        let before = inputs.before;
        let mut completed_permit = None;
        let mut result =
            self.execute_inner(target, recipes, automatic, inputs, &mut completed_permit);
        if let Some(buffer) = self.dispatch_output.take() {
            if result.status == Status::Fatal {
                self.fatal_outputs.push(buffer);
            } else if !self.parallel_jobs.values().any(|job| {
                job.continuation
                    .output
                    .as_ref()
                    .is_some_and(|active| std::rc::Rc::ptr_eq(active, &buffer))
            }) {
                self.output.select(None);
                if result.cleanup != Cleanup::None && buffer.has_spool() {
                    if self.cleanup_target(target, before, phony, result.cleanup) {
                        result.status = Status::Fatal;
                    }
                    result.cleanup = Cleanup::None;
                }
                buffer.dump();
            }
        }
        if result.status != Status::Fatal {
            self.release_completed_permit(completed_permit);
        }
        result
    }

    fn execute_inner(
        &mut self,
        target: &[u8],
        recipes: &[Recipe],
        automatic: &[(&[u8], Vec<u8>)],
        inputs: RecipeInputs<'_>,
        completed_permit: &mut Option<Permit>,
    ) -> Execution {
        let RecipeInputs {
            phony,
            before,
            branch,
            implicit_group,
            location,
            dependencies,
            dropped,
            ..
        } = inputs;
        self.observe_interrupt();
        if self.interrupt.is_some() {
            return Execution::fatal(false);
        }
        let recursive_line = recipe_is_recursive;
        let one_shell = self.database.rules.contains_key(b".ONESHELL".as_slice());
        let combined;
        let (recipes, has_commands, recursive) = if one_shell {
            let Some(first) = recipes.first() else {
                return Execution::success(false);
            };
            let script = crate::oneshell::collect(recipes.iter().map(Recipe::execution_bytes));
            combined = [Recipe {
                bytes: script.bytes,
                location: first.location.clone(),
                prefix: first.prefix,
                builtin: first.builtin,
                continued_inline: false,
            }];
            (combined.as_slice(), script.has_commands, script.recursive)
        } else {
            let has_commands = recipes.iter().any(|recipe| {
                recipe
                    .bytes
                    .iter()
                    .any(|byte| !matches!(byte, b' ' | b'\t' | b'\n' | b'@' | b'-' | b'+'))
            });
            let recursive = recipes.iter().any(recipe_is_recursive);
            (recipes, has_commands, recursive)
        };
        if self.options.touch && !recursive {
            return if phony {
                Execution::success(false)
            } else {
                self.touch_target(target)
            };
        }
        if !has_commands {
            return if self.options.touch
                && !phony
                && recipes.iter().any(|recipe| !recursive_line(recipe))
            {
                self.touch_target(target)
            } else {
                Execution::success(false)
            };
        }
        if self.database.loaded_modules.contains_key(target)
            && let Err(error) = self.native_module_operation(target, None, false)
        {
            self.expansion_error(recipes.first().map(|recipe| &recipe.location), &error);
            return Execution::fatal(false);
        }
        // Empty/shortcut recipes never consume a continuation bitmap. Keep
        // their raw recursion/touch decisions without allocating one per leaf.
        let flow = Flow::new(recipes.iter().map(recursive_line));
        let parallel = self.parallel_graph();
        if self.graph_abort == Some(Status::Fatal) {
            return Execution::fatal(false);
        }
        if self.forced_query_stop {
            return Execution::question(false, Cleanup::None);
        }
        if self.graph_abort.is_some()
            || self.remake_stopped()
            || (self.parallel_failed && !self.options.keep_going)
        {
            return Execution::failure(false, Cleanup::None);
        }
        let mut variables = match self.rule_variables(target, branch) {
            Ok(variables) => variables,
            Err(error) => {
                self.expansion_error(None, &error);
                return Execution::fatal(false);
            }
        };
        self.database.refresh_scoped_variables(&mut variables);
        if !implicit_group && !self.database_stem_prepared(target, branch) {
            self.record_explicit_stem_lookup();
        }
        for (name, value) in automatic {
            if let Err(error) = variables.define_literal(name, value, Origin::Automatic) {
                self.expansion_error(None, &error);
                return Execution::fatal(false);
            }
        }
        self.remember_prepared_prerequisite_roles(target, &inputs);
        self.retain_database_context(target, branch, &variables);
        let output = Some(self.output.recipe_buffer());
        if let Some(buffer) = &output
            && buffer.enabled()
        {
            buffer.set_owner(target, before, phony);
        }
        self.dispatch_output = output.clone();
        self.output.select(output.as_ref());
        let mut expanded = std::collections::VecDeque::new();
        let first = &recipes[0];
        // Runtime diagnostics count command slots from the first source line.
        // An expanded inline declaration can still contain several raw commands
        // in one stored recipe. Keep its locations until those slots are split.
        let mut logical_locations = None;
        for (index, source) in recipes.iter().enumerate() {
            let mut recipe = Recipe {
                bytes: source.expansion_source().into_owned(),
                location: source.location.clone(),
                prefix: source.prefix,
                builtin: source.builtin,
                continued_inline: false,
            };
            let line = first.location.line.saturating_add(index);
            if recipe.location.line != line
                && *logical_locations.get_or_insert_with(|| {
                    !first.location.file.is_null()
                        && one_logical_recipe_slot(&first.execution_bytes())
                        && recipes.iter().all(|recipe| recipe.prefix != b'\\')
                })
            {
                recipe.location.line = line;
            }
            match self.expand(&mut variables, &recipe.bytes, Some(&recipe.location)) {
                Ok(mut bytes) => {
                    if source.continued_inline {
                        rmake_core::reader::strip_recipe_continuation_prefix(
                            &mut bytes,
                            source.prefix,
                        );
                    }
                    expanded.push_back((index, bytes, recipe));
                }
                Err(err) => {
                    self.expansion_error(Some(&recipe.location), &err);
                    return Execution::fatal(false);
                }
            }
        }
        self.retain_database_context(target, branch, &variables);
        // A stopped optional remake can leave a live worker while ordinary
        // dry-run or question mode visits its next goal.
        let live_remake = (!self.remaking || self.parallel_makefile_remakes)
            && !recursive
            && (self.options.dry_run || self.options.question)
            && self
                .parallel_jobs
                .values()
                .any(|job| job.continuation.remake.is_some());
        if parallel || live_remake {
            while !self.forced_query_stop && self.parallel_jobs.len() >= self.job_slots() {
                self.reap_job();
            }
        } else {
            self.wait_all_jobs();
        }
        if self.graph_abort == Some(Status::Fatal) {
            return Execution::fatal(false);
        }
        if self.forced_query_stop {
            return Execution::question(false, Cleanup::None);
        }
        if self.graph_abort.is_some()
            || self.remake_stopped()
            || (self.parallel_failed && !self.options.keep_going)
        {
            return Execution::failure(false, Cleanup::None);
        }
        let debug_job = crate::debug::Jobs::record(target, self.output.debug_mask());
        // A new recipe's dispatch scans the existing live chain before asking
        // for its slot. Continuations retain their existing chain position.
        self.job_debug.live(self.output.debug_mask());
        // A configured jobserver grants the implicit slot to serial recipes
        // too. Synchronous execution has already drained the other jobs.
        let permit = if self.jobserver.is_some() {
            match self.reserve_job(debug_job.as_ref()) {
                Ok(permit) => permit,
                Err(error) => {
                    self.fatal(None, format!("jobserver: {}", os_error(&error)).as_bytes());
                    return Execution::fatal(false);
                }
            }
        } else {
            None
        };
        if self.graph_abort == Some(Status::Fatal) {
            return Execution::fatal(false);
        }
        if self.forced_query_stop {
            return Execution::question(false, Cleanup::None);
        }
        if self.graph_abort.is_some()
            || self.remake_stopped()
            || (self.parallel_failed && !self.options.keep_going)
        {
            return Execution::failure(false, Cleanup::None);
        }
        if self.output.debug_mask().has(crate::debug::Mask::WHY) {
            use crate::debug::TraceReason;

            let prerequisites;
            let reason = if phony {
                TraceReason::Phony
            } else if before.is_none() {
                TraceReason::Missing
            } else {
                let newer = match self.expand(&mut variables, b"$?", None) {
                    Ok(newer) => newer,
                    Err(error) => {
                        self.expansion_error(None, &error);
                        return Execution::fatal(false);
                    }
                };
                prerequisites = if newer.is_empty() {
                    joined(
                        dependencies
                            .iter()
                            .filter(|dependency| {
                                dependency.name != b".WAIT"
                                    && !dropped.contains(&dependency.name)
                                    && self.done.get(&dependency.name).is_some_and(|child| {
                                        child.time.is_none()
                                            && (child.completion != CompletionTime::AssumeNew
                                                || self.special(b".PHONY", &dependency.name, false))
                                    })
                            })
                            .map(|dependency| dependency.name.as_slice()),
                    )
                } else {
                    newer
                };
                if prerequisites.is_empty() {
                    TraceReason::Unknown
                } else {
                    TraceReason::Prerequisites(&prerequisites)
                }
            };
            crate::debug::trace_reason(&self.output, location, target, reason);
        }
        let state = RecipeState {
            branch,
            remake: self.active_makefile_remake.clone(),
            output,
            expanded,
            variables,
            flow,
            ran: false,
            one_shell,
            environment: None,
            permit,
            debug_job,
        };
        if parallel && self.parallel_jobs.values().any(|job| !job.waiting_load) && self.load_high()
        {
            let identity = self.next_job;
            self.next_job = self.next_job.wrapping_add(1);
            self.load_waiting.push((target.to_vec(), identity));
            self.parallel_jobs.insert(
                target.to_vec(),
                ParallelJob {
                    identity,
                    program: Vec::new(),
                    location: recipes[0].location.clone(),
                    ignore: false,
                    recursive: false,
                    before,
                    phony,
                    continuation: state,
                    stdin_owner: false,
                    waiting_load: true,
                    child: None,
                    signal_cleaned: false,
                },
            );
            self.output.select(None);
            return Execution::pending();
        }
        let (execution, permit) = self.continue_recipe(target, state, phony, before, parallel);
        *completed_permit = permit;
        execution
    }

    fn remember_prepared_prerequisite_roles(&mut self, target: &[u8], trace: &RecipeInputs<'_>) {
        if !trace.dropped.is_empty()
            || !trace.dependencies.iter().any(|dependency| {
                dependency.order_only
                    && !dependency.hidden
                    && trace.normal_names.contains(dependency.name.as_slice())
            })
        {
            return;
        }
        if let Some(captured) = self.expanded_prerequisites_mut(target, trace.branch)
            && captured.as_slice() == trace.dependencies
        {
            promote_order_only_duplicates(captured, |_| true);
        }
        let Some(head) = self.database.rules.get_mut(target) else {
            return;
        };
        let rule = match trace.branch {
            Some(ordinal) => head
                .double_colon
                .as_mut()
                .and_then(|branches| branches.get_mut(ordinal)),
            None => Some(head),
        };
        // Only a direct retained rule with the same occurrence chain can
        // receive the promotions. A prepared group or transformed chain
        // has a different owner or occurrence mapping.
        if let Some(rule) = rule
            && rule.group.is_none()
            && rule.secondary.is_none()
            && rule.dependencies.as_slice() == trace.dependencies
        {
            // $< and $+ have already captured the original normal
            // occurrences. The role changes survive for the database dump.
            promote_order_only_duplicates(&mut rule.dependencies, |_| true);
        }
    }

    fn retain_database_context(
        &mut self,
        target: &[u8],
        branch: Option<usize>,
        variables: &VariableStore,
    ) {
        if let Some(ordinal) = branch {
            self.database_branch_contexts
                .insert((target.to_vec(), ordinal), variables.clone());
        } else {
            self.database_contexts
                .insert(target.to_vec(), variables.clone());
        }
    }

    fn finish_debug_job(&mut self, record: Option<&crate::debug::JobHandle>) {
        if let Some(record) = record {
            let pending = self.parallel_jobs.values().any(|job| {
                job.continuation
                    .debug_job
                    .as_ref()
                    .is_some_and(|other| std::rc::Rc::ptr_eq(other, record))
            });
            if !pending {
                self.job_debug
                    .removed(self.output.debug_mask(), record, self.jobserver.is_some());
            }
        }
    }

    fn continue_recipe(
        &mut self,
        target: &[u8],
        state: RecipeState,
        phony: bool,
        before: Option<SystemTime>,
        parallel: bool,
    ) -> (Execution, Option<Permit>) {
        let mut state = state;
        let mut owner = RecipeOwner {
            permit: state.permit.take(),
            debug_job: state.debug_job.take(),
        };
        let execution =
            self.continue_recipe_inner(target, state, phony, before, parallel, &mut owner);
        self.finish_debug_job(owner.debug_job.as_ref());
        (execution, owner.permit)
    }

    fn continue_recipe_inner(
        &mut self,
        target: &[u8],
        state: RecipeState,
        phony: bool,
        before: Option<SystemTime>,
        parallel: bool,
        owner: &mut RecipeOwner,
    ) -> Execution {
        let RecipeState {
            branch,
            remake,
            output,
            mut expanded,
            mut variables,
            mut flow,
            mut ran,
            one_shell,
            mut environment,
            permit: _,
            debug_job: _,
        } = state;
        let variables = &mut variables;
        while let Some((index, bytes, recipe)) = expanded.pop_front() {
            self.observe_interrupt();
            if self.interrupt.is_some() {
                return Execution::fatal(ran);
            }
            if bytes.is_empty() {
                continue;
            }
            let mut command = trim_leading(&bytes);
            let raw_flags = crate::oneshell::prefixes(&recipe.bytes).1;
            let mut quiet = self.globally_silent()
                || self.special(b".SILENT", target, false)
                || raw_flags.quiet;
            let mut command_noerror = raw_flags.ignore_errors;
            let mut ignore = self.options.ignore_errors
                || self.special(b".IGNORE", target, true)
                || raw_flags.ignore_errors;
            let mut force = flow.recursive(index);
            while let Some(prefix) = command.first() {
                match prefix {
                    b'@' => quiet = true,
                    b'-' => {
                        ignore = true;
                        command_noerror = true;
                    }
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
            let selected_shell = if variables.get(b"SHELL").is_some() {
                self.expand(variables, b"$(SHELL)", Some(&recipe.location))
            } else {
                Ok(Vec::new())
            };
            let shell = match selected_shell {
                Ok(shell) => shell,
                Err(err) => {
                    self.recipe_expansion_error(
                        target,
                        &recipe.location,
                        &err,
                        &flow,
                        ignore,
                        output.as_ref(),
                    );
                    self.retain_configuration_abort(&mut owner.permit, &err);
                    return Execution::fatal(ran).with_recursion(!flow.has_ordinary());
                }
            };
            let shell = shell.as_slice();
            // GNU rescans the raw flag text even for a simple binding and
            // attributes direct errors to the recipe, not .SHELLFLAGS itself.
            let flag_text =
                variables
                    .database_lookup(b".SHELLFLAGS")
                    .map_or_else(Vec::new, |variable| {
                        if self.database.posix && variable.origin() == Origin::Default {
                            if command_noerror {
                                b"-c".to_vec()
                            } else {
                                b"-ec".to_vec()
                            }
                        } else {
                            variable.raw_value().to_vec()
                        }
                    });
            let flags = match self.expand(variables, &flag_text, Some(&recipe.location)) {
                Ok(flags) => flags,
                Err(err) => {
                    self.recipe_expansion_error(
                        target,
                        &recipe.location,
                        &err,
                        &flow,
                        ignore,
                        output.as_ref(),
                    );
                    self.retain_configuration_abort(&mut owner.permit, &err);
                    return Execution::fatal(ran).with_recursion(!flow.has_ordinary());
                }
            };
            let ifs = if variables.database_lookup(b"IFS").is_some() {
                match self.expand(variables, b"$(IFS)", Some(&recipe.location)) {
                    Ok(value) => Some(value),
                    Err(err) => {
                        self.recipe_expansion_error(
                            target,
                            &recipe.location,
                            &err,
                            &flow,
                            ignore,
                            output.as_ref(),
                        );
                        self.retain_configuration_abort(&mut owner.permit, &err);
                        return Execution::fatal(ran).with_recursion(!flow.has_ordinary());
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
                if self.remaking || !self.optional_remake() {
                    self.would_build = true;
                }
                return Execution::question(ran, flow.continuation_cleanup())
                    .with_recursion(!flow.has_ordinary());
            }
            if self.options.touch && !force {
                continue;
            }
            if one_shell
                && (!self.options.dry_run || force)
                && crate::oneshell::unsafe_configuration(shell, &flags)
            {
                self.recipe_expansion_error(
                    target,
                    &recipe.location,
                    &ExpandError::Safety(rmake_core::safety::SafetyError::OneShellConfiguration),
                    &flow,
                    ignore,
                    output.as_ref(),
                );
                return Execution::fatal(ran).with_recursion(!flow.has_ordinary());
            }
            let one_prepared = (one_shell && (!self.options.dry_run || force))
                .then(|| crate::oneshell::prepare_script(command, shell, &flags, ifs.as_deref()));
            let echo = one_prepared
                .as_ref()
                .and_then(|prepared| prepared.next)
                .map_or(command, |next| &command[..next - 1]);
            if let Some(buffer) = &output {
                let synchronize = self.output.mode() != crate::output::Mode::None
                    && (self.output.mode() == crate::output::Mode::Recurse || !force);
                if synchronize {
                    buffer.set_owner(target, before, phony);
                }
                buffer.set_enabled(synchronize);
                self.output.select(synchronize.then_some(buffer));
                if !synchronize {
                    buffer.dump();
                }
            }
            self.observe_interrupt();
            if self.interrupt.is_some() {
                return Execution::fatal(ran);
            }
            if self.options.dry_run
                || self.output.debug_mask().has(crate::debug::Mask::PRINT)
                || !quiet
            {
                self.stdout(echo);
            }
            self.commands_started = self.commands_started.wrapping_add(1);
            ran = true;
            if self.options.dry_run && !force {
                continue;
            }
            let invocation = if let Some(prepared) = one_prepared {
                if let Some(next) = prepared.next {
                    expanded.push_front((index, command[next..].to_vec(), recipe.clone()));
                }
                prepared.invocation
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
            self.output.start();
            // A recipe flushes the public streams before building its child
            // environment. A write failure does not decide the launch result.
            let _ = rmake_linux::native::flush_stream(false);
            let _ = rmake_linux::native::flush_stream(true);
            let mut child = Command::new(os(&program));
            child.args(arguments.iter().map(|arg| os(arg)));
            if let Some(values) = &environment {
                for (name, value) in &values.values {
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
                // The synthesized environment-origin binding occupies its table
                // slot. A later ordinary file binding can still be ineligible;
                // numeric MAKELEVEL is then synthesized at the vector tail.
                exports.insert(b"MAKELEVEL".to_vec());
                let export_all = self.database.export_all;
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
                let eligible: HashSet<Vec<u8>> = exports
                    .iter()
                    .filter(|name| {
                        export_selected(
                            &environment_variables,
                            &self.database.variables,
                            &self.database.unexports,
                            name,
                            export_all,
                        )
                    })
                    .cloned()
                    .collect();
                for name in exports.iter().filter(|name| !eligible.contains(*name)) {
                    child.env_remove(os(name));
                }
                let exports =
                    environment_variables.environment_order(|name, _| eligible.contains(name));
                let mut emitted_order = Vec::new();
                let mut inherited_mflags = false;
                let mut target_reads = TargetReads {
                    parents: &self.scope_parents,
                    retained_parents: &self.retained_scope_parents,
                    aliases: &self.aliases,
                    base: &self.target_contexts,
                    recipes: &mut self.database_contexts,
                    branches: &self.database_branch_contexts,
                };
                for name in &exports {
                    self.database.refresh_scoped_variables(variables);
                    let environment_variables = variables.for_environment();
                    if !export_selected(
                        &environment_variables,
                        &self.database.variables,
                        &self.database.unexports,
                        name,
                        export_all,
                    ) {
                        child.env_remove(os(name));
                        continue;
                    }
                    if let Some(variable) = environment_variables.get(name) {
                        if name == b"MFLAGS" {
                            inherited_mflags = variable.origin() == Origin::Environment;
                        }
                        // Imported values are exported verbatim. MAKEFLAGS still
                        // expands its command-variable reference under -e.
                        if name != b"MAKEFLAGS"
                            && matches!(
                                variable.origin(),
                                Origin::Environment | Origin::EnvironmentOverride
                            )
                        {
                            child.env(os(name), OsString::from_vec(variable.raw_value().to_vec()));
                            emitted_order.push(name.clone());
                            continue;
                        }
                        match self.database.expand_environment_in(
                            variables,
                            name,
                            runtime_context(
                                variable.definition(),
                                &mut RuntimeOutput::new(
                                    &self.output,
                                    &self.program,
                                    &self.flag_state.warn_undefined,
                                ),
                            )
                            .with_makeflags(&mut self.flag_state.receiver())
                            .with_target_reads(&mut target_reads)
                            .with_filesystem(&mut |effect| {
                                self.filesystem.borrow_mut().execute(effect)
                            })
                            .with_shell(&mut |request| {
                                crate::shell::run(
                                    request,
                                    &self.program,
                                    self.makelevel,
                                    &self.output,
                                    self.jobserver.as_ref(),
                                    || self.filesystem.borrow_mut().child_completed(),
                                )
                            })
                            .with_includes(&mut |name| self.include_search.borrow().read(name)),
                        ) {
                            Ok(value) => {
                                child.env(os(name), OsString::from_vec(value));
                                emitted_order.push(name.clone());
                            }
                            Err(err) => {
                                self.expansion_error(variable.definition(), &err);
                                return Execution::fatal(ran).with_recursion(!flow.has_ordinary());
                            }
                        }
                    } else {
                        child.env_remove(os(name));
                    }
                }
                if !export_selected(
                    &environment_variables,
                    &self.database.variables,
                    &self.database.unexports,
                    b"MAKEFLAGS",
                    export_all,
                ) {
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
                            self.recipe_expansion_error(
                                target,
                                &recipe.location,
                                &error,
                                &flow,
                                ignore,
                                output.as_ref(),
                            );
                            return Execution::fatal(ran).with_recursion(!flow.has_ordinary());
                        }
                    }
                }
                // Ordinary export expansion above still has its normal effects.
                // The child's numeric depth always comes from the startup snapshot,
                // even if the visible binding was changed, removed, or unexported.
                child.env("MAKELEVEL", self.makelevel.wrapping_add(1).to_string());
                if self
                    .jobserver
                    .as_ref()
                    .is_some_and(|server| !server.auth().starts_with(b"fifo:"))
                    && !flow.has_declared_recursion()
                {
                    crate::process::disable_pipe_jobserver(&mut child, inherited_mflags);
                }
                environment = Some(RecipeEnvironment {
                    values: child
                        .get_envs()
                        .map(|(name, value)| (name.to_owned(), value.map(OsStr::to_owned)))
                        .collect(),
                    order: emitted_order,
                });
            }
            let stdin_owner = !parallel || !self.parallel_jobs.values().any(|job| job.stdin_owner);
            if let Some(buffer) = &output {
                buffer.child_reaped(false);
            }
            let process = launch_child(
                &program,
                &child,
                &environment.as_ref().expect("prepared environment").order,
                self.database.variables.original_shell(),
                |mut command, image, arg0| {
                    let descriptor_limited_pool = self.jobserver.as_ref().filter(|server| {
                        (server.uses_compact_pipe()
                            || (server.auth().starts_with(b"fifo:")
                                && rmake_linux::child::Child::wake_pipe_unavailable()))
                            && output.as_ref().is_none_or(|buffer| !buffer.enabled())
                    });
                    if let Some(server) = descriptor_limited_pool {
                        let stdin = if stdin_owner {
                            None
                        } else {
                            match rmake_linux::process::closed_stdin_file() {
                                Ok(stdin) => Some(stdin),
                                // The reference inherits stdin when its
                                // unused-input pipe cannot be allocated.
                                Err(error) if matches!(error.raw_os_error(), Some(23 | 24)) => None,
                                Err(error) => return Err(error),
                            }
                        };
                        if let Some(process) = rmake_linux::child::Child::try_spawn_ordered_pool(
                            &command,
                            image,
                            os(arg0),
                            stdin.as_ref(),
                            server,
                            force,
                        ) {
                            return process;
                        }
                        if let Some(stdin) = stdin {
                            command.stdin(std::process::Stdio::from(stdin));
                        }
                    }
                    if let Some(server) = &self.jobserver {
                        server.configure_child(&mut command, force)?;
                    }
                    if !stdin_owner && descriptor_limited_pool.is_none() {
                        command.stdin(rmake_linux::process::closed_stdin()?);
                    }
                    if let Some(buffer) = &output {
                        // Descriptor pressure can leave room for the spool but
                        // no room to clone it or open std's spawn error pipe.
                        if buffer.enabled()
                            && stdin_owner
                            && self.jobserver.is_none()
                            && rmake_linux::child::Child::wake_pipe_unavailable()
                            && !crate::native::has_modules()
                            && let Some(spool) = buffer.child_spool()
                            && let Some(process) =
                                rmake_linux::child::Child::try_spawn_ordered_spool(
                                    &command,
                                    image,
                                    os(arg0),
                                    &spool,
                                )
                        {
                            return process;
                        }
                        buffer.configure_child(&mut command)?;
                    }
                    if stdin_owner
                        && output.as_ref().is_none_or(|buffer| !buffer.enabled())
                        && self.jobserver.is_none()
                        && let Some(process) =
                            rmake_linux::child::Child::try_spawn_inherited_ordered(
                                &command,
                                image,
                                os(arg0),
                            )
                    {
                        return process;
                    }
                    rmake_linux::child::Child::spawn_ordered(&mut command, image, os(arg0))
                },
            );
            self.retain_database_context(target, branch, variables);
            self.output.select(None);
            if let Ok(child) = &process {
                if owner.debug_job.is_none() {
                    owner.debug_job = crate::debug::Jobs::record(target, self.output.debug_mask());
                }
                if let Some(record) = owner.debug_job.as_ref() {
                    self.job_debug
                        .launched(self.output.debug_mask(), record, child.id());
                }
            }
            if process.is_ok() {
                self.load.started();
            }
            let mut signal_cleaned = false;
            #[cfg(feature = "guile")]
            let mut interpreter_failed = false;
            let result = if parallel {
                match process {
                    Ok(child) => {
                        let identity = self.next_job;
                        self.next_job = self.next_job.wrapping_add(1);
                        self.parallel_jobs.insert(
                            target.to_vec(),
                            ParallelJob {
                                identity,
                                program,
                                location: recipe.location.clone(),
                                ignore,
                                recursive: force,
                                before,
                                phony,
                                stdin_owner,
                                waiting_load: false,
                                child: Some(child),
                                signal_cleaned: false,
                                continuation: RecipeState {
                                    branch,
                                    remake,
                                    output,
                                    expanded,
                                    variables: variables.clone(),
                                    flow,
                                    ran,
                                    one_shell,
                                    environment,
                                    permit: owner.permit.take(),
                                    debug_job: owner.debug_job.clone(),
                                },
                            },
                        );
                        self.observe_interrupt();
                        return Execution::pending();
                    }
                    Err(error) => Err(error),
                }
            } else {
                match process {
                    Ok(mut child) => self
                        .wait_serial_child(
                            &mut child,
                            target,
                            before,
                            phony,
                            owner.debug_job.as_ref(),
                        )
                        .map(|wait| {
                            signal_cleaned = wait.cleaned;
                            #[cfg(feature = "guile")]
                            {
                                interpreter_failed = wait.interpreter_failed;
                            }
                            wait.status
                        }),
                    Err(error) => Err(error),
                }
            };
            self.output
                .select(output.as_ref().filter(|buffer| buffer.enabled()));
            self.filesystem.borrow_mut().child_completed();
            let launched = result.is_ok();
            if launched && !phony {
                // Real command completion invalidates the logical timestamp.
                // A no-child ':' shortcut does not pass this event boundary.
                self.database_unchecked_times
                    .borrow_mut()
                    .insert(target.to_vec());
            }
            if launched && let Some(buffer) = &output {
                buffer.child_reaped(true);
            }
            flow.attempted_in_pool(
                launched,
                parallel && self.parallel_jobs.values().any(|job| job.child.is_some()),
            );
            #[cfg(feature = "guile")]
            if interpreter_failed {
                self.output.select(None);
                return Execution::fatal(ran).with_recursion(!flow.has_ordinary());
            }
            let failure_cleanup = if !launched || signal_cleaned {
                Cleanup::None
            } else if result
                .as_ref()
                .is_ok_and(|status| status.signal().is_some())
            {
                Cleanup::ChangedOnSignal
            } else {
                Cleanup::ChangedOnError
            };
            let error_code = match &result {
                Ok(status) => status.code().filter(|code| *code != 0),
                Err(_) => Some(127),
            };
            let error = match result {
                Ok(status) if status.success() => None,
                // Recursive make uses exit 1 to report an out-of-date build in
                // query mode. GNU applies that convention to every forced
                // command; ignored errors retain their ordinary handling.
                Ok(status) if self.options.question && !ignore && status.code() == Some(1) => {
                    self.forced_query_completed();
                    return Execution::question(ran, failure_cleanup)
                        .with_recursion(!flow.has_ordinary());
                }
                Ok(status) => Some(
                    status
                        .code()
                        .map_or_else(|| signal_status(status), |code| format!("Error {code}")),
                ),
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
                    self.note_group_recipe_failure(target, &recipe.location);
                }
                if (ignore || !remake.as_ref().is_some_and(|remake| remake.optional))
                    && (!ignore || !self.globally_silent())
                {
                    self.show_recipe_makefile_error(remake.as_ref());
                    let source = location_prefix(&recipe.location);
                    let ignored = if ignore {
                        crate::messages::system(rmake_core::messages::Message::Ignored)
                    } else {
                        Vec::new()
                    };
                    let message =
                        if let Some((code, source)) = error_code.zip(source.strip_suffix(b": ")) {
                            crate::messages::system(rmake_core::messages::Message::RecipeError {
                                prefix: if ignore { b"" } else { b"*** " },
                                source,
                                target,
                                code,
                                ignored: &ignored,
                                suffix: self.shuffle.suffix(),
                            })
                        } else {
                            let mut message = if ignore { Vec::new() } else { b"*** ".to_vec() };
                            message.push(b'[');
                            message.extend_from_slice(&location_prefix(&recipe.location));
                            message.extend_from_slice(target);
                            message.extend_from_slice(format!("] {error}").as_bytes());
                            message.extend_from_slice(&ignored);
                            message.extend_from_slice(self.shuffle.suffix());
                            message
                        };
                    self.stderr(&message);
                }
                if !ignore {
                    self.output.select(None);
                    let mut execution = Execution::failure(ran, failure_cleanup)
                        .with_recursion(!flow.has_ordinary());
                    execution.launch_failed = !launched;
                    return execution;
                }
            }
            self.output.select(None);
            if self.output.mode() == crate::output::Mode::Line
                && let Some(buffer) = &output
            {
                buffer.dump();
            }
            if self.interrupt.is_some() {
                if !signal_cleaned && expanded.iter().any(|(_, bytes, _)| !bytes.is_empty()) {
                    self.cleanup_target(target, before, phony, Cleanup::ChangedOnSignal);
                }
                return Execution::fatal(ran);
            }
        }
        if !self.options.touch || phony || !flow.has_ordinary() {
            return Execution::success(ran).with_recursion(!flow.has_ordinary());
        }
        let passes = if one_shell {
            1
        } else {
            flow.completion_passes()
        };
        for pass in 0..passes {
            if pass + 1 == passes
                && let Some(buffer) = &output
            {
                buffer.dump();
            }
            let mut touched = self.touch_target(target);
            ran |= touched.ran;
            if matches!(touched.status, Status::Failure | Status::Fatal) {
                // Only dispatch completion can fail before a continuation owner
                // performs cleanup. The owner's final completion has no such step.
                if pass == 0 && passes > 1 && touched.status != Status::Fatal {
                    touched.cleanup = flow.continuation_cleanup();
                }
                touched.ran |= ran;
                return touched;
            }
        }
        Execution::success(ran).with_recursion(!flow.has_ordinary())
    }

    fn touch_target(&mut self, target: &[u8]) -> Execution {
        self.commands_started = self.commands_started.wrapping_add(1);
        if !self.globally_silent() {
            self.stdout(&[b"touch ".as_slice(), target].concat());
        }
        if self.options.dry_run {
            return Execution::success(true);
        }
        if let Some((archive, member)) = rmake_core::filenames::archive_parts(target) {
            return match rmake_linux::archive::touch(archive, member) {
                Ok(()) => Execution::success(true),
                Err(rmake_core::diagnostic::FilesystemError::Unlocated(message)) => {
                    self.archive_recipe_fatal(&message);
                    Execution::fatal(true)
                }
                Err(rmake_core::diagnostic::FilesystemError::Caller(message)) => {
                    self.stderr(&message);
                    Execution::failure(true, Cleanup::None)
                }
            };
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

    fn archive_recipe_fatal(&self, message: &[u8]) {
        self.fatal_with_owner(None, message, true);
    }

    fn cleanup_target(
        &self,
        target: &[u8],
        before: Option<SystemTime>,
        phony: bool,
        cleanup: Cleanup,
    ) -> bool {
        if matches!(cleanup, Cleanup::ChangedOnError | Cleanup::ChangedOnSignal)
            && !self.database_delete_policy.replace(true)
            && let Some(history) = self.database.database_history()
        {
            history.record_file_lookup(b".DELETE_ON_ERROR");
        }

        if self.cleanup_one(target, before, phony, cleanup, None) {
            return true;
        }
        if let Some(group) = self.running_groups.get(target) {
            for (peer, time, peer_phony) in group.members.iter().rev() {
                if peer != target
                    && self.cleanup_one(peer, *time, *peer_phony, cleanup, Some(target))
                {
                    return true;
                }
            }
        }
        false
    }

    fn cleanup_one(
        &self,
        target: &[u8],
        before: Option<SystemTime>,
        phony: bool,
        cleanup: Cleanup,
        owner: Option<&[u8]>,
    ) -> bool {
        if cleanup == Cleanup::None || phony || self.special_for_rule(b".PRECIOUS", target) {
            return false;
        }
        if cleanup == Cleanup::ChangedOnError
            && !self
                .database
                .rules
                .contains_key(b".DELETE_ON_ERROR".as_slice())
        {
            return false;
        }
        if let Some((archive, member)) = rmake_core::filenames::archive_parts(target) {
            let date = match rmake_linux::archive::date(archive, member) {
                Ok(date) => date.and_then(|seconds| {
                    SystemTime::UNIX_EPOCH.checked_add(std::time::Duration::from_secs(seconds))
                }),
                Err(error) => {
                    self.archive_recipe_fatal(&error.into_message());
                    return true;
                }
            };
            if cleanup == Cleanup::ExistingAfterContinuation || date != before {
                let context =
                    owner.map_or_else(Vec::new, |owner| [b"[".as_slice(), owner, b"] "].concat());
                self.stderr(
                    &[
                        b"*** ".as_slice(),
                        &context,
                        b"Archive member '",
                        target,
                        b"' may be bogus; not deleted",
                    ]
                    .concat(),
                );
            }
            return false;
        }
        let Ok(metadata) = fs::metadata(os(target)) else {
            return false;
        };
        if !metadata.is_file() {
            return false;
        }
        if matches!(cleanup, Cleanup::ChangedOnError | Cleanup::ChangedOnSignal)
            && metadata.modified().ok() == before
        {
            return false;
        }
        let context = owner.map_or_else(Vec::new, |owner| [b"[".as_slice(), owner, b"] "].concat());
        self.stderr(
            &[
                b"*** ".as_slice(),
                &context,
                b"Deleting file '",
                target,
                b"'",
            ]
            .concat(),
        );
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
        false
    }
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
fn recipe_is_recursive(recipe: &Recipe) -> bool {
    has_force_prefix(&recipe.bytes)
        || recipe
            .bytes
            .windows(7)
            .any(|part| part == b"$(MAKE)" || part == b"${MAKE}")
}

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

fn signal_status(status: std::process::ExitStatus) -> String {
    let mut message = rmake_linux::process::signal_name(status.signal().unwrap_or(0));
    if status.core_dumped() {
        message.push_str(" (core dumped)");
    }
    message
}

/// Keep the exact executable that returned ENOEXEC. Re-searching using file mode
/// bits is incorrect: an earlier PATH entry can have an execute bit yet deny the
/// current user execution. Only the kernel's actual exec result decides that.
pub(crate) fn launch_child<T>(
    program: &[u8],
    template: &Command,
    order: &[Vec<u8>],
    original_shell: Option<&[u8]>,
    mut run: impl FnMut(Command, &rmake_linux::environment::Image, &[u8]) -> io::Result<T>,
) -> io::Result<T> {
    let paths = if program.contains(&b'/') {
        vec![std::path::PathBuf::from(os(program))]
    } else {
        let path = template
            .get_envs()
            .find(|(key, _)| *key == "PATH")
            .and_then(|(_, value)| value.map(OsStr::to_owned))
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
    let environment =
        rmake_linux::environment::Image::from_command(template, order, original_shell)?;
    let mut denied = None;
    let mut missing = io::Error::from_raw_os_error(2);
    for path in &paths {
        let mut command = Command::new(path);
        command.arg0(os(program)).args(template.get_args());
        rmake_linux::signals::configure_child(&mut command);
        match run(command, &environment, program) {
            // Linux ENOEXEC: interpret the attempted file through /bin/sh,
            // preserving args/environment but not the recipe's -e/-c flags.
            Err(error) if error.raw_os_error() == Some(8) => {
                let mut shell = Command::new("/bin/sh");
                shell.arg(path).args(template.get_args());
                rmake_linux::signals::configure_child(&mut shell);
                return run(shell, &environment, b"/bin/sh");
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                // A failed fork has not checked the executable yet. Recover
                // the lookup diagnostic without retrying the child launch.
                let mut lookup = io::Error::from_raw_os_error(2);
                for candidate in &paths {
                    match rmake_linux::process::executable(candidate) {
                        Ok(()) => return Err(error),
                        Err(error) if error.raw_os_error() != Some(2) => lookup = error,
                        Err(_) => {}
                    }
                }
                return Err(lookup);
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

#[cfg(feature = "test-support")]
pub(crate) fn run_native_ownership_check(name: &str) -> bool {
    initial::run_native_ownership_check(name)
}

#[cfg(test)]
mod tests {
    #[test]
    fn native_source_prefixes_keep_null_empty_and_literal_builtin_distinct() {
        use rmake_core::diagnostic::SourceName;
        assert_eq!(
            super::location_prefix(&Location {
                file: SourceName::null(),
                line: 0
            }),
            b"<builtin>: "
        );
        assert_eq!(
            super::location_prefix(&Location {
                file: b"".as_slice().into(),
                line: 19
            }),
            b":19: "
        );
        assert_eq!(
            super::location_prefix(&Location {
                file: b"<builtin>".as_slice().into(),
                line: 0
            }),
            b"<builtin>:0: "
        );
    }

    use super::*;
    use rmake_core::variables::Flavor;

    #[test]
    fn process_pressure_lookup_does_not_retry_the_launch() {
        let mut command = Command::new("unavailable");
        command.env("PATH", "/dev/null:/dev/null");
        let mut attempts = 0;
        let result = launch_child(b"unavailable", &command, &[], None, |_, _, _| {
            attempts += 1;
            Err::<(), _>(io::Error::from_raw_os_error(11))
        });
        assert_eq!(attempts, 1);
        assert_eq!(result.unwrap_err().raw_os_error(), Some(20));
    }

    #[test]
    fn child_signal_diagnostics_keep_libc_names_and_core_dump_status() {
        use std::os::unix::process::ExitStatusExt;
        for (signal, name) in [
            (2, "Interrupt"),
            (3, "Quit"),
            (13, "Broken pipe"),
            (15, "Terminated"),
        ] {
            assert_eq!(
                signal_status(std::process::ExitStatus::from_raw(signal)),
                name
            );
            assert_eq!(
                signal_status(std::process::ExitStatus::from_raw(signal | 0x80)),
                format!("{name} (core dumped)")
            );
        }
    }

    #[test]
    fn exhausted_implicit_search_cannot_install_a_default_recipe() {
        struct Directory(std::path::PathBuf);
        impl Drop for Directory {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        let nonce = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("rmake-search-limit-{}-{nonce}", std::process::id()));
        fs::create_dir(&path).unwrap();
        let directory = Directory(path);
        let target = directory.0.join("input.p0");
        let target = target.as_os_str().as_bytes();
        let launch = Engine::invocation(
            [OsString::from("-rR")],
            b"rmake",
            OsStr::new("rmake"),
            None,
            None,
        )
        .unwrap();
        let mut engine =
            Engine::from_invocation(launch, b"rmake".to_vec(), OsStr::new("rmake")).unwrap();
        let mut source = b".DEFAULT:;echo wrong-recipe\n".to_vec();
        for index in 0..130 {
            source.extend_from_slice(format!("%.p{index}: %.p{};\n", index + 1).as_bytes());
        }
        source.extend_from_slice(b"%.p130:;\n");
        engine.database.read(&source, b"Makefile").unwrap();
        let result = engine.build(target, None, 0);
        assert!(result.failed && result.fatal && !result.ran);
        assert!(!engine.database.rules.contains_key(target));
        assert!(fs::read_dir(&directory.0).unwrap().next().is_none());
        assert!(
            engine.database.rules[b".DEFAULT".as_slice()]
                .recipes
                .is_some()
        );
    }

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

    fn engine_with_large_phony_rule() -> Engine {
        let launch = Engine::invocation(
            ["-rR"].into_iter().map(OsString::from),
            b"rmake",
            OsStr::new("rmake"),
            None,
            None,
        )
        .unwrap();
        let mut engine =
            Engine::from_invocation(launch, b"rmake".to_vec(), OsStr::new("rmake")).unwrap();
        let dependencies = (0..64)
            .map(|index| Dependency {
                name: format!("goal-{index:04}").into_bytes(),
                order_only: false,
                hidden: false,
            })
            .collect();
        engine.database.rules.insert(
            b".PHONY".to_vec(),
            Rule {
                dependencies,
                ..Rule::default()
            },
        );
        engine.database.freeze_rules();
        engine
    }

    #[test]
    fn large_phony_index_ignores_hidden_names_and_uses_live_aliases() {
        let mut engine = engine_with_large_phony_rule();
        let rule = engine.database.rules.get_mut(b".PHONY".as_slice()).unwrap();
        let last = rule.dependencies.pop().unwrap();
        rule.dependencies.push(Dependency {
            name: b"hidden".to_vec(),
            order_only: false,
            hidden: true,
        });
        engine.capture_phony_names();
        assert!(engine.phony_names.is_none());

        engine
            .database
            .rules
            .get_mut(b".PHONY".as_slice())
            .unwrap()
            .dependencies
            .push(last);
        engine.capture_phony_names();
        assert_eq!(engine.phony_names.as_ref().unwrap().len(), 64);
        assert!(engine.special(b".PHONY", b"goal-0000", false));
        assert!(engine.special(b".PHONY", b"goal-0063", false));
        assert!(!engine.special(b".PHONY", b"hidden", false));
        assert!(!engine.special(b".PHONY", b"ordinary", false));

        engine.aliases.insert(b"goal-0000", b"renamed", false);
        assert!(!engine.special(b".PHONY", b"goal-0000", false));
        assert!(engine.special(b".PHONY", b"renamed", false));
    }

    #[test]
    fn large_phony_index_yields_to_runtime_rule_changes_and_literal_visits() {
        let mut engine = engine_with_large_phony_rule();
        engine.capture_phony_names();
        let rule = Rule {
            dependencies: vec![Dependency {
                name: b"inferred".to_vec(),
                order_only: true,
                hidden: false,
            }],
            ..Rule::default()
        };
        engine.install_implicit_plan(
            b".PHONY",
            vec![ImplicitSelection {
                name: b".PHONY".to_vec(),
                rule,
                explicit_names: Vec::new(),
                intermediate_names: Vec::new(),
                prepared_scope: None,
            }],
            false,
        );
        assert!(engine.phony_names.is_none());
        assert!(engine.special(b".PHONY", b"inferred", false));
        assert!(engine.special(b".PHONY", b"goal-0000", false));

        engine.capture_phony_names();
        assert!(engine.phony_names.is_some());
        engine.drop_group_dependency(b".PHONY", None, 0, b"goal-0000");
        assert!(engine.phony_names.is_none());
        assert!(!engine.special(b".PHONY", b"goal-0000", false));

        engine.capture_phony_names();
        assert!(engine.phony_names.is_some());
        engine.done.insert(b".PHONY".to_vec(), Outcome::default());
        assert!(!engine.build(b".PHONY", None, 0).failed);
        assert!(engine.phony_names.is_none());
    }

    #[test]
    fn intermediate_goal_override_resolves_aliases_after_policy_selection() {
        let launch = Engine::invocation(
            ["-rR", "goal"].into_iter().map(OsString::from),
            b"rmake",
            OsStr::new("rmake"),
            None,
            None,
        )
        .unwrap();
        let mut engine =
            Engine::from_invocation(launch, b"rmake".to_vec(), OsStr::new("rmake")).unwrap();
        engine
            .database
            .read(
                b".INTERMEDIATE: listed resolved\n.SECONDARY: second\n.NOTINTERMEDIATE: excluded\n",
                b"Makefile",
            )
            .unwrap();
        engine.intermediate_policy = crate::intermediates::Policy::read(&engine.database).unwrap();
        engine.intermediates.insert(b"inferred".to_vec());
        engine.intermediates.insert(b"excluded".to_vec());
        assert!(engine.intermediate(b"resolved"));
        engine.aliases.insert(b"goal", b"resolved", false);

        assert!(engine.intermediate(b"listed"));
        assert!(engine.intermediate(b"second"));
        assert!(engine.intermediate(b"inferred"));
        assert!(!engine.intermediate(b"excluded"));
        assert!(!engine.intermediate(b"ordinary"));
        assert!(!engine.intermediate(b"resolved"));
        assert!(!engine.intermediate(b"goal"));
    }

    #[test]
    fn ordinary_mode_remakes_guard_a_named_source_after_cli_normalization() {
        let launch = Engine::invocation(
            ["-n", "./left.gen"].into_iter().map(OsString::from),
            b"rmake",
            OsStr::new("rmake"),
            None,
            None,
        )
        .unwrap();
        let mut engine =
            Engine::from_invocation(launch, b"rmake".to_vec(), OsStr::new("rmake")).unwrap();
        let modes = MakefileModes::from_options(&engine.options);
        let mut reads = [(
            Makefile {
                name: b"left.gen".to_vec(),
                optional: false,
                missing: true,
                location: None,
                read_error: None,
            },
            false,
        )];

        assert_eq!(engine.options.goals, [b"left.gen".to_vec()]);
        assert!(!engine.ordinary_mode_remakes_settle(&reads, None, modes));
        reads[0].1 = true;
        assert!(engine.ordinary_mode_remakes_settle(&reads, None, modes));

        reads[0].1 = false;
        engine.options.goals = vec![b"named".to_vec(), b"all".to_vec()];
        assert!(engine.ordinary_mode_remakes_settle(&reads, None, modes));
        engine.vpath_dirs.push(b"search".to_vec());
        assert!(!engine.ordinary_mode_remakes_settle(&reads, None, modes));
        engine.vpath_dirs.clear();
        for goal in ["~/all", "-lwidget", "archive.a(member)"] {
            engine.options.goals = vec![goal.as_bytes().to_vec()];
            assert!(!engine.ordinary_mode_remakes_settle(&reads, None, modes));
        }
    }

    #[test]
    fn indexed_makefile_goals_preserve_normalized_byte_names() {
        let mut args = vec![
            OsString::from("-n"),
            OsString::from_vec(b"./left-\xff.gen".to_vec()),
        ];
        args.extend((0..32).map(|index| OsString::from(format!("goal-{index}"))));
        let launch = Engine::invocation(args, b"rmake", OsStr::new("rmake"), None, None).unwrap();
        let engine =
            Engine::from_invocation(launch, b"rmake".to_vec(), OsStr::new("rmake")).unwrap();
        let modes = MakefileModes::from_options(&engine.options);
        let mut reads: Vec<_> = (0..16)
            .map(|index| {
                (
                    Makefile {
                        name: format!("other-{index}.gen").into_bytes(),
                        optional: false,
                        missing: false,
                        location: None,
                        read_error: None,
                    },
                    false,
                )
            })
            .collect();
        reads[0].0.name = b"./left-\xff.gen".to_vec();
        reads[1].0.name = b"left-\xff.gen".to_vec();
        reads[2].0.name = b"goal-0".to_vec();
        reads[2].1 = true;

        assert!(engine.index_named_remake_sources(&reads[..1]).is_none());
        let indexed = engine.index_named_remake_sources(&reads).unwrap();
        assert_eq!(indexed, HashSet::from([b"left-\xff.gen".as_slice()]));
        assert!(engine.is_makefile_goal(b"left-\xff.gen", Some(&indexed)));
        assert!(!engine.is_makefile_goal(b"other-3.gen", Some(&indexed)));
        assert_eq!(
            engine.ordinary_mode_remakes_settle(&reads, Some(&indexed), modes),
            engine.ordinary_mode_remakes_settle(&reads, None, modes),
        );
    }

    #[test]
    fn ordinary_mode_remakes_admit_plain_subdirectory_goals_with_leaf_sources() {
        let launch = Engine::invocation(
            ["-n", "./build/result", "obj/deep/output"]
                .into_iter()
                .map(OsString::from),
            b"rmake",
            OsStr::new("rmake"),
            None,
            None,
        )
        .unwrap();
        let mut engine =
            Engine::from_invocation(launch, b"rmake".to_vec(), OsStr::new("rmake")).unwrap();
        let modes = MakefileModes::from_options(&engine.options);
        let leaf = |name: &[u8]| Makefile {
            name: name.to_vec(),
            optional: false,
            missing: false,
            location: None,
            read_error: None,
        };
        let mut reads = [(leaf(b"././Makefile"), false), (leaf(b"left.gen"), false)];
        assert_eq!(engine.options.goals[0], b"build/result");
        assert!(engine.ordinary_mode_remakes_settle(&reads, None, modes));

        for name in [
            b"build/result".as_slice(),
            b"sub/other.gen",
            b"sub/../left.gen",
        ] {
            reads[1] = (leaf(name), false);
            assert!(!engine.ordinary_mode_remakes_settle(&reads, None, modes));
            reads[1].1 = true;
            assert!(!engine.ordinary_mode_remakes_settle(&reads, None, modes));
        }
        reads[1] = (leaf(b"left.gen"), false);
        for goal in [
            b"/build/result".as_slice(),
            b"build//result",
            b"build/./result",
            b"build/../result",
            b"build/result/",
            b"archive.a(build/result)",
        ] {
            engine.options.goals[0] = goal.to_vec();
            assert!(!engine.ordinary_mode_remakes_settle(&reads, None, modes));
        }
        engine.options.goals[0] = b"build/result".to_vec();
        engine.gpath_dirs.insert(b"build".to_vec());
        assert!(!engine.ordinary_mode_remakes_settle(&reads, None, modes));
        engine.gpath_dirs.clear();
        engine
            .database
            .read(b"vpath %.gen build\n", b"Makefile")
            .unwrap();
        assert!(!engine.ordinary_mode_remakes_settle(&reads, None, modes));
    }

    #[test]
    fn ordinary_mode_optional_sources_require_simple_names() {
        let launch = Engine::invocation(
            ["-r", "-n", "all"].into_iter().map(OsString::from),
            b"rmake",
            OsStr::new("rmake"),
            None,
            None,
        )
        .unwrap();
        let mut engine =
            Engine::from_invocation(launch, b"rmake".to_vec(), OsStr::new("rmake")).unwrap();
        let modes = MakefileModes::from_options(&engine.options);
        let source = Makefile {
            name: b"optional.gen".to_vec(),
            optional: true,
            missing: true,
            location: None,
            read_error: None,
        };
        let mut reads = vec![(source.clone(), false)];
        assert!(engine.ordinary_mode_remakes_settle(&reads, None, modes));
        engine.options.no_rules = false;
        assert!(!engine.ordinary_mode_remakes_settle(&reads, None, modes));
        engine.options.no_rules = true;

        let mut alias = source.clone();
        alias.name = b"./optional.gen".to_vec();
        alias.optional = false;
        reads.push((alias, false));
        assert!(!engine.ordinary_mode_remakes_settle(&reads, None, modes));
        reads.pop();

        engine.vpath_dirs.push(b"search".to_vec());
        assert!(!engine.ordinary_mode_remakes_settle(&reads, None, modes));
        engine.vpath_dirs.clear();
        engine
            .database
            .read(b"optional.gen: ; @:\n", b"Makefile")
            .unwrap();
        assert!(engine.ordinary_mode_remakes_settle(&reads, None, modes));
        engine.options.no_rules = false;
        assert!(engine.ordinary_mode_remakes_settle(&reads, None, modes));
        engine.options.no_rules = true;
        engine
            .database
            .read(b"optional.gen: prerequisite\n", b"Makefile")
            .unwrap();
        assert!(engine.ordinary_mode_remakes_settle(&reads, None, modes));
        engine.options.load_limit = Some(1.0);
        assert!(engine.ordinary_mode_remakes_settle(&reads, None, modes));
        let mut required = source;
        required.name = b"required.gen".to_vec();
        required.optional = false;
        engine
            .database
            .read(b"required.gen: ; @:\n", b"Makefile")
            .unwrap();
        reads.push((required, false));
        assert!(!engine.ordinary_mode_remakes_settle(&reads, None, modes));
        engine.options.load_limit = None;
        assert!(engine.ordinary_mode_remakes_settle(&reads, None, modes));
    }

    #[test]
    fn engine_installs_variants_even_when_builtin_variables_disabled() {
        for no_variables in [false, true] {
            let args = if no_variables {
                vec!["-R", "@D=ignored"]
            } else {
                vec!["@D=ignored"]
            };
            let launch = Engine::invocation(
                args.into_iter().map(OsString::from),
                b"rmake",
                OsStr::new("rmake"),
                None,
                None,
            )
            .unwrap();
            let engine =
                Engine::from_invocation(launch, b"rmake".to_vec(), OsStr::new("rmake")).unwrap();
            let variable = engine.database.variables.get(b"@D").unwrap();
            assert_eq!(variable.origin(), Origin::Automatic);
            assert_eq!(variable.flavor(), Flavor::Recursive);
            assert_eq!(variable.raw_value(), b"$(patsubst %/,%,$(dir $@))");
        }
    }
}
