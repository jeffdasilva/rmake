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
use crate::makefiles::IncludeSearch;
use crate::recipe_flow::{Cleanup, Execution, Flow, Shutdown, Status};
use rmake_core::diagnostic::{Diagnostic, ExpansionContext, Output, OutputReceiver};
use rmake_core::parser::{Database, Dependency, Location, Makefile, Recipe, Rule, trim, words};
use rmake_core::variables::{
    Assignment, ExpandError, Flavor, Origin, Pattern, TargetScope, VariableStore,
};
use rmake_linux::filesystem::Runtime as FilesystemRuntime;
use rmake_linux::jobserver::{Jobserver, Permit};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::ffi::{OsStr, OsString};
use std::fs::{self, FileTimes, OpenOptions};
use std::io::{self, Write};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::Path;
use std::process::Command;
use std::time::{Duration, SystemTime};

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
    let mut name = directory.to_vec();
    if !prefix.is_empty() {
        name.push(b'/');
        name.extend_from_slice(prefix);
    }
    if name.last() != Some(&b'/') {
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
    let mut first = HashMap::new();
    let mut promotions = Vec::new();
    for (index, (dependency, ready)) in dependencies.iter().zip(ready).enumerate() {
        if !ready || dependency.hidden || dependency.name == b".WAIT" {
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

#[derive(Clone, Copy, Default)]
struct Outcome {
    time: Option<SystemTime>,
    ran: bool,
    failed: bool,
    fatal: bool,
    question_outdated: bool,
    blocked: bool,
    // A real recipe invalidates its timestamp; a parent may search again.
    search_after_update: bool,
    completion: CompletionTime,
}

/// Freshness is a per-parent decision, independent of the intermediate's final
/// timestamp: a later branch can deliberately restore an older date.
struct Inspection {
    outcome: Outcome,
    pending: bool,
    needs_rebuild: bool,
}

fn merge_child(outcome: &mut Outcome, child: Outcome) {
    outcome.ran |= child.ran;
    outcome.failed |= child.failed;
    outcome.fatal |= child.fatal;
    outcome.blocked |= child.failed;
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
        if globals
            .get(name)
            .is_some_and(|variable| variable.export_default_reset())
        {
            // A pattern conditional can explicitly reset an
            // imported name's policy. Re-evaluate default
            // eligibility from the selected binding instead
            // of retaining the startup name-list decision.
            return default_export();
        }
        if globals
            .get(name)
            .is_some_and(|variable| variable.is_private())
        {
            // A suppressed private conditional can mark the
            // global binding private. Its export policy is then
            // hidden too; export-all still applies independently
            // to an unrelated visible target binding.
            export_all
                && automatic_export_name(name)
                && environment.get(name).is_some_and(|variable| {
                    !matches!(variable.origin(), Origin::Default | Origin::Automatic)
                })
        } else {
            !unexports.contains(name) && default_export()
        }
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
                controller.write_parts(true, &[&location_prefix(location), message, b"\n"]);
            } else {
                controller.write_parts(true, &[program, b": ", message, b"\n"]);
            }
        }
    }
}

fn emit_getcwd_error(output: &crate::output::Controller, program: &[u8], error: &io::Error) {
    let detail = os_error(error);
    let message = crate::messages::system(rmake_core::messages::Message::GetcwdError(
        detail.as_bytes(),
    ));
    emit_output(
        output,
        program,
        Output::Warning {
            location: None,
            message: &message,
        },
    );
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
    fn reader_event(&mut self, event: rmake_core::diagnostic::ReaderEvent<'_>) {
        crate::debug::reader(self.controller, event);
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

/// Rule observations borrowed until recipe dispatch; disabled tracing allocates
/// nothing and does not add state to suspended continuations.
struct TraceInputs<'a> {
    branch: Option<usize>,
    location: Option<&'a Location>,
    dependencies: &'a [Dependency],
    dropped: &'a HashSet<Vec<u8>>,
}

/// Interpreter-owned continuation. Workers receive only one prepared Command;
/// subsequent command configuration and exports are evaluated after completion.
struct RecipeState {
    branch: Option<usize>,
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

struct RecipeEnvironment {
    values: Vec<(OsString, Option<OsString>)>,
    order: Vec<Vec<u8>>,
}

/// A dependency traversal can yield without holding a recipe slot. Only the
/// interpreter owns these frames; effects and secondary expansion run once.
struct RuleState {
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
    serial_dependency: Option<usize>,
    serial_phase: bool,
    inspections: HashMap<usize, (u64, Outcome)>,
    // Rebuild evidence is rare; keep ordinary suspended graph frames compact.
    #[allow(clippy::box_collection)]
    intermediate_stale: Option<Box<HashSet<usize>>>,
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

/// Double-colon branches remain sequential for one target, while their
/// unfinished dependencies yield to unrelated targets in the enclosing graph.
struct DoubleColonState {
    depth: usize,
    before: Option<SystemTime>,
    phony: bool,
    branches: std::collections::VecDeque<(usize, Option<Rule>)>,
    active: Option<usize>,
    last_epoch: Option<u64>,
    combined: Outcome,
}

/// Only grouped rules need a second recipe view and peer traversal cursor.
struct GroupState {
    recipe_rule: Rule,
    peer_segments: Vec<(usize, Vec<u8>, usize)>,
    peer_targets: std::collections::VecDeque<(Vec<u8>, usize)>,
    run: GroupRun,
}

struct GroupRun {
    branch: Option<usize>,
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

struct ImplicitSelection {
    name: Vec<u8>,
    rule: Rule,
    pattern_scope: Option<Box<Option<TargetScope>>>,
    explicit_names: Vec<Vec<u8>>,
    intermediate_names: Vec<Vec<u8>>,
}

// A selected Makefile record can name a file that does not exist yet.
// Keep the search identity separate from its optional filesystem timestamp.
type PathObservation = Option<(Option<Vec<u8>>, Option<SystemTime>)>;

struct FailedOwner {
    target: Vec<u8>,
    location: Location,
    ignore: bool,
    output: Option<std::rc::Rc<crate::output::Buffer>>,
}

pub struct Engine {
    options: Options,
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
    database_contexts: HashMap<Vec<u8>, VariableStore>,
    database_branch_contexts: HashMap<(Vec<u8>, usize), VariableStore>,
    database_branch_outcomes: HashMap<(Vec<u8>, usize), Outcome>,
    database_unchecked_times: RefCell<HashSet<Vec<u8>>>,
    database_delete_policy: Cell<bool>,
    database_times: RefCell<HashMap<Vec<u8>, Option<SystemTime>>>,
    database_makefiles: HashMap<Vec<u8>, Outcome>,
    database_secondary: HashSet<Vec<u8>>,
    database_remake_started: HashSet<Vec<u8>>,
    program: Vec<u8>,
    makelevel: u32,
    inherited_exports: HashSet<Vec<u8>>,
    command_replay: bool,
    done: HashMap<Vec<u8>, Outcome>,
    visiting: HashSet<Vec<u8>>,
    scope_parents: HashMap<Vec<u8>, Option<Vec<u8>>>,
    target_contexts: HashMap<Vec<u8>, VariableStore>,
    implicit_pattern_scopes: HashMap<Vec<u8>, Option<TargetScope>>,
    impossible_prerequisites: HashSet<Vec<u8>>,
    impossible_generation: u64,
    would_build: bool,
    parallel_jobs: ParallelJobs,
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
    optional_remake: bool,
    optional_remake_failures: HashSet<Vec<u8>>,
    vpath_dirs: Vec<Vec<u8>>,
    gpath_dirs: HashSet<Vec<u8>>,
    makefile_names: HashSet<Vec<u8>>,
    aliases: Aliases,
    path_observations: HashMap<Vec<u8>, PathObservation>,
    resolved_names: HashMap<Vec<u8>, Vec<u8>>,
    archive_vpath_reads: RefCell<Option<Box<ArchiveVpathReads>>>,
    archive_completion_times: HashMap<Vec<u8>, ArchiveCompletion>,
    makefile_error: RefCell<Option<(Option<Location>, Vec<u8>)>>,
    initial_flags: Vec<u8>,
    initial_gnu_flags: Vec<u8>,
    intermediate_policy: crate::intermediates::Policy,
    intermediates: HashSet<Vec<u8>>,
    created_intermediates: Vec<Vec<u8>>,
    jobserver: Option<Jobserver>,
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
                crate::output::Mode::parse(raw).ok_or_else(|| {
                    Diagnostic::from([b"unknown output-sync type '".as_slice(), raw, b"'"].concat())
                })
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
            reader_output = output.make_buffer();
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
                .variables
                .define(
                    b"MAKEFLAGS",
                    &initial_flags,
                    Assignment::Recursive,
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
        if let Some(authentication) = &options.sync_mutex {
            retain!(output.decode_mutex(authentication).map_err(|error| {
                Diagnostic::from(
                    [
                        b"cannot open output sync mutex ".as_slice(),
                        authentication
                            .strip_prefix(b"fnm:")
                            .unwrap_or(authentication),
                        b": ",
                        os_error(&error).as_bytes(),
                    ]
                    .concat(),
                )
            }));
        }
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
            database_contexts: HashMap::new(),
            database_branch_contexts: HashMap::new(),
            database_branch_outcomes: HashMap::new(),
            database_unchecked_times: RefCell::new(HashSet::new()),
            database_delete_policy: Cell::new(false),
            database_times: RefCell::new(HashMap::new()),
            database_makefiles: HashMap::new(),
            database_secondary: HashSet::new(),
            database_remake_started: HashSet::new(),
            program,
            makelevel,
            inherited_exports,
            command_replay,
            done: HashMap::new(),
            visiting: HashSet::new(),
            scope_parents: HashMap::new(),
            target_contexts: HashMap::new(),
            implicit_pattern_scopes: HashMap::new(),
            impossible_prerequisites: HashSet::new(),
            impossible_generation: 0,
            would_build: false,
            parallel_jobs: ParallelJobs::default(),
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
            optional_remake: false,
            optional_remake_failures: HashSet::new(),
            vpath_dirs: Vec::new(),
            gpath_dirs: HashSet::new(),
            makefile_names: HashSet::new(),
            aliases: Aliases::default(),
            path_observations: HashMap::new(),
            resolved_names: HashMap::new(),
            archive_vpath_reads: RefCell::new(None),
            archive_completion_times: HashMap::new(),
            makefile_error: RefCell::new(None),
            initial_flags,
            initial_gnu_flags: Vec::new(),
            intermediate_policy: crate::intermediates::Policy::default(),
            intermediates: HashSet::new(),
            created_intermediates: Vec::new(),
            jobserver: None,
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
                    .variables
                    .define(
                        b"MAKEFLAGS",
                        &flags,
                        Assignment::Recursive,
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
                let result = crate::shell::run(
                    request,
                    &self.program,
                    self.makelevel,
                    &self.output,
                    self.jobserver.as_ref(),
                );
                self.filesystem.borrow_mut().child_completed();
                result
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
            let result = crate::shell::run(
                request,
                &self.program,
                self.makelevel,
                &self.output,
                self.jobserver.as_ref(),
            );
            self.filesystem.borrow_mut().child_completed();
            result
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

    fn show_makefile_error(&self) {
        if let Some((location, message)) = self.makefile_error.borrow_mut().take() {
            emit_output(
                &self.output,
                &self.program,
                Output::Warning {
                    location: location.as_ref(),
                    message: &message,
                },
            );
        }
    }

    fn stderr(&self, body: &[u8]) {
        self.output
            .write_parts(true, &[&self.program, b": ", body, b"\n"]);
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
        for name in words(&names) {
            let path = match crate::makefiles::prepare_argument(
                name,
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
                    let result = crate::shell::run(
                        request,
                        &self.program,
                        self.makelevel,
                        &self.output,
                        self.jobserver.as_ref(),
                    );
                    self.filesystem.borrow_mut().child_completed();
                    result
                }),
            ) {
                Ok(path) => path,
                Err(error) => {
                    self.expansion_error(None, &error);
                    return Err(());
                }
            };
            let name = path.as_os_str().as_bytes();
            crate::debug::reader(
                &self.output,
                rmake_core::diagnostic::ReaderEvent::Reading {
                    name,
                    search: true,
                    optional: true,
                    no_default_goal: true,
                    no_tilde: false,
                },
            );
            self.database.file_records.insert(name);
            let file = match self.include_search.borrow().read(name) {
                Ok(file) => file,
                Err(error) => {
                    if error == b"Is a directory" {
                        self.fatal(None, &[name, b": ", &error].concat());
                        return Err(());
                    }
                    self.database.makefiles.push(Makefile {
                        name: name.to_vec(),
                        optional: true,
                        missing: true,
                        read_error: None,
                    });
                    continue;
                }
            };
            found = true;
            self.database.makefiles.push(Makefile {
                name: file.name.clone(),
                optional: true,
                missing: false,
                read_error: None,
            });
            if let Err(error) = self.database.variables.append_literal_at(
                b"MAKEFILE_LIST",
                &file.name,
                Origin::File,
                &Location {
                    file: name.into(),
                    line: 1,
                },
            ) {
                self.expansion_error(None, &error);
                return Err(());
            }
            if let Err(error) = self.database.read_preload_input(
                file.input,
                name,
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
        self.print_database();
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
                Err(_) => {
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
            let mut files = self.options.files.clone();
            if files.is_empty() {
                let mut default_file = None;
                for name in ["GNUmakefile", "makefile", "Makefile"] {
                    if let Err(error) = self.directory_member(name.as_bytes()) {
                        self.expansion_error(None, &error);
                        return 2;
                    }
                    if Path::new(name).is_file() {
                        default_file = Some(name);
                        break;
                    }
                }
                if let Some(file) = default_file {
                    files.push(file.into());
                } else {
                    // Preloaded rules may generate a default main makefile.
                    for name in [b"GNUmakefile".as_slice(), b"makefile", b"Makefile"] {
                        self.database.file_records.insert(name);
                        self.database.makefiles.push(Makefile {
                            name: name.to_vec(),
                            optional: true,
                            missing: true,
                            read_error: None,
                        });
                    }
                }
            }
            found_makefile = startup_found
                || files
                    .iter()
                    .enumerate()
                    .any(|(index, _)| self.options.temp_stdin_file != Some(index));
            for file in &files {
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
                        emit_output(
                            &self.output,
                            &self.program,
                            Output::Warning {
                                location: None,
                                message: format!("{}: {}", file.display(), os_error(&err))
                                    .as_bytes(),
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
                };
                found_makefile = true;
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
                    .with_includes(&mut |name| self.include_search.borrow().read(name)),
                ) {
                    self.fatal_diagnostic(err.source_location(), &err.message, false);
                    return 2;
                }
            }
            if self.read_runtime_flags(restarts).is_err() {
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
            if let Some(authentication) = &self.options.sync_mutex
                && let Err(error) = self.output.decode_mutex(authentication)
            {
                self.fatal(
                    None,
                    &[
                        b"cannot open output sync mutex ".as_slice(),
                        authentication
                            .strip_prefix(b"fnm:")
                            .unwrap_or(authentication),
                        b": ",
                        os_error(&error).as_bytes(),
                    ]
                    .concat(),
                );
                return 2;
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
                    let result = crate::shell::run(
                        request,
                        &self.program,
                        self.makelevel,
                        &self.output,
                        self.jobserver.as_ref(),
                    );
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
            if let Err(error) = self.setup_jobserver(true) {
                self.fatal(None, error.as_bytes());
                return 2;
            }
            if self.options.jobs.unwrap_or(1) == 1 && self.output.mode().root_buffered() {
                self.output.select(None);
                if let Some(buffer) = self.reader_output.take() {
                    buffer.dump();
                }
                self.output.set_mode(crate::output::Mode::None);
            } else {
                match self.output.setup(self.options.sync_mutex.as_deref()) {
                    Ok(authentication) => self.options.sync_mutex = authentication,
                    Err(error) => {
                        self.fatal(
                            None,
                            format!("cannot open output sync mutex: {}", os_error(&error))
                                .as_bytes(),
                        );
                        return 2;
                    }
                }
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
            if let Err(error) = self.database.variables.define(
                b"MAKEFLAGS",
                &runtime_flags,
                Assignment::Recursive,
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
                files.bind(&mut self.database);
            }
            self.output.select(None);
            if let Some(buffer) = self.reader_output.take() {
                buffer.dump();
            }
            self.observe_interrupt();
            if self.interrupt.is_some() {
                return 2;
            }
            if let Err(error) = self.publish_phase_makeflags(true) {
                self.expansion_error(None, &error);
                return 2;
            }
            crate::debug::emit(&self.output, crate::debug::Event::UpdatingMakefiles);
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
            let debug_before_remake = self.output.debug_mask();
            if !debug_before_remake.has(crate::debug::Mask::MAKEFILES) {
                self.output.set_debug_mask(crate::debug::Mask::NONE);
            }
            self.remaking = true;
            let mut changed = false;
            let mut failed = false;
            let mut seen = HashSet::new();
            let read_files = self.database.makefiles.clone();
            let required_names: HashSet<_> = read_files
                .iter()
                .filter(|file| !file.optional)
                .map(|file| file.name.clone())
                .collect();
            for mut makefile in read_files {
                // The published stdin source is already a completed input,
                // retained across restarts. It is never a makefile remake goal.
                if self.options.temp_stdin_file.is_some_and(|index| {
                    rmake_core::filenames::without_dot_prefix(
                        self.options.files[index].as_os_str().as_bytes(),
                    ) == makefile.name
                }) {
                    continue;
                }
                self.database_remake_started.insert(makefile.name.clone());
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
                // Global symbol lookup can load a logical module name whose
                // file is absent. Loader success does not make that file exist.
                if self.database.loaded_modules.contains_key(&makefile.name) {
                    makefile.missing = self.modified(&makefile.name).is_none();
                }
                let promoted_required =
                    makefile.optional && required_names.contains(&makefile.name);
                if promoted_required {
                    makefile.optional = false;
                }
                if !self.ignores_file_rules(&makefile.name)
                    && !self.resolve_implicit(&makefile.name)
                {
                    self.remove_intermediates();
                    return 2;
                }
                let before = self.modified(&makefile.name);
                if !self.database.rules.contains_key(&makefile.name)
                    && (!makefile.missing || makefile.optional)
                {
                    if before.is_some() {
                        self.database_secondary.insert(makefile.name.clone());
                    }
                    self.database_makefiles.insert(
                        makefile.name.clone(),
                        Outcome {
                            time: before,
                            failed: before.is_none(),
                            ..Outcome::default()
                        },
                    );
                    continue;
                }
                *self.makefile_error.borrow_mut() = if makefile.optional {
                    None
                } else if promoted_required && makefile.missing && makefile.read_error.is_none() {
                    Some((
                        None,
                        [makefile.name.as_slice(), b": No such file or directory"].concat(),
                    ))
                } else {
                    makefile.read_error.map(|(location, error)| {
                        (
                            Some(location),
                            [makefile.name.as_slice(), b": ", &error].concat(),
                        )
                    })
                };
                self.optional_remake = makefile.optional;
                let result = self.build(&makefile.name, None, 0);
                self.optional_remake = false;
                self.makefile_error.borrow_mut().take();
                failed |= result.failed && !makefile.optional;
                if !result.failed {
                    self.database_unchecked_times
                        .borrow_mut()
                        .remove(&makefile.name);
                }
                changed |= !result.failed && self.modified(&makefile.name) != before;
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
            self.output.set_debug_mask(debug_before_remake);
            self.remaking = false;
            if self.shutdown.get() != Shutdown::Running {
                self.remove_intermediates();
                return 2;
            }
            if changed {
                self.remove_intermediates();
                break 'makefiles true;
            }
            if failed {
                self.remove_intermediates();
                return 2;
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
        self.unlink_stdin();
        crate::debug::emit(&self.output, crate::debug::Event::UpdatingGoals);
        let mut goals = if self.options.goals.is_empty() {
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
        if self.shuffle.enabled() {
            let disabled = self.special(b".NOTPARALLEL", b"", true);
            self.shuffle.start(&mut goals, &self.database, disabled);
        }
        let mut failed = false;
        let mut requested = Vec::new();
        for goal in goals {
            self.graph_epoch = self.graph_epoch.wrapping_add(1);
            let already_done =
                self.done.contains_key(self.aliases.resolve(&goal)) || self.pending(&goal);
            if already_done && !self.parallel_graph() {
                self.wait_target(&goal);
            }
            let outcome = self.build(&goal, None, 0);
            let outcome = if !self.parallel_graph() && !self.pending(&goal) {
                self.observe_updated(&goal, outcome)
            } else {
                outcome
            };
            if self.parallel_graph() {
                requested.push((goal, already_done, outcome));
            } else {
                failed |= outcome.failed;
                self.report_goal(&goal, already_done, outcome);
            }
            if outcome.fatal || (outcome.failed && !self.options.keep_going) {
                break;
            }
        }
        while requested.iter().any(|(goal, _, _)| self.pending(goal)) {
            self.graph_epoch = self.graph_epoch.wrapping_add(1);
            let pass_epoch = self.graph_epoch;
            for (goal, _, _) in &requested {
                if self.graph_pending(goal) {
                    self.build(goal, None, 0);
                }
            }
            // A completion observed while walking a later branch may make an
            // earlier one ready. Rescan that work before blocking on a child
            // which can itself be waiting for the newly ready recipe.
            if self.graph_epoch == pass_epoch
                && requested.iter().any(|(goal, _, _)| self.pending(goal))
                && !self.parallel_jobs.is_empty()
            {
                self.reap_job();
            }
        }
        for (goal, already_done, initial) in requested {
            let outcome = self
                .done
                .get(self.aliases.resolve(&goal))
                .copied()
                .unwrap_or(initial);
            let outcome = self.observe_updated(&goal, outcome);
            failed |= outcome.failed;
            self.report_goal(&goal, already_done, outcome);
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

    fn report_goal(&self, goal: &[u8], already_done: bool, outcome: Outcome) {
        if outcome.fatal {
            return;
        }
        if outcome.failed
            && self.options.keep_going
            && outcome.blocked
            && !self.options.question
            && !self.options.dry_run
        {
            self.stderr(&crate::messages::system(
                rmake_core::messages::Message::TargetNotRemade(goal),
            ));
        } else if !outcome.failed
            && (!outcome.ran || already_done)
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

    fn special(&self, special: &[u8], target: &[u8], empty_means_all: bool) -> bool {
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
        if self
            .options
            .goals
            .iter()
            .any(|goal| self.aliases.resolve(goal) == target)
        {
            return false;
        }
        if self.special(b".INTERMEDIATE", target, false)
            || self.special(b".SECONDARY", target, false)
        {
            return true;
        }
        !self.special_for_rule(b".NOTINTERMEDIATE", target)
            && !self.intermediate_policy.no_intermediates
            && (self.intermediate_policy.global_intermediate(target)
                || self.intermediates.contains(target))
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
        if self.created_intermediates.is_empty() {
            return;
        }
        if self.intermediate_policy.all_secondary || self.intermediate_policy.no_intermediates {
            self.created_intermediates.clear();
            return;
        }
        let created: HashSet<_> = std::mem::take(&mut self.created_intermediates)
            .into_iter()
            .collect();
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
                let mut out = io::stdout().lock();
                let _ = out.write_all(&line);
                let _ = out.flush();
                drop(out);
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
            return Ok(self.modified(target));
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
        parent: &[u8],
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
            .or_insert_with(|| Some(parent.to_vec()));
        if !self.resolve_implicit(target) {
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
            if let Err(error) = self.expand_dependencies(target, &mut rule) {
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
        let mut result = Outcome {
            time,
            ..Outcome::default()
        };
        let mut stale = self.options.always;
        let mut waiting = false;
        let serial = self.special(b".NOTPARALLEL", target, false);
        self.visiting.insert(target.to_vec());
        let mut next = 0;
        let mut order = self.shuffle.order(target, rule.dependencies.len());
        let mut dropped = false;
        while next < rule.dependencies.len() {
            let index = order.get(next).copied().unwrap_or(next);
            let dep = &rule.dependencies[index];
            if dep.name == b".WAIT" {
                if waiting {
                    break;
                }
                next += 1;
                continue;
            }
            if self.visiting.contains(&dep.name) {
                self.warn_circular_dependency(target, &dep.name);
                rule.dependencies.remove(index);
                self.shuffle.discard(target, index);
                if !order.is_empty() {
                    order.retain(|i| *i != index);
                    for i in &mut order {
                        if *i > index {
                            *i -= 1;
                        }
                    }
                }
                dropped = true;
                continue;
            }
            next += 1;
            let Inspection {
                outcome: child,
                pending,
                needs_rebuild,
            } = self.check_intermediate(&dep.name, target, time, depth + 1);
            merge_child(&mut result, child);
            waiting |= pending;
            // A running child has no final timestamp yet. Never materialize an
            // intermediate just because that provisional timestamp is absent.
            if !pending {
                stale |= !dep.order_only && needs_rebuild;
            }
            if child.fatal || (child.failed && !self.options.keep_going) || (serial && pending) {
                break;
            }
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
        if self.remaking
            || self.options.inherited_jobs
            || self
                .database
                .rules
                .get(b".NOTPARALLEL".as_slice())
                .is_some_and(|rule| rule.dependencies.iter().all(|dep| dep.hidden))
        {
            1
        } else if self.jobserver.is_some() {
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
            self.child_events(true, Some(server.readiness()))?;
            self.observe_interrupt();
            if self.graph_abort.is_some() || (self.parallel_failed && !self.options.keep_going) {
                return Ok(None);
            }
        }
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
            && !self.options.dry_run
            && !self.options.question
            && !self.options.touch
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
        additional: Option<std::os::fd::BorrowedFd<'_>>,
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
        let ready = {
            let mut workers =
                if block && !children.is_empty() && self.shutdown.get() == Shutdown::Running {
                    // Optional readiness must not prevent reaping owned children.
                    rmake_linux::guile::WorkerWait::begin()
                        .ok()
                        .filter(|wait| wait.descriptor().is_some())
                } else {
                    None
                };
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
                    maximum_wait,
                )?;
                let Some(wait) = &mut workers else {
                    break events.children;
                };
                drop(children);
                self.observe_interrupt();
                if self.interrupt.is_some() {
                    break events.children;
                }
                if events.notification_only {
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
                break events.children;
            }
        };
        #[cfg(not(feature = "guile"))]
        let ready = rmake_linux::child::wait(&children, block, true, additional, load_retry)?;
        // Idle nonblocking polls are an internal graph refresh, rather than a
        // visible reaping pass. A ready completion still reports its live scan.
        if !block && !ready.is_empty() {
            self.job_debug.live(self.output.debug_mask());
        }
        self.observe_interrupt();
        for identity in ready {
            let (target, job) = self
                .parallel_jobs
                .entries_mut()
                .find(|(_, job)| job.identity == identity)
                .expect("live child identity");
            let result = job.child.as_mut().expect("live child").try_wait();
            let result = match result {
                Ok(Some(status)) => Some(Ok(status)),
                Ok(None) => continue,
                Err(error) => Some(Err(error)),
            };
            let target = target.to_vec();
            self.complete_job((target, identity, result));
            return Ok(true);
        }
        Ok(false)
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
                return Ok(SerialWait {
                    status,
                    cleaned,
                    #[cfg(feature = "guile")]
                    interpreter_failed,
                });
            }
            #[cfg(feature = "guile")]
            let worker_wake = workers.descriptor();
            #[cfg(not(feature = "guile"))]
            let worker_wake = None;
            rmake_linux::child::wait(&[(0, child)], true, true, worker_wake, None)?;
        }
    }

    fn load_high(&mut self) -> bool {
        let Some(limit) = self.options.load_limit else {
            return false;
        };
        let (high, error) = self.load.high(limit);
        if let Some(error) = error {
            if error.raw_os_error() == Some(0) {
                self.stderr(b"cannot enforce load limits on this operating system");
            } else {
                self.stderr(
                    format!(
                        "cannot enforce load limit: getloadavg: {}",
                        os_error(&error)
                    )
                    .as_bytes(),
                );
            }
        }
        high
    }

    fn start_load_waiting(&mut self) {
        if self.graph_abort.is_some() || self.shutdown.get() != Shutdown::Running {
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
        while !self.load_waiting.is_empty() {
            if self.parallel_jobs.values().any(|job| !job.waiting_load) && self.load_high() {
                break;
            }
            let (target, identity) = self.load_waiting.pop().expect("nonempty waiting queue");
            self.complete_job((target, identity, None));
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

    /// The inherited slot belongs to the live pool, not permanently to its
    /// first recipe. Move an outstanding paid lease into the newly freed slot
    /// and return that lease's original byte to other jobserver participants.
    fn reuse_implicit_job_slot(&mut self) {
        if self.jobserver.is_none()
            || self.parallel_jobs.values().any(|job| {
                job.continuation
                    .permit
                    .as_ref()
                    .is_some_and(Permit::is_implicit)
            })
        {
            return;
        }
        if let Some((_, job)) = self
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
            let paid = job.continuation.permit.replace(Permit::implicit());
            drop(paid);
        }
    }

    fn complete_job(&mut self, (target, identity, result): JobResult) {
        self.graph_epoch = self.graph_epoch.wrapping_add(1);
        let mut job = self
            .parallel_jobs
            .remove(&target, identity)
            .expect("reserved parallel target");
        let output = job.continuation.output.clone();
        let debug_job = job.continuation.debug_job.clone();
        if let Some(result) = &result {
            crate::debug::Jobs::reaped(
                self.output.debug_mask(),
                debug_job.as_ref(),
                result.as_ref().is_ok_and(|status| status.success()),
            );
        }
        if result.as_ref().is_some_and(|status| status.is_ok())
            && let Some(buffer) = &output
        {
            buffer.child_reaped(true);
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
        let error = match result {
            None => None,
            Some(Ok(status)) if status.success() => None,
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
        let mut failed = error.is_some() && !job.ignore;
        if let Some(error) = error
            && (!job.ignore || !self.globally_silent())
        {
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
        if failed && launched && !job.signal_cleaned {
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
        if !failed {
            let execution =
                self.continue_recipe(&target, job.continuation, job.phony, job.before, true);
            failed |= matches!(execution.status, Status::Failure | Status::Fatal);
            outcome.fatal |= execution.status == Status::Fatal;
            if self.cleanup_target(&target, job.before, job.phony, execution.cleanup) {
                failed = true;
                outcome.fatal = true;
            }
        }
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
        // Inspect archive metadata only after the last command exits. A child
        // may temporarily leave a partial header while updating its archive.
        if !failed
            && !self.parallel_jobs.contains_key(&target)
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
        self.parallel_failed |= failed;
        outcome.ran = true;
        outcome.failed |= failed;
        self.record_graph_failure(outcome);
        if !self.parallel_jobs.contains_key(&target) {
            self.release_assumed_new(&target);
        }
        outcome.time = if job.phony {
            None
        } else if self.parallel_jobs.contains_key(&target) {
            outcome.time
        } else {
            self.modified(&target)
        };
        outcome.search_after_update = self.update_search_pending(&target, job.phony, true);
        if !job.phony && !self.parallel_jobs.contains_key(&target) {
            self.database_unchecked_times
                .borrow_mut()
                .insert(target.clone());
        }
        self.done.insert(target.clone(), outcome);
        if !self.parallel_jobs.contains_key(&target) {
            self.finish_group(&target, outcome);
        }
        self.finish_debug_job(debug_job.as_ref());
        self.reuse_implicit_job_slot();
        self.job_debug.live(self.output.debug_mask());
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
            || self.visiting.contains(target)
            || self.special(b".PHONY", target, false)
        {
            return Ok(None);
        }
        if let Some((archive, member)) = rmake_core::filenames::archive_parts(target) {
            let location = self
                .archive_location(target)
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
                    .archive_observation_at(target, &location)
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
                        .archive_observation_at(target, &location)
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
                .archive_observation_at(target, &location)
                .map_err(|error| self.expansion_error(None, &error))?;
            if location.container_time.is_some() {
                self.path_observations.insert(target.to_vec(), observation);
            }
            return Ok(None);
        }
        let observation = self
            .find_vpath(target)
            .map_err(|error| self.expansion_error(None, &error))?;
        if let Some((Some(name), _)) = &observation
            && (self.gpath_match(target, name) || self.other_file_record(name, target))
        {
            let name = name.clone();
            self.rename_search_record(target, &name)?;
            self.path_observations
                .insert(name.clone(), observation.map(|(_, time)| (None, time)));
            return Ok(Some(name));
        }
        if let Some((Some(name), _)) = &observation {
            self.database.file_records.rehash(target, name);
        }
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
        if let Some(scope) = self.database.target_scopes.remove(from) {
            self.database
                .target_scopes
                .entry(to.to_vec())
                .or_default()
                .merge_missing(&scope);
        }
        if let Some(parent) = self.scope_parents.remove(from) {
            self.scope_parents.entry(to.to_vec()).or_insert(parent);
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
        let mut unchecked_times = self.database_unchecked_times.borrow_mut();
        if unchecked_times.remove(from) {
            unchecked_times.insert(to.to_vec());
        }
        drop(unchecked_times);
        if let Some(outcome) = self.database_makefiles.remove(from) {
            self.database_makefiles
                .entry(to.to_vec())
                .or_insert(outcome);
        }
        {
            let times = self.database_times.get_mut();
            if let Some(time) = times.remove(from) {
                times.entry(to.to_vec()).or_insert(time);
            }
        }
        for flags in [
            &mut self.database_implicit,
            &mut self.database_secondary,
            &mut self.database_remake_started,
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
        if let Some(scope) = self.implicit_pattern_scopes.remove(from) {
            self.implicit_pattern_scopes
                .entry(to.to_vec())
                .or_insert(scope);
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
        let time = self.observed_modified(name);
        self.retain_database_time(name, time);
        time
    }

    fn observed_modified(&self, name: &[u8]) -> Option<SystemTime> {
        if let Some(time) = self.assumed_time(name) {
            return Some(time);
        }
        if let Some(cached) = self.archive_completion_times.get(name) {
            return cached.time;
        }
        if !self.options.check_symlink_times && rmake_core::filenames::archive_parts(name).is_some()
        {
            return modified(name);
        }
        if let Some((archive, member)) = rmake_core::filenames::archive_parts(name) {
            self.archive_time(archive, member, self.name_time(member))
        } else {
            self.name_time(name)
        }
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
                    let time = self.assumed_time(&lookup).or_else(|| {
                        if self.options.check_symlink_times {
                            self.name_time(&name)
                        } else {
                            physical
                        }
                    });
                    if let Some(history) = self.database.database_history() {
                        // Search supplies a directory/separator/filename buffer.
                        // Its retained span includes the separator even when
                        // the historical trailing-slash spelling ends earlier.
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
                        history.intern_buffer(&name);
                        name.truncate(visible);
                    }
                    return Ok(Some((Some(name), time)));
                }
            }
        }
        Ok(None)
    }

    fn archive_location(&mut self, target: &[u8]) -> Result<ArchiveLocation, ExpandError> {
        self.archive_location_for(target, false)
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
        Ok(Some((resolved, time)))
    }

    fn archive_observation(
        &mut self,
        target: &[u8],
    ) -> Result<(PathObservation, bool), ExpandError> {
        let location = self.archive_location(target)?;
        self.archive_observation_at(target, &location)
            .map(|observation| (observation, location.container_time.is_some()))
    }

    fn find_vpath(&mut self, target: &[u8]) -> Result<PathObservation, ExpandError> {
        self.database_unchecked_times.borrow_mut().remove(target);
        if let Some(time) = self.assumed_time(target) {
            return Ok(Some((None, Some(time))));
        }
        if rmake_core::filenames::archive_parts(target).is_some() {
            return self
                .archive_observation(target)
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
            return Ok(Some((None, Some(time))));
        }
        let searched = self.searched_file(target)?;
        if searched.is_some() || !target.starts_with(b"-l") || self.makefile_names.contains(target)
        {
            return Ok(searched);
        }
        self.library_search(&target[2..])
    }

    fn library_search(&mut self, library: &[u8]) -> Result<PathObservation, ExpandError> {
        let patterns = self.expand_global(b"$(.LIBPATTERNS)", None)?;
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
            for (rank, directory) in paths {
                let path = searched_name(directory, &name);
                let (lookup, membership) = search_probe_names(directory, &name, &path);
                let declared = self.search_record_matches(&name, &lookup);
                if declared || (self.directory_member(&membership)? && modified(&path).is_some()) {
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
    fn implicit_vpath_exists(&mut self, target: &[u8]) -> Result<bool, ExpandError> {
        if rmake_core::filenames::archive_parts(target).is_none() {
            return if self.directory_member(target)? {
                Ok(true)
            } else {
                Ok(self.searched_file(target)?.is_some())
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
            return Ok(true);
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
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    fn update_search_pending(&self, target: &[u8], phony: bool, all_recursive: bool) -> bool {
        !phony
            && self.vpath_searchable(target)
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
        match self.find_vpath(target) {
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
    fn resolve_implicit(&mut self, target: &[u8]) -> bool {
        self.resolve_implicit_at(target, usize::from(self.remaking))
    }
    fn resolve_implicit_at(&mut self, target: &[u8], debug_depth: usize) -> bool {
        let existing = match self.database.rules.get(target) {
            Some(rule) if rule.recipes.is_some() || rule.double_colon.is_some() => return true,
            rule => rule.cloned(),
        };
        self.database_implicit.insert(target.to_vec());
        let mut budget = 100_000;
        crate::debug::emit(
            &self.output,
            crate::debug::Event::Looking(target, debug_depth + 1),
        );
        let plan =
            match self.implicit_plan(target, &mut Vec::new(), false, &mut budget, debug_depth) {
                Ok(plan) => plan,
                Err(error) => {
                    self.expansion_error(None, &error);
                    return false;
                }
            };
        if plan.is_none() {
            crate::debug::emit(
                &self.output,
                crate::debug::Event::NoRule(target, debug_depth + 1),
            );
        }
        if plan.is_none() && budget == 0 {
            self.fatal(None, b"rmake: implicit rule search resource limit exceeded");
            return false;
        }
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
            // Recursive searches return unentered intermediate records. Commit
            // each accepted child's input list when its parent installs it;
            // the parent's candidate inputs are processed in reverse order.
            let inferred: HashMap<_, _> = plan
                .iter()
                .map(|selection| (selection.name.as_slice(), &selection.rule))
                .collect();
            for selection in &plan {
                for dependency in selection.rule.dependencies.iter().rev() {
                    if let Some(child) = inferred.get(dependency.name.as_slice()) {
                        self.database.file_records.insert(&dependency.name);
                        for input in child.dependencies.iter().filter(|dep| dep.name != b".WAIT") {
                            self.database.file_records.insert(&input.name);
                        }
                    } else if selection.name == target && dependency.name != b".WAIT" {
                        self.database.file_records.insert(&dependency.name);
                    }
                }
                if let Some(group) = &selection.rule.group {
                    for peer in &group.targets {
                        if peer != &selection.name {
                            self.database.file_records.insert(peer);
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
                    pattern_scope,
                    explicit_names: _,
                    intermediate_names,
                } = selection;
                // A selected implicit rule promotes its file to a target,
                // including recipes converted from undeclared builtin records.
                rule.record_only = false;
                if let Some(pattern) = pattern_scope {
                    self.implicit_pattern_scopes.insert(name.clone(), *pattern);
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
                if let Some(explicit) = self.database.rules.get(&name) {
                    rule.append_prerequisites(explicit);
                }
                if name != target
                    && !self.database.explicit_names.contains(&name)
                    && !self.options.goals.contains(&name)
                {
                    self.intermediates.insert(name.clone());
                }
                self.intermediates.extend(intermediate_names);
                if self.shuffle.enabled() {
                    let disabled = self.special(b".NOTPARALLEL", b"", true);
                    self.shuffle
                        .regenerate(&name, &rule, &self.database, disabled);
                }
                self.database.rules.insert(name, rule);
            }
        } else if existing.as_ref().is_none_or(|rule| rule.record_only)
            && let Some(default) = self.database.rules.get(b".DEFAULT".as_slice())
            && default.recipes.is_some()
        {
            self.database.rules.insert(
                target.to_vec(),
                Rule {
                    recipes: default.recipes.clone(),
                    recipe_location: default.recipe_location.clone(),
                    default_recipe: true,
                    dependencies: existing.map_or_else(Vec::new, |rule| rule.dependencies),
                    ..Rule::default()
                },
            );
        }
        true
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
        for archive_search in [false, true] {
            if let Some(plan) = self.implicit_search(
                target,
                active,
                compatibility,
                archive_search,
                budget,
                debug_depth,
            )? {
                return Ok(Some(plan));
            }
        }
        Ok(None)
    }

    fn implicit_search(
        &mut self,
        target: &[u8],
        active: &mut Vec<usize>,
        compatibility: bool,
        archive_search: bool,
        budget: &mut usize,
        debug_depth: usize,
    ) -> Result<Option<Vec<ImplicitSelection>>, ExpandError> {
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
        let mut pattern_scope = None;
        for chain in [false, true] {
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
                let debug_label = self
                    .output
                    .debug_mask()
                    .has(crate::debug::Mask::IMPLICIT)
                    .then(|| crate::debug::rule_label(&self.database, candidate.identity));
                if let Some(label) = &debug_label {
                    crate::debug::emit(
                        &self.output,
                        crate::debug::Event::TryingRule(
                            label,
                            candidate.rule.stem.as_deref().unwrap_or_default(),
                            debug_depth + active.len() + 2,
                        ),
                    );
                }
                let mut plan = Vec::new();
                let mut explicit_names = Vec::new();
                let mut intermediate_names = Vec::new();
                let mut dependencies = Vec::new();
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
                                let (context, prepared) = self.implicit_variables(
                                    target,
                                    intermediate,
                                    candidate.rule.stem.clone().unwrap_or_default(),
                                )?;
                                pattern_scope = prepared;
                                variables = Some(context);
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
                            let value =
                                self.expand(variables.as_mut().unwrap(), &rewritten, None)?;
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
                            self.refresh_impossible_prerequisites();
                            if name != b".WAIT" && self.impossible_prerequisites.contains(name) {
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
                                crate::debug::Event::TryingPrerequisite(
                                    name,
                                    debug_depth + active.len() + 1,
                                ),
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
                            if name != b".WAIT" && !explicit && !self.implicit_vpath_exists(name)? {
                                if self.implicit_known_file(name) {
                                    if compatibility {
                                        dependencies.push(dependency);
                                        continue;
                                    }
                                    compatibility_needed = true;
                                }
                                if chain {
                                    if let Some(child) = self.implicit_plan(
                                        name,
                                        active,
                                        compatibility,
                                        budget,
                                        debug_depth,
                                    )? {
                                        plan.extend(child);
                                    } else {
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
                        crate::debug::emit(
                            &self.output,
                            crate::debug::Event::FoundRule(
                                label,
                                target,
                                debug_depth + active.len() + 1,
                            ),
                        );
                    }
                    let mut selected = candidate.rule.clone();
                    selected.dependencies = dependencies;
                    selected.secondary = None;
                    plan.push(ImplicitSelection {
                        name: target.to_vec(),
                        rule: selected,
                        explicit_names,
                        intermediate_names,
                        pattern_scope: (intermediate && variables.is_some())
                            .then(|| Box::new(pattern_scope.clone())),
                    });
                    return Ok(Some(plan));
                }
            }
        }
        if compatibility_needed && !compatibility {
            return self.implicit_search(target, active, true, archive_search, budget, debug_depth);
        }
        Ok(None)
    }

    fn implicit_variables(
        &mut self,
        target: &[u8],
        intermediate: bool,
        stem: Vec<u8>,
    ) -> Result<(VariableStore, Option<TargetScope>), ExpandError> {
        let (mut context, pattern_scope) = if intermediate {
            let prepared = self.database.prepare_pattern_scope(
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
                    let result = crate::shell::run(
                        request,
                        &self.program,
                        self.makelevel,
                        &self.output,
                        self.jobserver.as_ref(),
                    );
                    self.filesystem.borrow_mut().child_completed();
                    result
                }),
            )?;
            let context = match prepared.as_ref() {
                Some(scope) => self.database.variables.with_target_scopes(&[scope]),
                None => self.database.variables.with_target_scopes(&[]),
            };
            (context, prepared)
        } else {
            (self.target_variables(target)?, None)
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
        Ok((context, pattern_scope))
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
        }
        if order.is_some() {
            *order_only = true;
        }
        Ok(result)
    }

    fn expand_dependencies(&mut self, target: &[u8], rule: &mut Rule) -> Result<(), ExpandError> {
        let Some(secondary) = rule.secondary.take() else {
            return Ok(());
        };
        let mut variables = self.target_variables(target)?;
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
        for clause in *secondary {
            let index = expanded_next + clause.index - original_next;
            // Empty deferred clauses cannot expose a prerequisite. Avoid
            // repeatedly scanning all remaining placeholders in that case.
            let visible = if ready_count == 0 {
                0
            } else {
                rule.dependencies.len()
            };
            let stem = clause
                .stem
                .as_deref()
                .or(rule.stem.as_deref())
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
                let result = crate::shell::run(
                    request,
                    &self.program,
                    self.makelevel,
                    &self.output,
                    self.jobserver.as_ref(),
                );
                self.filesystem.borrow_mut().child_completed();
                result
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
            let (normal, order) = if order_only {
                (std::borrow::Cow::Borrowed(value.as_slice()), None)
            } else {
                rmake_core::filenames::split_order(&value)
            };
            for (text, role) in [
                (normal.as_ref(), order_only),
                (order.unwrap_or_default(), true),
            ] {
                for name in self.database.rule_filenames(text, &mut context)? {
                    expanded.push(Dependency {
                        name: name.into_owned(),
                        order_only: role,
                        hidden: false,
                    });
                }
            }
            for dependency in expanded.iter().filter(|dep| dep.name != b".WAIT") {
                self.database.file_records.insert(&dependency.name);
            }
            original_next = clause.index + 1;
            expanded_next = index + expanded.len();
            ready_count += expanded.len();
            ready.splice(index..=index, std::iter::repeat_n(true, expanded.len()));
            rule.dependencies.splice(index..=index, expanded);
        }
        if let Some(context) = self.target_contexts.get_mut(target) {
            context.retain_runtime_metadata(&variables);
        }
        if self.shuffle.enabled() {
            let disabled = self.special(b".NOTPARALLEL", b"", true);
            self.shuffle
                .regenerate(target, rule, &self.database, disabled);
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

    fn record_graph_failure(&mut self, outcome: Outcome) {
        if !(outcome.fatal || (outcome.failed && !self.options.keep_going))
            || !self.parallel_graph()
        {
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
        if let Some(status) = self.graph_abort {
            outcome.failed = true;
            outcome.fatal |= status == Status::Fatal;
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
        match self.archive_observation_at(target, &location) {
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

    fn build(&mut self, target: &[u8], parent: Option<&[u8]>, depth: usize) -> Outcome {
        let known_alias = self.aliases.contains(target);
        let renamed = match self.prepare_search_name(target) {
            Ok(name) => name,
            Err(()) => {
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
        let target = renamed.as_deref().unwrap_or(target);
        self.poll_jobs();
        let archive_target = rmake_core::filenames::archive_parts(target).is_some();
        let previously_done = archive_target && self.done.contains_key(target);
        let mut outcome = if archive_target
            && !previously_done
            && self.assumed_time(target).is_none()
            && !self.pending(target)
            && self.archive_metadata_error(target, true)
        {
            Outcome {
                failed: true,
                fatal: true,
                ..Outcome::default()
            }
        } else {
            self.build_inner(target, parent, depth)
        };
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
        if self.optional_remake && outcome.failed && !outcome.fatal {
            self.optional_remake_failures.insert(target.to_vec());
        }
        self.record_graph_failure(outcome);
        outcome
    }

    fn build_inner(&mut self, target: &[u8], parent: Option<&[u8]>, depth: usize) -> Outcome {
        if !self.remaking && self.optional_remake_failures.remove(target) {
            self.report_missing_rule(target, parent);
        }
        if let Some(owner) = self.active_peers.get(target)
            && owner != target
        {
            let mut outcome = self.done.get(owner).copied().unwrap_or_default();
            outcome.time = self.modified(target);
            return outcome;
        }
        if self.pending_double_colon.contains_key(target) {
            return self.resume_double_colon(target);
        }
        if self.pending_rules.contains_key(target) {
            return self.resume_rule(target);
        }
        if let Some(mut outcome) = self.done.get(target).copied() {
            crate::debug::emit(
                &self.output,
                crate::debug::Event::Already(target, depth + usize::from(self.remaking)),
            );
            if outcome.completion == CompletionTime::ArchiveUnknown
                && rmake_core::filenames::archive_parts(target).is_some()
            {
                match self.archive_observation(target) {
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
        if self.graph_abort.is_some() {
            let mut outcome = Outcome::default();
            self.apply_graph_abort(&mut outcome);
            return outcome;
        }
        if self.ignores_file_rules(target) {
            let outcome = Outcome {
                time: self.assumed_time(target),
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
        crate::debug::emit(
            &self.output,
            crate::debug::Event::Considering(target, debug_depth),
        );
        // Record first ownership before implicit secondary expansion can ask
        // for this context. Pattern-scope activation itself remains lazy.
        self.scope_parents
            .entry(target.to_vec())
            .or_insert_with(|| parent.map(Vec::from));
        let phony = self.special(b".PHONY", target, false);
        // Search-time expansion can create or remove this target. GNU decides
        // freshness from the observation made before those effects occur.
        let observed = self.path_observations.remove(target);
        let resolved = if phony && observed.is_none() && self.assumed_time(target).is_none() {
            None
        } else {
            match observed.map(Ok).unwrap_or_else(|| self.find_vpath(target)) {
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
        let mut before = resolved.as_ref().and_then(|(_, time)| *time);
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
        if !phony && !self.resolve_implicit_at(target, debug_depth) {
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
                crate::debug::emit(
                    &self.output,
                    crate::debug::Event::NoNeed(target, debug_depth),
                );
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
            if !self.optional_remake {
                self.show_makefile_error();
                self.report_missing_rule(target, parent);
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
            if self.new_double_colon_entry(target) {
                branches.push_back((branches.len(), None));
            }
            self.pending_double_colon.insert(
                target.to_vec(),
                DoubleColonState {
                    depth,
                    before,
                    phony,
                    branches,
                    active: None,
                    last_epoch: None,
                    combined: Outcome {
                        time: before,
                        ..Outcome::default()
                    },
                },
            );
            let outcome = self.resume_double_colon(target);
            if !self.parallel_graph() {
                self.wait_target(target);
                return self.done.get(target).copied().unwrap_or(outcome);
            }
            return outcome;
        }
        self.build_rule(target, depth, before, comparison, phony, rule, None)
    }

    fn resume_double_colon(&mut self, target: &[u8]) -> Outcome {
        if let Some(state) = self.pending_double_colon.get(target)
            && state.last_epoch == Some(self.graph_epoch)
        {
            return state.combined;
        }
        let mut state = self
            .pending_double_colon
            .remove(target)
            .expect("pending double-colon continuation");
        state.last_epoch = Some(self.graph_epoch);
        loop {
            if let Some(ordinal) = state.active {
                if self.pending_rules.contains_key(target) {
                    self.resume_rule(target);
                }
                if self.pending_rules.contains_key(target)
                    || self.parallel_jobs.contains_key(target)
                {
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
                self.database_branch_outcomes
                    .insert((target.to_vec(), ordinal), result);
                state.combined.time = result.time;
                state.combined.ran |= result.ran;
                state.combined.failed |= result.failed;
                state.combined.fatal |= result.fatal;
                state.combined.blocked |= result.blocked;
                state.combined.question_outdated |= result.question_outdated;
                state.combined.search_after_update |= result.search_after_update;
                state.combined.completion = state.combined.completion.max(result.completion);
                self.record_graph_failure(result);
                state.active = None;
                if result.fatal
                    || ((result.failed || result.question_outdated) && !self.options.keep_going)
                {
                    state.branches.clear();
                }
            }
            self.apply_graph_abort(&mut state.combined);
            if self.graph_abort.is_some() {
                state.branches.clear();
            }
            let Some((ordinal, entry)) = state.branches.pop_front() else {
                state.combined.time = if state.phony {
                    None
                } else if state.combined.completion.preserves() || state.combined.failed {
                    state.combined.time
                } else {
                    self.modified(target)
                };
                self.done.insert(target.to_vec(), state.combined);
                return state.combined;
            };
            let Some(branch) = entry else {
                // An input-only double-colon record has no declared rule or
                // parent diagnostic locus. Process it in the same ordered queue.
                if !self.optional_remake {
                    self.show_makefile_error();
                    self.report_missing_rule(target, None);
                }
                state.combined.failed = true;
                self.record_graph_failure(state.combined);
                if !self.options.keep_going {
                    state.branches.clear();
                }
                continue;
            };
            if let Some(id) = branch.grouped_double_id() {
                if let Some(mut result) = self.completed_group_branches.get(&id).copied() {
                    // Commands executed for a previous goal must not suppress
                    // this goal's up-to-date diagnostic.
                    result.ran = false;
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
            let comparison = self.comparison_time(target, state.before);
            self.build_rule(
                target,
                state.depth,
                state.before,
                comparison,
                state.phony,
                branch,
                Some(ordinal),
            );
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
            self.stderr(
                &[
                    b"*** Warning: .LOW_RESOLUTION_TIME file '".as_slice(),
                    target,
                    b"' has a high resolution time stamp",
                ]
                .concat(),
            );
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
    ) -> Outcome {
        let double_colon = branch.is_some();
        let mut members = Vec::new();
        let grouped = rule.grouped_targets().is_some();
        if grouped {
            let implicit = rule.implicit_pattern.is_some();
            if !implicit {
                comparison = comparison.min(before);
            }
            let double = rule.grouped_double_id().is_some();
            let group = rule.group.as_mut().expect("grouped targets have metadata");
            let mut times = vec![None; group.targets.len()];
            let mut observed = vec![false; group.targets.len()];
            let mut compare_peers = before.is_some() && !implicit;
            for (index, peer) in group.targets.iter_mut().enumerate().rev() {
                let name = self.aliases.resolve(peer);
                if name != peer {
                    *peer = name.to_vec();
                }
                if peer.as_slice() == target {
                    times[index] = before;
                    observed[index] = true;
                    continue;
                }
                if !compare_peers {
                    continue;
                }
                match self.prepare_search_name(peer) {
                    Ok(Some(name)) => *peer = name,
                    Ok(None) => {}
                    Err(()) => {
                        return Outcome {
                            failed: true,
                            fatal: true,
                            ..Outcome::default()
                        };
                    }
                }
                let time = if self.special(b".PHONY", peer, false) {
                    None
                } else {
                    match self
                        .path_observations
                        .get(peer)
                        .cloned()
                        .map(Ok)
                        .unwrap_or_else(|| self.find_vpath(peer))
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
                times[index] = time;
                observed[index] = true;
                comparison = comparison.min(time);
                compare_peers = time.is_some();
            }
            let mut seen = HashSet::new();
            for (index, peer) in group.targets.iter().enumerate() {
                if !seen.insert(peer.clone()) {
                    continue;
                }
                let mut time = times[index];
                if double && observed[index] {
                    time = *self.double_colon_before.entry(peer.clone()).or_insert(time);
                }
                members.push((peer.clone(), time, self.special(b".PHONY", peer, false)));
            }
        }
        let had_secondary = rule.secondary.is_some();
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
        let peer_targets =
            rule.grouped_targets()
                .map_or_else(std::collections::VecDeque::new, |targets| {
                    let mut seen = HashMap::<&[u8], usize>::new();
                    let mut peers = Vec::new();
                    for peer in targets {
                        let ordinal = seen.entry(peer).or_default();
                        if rule.implicit_pattern.is_none() || peer.as_slice() != target {
                            peers.push((
                                peer.clone(),
                                if rule.grouped_double_id().is_some() {
                                    *ordinal
                                } else {
                                    0
                                },
                            ));
                        }
                        *ordinal += 1;
                    }
                    peers.into_iter().rev().collect()
                });
        if grouped {
            // Expansion is per file, not once for every peer traversal.
            self.store_expanded_rule(target, rule.clone());
        }
        let group = rule.grouped_targets().map(|_| {
            Box::new(GroupState {
                recipe_rule: rule.clone(),
                peer_segments: vec![(0, target.to_vec(), rule.grouped_double_ordinal())],
                peer_targets,
                run: GroupRun {
                    branch: rule.grouped_double_id(),
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
        let serial_phase = !rule.dependencies.is_empty()
            && rule.grouped_double_id().is_none()
            && self.special(b".NOTPARALLEL", target, false);
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
            serial_dependency: None,
            serial_phase,
            inspections: HashMap::new(),
            intermediate_stale: None,
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
        if !self.parallel_graph() {
            self.wait_target(target);
            self.done.get(target).copied().unwrap_or(outcome)
        } else {
            outcome
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
        let mut state = self
            .pending_rules
            .remove(target)
            .expect("pending rule frame");
        // A new update pass revisits the parent's cached missing-file state.
        // Resume its owned dependency frame without repeating observations.
        if state.last_epoch.is_some() && state.before.is_none() {
            crate::debug::emit(
                &self.output,
                crate::debug::Event::Missing(target, state.depth + usize::from(self.remaking) + 1),
            );
        }
        state.last_epoch = Some(self.graph_epoch);
        self.apply_graph_abort(&mut state.outcome);
        state.stopped |= self.graph_abort.is_some();
        self.visiting.insert(target.to_vec());
        let mut waiting = false;
        while !state.stopped && self.graph_abort.is_none() {
            if state.next == state.rule.dependencies.len() {
                let Some((peer, ordinal)) = state
                    .group
                    .as_mut()
                    .and_then(|group| group.peer_targets.pop_front())
                else {
                    break;
                };
                let mut peer_rule = self
                    .database
                    .rules
                    .get(&peer)
                    .and_then(|root| {
                        if let Some(id) = state.rule.grouped_double_id() {
                            root.double_colon.as_ref()?.iter().find(|branch| {
                                branch.grouped_double_id() == Some(id)
                                    && branch.grouped_double_ordinal() == ordinal
                            })
                        } else {
                            Some(root)
                        }
                    })
                    .cloned()
                    .unwrap_or_default();
                let peer_parent = self.scope_parents.get(target).cloned().flatten();
                self.scope_parents
                    .entry(peer.clone())
                    .or_insert(peer_parent);
                if let Err(error) = self.expand_dependencies(&peer, &mut peer_rule) {
                    self.expansion_error(peer_rule.prerequisite_location.as_ref(), &error);
                    state.outcome.failed = true;
                    state.outcome.fatal = true;
                    state.stopped = true;
                    break;
                }
                state.serial_phase = state.rule.grouped_double_id().is_none()
                    && self.special(b".NOTPARALLEL", &peer, false);
                state
                    .group
                    .as_mut()
                    .expect("peer traversal has group state")
                    .peer_segments
                    .push((state.rule.dependencies.len(), peer.clone(), ordinal));
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
            if let Some(index) = state.serial_dependency {
                let (child, pending) = self.visit_prerequisite(target, &mut state, index, false);
                merge_child(&mut state.outcome, child);
                if pending {
                    waiting = true;
                    break;
                }
                state.serial_dependency = None;
                if state.outcome.fatal || (state.outcome.failed && !self.options.keep_going) {
                    state.stopped = true;
                    break;
                }
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
            if dependency == b".WAIT" {
                for position in 0..state.next {
                    let index = state.order.get(position).copied().unwrap_or(position);
                    let prior = &state.rule.dependencies[index];
                    if prior.name == b".WAIT" || state.dropped.contains(&prior.name) {
                        continue;
                    }
                    let (child, pending) =
                        self.visit_prerequisite(target, &mut state, index, false);
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
            state.next += 1;
            if self.visiting.contains(dependency) {
                if self.shuffle.enabled() {
                    state
                        .presentation_drops
                        .get_or_insert_with(Default::default)
                        .insert(state.next - 1);
                }
                self.warn_circular_dependency(target, dependency);
                if let Some((_, owner, ordinal)) = state.group.as_ref().and_then(|group| {
                    group
                        .peer_segments
                        .iter()
                        .rev()
                        .find(|(start, _, _)| *start < state.next)
                }) {
                    self.drop_group_dependency(
                        owner,
                        state.rule.grouped_double_id(),
                        *ordinal,
                        dependency,
                    );
                }
                state.dropped.insert(dependency.clone());
                continue;
            }
            let (child, _) = self.visit_prerequisite(target, &mut state, index, true);
            merge_child(&mut state.outcome, child);
            if state.serial_phase {
                state.serial_dependency = Some(index);
            }
            if child.fatal
                || ((child.failed || child.question_outdated) && !self.options.keep_going)
            {
                state.stopped = true;
            }
        }
        // Revisit suspended children in traversal order without replaying the
        // parent's dependency expansion, circular diagnostics or scope setup.
        for position in 0..state.next {
            let index = state.order.get(position).copied().unwrap_or(position);
            let dependency = &state.rule.dependencies[index].name;
            if dependency == b".WAIT" || state.dropped.contains(dependency) {
                continue;
            }
            let (child, pending) = self.visit_prerequisite(target, &mut state, index, false);
            merge_child(&mut state.outcome, child);
            waiting |= pending;
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
                    let child = self.build(dependency, Some(target), state.depth + 1);
                    merge_child(&mut state.outcome, child);
                    waiting |= self.pending(dependency);
                }
            }
        }
        if waiting {
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
        first: bool,
    ) -> (Outcome, bool) {
        if let Some((epoch, outcome)) = state.inspections.get(&index)
            && *epoch == self.graph_epoch
        {
            return (*outcome, true);
        }
        let occurrence = &state.rule.dependencies[index];
        let dependency = &occurrence.name;
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
            } = self.check_intermediate(dependency, target, state.comparison, state.depth + 1);
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
            let unknown_archive = self
                .done
                .get(dependency)
                .is_some_and(|child| child.completion == CompletionTime::ArchiveUnknown)
                && rmake_core::filenames::archive_parts(dependency).is_some();
            let child = if first || self.graph_pending(dependency) || unknown_archive {
                self.build(dependency, Some(target), state.depth + 1)
            } else {
                self.done.get(dependency).copied().unwrap_or_default()
            };
            let pending = self.pending(dependency);
            let child = if pending {
                child
            } else {
                self.observe_updated(dependency, child)
            };
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
        if self.options.always {
            return true;
        }
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
        let debug_depth = depth + usize::from(self.remaking);
        crate::debug::emit(
            &self.output,
            crate::debug::Event::Finished(target, debug_depth),
        );
        let debug_remake = stale && !outcome.failed && !outcome.question_outdated;
        if debug_remake {
            crate::debug::emit(
                &self.output,
                crate::debug::Event::MustRemake(target, debug_depth),
            );
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
                self.resolved_names.remove(target);
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
                let recursive = recipes.iter().any(|recipe| {
                    has_force_prefix(&recipe.bytes)
                        || recipe
                            .bytes
                            .windows(7)
                            .any(|part| part == b"$(MAKE)" || part == b"${MAKE}")
                });
                if (!self.options.touch || recursive)
                    && let Some(group) = group
                {
                    let group = group.run;
                    if let Some(id) = group.branch {
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
                let execution = self.execute(
                    target,
                    recipes,
                    autos
                        .as_ref()
                        .map(|values| values.as_slice())
                        .unwrap_or(&[]),
                    phony,
                    before,
                    TraceInputs {
                        branch,
                        location: recipes.first().map(|recipe| &recipe.location),
                        dependencies: &rule.dependencies,
                        dropped: &dropped,
                    },
                );
                outcome.ran |= execution.ran;
                outcome.completion =
                    if (self.options.dry_run || self.options.question || self.options.touch)
                        && !execution.all_recursive
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
                outcome.search_after_update =
                    self.update_search_pending(target, phony, execution.all_recursive);
                outcome.failed |= matches!(execution.status, Status::Failure | Status::Fatal);
                outcome.fatal |= execution.status == Status::Fatal;
                outcome.question_outdated |= execution.status == Status::Question;
                if self.intermediate(target) && before.is_none() {
                    self.created_intermediates.push(target.to_vec());
                }
                if self.cleanup_target(target, before, phony, execution.cleanup) {
                    outcome.failed = true;
                    outcome.fatal = true;
                }
            } else if rule.record_only && !phony {
                if !self.optional_remake {
                    self.show_makefile_error();
                    let parent = self
                        .scope_parents
                        .get(target)
                        .and_then(|name| name.as_deref());
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
        }
        if !debug_remake && !outcome.failed && !self.special(b".NOTINTERMEDIATE", target, true) {
            self.database_secondary.insert(target.to_vec());
        }
        // A stopping synchronous child failure exits before GNU's ordinary
        // update-return diagnostic. Keep-going updates retain that diagnostic.
        let stopping_failure = debug_remake && outcome.failed && !self.options.keep_going;
        if !self.parallel_jobs.contains_key(target) && !stopping_failure {
            crate::debug::emit(
                &self.output,
                if debug_remake {
                    if outcome.failed {
                        crate::debug::Event::Failed(target, debug_depth)
                    } else {
                        crate::debug::Event::Remade(target, debug_depth)
                    }
                } else {
                    crate::debug::Event::NoNeed(target, debug_depth)
                },
            );
        }
        self.visiting.remove(target);
        self.done.insert(target.to_vec(), outcome);
        if !self.parallel_jobs.contains_key(target) {
            self.finish_group(target, outcome);
        }
        outcome
    }

    fn finish_group(&mut self, target: &[u8], outcome: Outcome) {
        let Some(group) = self.running_groups.remove(target) else {
            return;
        };
        if let Some(id) = group.branch {
            self.active_group_branches.remove(&id);
            self.completed_group_branches.insert(id, outcome);
            return;
        }
        let warn_missing = group.implicit
            && !outcome.completion.preserves()
            && self
                .modified(target)
                .is_some_and(|time| Some(time) > group.before);
        for (peer, before, phony) in group.members.into_iter().rev() {
            if peer == target {
                continue;
            }
            self.active_peers.remove(&peer);
            self.pending_rules.remove(&peer);
            let mut completed = outcome;
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
            variables.refresh_globals(&self.database.variables);
            return Ok(variables);
        }
        let parent = self.scope_parents.get(target).cloned().flatten();
        let inherited = if let Some(parent) = parent {
            self.target_variables(&parent)?
        } else {
            self.database.variables.clone()
        };
        let variables = if let Some(pattern) = self.implicit_pattern_scopes.remove(target) {
            self.database
                .variables_for_target_prepared(target, &inherited, pattern.as_ref())
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
                    let result = crate::shell::run(
                        request,
                        &self.program,
                        self.makelevel,
                        &self.output,
                        self.jobserver.as_ref(),
                    );
                    self.filesystem.borrow_mut().child_completed();
                    result
                }),
            )?
        };
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
        trace: TraceInputs<'_>,
    ) -> Execution {
        let mut result = self.execute_inner(target, recipes, automatic, phony, before, trace);
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
        result
    }

    fn execute_inner(
        &mut self,
        target: &[u8],
        recipes: &[Recipe],
        automatic: &[(&[u8], Vec<u8>)],
        phony: bool,
        before: Option<SystemTime>,
        trace: TraceInputs<'_>,
    ) -> Execution {
        self.observe_interrupt();
        if self.interrupt.is_some() {
            return Execution::fatal(false);
        }
        let recursive_line = |recipe: &Recipe| {
            has_force_prefix(&recipe.bytes)
                || recipe
                    .bytes
                    .windows(7)
                    .any(|part| part == b"$(MAKE)" || part == b"${MAKE}")
        };
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
                builtin: first.builtin,
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
        let flow = Flow::new(recipes.iter().map(recursive_line).collect());
        let parallel = self.job_slots() > 1
            && !self.options.dry_run
            && !self.options.question
            && !self.options.touch;
        if self.graph_abort == Some(Status::Fatal) {
            return Execution::fatal(false);
        }
        if self.graph_abort.is_some() || (self.parallel_failed && !self.options.keep_going) {
            return Execution::failure(false, Cleanup::None);
        }
        let branch_variables = trace.branch.filter(|ordinal| *ordinal > 0).and_then(|_| {
            self.database_branch_contexts
                .get(&(target.to_vec(), 0))
                .map(VariableStore::fresh_double_colon_scope)
        });
        let mut variables = match branch_variables
            .map(Ok)
            .unwrap_or_else(|| self.target_variables(target))
        {
            Ok(variables) => variables,
            Err(error) => {
                self.expansion_error(None, &error);
                return Execution::fatal(false);
            }
        };
        variables.refresh_globals(&self.database.variables);
        if let Some(history) = self.database.database_history() {
            history.intern(b".SUFFIXES");
        }
        self.database.file_records.insert(b".SUFFIXES");
        for (name, value) in automatic {
            if let Err(error) = variables.define_literal(name, value, Origin::Automatic) {
                self.expansion_error(None, &error);
                return Execution::fatal(false);
            }
        }
        self.retain_database_context(target, trace.branch, &variables);
        let output = Some(self.output.recipe_buffer());
        if let Some(buffer) = &output
            && buffer.enabled()
        {
            buffer.set_owner(target, before, phony);
        }
        self.dispatch_output = output.clone();
        self.output.select(output.as_ref());
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
        self.retain_database_context(target, trace.branch, &variables);
        if parallel {
            while self.parallel_jobs.len() >= self.job_slots() {
                self.reap_job();
            }
        } else {
            self.wait_all_jobs();
        }
        if self.graph_abort == Some(Status::Fatal) {
            return Execution::fatal(false);
        }
        if self.graph_abort.is_some() || (self.parallel_failed && !self.options.keep_going) {
            return Execution::failure(false, Cleanup::None);
        }
        let debug_job = crate::debug::Jobs::record(target, self.output.debug_mask());
        // A new recipe's dispatch scans the existing live chain before asking
        // for its slot. Continuations retain their existing chain position.
        self.job_debug.live(self.output.debug_mask());
        let permit = if parallel {
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
        if self.graph_abort.is_some() || (self.parallel_failed && !self.options.keep_going) {
            return Execution::failure(false, Cleanup::None);
        }
        if self.output.debug_mask().has(crate::debug::Mask::WHY) {
            let reason = if phony {
                b"target is .PHONY".to_vec()
            } else if before.is_none() {
                b"target does not exist".to_vec()
            } else {
                let newer = match self.expand(&mut variables, b"$?", None) {
                    Ok(newer) => newer,
                    Err(error) => {
                        self.expansion_error(None, &error);
                        return Execution::fatal(false);
                    }
                };
                if newer.is_empty() {
                    let missing = joined(
                        trace
                            .dependencies
                            .iter()
                            .filter(|dependency| {
                                dependency.name != b".WAIT"
                                    && !trace.dropped.contains(&dependency.name)
                                    && self.done.get(&dependency.name).is_some_and(|child| {
                                        child.time.is_none()
                                            && (child.completion != CompletionTime::AssumeNew
                                                || self.special(b".PHONY", &dependency.name, false))
                                    })
                            })
                            .map(|dependency| dependency.name.as_slice()),
                    );
                    if missing.is_empty() {
                        b"unknown reasons".to_vec()
                    } else {
                        missing
                    }
                } else {
                    newer
                }
            };
            let prefix = trace
                .location
                .map_or_else(|| b"<builtin>: ".to_vec(), location_prefix);
            self.output.write_parts(
                false,
                &[
                    &prefix,
                    b"update target '",
                    target,
                    b"' due to: ",
                    &reason,
                    b"\n",
                ],
            );
        }
        let state = RecipeState {
            branch: trace.branch,
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
            return Execution::success(true);
        }
        self.continue_recipe(target, state, phony, before, parallel)
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
    ) -> Execution {
        let mut state = state;
        let mut debug_job = state.debug_job.take();
        let execution =
            self.continue_recipe_inner(target, state, phony, before, parallel, &mut debug_job);
        self.finish_debug_job(debug_job.as_ref());
        execution
    }

    fn continue_recipe_inner(
        &mut self,
        target: &[u8],
        state: RecipeState,
        phony: bool,
        before: Option<SystemTime>,
        parallel: bool,
        debug_job: &mut Option<crate::debug::JobHandle>,
    ) -> Execution {
        let RecipeState {
            branch,
            output,
            mut expanded,
            mut variables,
            mut flow,
            mut ran,
            one_shell,
            mut environment,
            permit,
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
                self.would_build = true;
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
                for name in &exports {
                    variables.refresh_globals(&self.database.variables);
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
                        // Imported environment values are recursive when referenced
                        // by make syntax, but are passed back to children verbatim.
                        if matches!(
                            variable.origin(),
                            Origin::Environment | Origin::EnvironmentOverride
                        ) {
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
                            .with_filesystem(&mut |effect| {
                                self.filesystem.borrow_mut().execute(effect)
                            })
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
                    && !flow.any_recursive()
                {
                    crate::process::disable_pipe_jobserver(&mut child);
                }
                environment = Some(RecipeEnvironment {
                    values: child
                        .get_envs()
                        .map(|(name, value)| (name.to_owned(), value.map(OsStr::to_owned)))
                        .collect(),
                    order: emitted_order,
                });
            }
            let _ = io::stdout().flush();
            let stdin_owner = !parallel || !self.parallel_jobs.values().any(|job| job.stdin_owner);
            if let Some(buffer) = &output {
                buffer.child_reaped(false);
            }
            self.output.start();
            let process = launch_child(
                &program,
                &child,
                &environment.as_ref().expect("prepared environment").order,
                self.database.variables.original_shell(),
                |mut command, image, arg0| {
                    if let Some(server) = &self.jobserver {
                        server.configure_child(&mut command, force);
                    }
                    if !stdin_owner {
                        command.stdin(rmake_linux::process::closed_stdin()?);
                    }
                    if let Some(buffer) = &output {
                        buffer.configure_child(&mut command)?;
                    }
                    if crate::native::has_modules() {
                        rmake_linux::native::flush_standard_streams()?;
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
                if debug_job.is_none() {
                    *debug_job = crate::debug::Jobs::record(target, self.output.debug_mask());
                }
                if let Some(record) = debug_job.as_ref() {
                    self.job_debug
                        .launched(self.output.debug_mask(), record, child.id());
                }
            }
            if process.is_ok() && self.options.load_limit.is_some_and(|v| v >= 0.0) {
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
                                before,
                                phony,
                                stdin_owner,
                                waiting_load: false,
                                child: Some(child),
                                signal_cleaned: false,
                                continuation: RecipeState {
                                    branch,
                                    output,
                                    expanded,
                                    variables: variables.clone(),
                                    flow,
                                    ran,
                                    one_shell,
                                    environment,
                                    permit,
                                    debug_job: debug_job.clone(),
                                },
                            },
                        );
                        self.observe_interrupt();
                        return Execution::success(true);
                    }
                    Err(error) => Err(error),
                }
            } else {
                match process {
                    Ok(mut child) => self
                        .wait_serial_child(&mut child, target, before, phony, debug_job.as_ref())
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
                    self.would_build = true;
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
                    self.show_makefile_error();
                }
                if (ignore || !self.optional_remake) && (!ignore || !self.globally_silent()) {
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
                    return Execution::failure(ran, failure_cleanup)
                        .with_recursion(!flow.has_ordinary());
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

    fn touch_target(&self, target: &[u8]) -> Execution {
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
    for path in paths {
        let mut command = Command::new(&path);
        command.arg0(os(program)).args(template.get_args());
        rmake_linux::signals::configure_child(&mut command);
        match run(command, &environment, program) {
            // Linux ENOEXEC: interpret the attempted file through /bin/sh,
            // preserving args/environment but not the recipe's -e/-c flags.
            Err(error) if error.raw_os_error() == Some(8) => {
                let mut shell = Command::new("/bin/sh");
                shell.arg(&path).args(template.get_args());
                rmake_linux::signals::configure_child(&mut shell);
                return run(shell, &environment, b"/bin/sh");
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
