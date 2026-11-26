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
use crate::file_aliases::{AliasKind, Aliases};
use crate::makefiles::{IncludeSearch, source_open_exhausted};
use crate::recipe_flow::{Cleanup, Execution, Flow, Shutdown, Status, TouchCompletion};
use rmake_core::diagnostic::{
    Diagnostic, ExpansionContext, IncludeError, IncludeFailure, Output, OutputReceiver, ScopeParent,
};
use rmake_core::file_records::FileRecordId;
use rmake_core::messages::Message;
use rmake_core::parser::{
    Database, Dependency, Location, Makefile, Recipe, RecipeSource, Rule, words,
};
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
use std::num::{NonZeroU64, NonZeroUsize};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::sync::Arc;
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
        if let Some(wait) = dependencies[..index].last_mut()
            && wait.name == b".WAIT"
        {
            // GNU keeps the barrier on the occurrence whose role changed.
            wait.order_only = false;
        }
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
    SimulatedQuery,
    SimulatedRecipeCompletion,
    TouchedTarget,
    // A settled touch can supply the failed-source restart verdict.
    SimulatedTouch,
    PhysicalTouch,
    ObservedGroup,
    ObservedDoubleColon,
    RecipeUnstarted,
    UnfinishedRecipeExpansion,
    // A completed archive container merged into an unattempted file record.
    MergedArchiveContainer,
    // A completed ordinary source whose visited destination kept its file stem.
    MergedVisitedFile,
    // A stopping diagnostic interrupted the file's update.
    UnfinishedUpdate,
    // This recipe completed, then its failure stopped ordinary traversal.
    StoppedAfterChildFailure,
    // A completed failure whose prerequisite consumer may still sample its time.
    ContinuedAfterChildFailure,
}

impl DatabaseResult {
    fn has_simulated_time(self) -> bool {
        matches!(
            self,
            Self::Simulated
                | Self::SimulatedQuery
                | Self::SimulatedRecipeCompletion
                | Self::TouchedTarget
        )
    }
}

#[derive(Clone, Copy)]
enum GroupPeerTime {
    Unchecked,
    Observed(Option<SystemTime>),
    Outcome,
}

#[derive(Clone, Copy, Default)]
enum RemakeFailure {
    #[default]
    None,
    SettledPrerequisiteOwner,
    DirectExplicitGroupOwner,
    RequiredSyntheticInput,
    RequiredSyntheticRecipe,
    RequiredDeclaredTailRecipe,
    Optional,
    RequiredClockUnchanged,
    RequiredClockChanged,
    OptionalClockUnchanged,
    OptionalClockChanged,
    ExplicitRequiredClockUnchanged,
    ExplicitRequiredClockChanged,
    ExplicitOptionalClockUnchanged,
    ExplicitOptionalClockChanged,
}

impl RemakeFailure {
    fn is_optional(self) -> bool {
        matches!(
            self,
            Self::Optional
                | Self::OptionalClockUnchanged
                | Self::OptionalClockChanged
                | Self::ExplicitOptionalClockUnchanged
                | Self::ExplicitOptionalClockChanged
        )
    }

    fn with_optional(self) -> Self {
        match self {
            Self::None | Self::SettledPrerequisiteOwner | Self::DirectExplicitGroupOwner => {
                Self::Optional
            }
            Self::RequiredClockUnchanged => Self::OptionalClockUnchanged,
            Self::RequiredClockChanged => Self::OptionalClockChanged,
            Self::ExplicitRequiredClockUnchanged => Self::ExplicitOptionalClockUnchanged,
            Self::ExplicitRequiredClockChanged => Self::ExplicitOptionalClockChanged,
            optional => optional,
        }
    }

    fn peer_clock_changed(self, modes: MakefileModes) -> Option<bool> {
        match self {
            Self::ExplicitRequiredClockUnchanged | Self::ExplicitOptionalClockUnchanged
                if !modes.touch =>
            {
                Some(false)
            }
            Self::ExplicitRequiredClockChanged | Self::ExplicitOptionalClockChanged
                if !modes.touch =>
            {
                Some(true)
            }
            Self::RequiredClockUnchanged | Self::OptionalClockUnchanged if !modes.active() => {
                Some(false)
            }
            Self::RequiredClockChanged | Self::OptionalClockChanged if !modes.active() => {
                Some(true)
            }
            _ => None,
        }
    }
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
    remake_failure: RemakeFailure,
    // A successful grouped remake invalidates each other read peer,
    // including one whose physical timestamp did not change.
    remake_group_peer_updated: bool,
    // The cached declared head can remain successful after the tail fails.
    late_input_failed: bool,
    // A real recipe invalidates its timestamp; a parent may search again.
    search_after_update: bool,
    completion: CompletionTime,
    database_result: DatabaseResult,
    debug_time_changed: bool,
    // A serial peer completed with its owner before its own diagnostic visit.
    debug_serial_group_peer: bool,
    replayed_double_colon_goal_failure: bool,
}

impl Outcome {
    fn ran_refresh(self) -> bool {
        self.ran && self.completion == CompletionTime::Refresh && !self.fatal && !self.blocked
    }

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

struct LeadingOptionalRemake<'a> {
    required: &'a Makefile,
    neighbor: &'a Makefile,
    trailing: Option<TrailingOptionalRemake<'a>>,
}

struct TrailingOptionalRemake<'a> {
    required: &'a Makefile,
    neighbor: &'a [u8],
    kind: TrailingOptionalKind,
}

#[derive(PartialEq, Eq)]
enum TrailingOptionalKind {
    Unruled,
    Declared,
    Missing,
}

/// State owned by one makefile update. Intermediate inspection can suppress
/// the optional policy temporarily without discarding its retained error.
#[derive(Clone)]
struct MakefileRemake {
    optional: bool,
    retain_failure: bool,
    use_declared_synthetic_head: bool,
    use_declared_tail_head: bool,
    trace_initial_synthetic: bool,
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

    fn only_dry_run(self) -> bool {
        self.dry_run && !self.question && !self.touch
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum RemakeWait {
    Command,
    Recipe,
    TerminalDryRecipe,
    LoadPass,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AssumedDoubleColonDryRemake {
    Required,
    OptionalHandoff,
    TerminalOptionalHandoff,
}

struct PendingMakefileRemake<'a> {
    modes: MakefileModes,
    file: &'a Makefile,
    context: MakefileRemake,
    duplicate: bool,
    active: bool,
    initial: Outcome,
    trace_synthetic_reap: bool,
    optional_visit_settled: bool,
    same_clock_inline_optional_settled: bool,
    required_worker_pair: Option<Box<RequiredRemakeWorkerPair<'a>>>,
    trailing_optional_worker: Option<Rc<TrailingOptionalWorker>>,
    trailing_prerequisite_child: Option<Box<RemakeChild<'a>>>,
    trailing_prerequisite_changed: bool,
}

struct RequiredRemakeWorkerPair<'a> {
    first_identity: u64,
    first_before: SystemTime,
    first_branch: Option<usize>,
    first_reap: Option<RequiredRemakeWorkerReap>,
    second: &'a [u8],
    second_identity: u64,
    second_before: SystemTime,
    second_failure: Option<Rc<Cell<Option<SystemTime>>>>,
    third: Option<RemakeChild<'a>>,
    third_failure: Option<Rc<Cell<Option<SystemTime>>>>,
}

struct TrailingOptionalWorker {
    required: Vec<u8>,
    required_identity: u64,
    optional_identity: u64,
    required_before: SystemTime,
    direct_reread: bool,
    no_required_tail_remains: bool,
    only_synthetic_tail_remains: bool,
    required_failure_settled: Cell<bool>,
    required_reap: Cell<Option<RequiredRemakeWorkerReap>>,
    result: Cell<Option<TrailingOptionalWorkerResult>>,
}

#[derive(Clone, Copy)]
enum RequiredRemakeWorkerReap {
    Completed(SystemTime),
    FailedDeclaredTail { ordinal: usize, clock: SystemTime },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TrailingOptionalWorkerResult {
    Completed(SystemTime),
    Failed(SystemTime),
}

struct InlineOptionalRemakeClock<'a> {
    required: &'a [u8],
    source: &'a [u8],
    before: SystemTime,
    inline: SystemTime,
    changed_on_visit: bool,
}

struct RemakeChild<'a> {
    name: &'a [u8],
    identity: u64,
    before: SystemTime,
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

enum NamedRemakePeerTime {
    File(Option<SystemTime>),
    GroupedDoubleColon(SystemTime),
}

/// Grouped logical updates are provisional until the serial source pass settles.
/// After a failed chain, only an optional failed tail can leave its successful
/// cached head invalidated. Physical timestamp changes are tracked separately.
#[derive(Default)]
struct SerialGroupedRemakeChanges {
    failed: bool,
    updated: bool,
    optional_failed_tail_updated: bool,
}

impl SerialGroupedRemakeChanges {
    fn changed(&self) -> bool {
        if self.failed {
            self.optional_failed_tail_updated
        } else {
            self.updated
        }
    }
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
    environment.get(name).is_some_and(|variable| {
        export_selected_binding(
            globals,
            unexports,
            name,
            variable,
            environment.resolve_export_policy(name),
            export_all,
        )
    })
}

fn export_selected_binding(
    globals: &VariableStore,
    unexports: &HashSet<Vec<u8>>,
    name: &[u8],
    variable: &Variable,
    policy: Option<bool>,
    export_all: bool,
) -> bool {
    policy.unwrap_or_else(|| {
        let default_export = automatic_export_name(name)
            && !matches!(variable.origin(), Origin::Default | Origin::Automatic)
            && (export_all
                || matches!(
                    variable.origin(),
                    Origin::Environment | Origin::EnvironmentOverride | Origin::CommandLine
                ));
        // A reset or private global no longer supplies export policy.
        // Evaluate default eligibility from the selected target binding.
        let ignore_global_policy = globals
            .get(name)
            .is_some_and(|variable| variable.export_default_reset() || variable.is_private());
        (ignore_global_policy || !unexports.contains(name)) && default_export
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

pub(crate) fn emit_output(
    controller: &crate::output::Controller,
    program: &[u8],
    output: Output<'_>,
) {
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
    let detail = rmake_linux::filesystem::source_error_detail(error);
    let message = crate::messages::system(Message::OsError {
        action: b"getcwd",
        path: b"",
        detail: &detail,
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

struct OwnedScopeParent {
    target: Vec<u8>,
    // A stored branch is the zero-based ordinal plus one.
    packed_branch: Option<NonZeroUsize>,
    record: Option<FileRecordId>,
}

impl OwnedScopeParent {
    fn new(parent: ScopeParent<&[u8]>, record: Option<FileRecordId>) -> Self {
        Self {
            target: parent.target.to_vec(),
            packed_branch: Self::pack_branch(parent.branch),
            record,
        }
    }

    fn pack_branch(branch: Option<usize>) -> Option<NonZeroUsize> {
        branch.map(|index| {
            NonZeroUsize::MIN
                .checked_add(index)
                .expect("double-colon branch index overflow")
        })
    }

    fn branch(&self) -> Option<usize> {
        self.packed_branch.map(|index| index.get() - 1)
    }
}

struct FileVisit {
    parent: Option<OwnedScopeParent>,
    // Ordinary pass plus one, recorded when verbose output was disabled.
    quiet_consideration: Option<NonZeroU64>,
}

// Scope is established before expansion. An update result can be
// cleared without forgetting that first visit.
#[derive(Default)]
struct FileTraversal {
    visit: Option<FileVisit>,
    outcome: Option<Outcome>,
}

enum FileTraversals {
    // A small build can finish after processing its Makefile and one goal.
    Inline([Option<(Vec<u8>, FileTraversal)>; 2]),
    Map(HashMap<Vec<u8>, FileTraversal>),
}

impl Default for FileTraversals {
    fn default() -> Self {
        Self::Inline([None, None])
    }
}

impl FileTraversals {
    fn record(&self, name: &[u8]) -> Option<&FileTraversal> {
        match self {
            Self::Inline(entries) => entries
                .iter()
                .flatten()
                .find_map(|(key, record)| (key == name).then_some(record)),
            Self::Map(entries) => entries.get(name),
        }
    }

    fn record_mut(&mut self, name: &[u8]) -> Option<&mut FileTraversal> {
        match self {
            Self::Inline(entries) => entries
                .iter_mut()
                .flatten()
                .find_map(|(key, record)| (key.as_slice() == name).then_some(record)),
            Self::Map(entries) => entries.get_mut(name),
        }
    }

    fn record_or_insert(&mut self, name: &[u8]) -> &mut FileTraversal {
        if let Self::Inline(entries) = &*self {
            let position = entries
                .iter()
                .position(|entry| entry.as_ref().is_some_and(|(key, _)| key == name))
                .or_else(|| entries.iter().position(Option::is_none));
            if let Some(position) = position {
                let Self::Inline(entries) = self else {
                    unreachable!()
                };
                return &mut entries[position]
                    .get_or_insert_with(|| (name.to_vec(), FileTraversal::default()))
                    .1;
            }
            let Self::Inline(entries) = std::mem::take(self) else {
                unreachable!()
            };
            *self = Self::Map(entries.into_iter().flatten().collect());
        }
        let Self::Map(entries) = self else {
            unreachable!()
        };
        entries.entry(name.to_vec()).or_default()
    }

    fn remove_record(&mut self, name: &[u8]) {
        match self {
            Self::Inline(entries) => {
                if let Some(entry) = entries
                    .iter_mut()
                    .find(|entry| entry.as_ref().is_some_and(|(key, _)| key == name))
                {
                    *entry = None;
                }
            }
            Self::Map(entries) => {
                entries.remove(name);
            }
        }
    }

    fn visit(&self, name: &[u8]) -> Option<&FileVisit> {
        self.record(name)?.visit.as_ref()
    }

    fn remember_visit(
        &mut self,
        name: &[u8],
        make_visit: impl FnOnce() -> FileVisit,
    ) -> &mut FileVisit {
        self.record_or_insert(name)
            .visit
            .get_or_insert_with(make_visit)
    }

    fn outcome(&self, name: &[u8]) -> Option<&Outcome> {
        self.record(name)?.outcome.as_ref()
    }

    fn outcome_mut(&mut self, name: &[u8]) -> Option<&mut Outcome> {
        self.record_mut(name)?.outcome.as_mut()
    }

    fn clear_recipe_activity(&mut self) {
        let clear = |record: &mut FileTraversal| {
            if let Some(outcome) = &mut record.outcome {
                outcome.ran = false;
            }
        };
        match self {
            Self::Inline(entries) => entries
                .iter_mut()
                .flatten()
                .for_each(|(_, record)| clear(record)),
            Self::Map(entries) => entries.values_mut().for_each(clear),
        }
    }

    fn has_outcome(&self, name: &[u8]) -> bool {
        self.outcome(name).is_some()
    }

    fn set_outcome(&mut self, name: &[u8], outcome: Outcome) {
        if let Some(record) = self.record_mut(name) {
            record.outcome = Some(outcome);
        } else {
            self.record_or_insert(name).outcome = Some(outcome);
        }
    }

    fn remove_outcome(&mut self, name: &[u8]) -> Option<Outcome> {
        let record = self.record_mut(name)?;
        let result = record.outcome.take();
        if record.visit.is_none() {
            self.remove_record(name);
        }
        result
    }

    fn rename_visit(&mut self, from: &[u8], to: &[u8]) {
        let Some(record) = self.record_mut(from) else {
            return;
        };
        if let Some(visit) = record.visit.take() {
            if record.outcome.is_none() {
                self.remove_record(from);
            }
            self.remember_visit(to, || visit);
        }
    }
}

struct MovedArchiveRecord {
    lookup: Vec<u8>,
    original: Vec<u8>,
    command_goal: bool,
}

enum MergedGoalReport {
    CompletedSource(Vec<u8>),
    CompletedVisitedSource(Vec<u8>),
    UnattemptedDestination,
}

#[derive(Clone, Copy)]
struct MergedGoalOwners {
    source: FileRecordId,
    destination: FileRecordId,
    destination_unattempted: bool,
}

struct TargetReads<'a> {
    visits: &'a FileTraversals,
    retained_parents: &'a HashMap<Vec<u8>, OwnedScopeParent>,
    moved_archives: &'a HashMap<FileRecordId, MovedArchiveRecord>,
    aliases: &'a Aliases,
    base: &'a HashMap<Vec<u8>, VariableStore>,
    recipes: &'a mut HashMap<Vec<u8>, VariableStore>,
    branches: &'a mut HashMap<(Vec<u8>, usize), VariableStore>,
    direct_heads: bool,
}

impl TargetReads<'_> {
    fn resolve_parent<'a>(&'a self, parent: &'a OwnedScopeParent) -> ScopeParent<&'a [u8]> {
        ScopeParent {
            target: parent
                .record
                .and_then(|id| self.moved_archives.get(&id))
                .map_or_else(
                    || self.aliases.resolve(&parent.target),
                    |file| self.aliases.resolve(&file.lookup),
                ),
            branch: parent.branch(),
        }
    }
}

impl rmake_core::diagnostic::TargetReadReceiver for TargetReads<'_> {
    fn variables(&self, target: &[u8]) -> Option<&VariableStore> {
        self.recipes.get(target).or_else(|| self.base.get(target))
    }

    fn has_file_aliases(&self) -> bool {
        !self.aliases.is_empty()
    }

    fn named_owner<'a>(&self, mut owner: ScopeParent<&'a [u8]>) -> ScopeParent<&'a [u8]> {
        if owner.branch.is_some()
            && (!self.direct_heads || self.aliases.contains_source_or_destination(owner.target))
        {
            owner.branch = None;
        }
        owner
    }

    fn named_variables(&self, owner: ScopeParent<&[u8]>) -> Option<&VariableStore> {
        owner
            .branch
            .and_then(|ordinal| self.branches.get(&(owner.target.to_vec(), ordinal)))
            .or_else(|| self.variables(owner.target))
    }

    fn retain_named_locals(&mut self, owner: ScopeParent<&[u8]>, variables: VariableStore) {
        if let Some(ordinal) = owner.branch {
            self.branches
                .insert((owner.target.to_vec(), ordinal), variables);
        } else {
            self.retain_locals(owner.target, variables);
        }
    }

    fn parent(&self, target: &[u8]) -> Option<Option<ScopeParent<&[u8]>>> {
        self.visits.visit(target).map(|visit| {
            visit
                .parent
                .as_ref()
                .map(|parent| self.resolve_parent(parent))
        })
    }

    fn has_parent_changes(&self) -> bool {
        !self.retained_parents.is_empty() || !self.moved_archives.is_empty()
    }

    fn changed_parent(&self, target: &[u8]) -> Option<ScopeParent<&[u8]>> {
        if let Some(parent) = self.retained_parents.get(target) {
            return Some(self.resolve_parent(parent));
        }
        let visit = self.visits.visit(target)?;
        let parent = visit.parent.as_ref()?;
        self.moved_archives.get(&parent.record?)?;
        Some(self.resolve_parent(parent))
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
    required_pair_failure: Option<Rc<Cell<Option<SystemTime>>>>,
    trailing_optional_worker: Option<Rc<TrailingOptionalWorker>>,
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
    record_suffix_stem_lookup: bool,
    record_expansion_failure: bool,
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
    NamedRemakeWithoutPoll,
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
    assumed_phony_input: bool,
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
    query_skipped_dependencies: HashSet<usize>,
    // Dependency-order ranges skipped at blocked peer barriers.
    wait_frontiers: Vec<std::ops::Range<usize>>,
    frontiers_enabled: bool,
    run: GroupRun,
}

impl GroupState {
    fn ordinary_explicit(&self) -> bool {
        self.run.ordinary && !self.run.implicit
    }
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

enum JobCompletion {
    Child(io::Result<std::process::ExitStatus>),
    FailedDispatch,
    LoadReady,
}

type JobResult = (Vec<u8>, u64, JobCompletion);

#[derive(Default)]
struct ArchiveVpathReads {
    members: HashSet<Vec<u8>>,
    archives: HashSet<Vec<u8>>,
    paths: HashMap<Vec<u8>, Vec<u8>>,
    permanent_containers: HashMap<FileRecordId, Vec<u8>>,
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
    settled_missing_container: bool,
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

enum ImplicitFilenames {
    Single(Dependency),
    Expanded(Vec<Dependency>),
}

struct ImplicitScope {
    pattern: Option<TargetScope>,
    runtime_metadata: VariableStore,
    revision: usize,
}

struct ImplicitSearch {
    compatibility: bool,
    archive: bool,
    intermediate_scope: Option<ImplicitScope>,
    branch: Option<usize>,
}

struct ImplicitSearchSession {
    active: Vec<usize>,
    budget: usize,
    debug_depth: usize,
    root_children: Vec<ImplicitSelection>,
    published: HashMap<Vec<u8>, PublishedImplicit>,
    scope_revision: usize,
}

#[derive(Default)]
struct PublishedImplicit {
    declaration: Option<Rule>,
    scope_revision: usize,
    declaration_revision: u64,
    pattern_bindings: HashSet<Arc<[u8]>>,
}

struct PendingImplicitRoot {
    selection: ImplicitSelection,
    children: Vec<ImplicitSelection>,
    published: HashMap<Vec<u8>, PublishedImplicit>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ImplicitPlacement {
    Root,
    DoubleColonRoot,
    Descendant,
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
    requested_goal_names: Option<HashSet<Vec<u8>>>,
    requested_goal_aliases: HashSet<Vec<u8>>,
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
    database_branch_implicit: HashSet<(Vec<u8>, usize)>,
    database_branch_secondary: BTreeSet<(Vec<u8>, usize)>,
    database_group_peer_times: HashMap<Vec<u8>, Vec<GroupPeerTime>>,
    database_unchecked_times: RefCell<HashSet<Vec<u8>>>,
    database_delete_policy: Cell<bool>,
    database_times: RefCell<HashMap<Vec<u8>, Option<SystemTime>>>,
    database_secondary: HashSet<Vec<u8>>,
    database_optional_visits: Vec<Vec<u8>>,
    database_fatal_optional_visits: RefCell<Vec<Vec<u8>>>,
    program: Vec<u8>,
    makelevel: u32,
    inherited_exports: HashSet<Vec<u8>>,
    command_replay: bool,
    #[allow(clippy::box_collection)]
    debug_considered: Option<Box<HashSet<Vec<u8>>>>,
    recipe_reaps: u64,
    commands_started: u64,
    debug_pass: u64,
    visiting: HashSet<Vec<u8>>,
    file_traversals: FileTraversals,
    // A later encounter affects declarations that revisit a completed file.
    // Its first traversal context still owns the recipe and diagnostics.
    retained_scope_parents: HashMap<Vec<u8>, OwnedScopeParent>,
    moved_archives: HashMap<FileRecordId, MovedArchiveRecord>,
    merged_goal_reports: HashMap<FileRecordId, MergedGoalReport>,
    target_contexts: HashMap<Vec<u8>, VariableStore>,
    implicit_scopes: HashMap<Vec<u8>, ImplicitScope>,
    impossible_prerequisites: HashSet<Vec<u8>>,
    impossible_generation: u64,
    would_build: bool,
    forced_query_stop: bool,
    ordinary_queries: OrdinaryQueries,
    parallel_jobs: ParallelJobs,
    // An executable lookup can fail without admitting a worker. Keep its
    // completion for the scheduler boundary, rather than a graph-only poll.
    failed_dispatch: Option<(Vec<u8>, u64)>,
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
    // Missing-rule leaves retain their latest actual prerequisite edge for
    // later diagnostics. Successful files keep no additional record.
    missing_rule_parents: HashMap<Vec<u8>, Option<Vec<u8>>>,
    // Existing read peers can change before their own serial remake visit.
    serial_remake_peer_times: HashMap<Vec<u8>, SystemTime>,
    vpath_dirs: Vec<Vec<u8>>,
    gpath_dirs: HashSet<Vec<u8>>,
    makefile_names: HashSet<Vec<u8>>,
    aliases: Aliases,
    path_observations: HashMap<Vec<u8>, PathObservation>,
    ordinary_include_open_times: bool,
    reopened_include_times: HashSet<Vec<u8>>,
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
                let detail = error
                    .raw_os_error()
                    .and_then(rmake_linux::messages::errno_detail)
                    .unwrap_or_else(|| os_error(&error).into_bytes());
                StartupError::from(Diagnostic::from(crate::messages::system(
                    Message::SourceError {
                        path: directory.as_os_str().as_bytes(),
                        detail: &detail,
                    },
                )))
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
                .define_literal(b"MAKE_HOST", crate::COMPATIBILITY_HOST, Origin::Default)
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
        // Preserve the ordered operands for execution. Large invocations also
        // need repeated membership checks during implicit graph traversal.
        let requested_goal_names =
            (options.goals.len() >= 32).then(|| options.goals.iter().cloned().collect());
        let mut engine = Self {
            requested_goal_names,
            requested_goal_aliases: HashSet::new(),
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
            database_branch_implicit: HashSet::new(),
            database_branch_secondary: BTreeSet::new(),
            database_group_peer_times: HashMap::new(),
            database_unchecked_times: RefCell::new(HashSet::new()),
            database_delete_policy: Cell::new(false),
            database_times: RefCell::new(HashMap::new()),
            database_secondary: HashSet::new(),
            database_optional_visits: Vec::new(),
            database_fatal_optional_visits: RefCell::new(Vec::new()),
            program,
            makelevel,
            inherited_exports,
            command_replay,
            debug_considered: None,
            recipe_reaps: 0,
            commands_started: 0,
            debug_pass: 0,
            visiting: HashSet::new(),
            file_traversals: FileTraversals::default(),
            retained_scope_parents: HashMap::new(),
            moved_archives: HashMap::new(),
            merged_goal_reports: HashMap::new(),
            target_contexts: HashMap::new(),
            implicit_scopes: HashMap::new(),
            impossible_prerequisites: HashSet::new(),
            impossible_generation: 0,
            would_build: false,
            forced_query_stop: false,
            ordinary_queries: OrdinaryQueries::default(),
            parallel_jobs: ParallelJobs::default(),
            failed_dispatch: None,
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
            missing_rule_parents: HashMap::new(),
            serial_remake_peer_times: HashMap::new(),
            vpath_dirs: Vec::new(),
            gpath_dirs: HashSet::new(),
            makefile_names: HashSet::new(),
            aliases: Aliases::default(),
            path_observations: HashMap::new(),
            ordinary_include_open_times: false,
            reopened_include_times: HashSet::new(),
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
        self.with_scoped_expansion(location, false, |database, context| {
            database.expand_in(variables, bytes, context)
        })
    }

    fn with_scoped_expansion<T>(
        &mut self,
        location: Option<&Location>,
        mute_undefined: bool,
        operation: impl FnOnce(&mut Database, ExpansionContext<'_>) -> T,
    ) -> T {
        let warnings = mute_undefined.then(|| self.flag_state.replace_scoped_warning_policy(false));
        let result = self.with_target_read_expansion(location, operation);
        if let Some(warnings) = warnings {
            self.flag_state.replace_scoped_warning_policy(warnings);
        }
        if let Some(options) = self.flag_state.take() {
            self.options = options;
        }
        result
    }

    fn with_target_read_expansion<T>(
        &mut self,
        location: Option<&Location>,
        operation: impl FnOnce(&mut Database, ExpansionContext<'_>) -> T,
    ) -> T {
        let direct_heads = self.direct_named_head_reads();
        let mut target_reads = TargetReads {
            visits: &self.file_traversals,
            retained_parents: &self.retained_scope_parents,
            moved_archives: &self.moved_archives,
            aliases: &self.aliases,
            base: &self.target_contexts,
            recipes: &mut self.database_contexts,
            branches: &mut self.database_branch_contexts,
            direct_heads,
        };
        let mut output =
            RuntimeOutput::new(&self.output, &self.program, &self.flag_state.warn_undefined);
        let mut flags = self.flag_state.receiver();
        let mut filesystem = |effect: rmake_core::diagnostic::FilesystemEffect<'_>| {
            self.filesystem.borrow_mut().execute(effect)
        };
        let shell = |request: rmake_core::diagnostic::ShellRequest<'_>| {
            crate::shell::run(
                request,
                &self.program,
                self.makelevel,
                &self.output,
                self.jobserver.as_ref(),
                || self.filesystem.borrow_mut().child_completed(),
            )
        };
        let record_open_times = self.ordinary_include_open_times && direct_heads;
        let mut open_results = Vec::new();
        let mut include = |name: &[u8]| {
            let result = self.include_search.borrow().read(name);
            if record_open_times {
                match &result {
                    Ok(included) => open_results.push((included.name.clone(), true)),
                    Err(IncludeFailure::Deferred { resolved_name, .. }) => {
                        open_results
                            .push((resolved_name.as_deref().unwrap_or(name).to_vec(), false));
                    }
                    Err(IncludeFailure::Fatal(_)) => {}
                }
            }
            result
        };
        let context = runtime_context(location, &mut output)
            .with_makeflags(&mut flags)
            .with_target_reads(&mut target_reads)
            .with_filesystem(&mut filesystem)
            .with_shell(&shell)
            .with_includes(&mut include);
        let result = operation(&mut self.database, context);
        for (name, opened) in open_results {
            self.retain_include_open_time(name, opened);
        }
        result
    }

    fn retain_include_open_time(&mut self, name: Vec<u8>, opened: bool) {
        if self.assumed_time(&name).is_some()
            || self.aliases.contains_source_or_destination(&name)
            || rmake_core::filenames::archive_parts(&name).is_some()
            || self
                .database
                .rules
                .get(&name)
                .is_some_and(|rule| rule.double_colon.is_some() || rule.grouped_targets().is_some())
        {
            return;
        }
        if !opened {
            // The recorded open already established that this file is missing.
            // A later graph visit consumes that observation before searching.
            self.retain_database_time(&name, None);
            self.reopened_include_times.remove(&name);
            self.database_unchecked_times.get_mut().remove(&name);
            if let Some(outcome) = self.file_traversals.outcome_mut(&name) {
                outcome.time = None;
            }
            self.path_observations.insert(name, None);
        } else if self
            .database_times
            .get_mut()
            .get(&name)
            .is_some_and(Option::is_none)
        {
            // Reopening only resets a missing time. An existing physical
            // observation and the earlier update verdict keep their owners.
            self.database_times.get_mut().remove(&name);
            if self
                .path_observations
                .get(&name)
                .is_some_and(Option::is_none)
            {
                self.path_observations.remove(&name);
            }
            if let Some(outcome) = self.file_traversals.outcome(&name) {
                if !outcome.failed && !outcome.question_outdated {
                    self.reopened_include_times.insert(name.clone());
                }
                self.database_unchecked_times.get_mut().insert(name);
            }
        }
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
            .with_shell(&|request| {
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
            let shell = |request: rmake_core::diagnostic::ShellRequest<'_>| {
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
                .with_shell(&shell)
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
        let shell = |request: rmake_core::diagnostic::ShellRequest<'_>| {
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
            .with_shell(&shell)
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
        let cached = self
            .file_traversals
            .outcome(identity)
            .is_some_and(|outcome| outcome.remake_failure.is_optional());
        // A single double-colon peer has a branch record before its own visit.
        let declared_peer = !cached
            && self.remaking
            && !self.parallel_makefile_remakes
            && !MakefileModes::from_options(&self.options).active()
            && self
                .database
                .rules
                .get(identity)
                .and_then(|rule| rule.double_colon.as_ref())
                .is_some_and(|branches| {
                    matches!(branches.as_slice(), [rule]
                    if rule.grouped_double_id().is_some())
                })
            && self
                .database_branch_outcomes
                .get(&(identity.to_vec(), 0))
                .is_some_and(|outcome| outcome.remake_failure.is_optional());
        (cached || declared_peer) && !self.optional_remake_failures.contains_key(identity)
    }

    fn observed_failed_duplicate_peer(&self, name: &[u8]) -> bool {
        if self.database_branch_outcomes.is_empty() || self.file_traversals.has_outcome(name) {
            return false;
        }
        let Some((head, rest)) = self
            .database
            .rules
            .get(name)
            .and_then(|rule| rule.double_colon.as_deref())
            .and_then(<[Rule]>::split_first)
        else {
            return false;
        };
        // Repeated occurrences of a completed peer came from one declaration.
        // There is no new optional failure to report when visiting that peer.
        let group = head.grouped_double_id();
        group.is_some()
            && !rest.is_empty()
            && rest.iter().all(|rule| rule.grouped_double_id() == group)
            && self
                .database_branch_outcomes
                .get(&(name.to_vec(), 0))
                .is_some_and(|head| {
                    head.failed && head.database_result == DatabaseResult::ObservedGroup
                })
    }

    fn cached_recipe_outcome(&self, mut outcome: Outcome) -> Outcome {
        if !self.remaking && self.active_makefile_remake.is_some() {
            // A recipe left running by a stopped optional remake can finish
            // during ordinary goals without becoming ordinary recipe activity.
            outcome.ran = false;
        }
        outcome
    }

    fn mark_remade_group_peer(&self, target: &[u8], phony: bool, outcome: &mut Outcome) {
        outcome.remake_group_peer_updated = self.remaking
            && outcome.completion == CompletionTime::Refresh
            && !outcome.failed
            && !phony
            && self.makefile_names.contains(target);
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
        if shutdown == Shutdown::Running && !self.database_optional_visits.is_empty() {
            // The database is printed after the serial update stack unwinds.
            self.database_fatal_optional_visits
                .borrow_mut()
                .clone_from(&self.database_optional_visits);
        }
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
                .with_shell(&|request| {
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
                .with_shell(&|request| {
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
                .with_shell(&|request| {
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
                self.fatal_system(Message::DuplicateStdinMakefile);
                return Err(());
            }
            let source = match rmake_linux::stdin_source::Source::copy_stdin() {
                Ok(source) => source,
                Err(_) if rmake_linux::signals::pending().is_some() => return Err(()),
                Err(rmake_linux::stdin_source::CopyError::Write { path, error }) => {
                    let detail = rmake_linux::filesystem::source_error_detail(&error);
                    self.fatal_system(Message::StdinTemporaryWrite {
                        path: path.as_os_str().as_bytes(),
                        detail: &detail,
                    });
                    return Err(());
                }
                Err(rmake_linux::stdin_source::CopyError::Create { path, error }) => {
                    rmake_linux::stdin_source::retain_create_failure_mask();
                    let message = crate::output::temporary_creation_message(&path, &error);
                    emit_output(
                        &self.output,
                        &self.program,
                        Output::Warning {
                            location: None,
                            message: &message,
                        },
                    );
                    self.fatal_system(Message::StdinTemporaryStorage);
                    return Err(());
                }
                Err(rmake_linux::stdin_source::CopyError::Storage) => {
                    self.fatal_system(Message::StdinTemporaryStorage);
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
            self.file_traversals.set_outcome(
                name,
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
        self.job_debug
            .drain(self.output.debug_mask(), retired_slots);
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
                        let detail = rmake_linux::filesystem::source_error_detail(&error);
                        self.stderr(&crate::messages::system(Message::OsError {
                            action: b"chdir",
                            path: b"",
                            detail: &detail,
                        }));
                        false
                    }
                }
            } else {
                false
            };
            if !restored {
                self.fatal_system(Message::RestartOriginalDirectory);
                crate::stdin_source::unlink(
                    plan.stdin_source.as_ref(),
                    &self.output,
                    &self.program,
                );
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
                    .with_shell(&|request| {
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
                .with_shell(&|request| {
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
                .with_shell(&|request| {
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
                .with_shell(&|request| {
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
            // The announcement uses the current request after reader flags settle.
            if let Some(mode) = self.options.shuffle.as_deref() {
                crate::debug::emit(&self.output, crate::debug::Event::ShuffleEnabled(mode));
            }
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
            self.set_makefile_modes(MakefileModes::default());
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
                    && self.jobserver.is_some());
            let basic_load_debug = self.options.load_limit.is_some()
                && debug_before_remake.bits() == remake_debug_bits;
            // Required makefiles settle before the ordinary mode resumes.
            // An explicit goal can also name a makefile and use that mode.
            let ordinary_mode = modes.active();
            let synthetic_trace_source = if !ordinary_mode
                && (debug_before_remake == Mask::ALL
                    || debug_before_remake.bits() == remake_debug_bits | Mask::VERBOSE.bits())
                && self.jobserver.is_some()
                && !always
                && self.options.output_sync.is_none()
                && self.options.inherited_output_sync.is_none()
            {
                self.initial_synthetic_remake_trace_source(&read_files)
            } else {
                None
            };
            let named_sources = if ordinary_mode {
                self.index_named_remake_sources(&read_files)
            } else {
                None
            };
            let named_include = |file: &Makefile| {
                self.is_makefile_goal(
                    rmake_core::filenames::without_dot_prefix(&file.name),
                    named_sources.as_ref(),
                )
            };
            let limited_load = self.options.load_limit.is_some();
            let final_named_sources = match read_files.as_slice() {
                [(_, false), (first, false), named @ ..]
                    if named.len() >= 2
                        && (modes.touch || modes.dry_run)
                        && !(first.optional || named_include(first))
                        && named
                            .iter()
                            .all(|(file, skip)| !skip && named_include(file)) =>
                {
                    named
                }
                _ => &[],
            };
            let optional_named_sources = final_named_sources
                .iter()
                .filter(|(file, _)| file.optional)
                .count();
            let final_required_dry = final_named_sources.len() == 2
                && optional_named_sources == 0
                && modes.dry_run
                && !(modes.question || modes.touch);
            let final_named_dry = limited_load
                && final_required_dry
                && self.options.jobs.is_some_and(|slots| slots >= 4);
            let three_named_two_optional =
                optional_named_sources == 2 && final_named_sources.len() == 3;
            let final_named_touch_debug = debug_before_remake.bits() == remake_debug_bits
                && (final_named_sources.len() == 2
                    || (!final_named_sources.is_empty() && optional_named_sources == 0)
                    || (final_named_sources.len() == 3 && optional_named_sources == 1)
                    || (limited_load
                        && optional_named_sources == 1
                        && matches!(final_named_sources, [(first, _), _, _, (last, _)]
                            if first.optional || last.optional))
                    || three_named_two_optional);
            let two_optional_touch = three_named_two_optional
                && (debug_before_remake.bits() == Mask::NONE.bits() || final_named_touch_debug);
            let optional_pair_before_required = two_optional_touch
                && matches!(final_named_sources, [_, _, (last, _)] if !last.optional);
            let final_named_touch = modes.touch
                && !final_named_sources.is_empty()
                && (optional_named_sources <= 1
                    || final_named_sources.len() == 2
                    || two_optional_touch)
                && (!debug_before_remake.has(Mask::MAKEFILES) || final_named_touch_debug)
                && self.options.jobs.is_some_and(|slots| slots >= 2);
            let unruled_primary = ordinary_mode
                && (limited_load || final_named_touch)
                && self.unruled_primary_before_named_includes(&read_files, named_sources.as_ref());
            let no_load_primary_dry = !limited_load
                && final_required_dry
                && !debug_before_remake.has(Mask::MAKEFILES)
                && self
                    .options
                    .jobs
                    .is_some_and(|slots| (3..usize::MAX).contains(&slots))
                && self.unruled_primary_before_named_includes(&read_files, named_sources.as_ref());
            let named_load_files = &read_files[usize::from(unruled_primary)..];
            let mixed_named_overlap = unruled_primary
                && (final_named_touch
                    || match named_load_files {
                        [(_, false), (_, false)] => {
                            !debug_before_remake.has(Mask::MAKEFILES) || qualified_remake_debug
                        }
                        [(first, false), (middle, false), (last, false)] => {
                            let named_first = named_include(first);
                            let named_last = named_include(last);
                            let early_named = named_first || (!named_last && named_include(middle));
                            let required_ends = !(first.optional || last.optional);
                            let all_required = required_ends && !middle.optional;
                            let required_touch = !named_last
                                && all_required
                                && modes.touch
                                && (!modes.dry_run || !modes.question);
                            let optional_middle_touch = !(named_first || named_last)
                                && required_ends
                                && middle.optional
                                && modes.touch;
                            !debug_before_remake.has(Mask::MAKEFILES)
                                && ((modes.question && !modes.touch)
                                    || (named_first && !named_last)
                                    || final_named_dry
                                    || (early_named
                                        && ((modes.dry_run && !modes.touch)
                                            || required_touch
                                            || optional_middle_touch)))
                        }
                        _ => false,
                    })
                && named_load_files
                    .iter()
                    .any(|(file, _)| !named_include(file));
            let overlap_primary_recipe = unruled_primary
                && named_load_files.iter().all(|(file, skip)| {
                    *skip
                        || (mixed_named_overlap && !file.optional)
                        || self.is_makefile_goal(
                            rmake_core::filenames::without_dot_prefix(&file.name),
                            named_sources.as_ref(),
                        )
                });
            let check_primary_before_overlap = unruled_primary && !overlap_primary_recipe;
            let verbose_mode_remakes = ordinary_mode
                && qualified_remake_debug
                && debug_before_remake.has(Mask::VERBOSE)
                && read_files
                    .iter()
                    .all(|(file, skip)| *skip || !file.optional)
                && read_files.iter().any(|(file, skip)| {
                    let name = rmake_core::filenames::without_dot_prefix(&file.name);
                    !*skip
                        && self.is_makefile_goal(name, named_sources.as_ref())
                        && self.database.rules.get(name).is_some_and(|rule| {
                            !rule.dependencies.is_empty()
                                && (file.location.is_none()
                                    || rule.recipes.as_ref().is_some_and(|recipes| {
                                        matches!(recipes.as_slice(), [recipe] if !recipe_is_recursive(recipe))
                                    }))
                        })
                });
            let direct_mode_remakes = ordinary_mode
                && qualified_remake_debug
                && (debug_before_remake.has(Mask::VERBOSE) || basic_load_debug)
                && {
                    let mut seen = HashSet::new();
                    let no_recipe_fallback = self.no_remake_recipe_fallback();
                    read_files.iter().all(|(file, skip)| {
                        let name = rmake_core::filenames::without_dot_prefix(&file.name);
                        *skip
                            || (seen.insert(name)
                                && !self.is_makefile_goal(name, named_sources.as_ref())
                                && self
                                    .database
                                    .rules
                                    .get(name)
                                    .and_then(|rule| rule.recipes.as_ref())
                                    .map_or(no_recipe_fallback, |recipes| recipes.len() <= 1))
                    })
                };
            let verbose_load_trace = self.options.load_limit.is_some()
                && debug_before_remake.bits() == (remake_debug_bits | Mask::VERBOSE.bits());
            let verbose_load_debug = verbose_load_trace
                && (!ordinary_mode || direct_mode_remakes)
                && self.qualified_load_debug_sources(&read_files, false, 1, false);
            let optional_dependency_load_debug = ordinary_mode
                && (basic_load_debug || verbose_load_trace)
                && direct_mode_remakes
                && self.qualified_load_debug_sources(&read_files, true, 1, false);
            let optional_named_load = ordinary_mode
                && self.options.load_limit.is_some()
                && (!debug_before_remake.has(Mask::MAKEFILES)
                    || (qualified_remake_debug && (verbose_load_trace || mixed_named_overlap)))
                && read_files.iter().any(|(file, skip)| {
                    !*skip
                        && file.optional
                        && self.is_makefile_goal(
                            rmake_core::filenames::without_dot_prefix(&file.name),
                            named_sources.as_ref(),
                        )
                })
                && self.qualified_load_debug_sources(named_load_files, false, 2, true)
                && self.qualified_debug_remake_sources(&read_files, named_sources.as_ref());
            let named_load_debug = ordinary_mode
                && qualified_remake_debug
                && (basic_load_debug
                    || (verbose_load_trace
                        && read_files
                            .iter()
                            .all(|(file, skip)| *skip || !file.optional)))
                && read_files.iter().any(|(file, skip)| {
                    !*skip
                        && self.is_makefile_goal(
                            rmake_core::filenames::without_dot_prefix(&file.name),
                            named_sources.as_ref(),
                        )
                })
                && self.qualified_load_debug_sources(named_load_files, false, 2, true)
                && self.qualified_debug_remake_sources(&read_files, named_sources.as_ref());
            let parallel_remake_debug = !debug_before_remake.has(Mask::MAKEFILES)
                || synthetic_trace_source.is_some()
                || (qualified_remake_debug
                    && (self.options.load_limit.is_none()
                        || basic_load_debug
                        || verbose_load_debug
                        || optional_dependency_load_debug
                        || named_load_debug
                        || optional_named_load));
            let assumed_double_colon_dry_run =
                if !debug_before_remake.has(Mask::MAKEFILES) && self.job_slots() > 1 {
                    self.qualified_assumed_double_colon_dry_run(&read_files, modes)
                } else {
                    None
                };
            let terminal_optional_assumed_dry_handoff = assumed_double_colon_dry_run
                == Some(AssumedDoubleColonDryRemake::TerminalOptionalHandoff);
            let optional_assumed_dry_handoff = terminal_optional_assumed_dry_handoff
                || assumed_double_colon_dry_run
                    == Some(AssumedDoubleColonDryRemake::OptionalHandoff);
            let unrelated_assumption_dry_handoff = modes.only_dry_run()
                && !debug_before_remake.has(Mask::MAKEFILES)
                && self.job_slots() > 1
                && self.qualified_unrelated_assumption_dry_handoff(&read_files);
            let remakes_can_overlap = !ordinary_mode
                || ((!debug_before_remake.has(Mask::MAKEFILES)
                    || (final_named_touch && final_named_touch_debug)
                    || named_load_debug
                    || optional_named_load
                    || (((debug_before_remake.bits() == remake_debug_bits && !basic_load_debug)
                        || verbose_mode_remakes
                        || direct_mode_remakes)
                        && (optional_dependency_load_debug
                            || self.qualified_debug_remake_sources(
                                &read_files,
                                named_sources.as_ref(),
                            ))))
                    && (assumed_double_colon_dry_run.is_some()
                        || unrelated_assumption_dry_handoff
                        || self.ordinary_mode_remakes_settle(
                            &read_files,
                            named_sources.as_ref(),
                            modes,
                            optional_named_load,
                        )));
            let parallel_remakes =
                self.job_slots() > 1 && remakes_can_overlap && parallel_remake_debug;
            // An existing unruled primary has no recipe to run in dry-run mode.
            let serial_ordinary_source_clocks = self.job_slots() == 1
                && modes.active()
                && read_files.iter().enumerate().all(|(index, (file, skip))| {
                    *skip
                        || !named_include(file)
                        || (index == 0
                            && modes.only_dry_run()
                            && self.options.goals.len() == 1
                            && !file.optional
                            && !file.missing
                            && file.location.is_none()
                            && self.no_remake_recipe_fallback()
                            && !self.database.rules.contains_key(
                                rmake_core::filenames::without_dot_prefix(&file.name),
                            ))
                });
            self.parallel_makefile_remakes = parallel_remakes && !check_primary_before_overlap;
            let use_required_source_head =
                parallel_remakes && self.assumptions.is_some() && !modes.active();
            let all_required_sources = use_required_source_head
                && read_files
                    .iter()
                    .all(|(source, skip)| *skip || !source.optional);
            let leading_optional_source = if use_required_source_head && !all_required_sources {
                self.unruled_leading_optional_source(&read_files)
            } else {
                None
            };
            let use_failed_leading_head = use_required_source_head
                && !all_required_sources
                && leading_optional_source.is_none()
                && self.options.new_files.len() == 1;
            let check_leading_clock_after_reap = self.parallel_makefile_remakes
                && !modes.active()
                && read_files.len() == 4
                && self.options.new_files.len() <= 1;
            let leading_optional_layout =
                if use_failed_leading_head || check_leading_clock_after_reap {
                    self.leading_optional_remake_layout(&read_files)
                } else {
                    None
                };
            let failed_leading_optional = leading_optional_layout
                .as_ref()
                .filter(|_| use_failed_leading_head);
            let direct_trailing_layout = leading_optional_layout.as_ref().filter(|leading| {
                check_leading_clock_after_reap
                    && leading.trailing.as_ref().is_some_and(|trailing| {
                        trailing.kind == TrailingOptionalKind::Declared
                            && self.unassumed_direct_double_colon_remake(&leading.required.name)
                    })
            });
            let deferred_trailing_layout =
                direct_trailing_layout.filter(|_| self.options.keep_going);
            // A completed pair can also supply the ordinary goal clock in
            // stopping mode after the remake pass continues.
            let required_clock_layout = deferred_trailing_layout.or_else(|| {
                direct_trailing_layout
                    .filter(|leading| !self.has_assumed_double_colon_entry(&leading.required.name))
            });
            let ordinary_current_leading = direct_trailing_layout.filter(|leading| {
                !self.has_assumed_double_colon_entry(&leading.required.name)
                    && !leading.neighbor.missing
                    && self.current_leading_optional_source(&leading.neighbor.name)
            });
            let assumed_current_leading = leading_optional_layout.as_ref().filter(|leading| {
                check_leading_clock_after_reap
                    && use_failed_leading_head
                    && self.options.keep_going
                    && !leading.neighbor.missing
                    && self.current_leading_optional_source(&leading.neighbor.name)
                    && leading
                        .trailing
                        .as_ref()
                        .is_some_and(|trailing| trailing.kind == TrailingOptionalKind::Declared)
                    && self
                        .assumed_new_double_colon_entries(&leading.required.name)
                        .is_some_and(|(_, count)| count == 1)
                    && self
                        .database
                        .rules
                        .get(&leading.required.name)
                        .and_then(|rule| rule.double_colon.as_ref())
                        .is_some_and(|branches| branches.len() == 2)
            });
            let trailing_optional_source = if use_required_source_head
                && !all_required_sources
                && leading_optional_source.is_none()
            {
                self.trailing_optional_remake(&read_files)
            } else {
                None
            };
            let ordinary_trailing_only = if self.parallel_makefile_remakes
                && !modes.active()
                && read_files.len() == 3
                && self.options.new_files.len() <= 1
            {
                self.trailing_optional_remake_layout(&read_files)
                    .filter(|trailing| {
                        trailing.kind == TrailingOptionalKind::Declared
                            && self.unassumed_direct_double_colon_remake(&trailing.required.name)
                    })
            } else {
                None
            };
            let mut ordinary_trailing_optional = ordinary_trailing_only.as_ref();
            let uses_declared_synthetic_head = |engine: &Self, source: &Makefile| {
                !source.optional
                    && !source.missing
                    && source.location.is_some()
                    && (all_required_sources
                        || leading_optional_source.is_some_and(|neighbor| {
                            engine
                                .file_traversals
                                .outcome(neighbor)
                                .is_some_and(|outcome| {
                                    // This source has settled before the required visit.
                                    outcome.time.is_some()
                                        && !outcome.ran
                                        && !outcome.fatal
                                        && outcome.terminal_verdict().is_none()
                                })
                        }))
                    && !engine.has_requested_goal_name(rmake_core::filenames::without_dot_prefix(
                        &source.name,
                    ))
            };
            let source_outcome = |engine: &Self,
                                  source: &Makefile,
                                  mut outcome: Outcome,
                                  prior_visits: Option<(
                &MakefileVisits<'_>,
                &PendingMakefileRemake<'_>,
            )>| {
                // Keep each branch failure for ordinary prerequisite visits.
                // The remake pass uses the original successful source head.
                let declared_tail_failed = matches!(
                    outcome.remake_failure,
                    RemakeFailure::RequiredDeclaredTailRecipe
                ) && !outcome.fatal
                    && !outcome.question_outdated;
                let explicit_refresh = outcome.ran_refresh();
                let (retained_worker_restart, missing_after_current_leading) = if explicit_refresh
                    && outcome.failed
                    && !outcome.question_outdated
                    && matches!(
                        outcome.remake_failure,
                        RemakeFailure::RequiredSyntheticInput
                    )
                    && let Some((visited, request)) = prior_visits
                {
                    let current_leading_trailing = assumed_current_leading
                        .filter(|leading| {
                            std::ptr::eq(leading.required, source)
                                && request
                                    .trailing_optional_worker
                                    .as_ref()
                                    .is_some_and(|worker| worker.only_synthetic_tail_remains)
                                && engine.settled_current_leading_optional(
                                    &leading.neighbor.name,
                                    visited,
                                )
                        })
                        .and_then(|leading| leading.trailing.as_ref());
                    (
                        trailing_optional_source
                            .as_ref()
                            .or(current_leading_trailing)
                            .is_some_and(|trailing| {
                                std::ptr::eq(trailing.required, source)
                                    && (engine
                                        .retained_trailing_worker_clock(request, trailing, visited)
                                        || engine.restored_required_worker_clock(
                                            request, trailing, visited,
                                        ))
                            }),
                        current_leading_trailing.is_some_and(|trailing| {
                            engine.settled_trailing_optional_worker(request, trailing, visited)
                        }),
                    )
                } else {
                    (false, false)
                };
                let missing_after_leading_failure = explicit_refresh
                    && matches!(
                        outcome.remake_failure,
                        RemakeFailure::RequiredSyntheticInput
                    )
                    && prior_visits.is_some_and(|(visited, request)| {
                        request.optional_visit_settled
                            && failed_leading_optional.is_some_and(|leading| {
                                std::ptr::eq(leading.required, source)
                                    && leading.trailing.as_ref().is_none_or(|trailing| {
                                        engine.settled_trailing_optional_visit(
                                            trailing,
                                            visited,
                                            request.same_clock_inline_optional_settled,
                                        ) || engine.settled_trailing_optional_worker(
                                            request, trailing, visited,
                                        )
                                    })
                            })
                    });
                if declared_tail_failed
                    || missing_after_leading_failure
                    || missing_after_current_leading
                    || retained_worker_restart
                    || (matches!(
                        outcome.remake_failure,
                        RemakeFailure::RequiredSyntheticInput
                            | RemakeFailure::RequiredSyntheticRecipe
                    ) && (uses_declared_synthetic_head(engine, source)
                        || trailing_optional_source.as_ref().is_some_and(|trailing| {
                            std::ptr::eq(trailing.required, source)
                                && match outcome.remake_failure {
                                    // The child can set this verdict only after
                                    // the optional visit settles.
                                    RemakeFailure::RequiredSyntheticRecipe => true,
                                    RemakeFailure::RequiredSyntheticInput => {
                                        if let Some((visited, request)) = prior_visits {
                                            // A real explicit recipe can finish
                                            // after the optional source settles,
                                            // then discover the missing rule.
                                            explicit_refresh
                                                && (request.optional_visit_settled
                                                    || engine.settled_trailing_optional_worker(
                                                        request, trailing, visited,
                                                    )
                                                    || visited
                                                        .get(trailing.neighbor)
                                                        .zip(
                                                            engine
                                                                .file_traversals
                                                                .outcome(trailing.neighbor),
                                                        )
                                                        .is_some_and(|((before, _), optional)| {
                                                            engine.settled_trailing_optional_remake(
                                                                trailing, *before, *optional, false,
                                                            )
                                                        }))
                                        } else {
                                            // A synchronous no-rule result
                                            // can precede the optional visit.
                                            trailing.kind == TrailingOptionalKind::Unruled
                                                && !engine
                                                    .file_traversals
                                                    .has_outcome(trailing.neighbor)
                                                && !engine.pending(trailing.neighbor)
                                                && engine.unruled_remake_source(trailing.neighbor)
                                                && !engine
                                                    .database
                                                    .rules
                                                    .contains_key(b".DEFAULT".as_slice())
                                                && fs::symlink_metadata(os(trailing.neighbor))
                                                    .is_ok_and(|file| file.is_file())
                                        }
                                    }
                                    _ => false,
                                }
                        })))
                {
                    outcome.failed = false;
                    outcome.first_branch_verdict = None;
                }
                (outcome, retained_worker_restart)
            };
            let mut loaded_prerequisite_times = HashMap::new();
            if !parallel_remakes {
                let has_implicit_recipes = !self.database.database_implicit_rules().is_empty();
                for (source_index, (makefile, skip)) in read_files.iter().enumerate() {
                    let name = makefile.name.as_slice();
                    if !skip
                        && !makefile.missing
                        && !self.path_observations.contains_key(name)
                        && rmake_core::filenames::archive_parts(name).is_none()
                        && let Some(time) = self.modified(name)
                    {
                        // A prior remake can remove an already loaded source.
                        // Its visit still sees the pre-remake file.
                        self.check_clock_skew(name, Some(time));
                        self.path_observations
                            .insert(name.to_vec(), Some((None, Some(time))));
                        if source_index != 0
                            && !modes.active()
                            && self.assumptions.is_none()
                            && !self.special(b".PHONY", name, false)
                        {
                            let needs_implicit_recipe =
                                self.database.rules.get(name).map_or(Some(true), |rule| {
                                    (rule.double_colon.is_none() && rule.implicit_pattern.is_none())
                                        .then_some(rule.recipes.is_none())
                                });
                            if let Some(needs_implicit_recipe) = needs_implicit_recipe
                                && (!needs_implicit_recipe || has_implicit_recipes)
                            {
                                loaded_prerequisite_times
                                    .insert(name, (time, needs_implicit_recipe));
                            }
                        }
                    }
                }
            }
            self.remaking = true;
            self.defer_stopped_remake_load = ordinary_mode;
            let mut changed = false;
            let repeated_named_clock_mode = !modes.touch && modes.dry_run != modes.question;
            let track_assumed_remake =
                !parallel_remakes && restarts == 0 && self.assumptions.is_some();
            let mut assumed_remake_changed = false;
            let mut assumed_remake_blocked = false;
            let mut grouped_changes =
                (!parallel_remakes && !ordinary_mode).then(SerialGroupedRemakeChanges::default);
            let mut ran_remake_recipe = false;
            let mut failed = false;
            let mut stopped = false;
            let mut pending = Vec::new();
            let mut deferred_inline_optional_clock = None;
            let initial_reaps = self.recipe_reaps;
            let mut pass_reaps = initial_reaps;
            let mut pass_progress = self.filesystem.borrow().generation();
            let track_remake_effects = direct_mode_remakes
                || basic_load_debug
                || verbose_load_debug
                || named_load_debug
                || optional_named_load;
            let first_effect_generation = track_remake_effects.then_some(pass_progress);
            let mut skipped_errors = Vec::new();
            let mut failed_remakes = HashMap::new();
            let mut settled_failed_source_changes = HashSet::new();
            let mut settled_named_peer_times = Vec::new();
            let mut optional_failures: HashMap<&[u8], OptionalMakefileFailure> = HashMap::new();
            let mut visited = MakefileVisits::new();
            let mut optional_remake_failed = false;
            let mut touched_optional_remakes = HashSet::new();
            let mut remake_question = false;
            let required_names: HashSet<_> = read_files
                .iter()
                .filter(|(file, _)| !file.optional)
                .map(|(file, _)| file.name.as_slice())
                .collect();
            for (source_index, (makefile, skip)) in read_files.iter().enumerate() {
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
                self.set_makefile_modes(visit_modes);
                let visit = match visited.entry(name) {
                    std::collections::hash_map::Entry::Vacant(visit) => visit,
                    std::collections::hash_map::Entry::Occupied(first) => {
                        let context = self.parallel_makefile_remakes.then(|| MakefileRemake {
                            optional: makefile.optional,
                            retain_failure: false,
                            use_declared_synthetic_head: uses_declared_synthetic_head(
                                self, makefile,
                            ),
                            use_declared_tail_head: false,
                            trace_initial_synthetic: false,
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
                                self.debug_cached_prerequisite_visit(name, 1, Outcome::default());
                                pending.push(PendingMakefileRemake {
                                    modes: visit_modes,
                                    file: makefile,
                                    context: context.clone(),
                                    duplicate: true,
                                    active: true,
                                    initial: Outcome::default(),
                                    trace_synthetic_reap: false,
                                    optional_visit_settled: false,
                                    same_clock_inline_optional_settled: false,
                                    required_worker_pair: None,
                                    trailing_optional_worker: None,
                                    trailing_prerequisite_child: None,
                                    trailing_prerequisite_changed: false,
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
                                && self.file_traversals.outcome(source).is_some_and(|outcome| {
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
                        } else if context.is_some() {
                            let cached = self
                                .file_traversals
                                .outcome(self.aliases.resolve(name))
                                .copied()
                                .unwrap_or_default();
                            // A settled duplicate already prunes its first
                            // entry. Its remaining declarations revisit the
                            // same considered chain head.
                            let before = first.get().0;
                            let extra_prunes = before.is_some()
                                && before == cached.time
                                && !cached.failed
                                && !cached.ran
                                && cached.completion == CompletionTime::Unchanged
                                && self.debug_was_considered(name)
                                && self.parallel_remake_debug_declarations(name);
                            self.debug_cached_prerequisite_visit(name, 1, cached);
                            if extra_prunes {
                                self.debug_double_colon_goal_prunes(name, cached);
                            }
                        }
                        if makefile.optional
                            && visit_modes.touch
                            && touched_optional_remakes.contains(name)
                        {
                            // A settled repeat can end the next pass too.
                            (pass_reaps, pass_progress) =
                                self.settle_optional_remake_pass(pass_progress, false);
                        }
                        continue;
                    }
                };
                let trace_parallel_declarations = self.parallel_remake_debug_declarations(name)
                    && !makefile.missing
                    && !self.pending(name)
                    && self
                        .file_traversals
                        .outcome(name)
                        .is_none_or(|outcome| !outcome.failed && self.debug_was_considered(name));
                let optional = makefile.optional;
                let may_replay = optional && required_names.contains(name);
                let deferred_peer_failure = !optional
                    && !self.options.keep_going
                    && self.cached_optional_group_peer_failure(name);
                let pending_peer = self.parallel_makefile_remakes && self.pending(name);
                // A previous source can have remade this prerequisite.
                // Preserve the owner's settled clock even if a later recipe
                // retimed the file. Group peers keep their own update policy.
                let cached_prerequisite = loaded_prerequisite_times.get(name).and_then(
                    |&(time, needs_implicit_recipe)| {
                        let cached = self.file_traversals.outcome(name)?;
                        (self.aliases.resolve(name) == name
                            && !self.pending(name)
                            && self
                                .file_traversals
                                .visit(name)
                                .is_some_and(|visit| visit.parent.is_some())
                            && !cached.fatal
                            && !cached.question_outdated
                            && !cached.remake_group_peer_updated
                            && cached.ran
                            && cached.completion == CompletionTime::Refresh
                            && (!needs_implicit_recipe
                                || self
                                    .database
                                    .rules
                                    .get(name)
                                    .is_some_and(|rule| rule.implicit_pattern.is_some())))
                        .then_some((time, cached))
                    },
                );
                let cached_prerequisite_change = cached_prerequisite
                    .filter(|(_, cached)| !cached.failed)
                    .map(|(loaded, cached)| cached.time != Some(loaded));
                let cached_failed_owner_change = cached_prerequisite
                    .filter(|(_, cached)| {
                        matches!(
                            cached.remake_failure,
                            RemakeFailure::SettledPrerequisiteOwner
                        )
                    })
                    .map(|(loaded, cached)| cached.time.is_some_and(|time| time != loaded));
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
                let unchanged_named_dry_source = if self.job_slots() == 1
                    && visit_modes.only_dry_run()
                    && !failed
                    && !optional_remake_failed
                    && !remake_question
                {
                    match before {
                        Some(None) => self
                            .removed_required_dry_declaration_count(&read_files, makefile)
                            .map(|declarations| (None, declarations)),
                        // A still-present source can retain its prerequisites on reread.
                        Some(Some(time)) if restarts == 0 => self
                            .present_required_dry_declaration_count(&read_files, makefile, time)
                            .map(|declarations| (Some(time), declarations)),
                        _ => None,
                    }
                } else {
                    None
                };
                // A fresh serial visit still sees the local timestamp
                // retained before earlier remakes wrote or removed the file.
                let loaded_local_time = if !parallel_remakes
                    && (!modes.active()
                        || serial_ordinary_source_clocks
                        || (self.job_slots() == 1
                            && modes.only_dry_run()
                            && !visit_modes.active()
                            && !failed
                            && !optional_remake_failed
                            && !remake_question
                            && self.follows_forced_named_dry_remake(
                                &read_files,
                                makefile,
                                named_sources.as_ref(),
                            )))
                    && self.assumptions.is_none()
                    && !makefile.missing
                    && before.is_some()
                    && let Some(Some((None, Some(time)))) = self.path_observations.get(name)
                    && self.aliases.resolve(name) == name
                    && !self.file_traversals.has_outcome(name)
                    && !self.pending(name)
                {
                    Some(*time)
                } else {
                    None
                };
                let (loaded_source, loaded_implicit_time) = if let Some(time) = loaded_local_time
                    && before.is_some_and(|current| current != Some(time))
                    && let Some(rule) = self.database.rules.get(name)
                {
                    let declared = rule
                        .double_colon
                        .as_deref()
                        .unwrap_or_else(|| std::slice::from_ref(rule));
                    if rule.double_colon.is_none()
                        && rule.grouped_targets().is_none()
                        && rule.recipes.is_none()
                    {
                        // Its visit can still select an implicit recipe.
                        (None, (!modes.active()).then_some(time))
                    } else {
                        let declared_time = (!declared.is_empty()
                            && declared.iter().all(|branch| {
                                branch.grouped_targets().is_none()
                                    && branch.recipes.is_some()
                                    && (!modes.active() || branch.implicit_pattern.is_none())
                            }))
                        .then_some((time, declared.len()));
                        (declared_time, None)
                    }
                } else {
                    (None, None)
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
                    use_declared_synthetic_head: uses_declared_synthetic_head(self, makefile),
                    use_declared_tail_head: self.parallel_makefile_remakes
                        && !modes.active()
                        && !optional
                        && !makefile.missing,
                    trace_initial_synthetic: self.parallel_makefile_remakes
                        && synthetic_trace_source == Some(name),
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
                // A named visit can precede the next reap when an earlier
                // unnamed recipe could change the order of its load decision.
                let optional_assumed_predecessor = if optional_assumed_dry_handoff
                    && optional
                    && named_source
                    && visit_modes.only_dry_run()
                {
                    self.sole_required_dry_worker(&pending)
                        .and_then(|(name, wait)| (wait == RemakeWait::Command).then_some(name))
                } else {
                    None
                };
                let build_visit = if optional_assumed_predecessor.is_some()
                    || (mixed_named_overlap
                        && self.parallel_makefile_remakes
                        && named_source
                        && (final_named_dry
                            || final_named_touch
                            || (modes.touch
                                && source_index == 2
                                && self.pending(&read_files[1].0.name))))
                {
                    BuildVisit::NamedRemakeWithoutPoll
                } else {
                    BuildVisit::Ordinary
                };
                let prior_peer_failure = optional
                    && !self.parallel_makefile_remakes
                    && !modes.active()
                    && self.observed_failed_duplicate_peer(name);
                let track_named_source = track_assumed_remake
                    && named_source
                    && modes.active()
                    && (repeated_named_clock_mode
                        || self
                            .assumed_new_double_colon_entries(self.aliases.resolve(name))
                            .is_some_and(|(_, count)| count == 1));
                let mut source_started_command = false;
                if let Some(trailing) = &trailing_optional_source
                    && name == trailing.neighbor
                    && pending_peer
                    && self.options.keep_going
                    && let Some(request) = pending.iter_mut().find(|request| {
                        !request.duplicate && std::ptr::eq(request.file, trailing.required)
                    })
                {
                    request.trailing_prerequisite_child = self
                        .trailing_remake_prerequisite_child(trailing, &read_files[..source_index]);
                }
                let mixed_trailing_source = failed_leading_optional
                    .as_ref()
                    .and_then(|leading| leading.trailing.as_ref());
                let inline_optional_dispatch = (trailing_optional_source
                    .as_ref()
                    .or(mixed_trailing_source)
                    .or_else(|| {
                        deferred_trailing_layout.and_then(|leading| leading.trailing.as_ref())
                    })
                    .is_some_and(|trailing| {
                        name == trailing.neighbor && trailing.kind == TrailingOptionalKind::Declared
                    })
                    && !pending_peer
                    && !self.file_traversals.has_outcome(self.aliases.resolve(name)))
                .then_some(self.next_job);
                let mut result = self.build_recording_initial_time(
                    name,
                    None,
                    0,
                    initial_time,
                    build_visit,
                    track_named_source.then_some(&mut source_started_command),
                );
                result = source_outcome(self, makefile, result, None).0;
                let chain_failed = result.failed;
                let refreshed_implicit_time = loaded_implicit_time.filter(|_| {
                    !MakefileModes::from_options(&self.options).active()
                        && !result.fatal
                        && !result.question_outdated
                        && !result.blocked
                        && result.ran
                        && result.completion == CompletionTime::Refresh
                        && self.aliases.resolve(name) == name
                        && self.database.rules.get(name).is_some_and(|rule| {
                            rule.implicit_pattern.is_some() && rule.grouped_targets().is_none()
                        })
                });
                // The raw branch results retain whether a recipe really
                // refreshed the source before policy reduces the verdict to
                // the declared head.
                let failed_source_time = loaded_source
                    .filter(|&(_, declarations)| {
                        !modes.active()
                            && chain_failed
                            && !result.fatal
                            && !result.question_outdated
                            && self.options.keep_going
                            && !MakefileModes::from_options(&self.options).active()
                            && match declarations {
                                1 => result.completion == CompletionTime::Refresh,
                                _ => self
                                    .settled_serial_source_declarations(name, declarations)
                                    .is_some_and(|(_, refreshed)| refreshed),
                            }
                    })
                    .map(|(time, _)| time);
                if track_assumed_remake {
                    // This verdict belongs to the source pass. A later
                    // failure can override a logical timestamp change.
                    assumed_remake_blocked |=
                        chain_failed || result.fatal || result.question_outdated;
                    if !assumed_remake_blocked
                        && !assumed_remake_changed
                        && !makefile.missing
                        && let Some(before) = before.flatten()
                    {
                        let live_modes = MakefileModes::from_options(&self.options);
                        // An unnamed makefile can run in ordinary mode
                        // while the eventual goals retain nonexecuting flags.
                        let regular_visit = !visit_modes.active()
                            && !live_modes.active()
                            && (!modes.active()
                                || (!named_source
                                    && self
                                        .assumed_new_double_colon_entries(
                                            self.aliases.resolve(name),
                                        )
                                        .is_some_and(|(_, count)| count == 1)));
                        // A named source can leave every branch current
                        // under dry-run, query or touch. A simulated
                        // declaration can propagate its assumed-new time to
                        // the entire chain.
                        let current_named_source = track_named_source
                            && !source_started_command
                            && !result.ran
                            && matches!(
                                result.completion,
                                CompletionTime::Unchanged | CompletionTime::Preserve
                            )
                            && self.modified(name) == Some(before);
                        if regular_visit || current_named_source {
                            // A stopping pass can also settle every synthetic
                            // entry when the source stays physically current.
                            let allow_stopping_repeats = if regular_visit {
                                !result.ran && result.time == Some(before)
                            } else {
                                repeated_named_clock_mode
                                    && (modes.dry_run || self.options.goals.len() == 1)
                                    && live_modes.dry_run == modes.dry_run
                                    && live_modes.question == modes.question
                                    && !live_modes.touch
                            };
                            assumed_remake_changed = self.settled_assumed_new_makefile_clock(
                                name,
                                allow_stopping_repeats,
                                None,
                            );
                        }
                    }
                }
                if let Some(changes) = &mut grouped_changes {
                    changes.failed |= chain_failed;
                }
                if prior_peer_failure
                    && result.failed
                    && !result.fatal
                    && self.output.debug_mask().has(crate::debug::Mask::VERBOSE)
                {
                    use crate::debug::{Event, emit};

                    // The visit already emitted its first Considering event.
                    // Remaining occurrences replay or prune the observed peer.
                    let declarations = self
                        .database
                        .rules
                        .get(name)
                        .and_then(|rule| rule.double_colon.as_ref())
                        .map_or(1, Vec::len);
                    emit(&self.output, Event::RecentlyFailed(name, 1));
                    if self.options.keep_going {
                        for _ in 1..declarations {
                            emit(&self.output, Event::Considering(name, 1));
                            emit(&self.output, Event::RecentlyFailed(name, 1));
                        }
                    }
                    for _ in 1..declarations {
                        emit(&self.output, Event::Pruning(name, 1));
                    }
                }
                let serial_declarations = if result.failed && !result.fatal && !prior_peer_failure {
                    self.serial_double_colon_branch_count(name)
                        .filter(|&count| {
                            count > 1
                                // A stopped optional head still has another
                                // goal entry when an assumption is active.
                                || (optional
                                    && !self.options.keep_going
                                    && self.has_assumed_double_colon_entry(name)
                                    && self.optional_remake_failures
                                        .get(name)
                                        .is_some_and(Option::is_some)
                                    && self.database_branch_outcomes
                                        .get(&(name.to_vec(), 0))
                                        .is_some_and(|head| head.failed))
                        })
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
                        if !optional {
                            // A failed required declaration revisits the shared
                            // physical head time, replacing its grouped update.
                            result.remake_group_peer_updated = false;
                        }
                    } else if optional {
                        result.fatal = true;
                    }
                }
                let tracked_group_change = result.remake_group_peer_updated
                    && grouped_changes.is_some()
                    && self
                        .database
                        .rules
                        .get(name)
                        .and_then(|rule| rule.double_colon.as_ref())
                        .is_some_and(|branches| {
                            branches
                                .iter()
                                .any(|rule| rule.grouped_double_id().is_some())
                        });
                if tracked_group_change && let Some(changes) = &mut grouped_changes {
                    let updated = visit_modes.check_update(result) && !result.failed;
                    changes.updated |= updated;
                    changes.optional_failed_tail_updated |= updated && optional && chain_failed;
                }
                let observed_peer_change = self
                    .serial_remake_peer_times
                    .remove(name)
                    .filter(|_| !failed_opens.contains(name));
                let before = observed_peer_change.or(before.unwrap_or(result.time));
                // A required chain under -k can revisit its cached head
                // after a failed declaration and report the prune.
                let settled_chain = !result.failed
                    || (!optional && self.options.keep_going && serial_declarations.is_some());
                let settled_parallel_declarations = trace_parallel_declarations
                    && before.is_some()
                    && before == result.time
                    && !result.ran
                    && result.completion == CompletionTime::Unchanged
                    && (!result.failed || !optional)
                    && !self.pending(name);
                if settled_parallel_declarations
                    || (!self.parallel_makefile_remakes
                        && settled_chain
                        && (!optional || before == result.time)
                        && !(optional
                            && serial_declarations.is_some()
                            && result.remake_group_peer_updated))
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
                    let held_synthetic =
                        if context.trace_initial_synthetic && self.debug_was_considered(name) {
                            self.parallel_jobs.primary.get(name).filter(|job| {
                                job.child.is_some() && job.continuation.branch == Some(1)
                            })
                        } else {
                            None
                        };
                    if held_synthetic.is_some() {
                        // A second source entry sees the considered head
                        // while the actual synthetic command is pending.
                        crate::debug::emit(&self.output, crate::debug::Event::Pruning(name, 1));
                    }
                    let trace_synthetic_reap = held_synthetic
                        .is_some_and(|job| !job.recursive && job.continuation.expanded.is_empty());
                    if let Some(trailing) = &trailing_optional_source
                        && name == trailing.neighbor
                        && self.options.keep_going
                        && before.is_some()
                        && !result.failed
                        && !result.fatal
                        && self.active_trailing_optional_remake(trailing)
                    {
                        self.qualify_synthetic_recipe_after_optional_read(&trailing.required.name);
                    }
                    if let Some(leading) = ordinary_current_leading
                        && let Some(trailing) = leading.trailing.as_ref()
                        && name == trailing.neighbor
                        && self.settled_current_leading_optional(&leading.neighbor.name, &visited)
                    {
                        ordinary_trailing_optional = Some(trailing);
                    }
                    let current_leading_trailing = assumed_current_leading.and_then(|leading| {
                        let trailing = leading.trailing.as_ref()?;
                        (name == trailing.neighbor
                            && self
                                .settled_current_leading_optional(&leading.neighbor.name, &visited))
                        .then_some(trailing)
                    });
                    let settled_failed_leading_trailing =
                        required_clock_layout.and_then(|leading| {
                            let trailing = leading.trailing.as_ref()?;
                            (name == trailing.neighbor
                                && self.settled_leading_optional_visit_failure(leading, &visited))
                            .then_some(trailing)
                        });
                    let trailing_worker_source = ordinary_trailing_optional
                        .or(current_leading_trailing)
                        .or_else(|| {
                            if !self.options.keep_going {
                                return settled_failed_leading_trailing;
                            }
                            mixed_trailing_source
                                .or_else(|| {
                                    trailing_optional_source
                                        .as_ref()
                                        .filter(|_| read_files.len() == 3)
                                })
                                .or(settled_failed_leading_trailing)
                        });
                    if let Some(trailing) = trailing_worker_source
                        && name == trailing.neighbor
                        && !visit_modes.active()
                        && !pending_peer
                        && !result.failed
                        && !result.fatal
                        && let Some(before) = before
                        && let Some(request) = pending.iter_mut().find(|request| {
                            !request.duplicate
                                && (mixed_trailing_source.is_none()
                                    || ordinary_trailing_optional.is_some()
                                    || current_leading_trailing.is_some()
                                    || request.optional_visit_settled)
                                && std::ptr::eq(request.file, trailing.required)
                        })
                    {
                        let direct_reread = ordinary_trailing_optional.is_some()
                            || current_leading_trailing.is_some()
                            || (read_files.len() == 3 && trailing_optional_source.is_some());
                        let retain_required_clock = direct_reread
                            || (settled_failed_leading_trailing.is_some()
                                && !self.has_assumed_double_colon_entry(&trailing.required.name));
                        request.trailing_optional_worker = self.track_trailing_optional_worker(
                            trailing,
                            before,
                            retain_required_clock,
                            direct_reread,
                        );
                    }
                    let optional_visit_settled = failed_leading_optional.is_some_and(|leading| {
                        std::ptr::eq(leading.required, makefile)
                            && self.pending_explicit_remake_recipe(name)
                            && visited.get(leading.neighbor.name.as_slice()).is_some_and(
                                |(before, _)| {
                                    self.settled_leading_optional_failure(leading.neighbor, *before)
                                },
                            )
                    });
                    pending.push(PendingMakefileRemake {
                        modes: visit_modes,
                        file: makefile,
                        context,
                        duplicate: false,
                        active: true,
                        initial: result,
                        trace_synthetic_reap,
                        optional_visit_settled,
                        same_clock_inline_optional_settled: false,
                        required_worker_pair: None,
                        trailing_optional_worker: None,
                        trailing_prerequisite_child: None,
                        trailing_prerequisite_changed: false,
                    });
                    continue;
                }
                let inline_completion = inline_optional_dispatch == Some(self.next_job);
                if let Some(trailing) = &trailing_optional_source
                    && name == trailing.neighbor
                    && self.options.keep_going
                    && self.settled_trailing_optional_remake(
                        trailing,
                        before,
                        result,
                        inline_completion,
                    )
                {
                    let required = &trailing.required.name;
                    self.qualify_synthetic_recipe_after_optional_read(required);
                    if inline_completion
                        && result.completion == CompletionTime::Refresh
                        && self.pending_explicit_remake_recipe(required)
                        && let Some(request) = pending.iter_mut().find(|request| {
                            !request.duplicate && std::ptr::eq(request.file, trailing.required)
                        })
                    {
                        // Keep the inline receipt while the direct explicit
                        // worker is alive. The later missing rule has no job.
                        request.optional_visit_settled = true;
                    }
                }
                if let Some(trailing) = mixed_trailing_source
                    && trailing.kind == TrailingOptionalKind::Declared
                    && name == trailing.neighbor
                    && self.options.keep_going
                    && inline_completion
                    && Self::same_clock_optional_remake_outcome(before, result)
                    && fs::symlink_metadata(os(name))
                        .is_ok_and(|file| file.is_file() && file.modified().ok() == before)
                    && self.settled_trailing_optional_remake(trailing, before, result, true)
                    && self.pending_explicit_remake_recipe(&trailing.required.name)
                    && let Some(request) = pending.iter_mut().find(|request| {
                        !request.duplicate
                            && request.optional_visit_settled
                            && std::ptr::eq(request.file, trailing.required)
                    })
                {
                    // Record the inline clock before the required worker can change it.
                    request.same_clock_inline_optional_settled = true;
                }
                let deferred_peer_failure = deferred_peer_failure
                    || (pending_peer
                        && result.remake_failure.is_optional()
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
                    // Keep the loaded clock for the required failure pass,
                    // even if an optional occurrence performed the remake.
                    let failed_implicit_time =
                        refreshed_implicit_time.filter(|_| chain_failed && self.options.keep_going);
                    // A serial declaration may change the file after a
                    // blocked head. Other roots use their own recipe result.
                    let direct_group_change = loaded_local_time
                        .filter(|_| {
                            matches!(
                                result.remake_failure,
                                RemakeFailure::DirectExplicitGroupOwner
                            ) && !failed_opens.contains(name)
                        })
                        .map(|loaded| result.time.is_some_and(|time| time != loaded));
                    let settled_failure_change =
                        result.remake_failure.peer_clock_changed(modes).or_else(|| {
                            if modes.active() {
                                None
                            } else {
                                cached_failed_owner_change.or(direct_group_change)
                            }
                        });
                    let refresh_timestamp = settled_failure_change.is_none()
                        && (serial_declarations.is_some() || (result.ran && !result.blocked));
                    if settled_failure_change == Some(true)
                        || (!result.ran
                            && observed_peer_change.is_some()
                            && self
                                .database
                                .rules
                                .get(name)
                                .and_then(|rule| rule.double_colon.as_ref())
                                .is_some_and(|branches| {
                                    matches!(branches.as_slice(), [rule]
                                if rule.grouped_double_id().is_some())
                                }))
                    {
                        settled_failed_source_changes.insert(name);
                    }
                    if may_replay {
                        optional_failures.insert(
                            name,
                            OptionalMakefileFailure {
                                open_error: pending_open_error(),
                                before: failed_implicit_time.or(before),
                                refresh_timestamp,
                            },
                        );
                    } else if !optional && (self.options.keep_going || deferred_peer_failure) {
                        let failed_before = observed_peer_change
                            .or(failed_source_time)
                            .or(failed_implicit_time)
                            .or(before);
                        failed_remakes.insert(name, (failed_before, refresh_timestamp));
                    }
                } else if !result.failed {
                    self.database_unchecked_times
                        .borrow_mut()
                        .remove(&makefile.name);
                    // A completed recipe can invalidate the makefile's time.
                    // Expansion effects alone leave its cached observation intact.
                    // Expansion can enable touch after the visit's initial
                    // modes were set. Check the resulting physical update.
                    let record_change = visit_modes.check_update(result)
                        || (!visit_modes.active()
                            && self.options.touch
                            && !result.question_outdated
                            && MakefileModes::from_options(&self.options).check_update(result));
                    // A real group recipe can complete a named peer even
                    // while its own dry-run or query suppresses this restart.
                    if !parallel_remakes
                        && (visit_modes.dry_run || visit_modes.question)
                        && !visit_modes.touch
                        && self.assumptions.is_none()
                        && result.remake_group_peer_updated
                        && !result.question_outdated
                        && let Some(before) = observed_peer_change
                    {
                        if result.ran
                            && result.time != Some(before)
                            && self.database.rules.get(name).is_some_and(|rule| {
                                rule.double_colon.is_none()
                                    && rule.grouped_targets().is_some()
                                    && rule.implicit_pattern.is_none()
                            })
                        {
                            settled_named_peer_times
                                .push((name, NamedRemakePeerTime::File(result.time)));
                        } else if visit_modes.only_dry_run()
                            && !makefile.missing
                            && makefile.location.is_some()
                            && !self.options.check_symlink_times
                            && let Some(time) = self.settled_grouped_double_peer_time(name)
                            && time != before
                        {
                            // A cached branch may have restored the root's visit time.
                            settled_named_peer_times
                                .push((name, NamedRemakePeerTime::GroupedDoubleColon(time)));
                        }
                    }
                    // Question mode suppresses the restart verdict, but a
                    // physical optional touch can still end this visit pass.
                    let check_optional_touch = optional
                        && !remake_question
                        && !failed
                        && !optional_remake_failed
                        && visit_modes.question
                        && visit_modes.touch
                        && !visit_modes.dry_run
                        && result.ran
                        && !result.question_outdated;
                    let physically_changed = if record_change
                        && observed_peer_change.is_none()
                        && !MakefileModes::from_options(&self.options).active()
                        && let Some(changed) = cached_prerequisite_change
                    {
                        changed
                    } else if record_change || check_optional_touch {
                        let after = self.modified(&makefile.name);
                        let loaded_time = failed_source_time.or_else(|| {
                            loaded_source
                                .filter(|&(_, declarations)| {
                                    !MakefileModes::from_options(&self.options).active()
                                        && (declarations == 1
                                            || (!chain_failed
                                                && self
                                                    .settled_serial_source_declarations(
                                                        name,
                                                        declarations,
                                                    )
                                                    .is_some_and(|(succeeded, _)| succeeded)))
                                })
                                .map(|(time, _)| time)
                        });
                        let implicit_time = refreshed_implicit_time.filter(|_| !chain_failed);
                        // Completed declarations share the final physical
                        // observation, including a restored timestamp or an absent source.
                        let before_refresh = observed_peer_change
                            .or(loaded_time)
                            .or(implicit_time)
                            .or(before);
                        after != before_refresh
                    } else {
                        false
                    };
                    if record_change
                        && inline_completion
                        && !visit_modes.active()
                        && self.options.keep_going
                        && let Some(leading) = deferred_trailing_layout
                        && let Some(trailing) = leading.trailing.as_ref()
                        && name == trailing.neighbor
                        && let (Some(before), Some(inline)) = (before, result.time)
                        && inline != before
                        && result.ran
                        && self.settled_trailing_optional_remake(
                            trailing,
                            Some(before),
                            result,
                            true,
                        )
                        && self.pending_explicit_remake_recipe(&leading.required.name)
                        && visited.get(leading.neighbor.name.as_slice()).is_some_and(
                            |(before, _)| {
                                self.settled_leading_optional_failure(leading.neighbor, *before)
                            },
                        )
                    {
                        // The earlier failure leaves this source's new clock
                        // to be sampled after the required worker finishes.
                        deferred_inline_optional_clock = Some(InlineOptionalRemakeClock {
                            required: &leading.required.name,
                            source: name,
                            before,
                            inline,
                            changed_on_visit: physically_changed,
                        });
                    } else {
                        changed |= record_change && physically_changed;
                    }
                    changed |=
                        record_change && result.remake_group_peer_updated && !tracked_group_change;
                    // Use the simulated clock of a loaded required source after
                    // all named declarations have settled at the same physical time.
                    if !changed
                        && result.completion == CompletionTime::AssumeNew
                        && result.database_result == DatabaseResult::Simulated
                        && !result.fatal
                        && result.terminal_verdict().is_none()
                        && MakefileModes::from_options(&self.options).only_dry_run()
                        && let Some((physical_time, declarations)) = unchanged_named_dry_source
                        && physical_time.is_none_or(|time| result.time == Some(time))
                        && self
                            .settled_serial_source_declarations(name, declarations)
                            .is_some_and(|(succeeded, refreshed)| succeeded && !refreshed)
                    {
                        changed = self.modified(name) == physical_time;
                    }
                    let simulated_change =
                        visit_modes.dry_run && result.completion == CompletionTime::AssumeNew;
                    if self.parallel_makefile_remakes
                        && optional
                        && visit_modes.active()
                        && !result.question_outdated
                        && (physically_changed || simulated_change)
                        && (!optional_assumed_dry_handoff || optional_assumed_predecessor.is_some())
                        && !pending.is_empty()
                        && ((terminal_optional_assumed_dry_handoff && simulated_change)
                            || read_files[source_index + 1..]
                                .iter()
                                .any(|(_, skip)| !*skip))
                    {
                        // An immediate optional update ends this visit pass.
                        // The required predecessor can then finish under the named mode.
                        if visit_modes.touch && physically_changed {
                            touched_optional_remakes.insert(name);
                        }
                        // Delay the earlier unnamed include through the primary continuation.
                        // A second optional include can refill that slot before the required
                        // tail once the primary has finished.
                        let defer_load_refill = mixed_named_overlap
                            && (final_named_touch || (read_files.len() == 4 && source_index == 2))
                            && visit_modes.touch
                            && pending.iter().any(|request| {
                                request.file.location.is_some() && !request.modes.active()
                            })
                            && !(optional_pair_before_required
                                && limited_load
                                && !visit_modes.question
                                && source_index + 2 == read_files.len()
                                && !self.pending(&read_files[0].0.name));
                        (pass_reaps, pass_progress) =
                            self.settle_optional_remake_pass(pass_progress, defer_load_refill);
                        if simulated_change && let Some(prior) = optional_assumed_predecessor {
                            // The optional source ended the visit pass. Its
                            // predecessor may finish under the retained dry mode.
                            changed |= self
                                .file_traversals
                                .outcome(self.aliases.resolve(prior))
                                .is_some_and(|outcome| {
                                    outcome.terminal_verdict().is_none()
                                        && !outcome.fatal
                                        && outcome.ran
                                        && outcome.completion == CompletionTime::AssumeNew
                                        && outcome.database_result == DatabaseResult::Simulated
                                });
                        }
                        if optional_named_load
                            && verbose_load_trace
                            && self.shutdown.get() == Shutdown::Running
                        {
                            for request in pending.iter_mut().filter(|request| request.active) {
                                let prior = self.aliases.resolve(&request.file.name);
                                let queried_primary = mixed_named_overlap
                                    && visit_modes.touch
                                    && visit_modes.question
                                    && std::ptr::eq(request.file, &read_files[0].0);
                                if !self.pending(prior)
                                    && self.file_traversals.outcome(prior).is_some_and(|outcome| {
                                        !outcome.failed
                                            && (!outcome.question_outdated || queried_primary)
                                    })
                                {
                                    // A settled earlier source is pruned
                                    // before the first unvisited root starts.
                                    crate::debug::emit(
                                        &self.output,
                                        crate::debug::Event::Pruning(prior, 1),
                                    );
                                    request.active = false;
                                }
                            }
                        }
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
                if source_index == 0 && parallel_remakes && check_primary_before_overlap {
                    // The primary used the normal graph visit and implicit
                    // lookup. Only an unchanged source with no selected recipe
                    // can be left behind when the other roots overlap.
                    self.parallel_makefile_remakes = !result.ran
                        && !result.failed
                        && !result.question_outdated
                        && result.time == before
                        && self
                            .database
                            .rules
                            .get(name)
                            .is_none_or(|rule| rule.recipes.is_none());
                }
            }
            if !stopped && !pending.is_empty() {
                // Reaping before the next source visit retains the last
                // named mode for a still-running implicit primary or named recipe.
                // Without a load limit, keep this only while the earlier unnamed
                // include and the primary both have running recipe children.
                let active_primary = pending
                    .first()
                    .is_some_and(|remake| std::ptr::eq(remake.file, &read_files[0].0))
                    && (overlap_primary_recipe
                        || (no_load_primary_dry
                            && read_files[..2].iter().all(|(file, _)| {
                                self.parallel_jobs
                                    .primary
                                    .get(self.aliases.resolve(&file.name))
                                    .is_some_and(|job| job.child.is_some())
                            })));
                let live_modes = MakefileModes::from_options(&self.options);
                let terminal_named_dry_wait = if modes.only_dry_run()
                    && live_modes.only_dry_run()
                    && !failed
                    && !optional_remake_failed
                    && !remake_question
                    && !debug_before_remake.has(Mask::MAKEFILES)
                {
                    self.terminal_named_dry_wait(
                        &read_files,
                        named_sources.as_ref(),
                        &pending,
                        unrelated_assumption_dry_handoff,
                    )
                } else {
                    None
                };
                let terminal_named_dry_reap = terminal_named_dry_wait.is_some();
                if live_modes.active()
                    && !terminal_named_dry_reap
                    && !active_primary
                    && !pending.last().is_some_and(|remake| remake.modes.active())
                {
                    // Pending sources resume with their own build mode. The
                    // last pending source supplies the open error.
                    self.remake_error_cursor = pending
                        .last()
                        .and_then(|remake| remake.context.open_error.clone());
                    self.set_makefile_modes(MakefileModes::default());
                }
                let primary_after_optional_pass = active_primary
                    && final_named_touch
                    && pass_reaps != initial_reaps
                    && self.pending(&read_files[0].0.name);
                let previous_progress = if primary_after_optional_pass {
                    // The optional pass already reaped an earlier include.
                    // Keep the trailing named mode until the primary finishes.
                    Some(if track_remake_effects {
                        self.filesystem.borrow().generation()
                    } else {
                        self.recipe_reaps
                    })
                } else if verbose_mode_remakes {
                    None
                } else {
                    // An optional source can finish an earlier pass while
                    // the primary is running. Resume after that pass.
                    first_effect_generation
                        .map(|initial| {
                            if mixed_named_overlap && verbose_load_trace {
                                pass_progress
                            } else {
                                initial
                            }
                        })
                        .or(Some(pass_reaps))
                };
                let question_or_failure = remake_question || failed || optional_remake_failed;
                // A mixed touch pass restores the unnamed root's mode
                // before refilling its load reservation after a reap.
                let wait = if let Some(wait) = terminal_named_dry_wait {
                    wait
                } else if optional_named_load
                    || (named_load_debug && verbose_load_trace)
                    || (active_primary && mixed_named_overlap && modes.touch)
                    || (mixed_named_overlap && final_named_dry)
                {
                    RemakeWait::LoadPass
                } else if optional_dependency_load_debug {
                    RemakeWait::Recipe
                } else {
                    RemakeWait::Command
                };
                if !modes.active()
                    && let Some(pair) =
                        self.required_remake_worker_pair(&read_files, &pending, &visited)
                {
                    pending[0].required_worker_pair = Some(pair);
                }
                stopped = !self.wait_makefile_remakes(
                    &mut pending,
                    &visited,
                    previous_progress,
                    track_remake_effects,
                    wait,
                    question_or_failure,
                );
                if !stopped {
                    if let Some(leading) = deferred_trailing_layout {
                        self.publish_restored_trailing_worker_clock(leading, &visited, &pending);
                    }
                    for remake in &pending {
                        changed |= remake.trailing_prerequisite_changed;
                        let name = remake.file.name.as_slice();
                        let before = visited[name].0;
                        let result = self
                            .file_traversals
                            .outcome(self.aliases.resolve(name))
                            .copied()
                            .unwrap_or(remake.initial);
                        changed |= remake.required_worker_pair.as_deref().is_some_and(|pair| {
                            self.restored_required_remake_pair(name, result, pair)
                        });
                        if !changed
                            && let Some(trailing) = ordinary_trailing_optional
                            && !remake.duplicate
                            && std::ptr::eq(remake.file, trailing.required)
                            && !remake.modes.active()
                            && !MakefileModes::from_options(&self.options).active()
                            && self.settled_required_worker_refresh(result)
                            && self.parallel_jobs.is_empty()
                            && let Some(worker) = remake.trailing_optional_worker.as_deref()
                            && worker.no_required_tail_remains
                            && let Some(&(Some(before), _)) = visited.get(trailing.neighbor)
                            && ordinary_current_leading.is_none_or(|leading| {
                                self.settled_current_leading_optional(
                                    &leading.neighbor.name,
                                    &visited,
                                )
                            })
                        {
                            // Either child can supply a reread clock before
                            // the other child restores that source.
                            changed |= self
                                .restored_trailing_worker_outcome(worker, trailing.neighbor, before)
                                .is_some()
                                || self.restored_required_worker_clock(remake, trailing, &visited);
                        }
                        if !changed
                            && self.options.keep_going
                            && !remake.duplicate
                            && !remake.modes.active()
                            && !MakefileModes::from_options(&self.options).active()
                            && self.parallel_jobs.is_empty()
                            && result.ran_refresh()
                            && result.failed
                            && !result.question_outdated
                            && matches!(
                                result.remake_failure,
                                RemakeFailure::RequiredDeclaredTailRecipe
                            )
                        {
                            let trailing = ordinary_trailing_optional
                                .filter(|_| {
                                    ordinary_current_leading.is_none_or(|leading| {
                                        self.settled_current_leading_optional(
                                            &leading.neighbor.name,
                                            &visited,
                                        )
                                    })
                                })
                                .or_else(|| {
                                    trailing_optional_source
                                        .as_ref()
                                        .filter(|_| read_files.len() == 3)
                                        .or_else(|| {
                                            assumed_current_leading
                                                .filter(|leading| {
                                                    self.settled_current_leading_optional(
                                                        &leading.neighbor.name,
                                                        &visited,
                                                    )
                                                })
                                                .and_then(|leading| leading.trailing.as_ref())
                                        })
                                });
                            changed |= trailing.is_some_and(|trailing| {
                                std::ptr::eq(remake.file, trailing.required)
                                    && self.restored_failed_required_worker_clock(
                                        remake, trailing, &visited,
                                    )
                            });
                        }
                        let (result, retained_worker_restart) =
                            source_outcome(self, remake.file, result, Some((&visited, remake)));
                        if remake.duplicate && (remake.file.optional || !result.failed) {
                            continue;
                        }
                        changed |= retained_worker_restart;
                        ran_remake_recipe |= result.ran;
                        if result.question_outdated {
                            if overlap_primary_recipe && remake.file.optional {
                                // A load-delayed optional query finishes through
                                // the job queue. Retain the same required-goal
                                // diagnostic as an immediately settled query.
                                self.optional_remake_failures
                                    .entry(name.to_vec())
                                    .or_default();
                            }
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
                                    let refresh_timestamp = result.ran && !result.blocked;
                                    failed_remakes.insert(name, (before, refresh_timestamp));
                                    if terminal_named_dry_reap
                                        && self.options.keep_going
                                        && refresh_timestamp
                                        && !result.question_outdated
                                        && result.completion == CompletionTime::AssumeNew
                                    {
                                        // Settle the failed worker's logical timestamp
                                        // before reporting the required source failure.
                                        settled_failed_source_changes.insert(name);
                                    }
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
                                changed |= self.modified(name) != before
                                    || result.remake_group_peer_updated
                                    || (terminal_named_dry_reap
                                        && completed_in_other_mode
                                        && result.database_result == DatabaseResult::Simulated);
                            }
                        }
                    }
                }
            }
            if let Some(clock) = deferred_inline_optional_clock {
                changed |= if stopped {
                    clock.changed_on_visit
                } else {
                    self.settle_inline_optional_remake_clock(clock)
                };
            }
            // Only a continuing ordinary build may overlap an already
            // running sibling from a stopped optional remake.
            self.defer_stopped_remake_load = false;
            if stopped || changed || failed || self.shutdown.get() != Shutdown::Running {
                self.wait_all_jobs();
            }
            let settled_in_ordinary_mode = !MakefileModes::from_options(&self.options).active();
            self.set_makefile_modes(modes);
            self.options.always = always;
            self.output.set_debug_mask(debug_before_remake);
            self.remaking = false;
            self.parallel_makefile_remakes = false;
            self.serial_remake_peer_times.clear();
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
            let skipped_required_read = !skipped_errors.is_empty();
            let unsuccessful_remake = failed || optional_remake_failed;
            failed |= skipped_required_read;
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
                        && let Some(outcome) =
                            self.file_traversals.outcome(self.aliases.resolve(name))
                        && outcome.remake_failure.is_optional()
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
                    if settled_failed_source_changes.contains(name) {
                        // This existing source already has a settled timestamp.
                        // Later recipes do not replace that serialized observation.
                        changed = true;
                        *refresh_timestamp = false;
                    }
                    // Other failed sources still use their existing refresh policy.
                    if *refresh_timestamp {
                        let before = grouped_peer_times
                            .get(name)
                            .copied()
                            .filter(|_| {
                                self.file_traversals
                                    .outcome(name)
                                    .is_some_and(|outcome| outcome.remake_failure.is_optional())
                            })
                            .or(*before);
                        let after = self.modified(&makefile.name);
                        changed |= after.is_some() && after != before;
                        *refresh_timestamp = false;
                    }
                }
            }
            changed |= assumed_remake_changed && !assumed_remake_blocked && !unsuccessful_remake;
            if !changed
                && parallel_remakes
                // With no pending worker, a current terminal include retains its named mode.
                && (settled_in_ordinary_mode || terminal_optional_assumed_dry_handoff)
                && restarts == 0
                && self.assumptions.is_some()
                && self.parallel_jobs.is_empty()
                && !unsuccessful_remake
                && !skipped_required_read
                && !remake_question
            {
                // Only an explicitly requested makefile can use the goal modes.
                let ordinary_visits = !modes.active()
                    || read_files.iter().all(|(source, skip)| {
                        *skip
                            || !self.is_makefile_goal(
                                rmake_core::filenames::without_dot_prefix(&source.name),
                                named_sources.as_ref(),
                            )
                    });
                changed = (ordinary_visits || assumed_double_colon_dry_run.is_some())
                    && self.settled_parallel_assumed_source_has_new_clock(
                        &read_files,
                        &visited,
                        named_sources.as_ref(),
                        ordinary_visits,
                    );
            }
            changed |= grouped_changes
                .as_ref()
                .is_some_and(SerialGroupedRemakeChanges::changed);
            // Keep-going can leave a required failure and a completed
            // simulated or touched source in the same settled pass.
            let required_remake_failed =
                failed && !skipped_required_read && self.options.keep_going;
            let use_failed_source_clocks =
                !remake_question && ((optional_remake_failed && !failed) || required_remake_failed);
            if !changed && use_failed_source_clocks {
                // The failed pass also compares successful source records.
                // Preserve the group's sample through later physical writes.
                changed = settled_named_peer_times.iter().any(|(name, time)| {
                    self.aliases.resolve(name) == *name
                        && self.file_traversals.outcome(name).is_some_and(|outcome| {
                            outcome.remake_group_peer_updated
                                && !outcome.failed
                                && !outcome.fatal
                                && !outcome.question_outdated
                                && match *time {
                                    NamedRemakePeerTime::File(time) => outcome.time == time,
                                    NamedRemakePeerTime::GroupedDoubleColon(time) => {
                                        self.settled_grouped_double_peer_time(name) == Some(time)
                                    }
                                }
                        })
                });
            }
            if !changed
                && (!parallel_remakes
                    || (assumed_double_colon_dry_run.is_some()
                        && settled_in_ordinary_mode
                        && self.parallel_jobs.is_empty()))
                && restarts == 0
                && self.assumptions.is_some()
                && (modes.dry_run || modes.question)
                && use_failed_source_clocks
            {
                let completion = if modes.touch {
                    if modes.dry_run {
                        DatabaseResult::SimulatedTouch
                    } else {
                        DatabaseResult::PhysicalTouch
                    }
                } else if modes.question {
                    DatabaseResult::SimulatedQuery
                } else {
                    DatabaseResult::Simulated
                };
                changed = self.failed_remake_has_updated_assumed_source(
                    &read_files,
                    &visited,
                    named_sources.as_ref(),
                    completion,
                    !required_remake_failed,
                );
            }
            if !changed
                && required_remake_failed
                && !remake_question
                && modes.dry_run
                && !modes.question
                && !modes.touch
                && self.assumptions.is_none()
                && read_files.iter().any(|(source, skip)| {
                    !*skip
                        && !source.optional
                        && !source.missing
                        && source.location.is_some()
                        && failed_remakes.contains_key(source.name.as_slice())
                        && self
                            .file_traversals
                            .outcome(self.aliases.resolve(&source.name))
                            .is_some_and(|outcome| {
                                outcome.failed && !outcome.remake_failure.is_optional()
                            })
                })
            {
                changed = self.settled_named_dry_remake_has_new_clock(
                    &read_files,
                    &visited,
                    named_sources.as_ref(),
                );
            }
            // A skipped required read suppresses an otherwise successful
            // restart. A failed remake still checks for physically updated peers.
            if changed && !remake_question && (!skipped_required_read || unsuccessful_remake) {
                self.wait_all_jobs();
                self.remove_intermediates();
                break 'makefiles true;
            }
            if failed {
                self.wait_all_jobs();
                self.remove_intermediates();
                return 2;
            }
            if !changed
                && !remake_question
                && !modes.active()
                && self.parallel_jobs.is_empty()
                && let Some(leading) = required_clock_layout
            {
                self.publish_restored_required_worker_clock(leading, &visited, &pending);
            }
            if ran_remake_recipe || self.recipe_reaps != initial_reaps {
                // Keep the cached outcomes and timestamps, but report ordinary
                // goal activity only for recipes started in that phase.
                self.file_traversals.clear_recipe_activity();
            }
            // A query verdict leaves any objects unloaded by the remake pass
            // in that state for the ordinary goals.
            if !remake_question {
                for module in crate::native::unloaded() {
                    let location = self.database.loaded_modules.get(&module).cloned();
                    if let Err(error) =
                        self.native_module_operation(&module, location.as_ref(), true)
                    {
                        self.expansion_error(location.as_ref(), &error);
                        return 2;
                    }
                }
            }
            false
        };
        if restart_pending {
            return self.restart_after_makefiles(restarts.wrapping_add(1));
        }
        // The ordinary goals decide the final query status. A queried source
        // still carries its cached verdict if a goal later depends on it.
        self.would_build = false;
        if let Err(error) = self.publish_phase_makeflags(false) {
            self.expansion_error(None, &error);
            return 2;
        }
        if restarts != 0 {
            self.publish_restarted_assumptions();
        }
        self.unlink_stdin();
        self.ordinary_include_open_times = true;
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
                Ok(mut goal) if !goal.is_empty() => {
                    // GNU first looks up the complete spelling. An interned
                    // filename can contain only blanks, or contain blanks
                    // without naming multiple default goals.
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
                            .with_shell(&|request| {
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
                        if goals.is_empty() {
                            self.fatal_system(rmake_core::messages::Message::NoTargets {
                                makefile_found: found_makefile,
                            });
                            return 2;
                        }
                        if goals.len() > 1 {
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
        self.database.file_records.begin_lookup_snapshot();
        for (index, goal) in goals.into_iter().enumerate() {
            let goal = if self.moved_archives.is_empty() {
                goal
            } else {
                self.database
                    .file_records
                    .moved_lookup_id_at_snapshot(&goal)
                    .and_then(|id| self.moved_archives.get(&id))
                    .map_or(goal, |owner| owner.lookup.clone())
            };
            self.graph_epoch = self.graph_epoch.wrapping_add(1);
            let name = self.aliases.resolve(&goal);
            let already_done = (self.file_traversals.has_outcome(name)
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
            // A previous failure settles the invocation verdict. Later serial
            // goals can still complete, and a prerequisite can observe their
            // new timestamp when it actually consumes the file.
            let outcome =
                if pending || self.defer_goal_timestamp_after_failure(&goal, outcome.completion) {
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
                    completed_report = self.file_traversals.outcome(name).copied();
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
                self.report_goal(
                    &goal,
                    started_command,
                    completed_report.unwrap_or(outcome),
                    already_done,
                );
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
            if self.complete_failed_dispatch() && self.graph_abort.is_some() {
                break;
            }
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
                        && let Some(outcome) = self
                            .file_traversals
                            .outcome(self.aliases.resolve(&goal.name))
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
                .file_traversals
                .outcome(self.aliases.resolve(&goal.name))
                .copied()
                .unwrap_or(goal.initial);
            let outcome = self.observe_updated(&goal.name, outcome);
            failed |= outcome.failed;
            fatal |= outcome.fatal;
            self.ordinary_queries.record(outcome.goal_verdict());
            self.report_goal(&goal.name, goal.started_command, outcome, goal.already_done);
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

    fn has_requested_goal_name(&self, name: &[u8]) -> bool {
        self.requested_goal_names.as_ref().map_or_else(
            || self.options.goals.iter().any(|goal| goal == name),
            |goals| goals.contains(name),
        )
    }

    fn index_named_remake_sources<'files>(
        &self,
        read_files: &'files [(Makefile, bool)],
    ) -> Option<HashSet<&'files [u8]>> {
        // A short list is cheap to scan, especially the single-goal case.
        if read_files.len() < 16 || self.options.goals.len() < 32 {
            return None;
        }
        // A file-name subset keeps each remake lookup independent of the
        // number of sources and of changes to runtime flags.
        Some(
            read_files
                .iter()
                .filter(|(_, skip)| !skip)
                .map(|(file, _)| rmake_core::filenames::without_dot_prefix(&file.name))
                .filter(|name| self.has_requested_goal_name(name))
                .collect(),
        )
    }

    fn is_makefile_goal(&self, name: &[u8], indexed: Option<&HashSet<&[u8]>>) -> bool {
        if let Some(indexed) = indexed {
            indexed.contains(name)
        } else {
            self.has_requested_goal_name(name)
        }
    }

    fn ordinary_mode_remakes_settle(
        &self,
        read_files: &[(Makefile, bool)],
        named_sources: Option<&HashSet<&[u8]>>,
        modes: MakefileModes,
        optional_named_load: bool,
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
        let has_optional_sources = read_files.iter().any(|(file, skip)| file.optional && !skip);
        if has_optional_sources && !self.optional_mode_sources_settle(read_files, named_sources) {
            return false;
        }
        let direct_required_dry_run = modes.dry_run
            && !modes.question
            && !modes.touch
            && !has_optional_sources
            && goals.len() == 1
            && self.options.keep_going
            && self.options.load_limit.is_none()
            && !self.options.check_symlink_times
            && self.no_remake_recipe_fallback();
        let named_double_colon_dry_run = direct_required_dry_run && self.assumptions.is_none();
        let named_assumed_dry_run = direct_required_dry_run
            && !self.options.always
            && self.options.old_files.is_empty()
            && self.options.new_files.len() == 1;
        let mut named = HashMap::new();
        let mut load_sources_without_fallback = None;
        read_files.iter().enumerate().all(|(index, (file, skip))| {
            let name = without_dot_prefix(&file.name);
            // Directory-qualified includes need a directly matched goal
            // before they can use the parallel named remake path.
            if has_subdirectory_goal
                && name.contains(&b'/')
                && !self.qualified_directory_named_include(file, name, named_sources)
            {
                return false;
            }
            *skip
                || goals.is_empty()
                || (!name.starts_with(b"-l")
                    && archive_parts(name).is_none()
                    && (!self.is_makefile_goal(name, named_sources) || {
                        let include_policy = file.location.as_ref().map(|_| file.optional);
                        let earlier_policy = named.insert(name, include_policy);
                        let compatible_read = earlier_policy.is_none_or(|earlier_policy| {
                            include_policy.is_some() && include_policy == earlier_policy
                        });
                        compatible_read
                            && (!file.optional
                                || self.options.load_limit.is_none()
                                || optional_named_load)
                            && (self.options.load_limit.is_none()
                                || *load_sources_without_fallback.get_or_insert_with(|| {
                                    let start =
                                        usize::from(self.unruled_primary_before_named_includes(
                                            read_files,
                                            named_sources,
                                        ));
                                    self.load_remakes_without_fallback(&read_files[start..])
                                }))
                            && (self.simple_named_remake(file, name, &read_files[..index], modes)
                                || (named_double_colon_dry_run
                                    && self.simple_named_double_colon_dry_run(file, name))
                                || (named_assumed_dry_run
                                    && earlier_policy.is_none()
                                    && self.simple_named_assumed_dry_run(file, name)
                                    && read_files[index + 1..]
                                        .iter()
                                        .find(|(_, skip)| !*skip)
                                        .is_some_and(|(source, _)| {
                                            !source.missing
                                                && !self.is_makefile_goal(
                                                    without_dot_prefix(&source.name),
                                                    named_sources,
                                                )
                                        })))
                    }))
        })
    }

    fn qualified_assumed_double_colon_dry_run(
        &self,
        read_files: &[(Makefile, bool)],
        modes: MakefileModes,
    ) -> Option<AssumedDoubleColonDryRemake> {
        use rmake_core::filenames::{archive_parts, without_dot_prefix};

        let [name] = self.options.goals.as_slice() else {
            return None;
        };
        let name = name.as_slice();
        if !modes.only_dry_run()
            || self.options.always
            || !self.options.no_rules
            || self.options.check_symlink_times
            || self.options.load_limit.is_some()
            || !self.options.include_directories.is_empty()
            || !self.options.old_files.is_empty()
            || self.options.new_files.len() != 1
            || self.database.has_vpaths()
            || !self.vpath_dirs.is_empty()
            || !self.gpath_dirs.is_empty()
            || self.database.rules.contains_key(b".DEFAULT".as_slice())
            || name.contains(&b'/')
            || name.starts_with(b"~")
            || name.starts_with(b"-l")
            || archive_parts(name).is_some()
            || self.database.loaded_modules.contains_key(name)
            || self
                .assumed_new_double_colon_entries(name)
                .is_none_or(|(_, count)| count != 1)
        {
            return None;
        }
        let mut sources = read_files
            .iter()
            .filter_map(|(file, skip)| (!skip).then_some(file));
        if sources.clone().any(|file| file.missing) {
            return None;
        }
        let mut later = sources.clone();
        let source = later.find(|file| without_dot_prefix(&file.name) == name)?;
        if source.location.is_none()
            || later
                .clone()
                .any(|file| without_dot_prefix(&file.name) == name)
        {
            return None;
        }
        let scope = if source.optional {
            self.qualified_optional_assumed_dry_handoff(read_files, source)?
        } else {
            if !self.options.keep_going
                || later.next().is_none()
                || sources.any(|file| file.optional)
            {
                return None;
            }
            AssumedDoubleColonDryRemake::Required
        };
        let [(_, implicit)] = self.database.database_implicit_rules() else {
            return None;
        };
        // The synthetic entry can check freshness without starting a child
        // or expanding a second-stage prerequisite.
        let simple_synthetic_entry = implicit.group.is_none()
            && implicit.secondary.is_none()
            && !implicit.terminal
            && matches!(implicit.dependencies.as_slice(), [dep]
                if !dep.order_only
                    && !dep.name.contains(&b'%')
                    && self.simple_remake_prerequisite(dep, &[]))
            && implicit.recipes.as_ref().is_some_and(|recipes| {
                recipes
                    .iter()
                    .all(|recipe| recipe.bytes.iter().all(u8::is_ascii_whitespace))
            })
            && self.database.rules.get(name).is_some_and(|rule| {
                matches!(rule.double_colon.as_deref(), Some([branch])
                    if branch.group.is_none()
                        && branch.secondary.is_none()
                        && matches!(branch.dependencies.as_slice(), [dep]
                            if dep.order_only && self.simple_remake_prerequisite(dep, &[]))
                        && matches!(branch.recipes.as_deref(), Some([recipe])
                            if !recipe_is_recursive(recipe)))
            })
            && !self.database.implicit_candidates(name).is_empty();
        simple_synthetic_entry.then_some(scope)
    }

    fn qualified_optional_assumed_dry_handoff(
        &self,
        read_files: &[(Makefile, bool)],
        source: &Makefile,
    ) -> Option<AssumedDoubleColonDryRemake> {
        use rmake_core::filenames::without_dot_prefix;

        let [
            (primary, false),
            (predecessor, false),
            (named, false),
            remaining @ ..,
        ] = read_files
        else {
            return None;
        };
        let scope = match remaining {
            [] => AssumedDoubleColonDryRemake::TerminalOptionalHandoff,
            [(tail, false)] => {
                let name = without_dot_prefix(&tail.name);
                if tail.optional
                    || tail.location.is_none()
                    || name.contains(&b'/')
                    || self.database.loaded_modules.contains_key(name)
                    || !self.database.rules.get(name).is_some_and(|rule| {
                        rule.double_colon.is_none()
                            && rule.group.is_none()
                            && rule.secondary.is_none()
                            && rule.dependencies.is_empty()
                            && rule.recipes.as_ref().is_some_and(|recipes| {
                                recipes
                                    .iter()
                                    .all(|recipe| recipe.bytes.iter().all(u8::is_ascii_whitespace))
                            })
                    })
                {
                    return None;
                }
                AssumedDoubleColonDryRemake::OptionalHandoff
            }
            _ => return None,
        };
        let primary_name = without_dot_prefix(&primary.name);
        let predecessor_name = without_dot_prefix(&predecessor.name);
        (std::ptr::eq(named, source)
            && !primary.optional
            && primary.location.is_none()
            && !predecessor.optional
            && predecessor.location.is_some()
            && [primary_name, predecessor_name].iter().all(|name| {
                !name.contains(&b'/') && !self.database.loaded_modules.contains_key(*name)
            })
            && !self.database.rules.contains_key(b".ONESHELL".as_slice())
            && !self.database.rules.contains_key(primary_name)
            && self.database.implicit_candidates(primary_name).is_empty()
            && self.simple_dry_remake_worker(predecessor_name, None))
        .then_some(scope)
    }

    fn simple_dry_remake_worker(&self, target: &[u8], assumed: Option<&[u8]>) -> bool {
        self.database.rules.get(target).is_some_and(|rule| {
            rule.double_colon.is_none()
                && rule.group.is_none()
                && rule.secondary.is_none()
                && matches!(rule.dependencies.as_slice(), [dep]
                    if !dep.order_only
                        && assumed != Some(dep.name.as_slice())
                        && self.simple_remake_prerequisite(dep, &[]))
                && rule.recipes.as_ref().is_some_and(|recipes| {
                    (1..=2).contains(&recipes.len())
                        && recipes.iter().all(|recipe| {
                            let (command, _) = crate::oneshell::prefixes(&recipe.bytes);
                            !recipe_is_recursive(recipe)
                                && !command.contains(&b'$')
                                && !command.contains(&b'\n')
                                && command.iter().any(|byte| !byte.is_ascii_whitespace())
                        })
                })
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
                                && rule.dependencies.iter().all(|dep| dep.name != b".WAIT")
                                && self.simple_named_remake(
                                    file,
                                    name,
                                    &[],
                                    MakefileModes::default(),
                                )))
                })
        })
    }

    fn qualified_directory_named_include(
        &self,
        file: &Makefile,
        name: &[u8],
        named_sources: Option<&HashSet<&[u8]>>,
    ) -> bool {
        self.options.load_limit.is_some()
            && file.location.is_some()
            && !self.options.check_symlink_times
            && self.options.include_directories.is_empty()
            && !name.starts_with(b"~")
            && !name.starts_with(b"-l")
            && rmake_core::filenames::archive_parts(name).is_none()
            && name.contains(&b'/')
            && name
                .split(|&byte| byte == b'/')
                .all(|component| !matches!(component, b"" | b"." | b".."))
            && self.is_makefile_goal(name, named_sources)
            && self.simple_named_remake(file, name, &[], MakefileModes::default())
    }

    fn qualified_load_debug_sources(
        &self,
        read_files: &[(Makefile, bool)],
        optional_dependencies_only: bool,
        max_recipe_lines: usize,
        named_load_sources: bool,
    ) -> bool {
        use rmake_core::filenames::{archive_parts, without_dot_prefix};

        let sources_without_fallback = if named_load_sources {
            self.load_remakes_without_fallback(read_files)
        } else {
            self.no_remake_recipe_fallback()
        };
        if !sources_without_fallback {
            return false;
        }
        let plain_name = |name: &[u8]| {
            !name.contains(&b'/')
                && !name.starts_with(b".")
                && !name.starts_with(b"~")
                && !name.starts_with(b"-l")
                && archive_parts(name).is_none()
        };
        let plain_rule = |rule: &Rule| {
            rule.secondary.is_none() && rule.group.is_none() && rule.double_colon.is_none()
        };
        let nonrecursive =
            |recipes: &[Recipe]| recipes.iter().all(|recipe| !recipe_is_recursive(recipe));
        let mut seen = HashSet::new();
        let mut has_dependencies = false;
        for (file, skip) in read_files {
            if *skip {
                continue;
            }
            let name = without_dot_prefix(&file.name);
            let supported_name = plain_name(name)
                || (named_load_sources && self.qualified_directory_named_include(file, name, None));
            if !supported_name
                || !seen.insert(name)
                || self.database.loaded_modules.contains_key(name)
            {
                return false;
            }
            let Some(rule) = self.database.rules.get(name) else {
                continue;
            };
            if !plain_rule(rule)
                || rule.recipes.as_ref().is_some_and(|recipes| {
                    recipes.len() > max_recipe_lines || !nonrecursive(recipes)
                })
            {
                return false;
            }
            if rule.dependencies.is_empty() {
                continue;
            }
            if (optional_dependencies_only && !file.optional)
                || !rule.dependencies.iter().all(|dep| {
                    !dep.hidden
                        && plain_name(&dep.name)
                        && self.database.rules.get(&dep.name).is_some_and(|child| {
                            child.dependencies.is_empty()
                                && plain_rule(child)
                                && child.recipes.as_ref().is_some_and(|recipes| {
                                    (optional_dependencies_only || recipes.len() <= 1)
                                        && nonrecursive(recipes)
                                })
                        })
                })
            {
                return false;
            }
            has_dependencies = true;
        }
        !optional_dependencies_only || has_dependencies
    }

    fn simple_named_remake(
        &self,
        file: &Makefile,
        name: &[u8],
        earlier_sources: &[(Makefile, bool)],
        modes: MakefileModes,
    ) -> bool {
        let primary = file.location.is_none();
        if (primary && (file.optional || file.missing))
            || self.database.loaded_modules.contains_key(name)
            || self.assumptions.is_some()
        {
            return false;
        }
        let limited_load = self.options.load_limit.is_some();
        if limited_load && (primary || self.special(b".PHONY", name, false)) {
            return false;
        }
        let no_default_recipe = !self.database.rules.contains_key(b".DEFAULT".as_slice());
        let earlier_sources = if primary { earlier_sources } else { &[] };
        self.database
            .rules
            .get(name)
            .map_or(primary && no_default_recipe, |rule| {
                if limited_load {
                    // A direct named include may carry its mode across a
                    // load-delayed sibling without expanding a dependency.
                    return rule.dependencies.is_empty()
                        && rule.secondary.is_none()
                        && rule.group.is_none()
                        && rule.double_colon.is_none()
                        && rule.recipes.as_ref().is_some_and(|recipes| {
                            matches!(recipes.as_slice(), [recipe] if !recipe_is_recursive(recipe))
                        });
                }
                if rule.secondary.is_some() && !no_default_recipe {
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
                        && !modes.touch
                        && (modes.dry_run ^ modes.question)
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
                                && (!self.makefile_names.contains(peer)
                                    || (modes.question && rule.secondary.is_none()))
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
                                    || (primary
                                        && rule.group.is_none()
                                        && !dep.hidden
                                        && dep.name == b".WAIT")
                                    || self.simple_remake_prerequisite(dep, earlier_sources)
                            })))
                    && rule.recipes.is_some()
            })
    }

    fn qualified_unrelated_assumption_dry_handoff(&self, read_files: &[(Makefile, bool)]) -> bool {
        use rmake_core::filenames::{archive_parts, without_dot_prefix};

        let ([assumed], [goal], [(primary, false), (worker, false), (terminal, false)]) = (
            self.options.new_files.as_slice(),
            self.options.goals.as_slice(),
            read_files,
        ) else {
            return false;
        };
        let assumed = without_dot_prefix(assumed.as_os_str().as_bytes());
        let primary_name = without_dot_prefix(&primary.name);
        let worker_name = without_dot_prefix(&worker.name);
        let terminal_name = without_dot_prefix(&terminal.name);
        if goal.as_slice() != terminal_name
            || !self.options.old_files.is_empty()
            || self.options.always
            || self.options.check_symlink_times
            || self.options.load_limit.is_some()
            || !self.options.include_directories.is_empty()
            || self.database.has_vpaths()
            || !self.vpath_dirs.is_empty()
            || !self.gpath_dirs.is_empty()
            || !self.no_remake_recipe_fallback()
            || self.database.rules.contains_key(b".ONESHELL".as_slice())
            || primary.location.is_some()
            || worker.location.is_none()
            || terminal.location.is_none()
            || [primary, worker, terminal]
                .iter()
                .any(|file| file.optional || file.missing)
            || [assumed, primary_name, worker_name, terminal_name]
                .iter()
                .any(|name| {
                    name.is_empty()
                        || name.contains(&b'/')
                        || name.starts_with(b"~")
                        || name.starts_with(b"-l")
                        || archive_parts(name).is_some()
                        || self.database.loaded_modules.contains_key(*name)
                })
            || self.database.rules.contains_key(assumed)
            || self.database.rules.contains_key(primary_name)
            || [primary_name, worker_name, terminal_name].contains(&assumed)
            || [primary_name, worker_name, terminal_name]
                .iter()
                .any(|name| self.special(b".PHONY", name, false))
        {
            return false;
        }
        self.database.rules.get(terminal_name).is_some_and(|rule| {
            rule.double_colon.is_none()
                && rule.group.is_none()
                && rule.secondary.is_none()
                && rule.dependencies.is_empty()
                && rule.recipes.is_some()
        }) && self.simple_dry_remake_worker(worker_name, Some(assumed))
    }

    fn terminal_named_dry_wait(
        &self,
        read_files: &[(Makefile, bool)],
        named_sources: Option<&HashSet<&[u8]>>,
        pending: &[PendingMakefileRemake<'_>],
        unrelated_assumption: bool,
    ) -> Option<RemakeWait> {
        use rmake_core::filenames::without_dot_prefix;

        if (self.assumptions.is_some() && !unrelated_assumption)
            || self.options.load_limit.is_some()
            || self.options.goals.len() != 1
        {
            return None;
        }
        let (worker, wait) = self.sole_required_dry_worker(pending)?;
        // A current terminal named source supplies the dry-run cursor. A
        // forced continuation keeps that cursor until its recipe completes.
        read_files
            .iter()
            .rfind(|(_, skip)| !*skip)
            .is_some_and(|(source, _)| {
                let name = without_dot_prefix(&source.name);
                let target = self.aliases.resolve(name);
                let eligible_source = !source.optional
                    || (self.assumptions.is_none()
                        && wait == RemakeWait::Command
                        && self.no_remake_recipe_fallback()
                        && matches!(read_files, [(primary, false), (predecessor, false), (_, false)]
                            if !primary.optional
                                && !primary.missing
                                && primary.location.is_none()
                                && !self.database.rules.contains_key(without_dot_prefix(&primary.name))
                                && predecessor.name.as_slice() == worker
                                && self.simple_dry_remake_worker(without_dot_prefix(worker), None)));
                eligible_source
                    && !source.missing
                    && source.location.is_some()
                    && self.is_makefile_goal(name, named_sources)
                    && self.database.rules.get(target).is_some_and(|rule| {
                        rule.double_colon.is_none()
                            && rule.group.is_none()
                            && rule.dependencies.is_empty()
                            && rule.secondary.is_none()
                    })
                    && self.file_traversals.outcome(target).is_some_and(|outcome| {
                        outcome.terminal_verdict().is_none()
                            && !outcome.fatal
                            && !outcome.ran
                            && outcome.time.is_some()
                            && outcome.completion == CompletionTime::Unchanged
                    })
            })
            .then_some(wait)
    }

    fn sole_required_dry_worker<'files>(
        &self,
        pending: &[PendingMakefileRemake<'files>],
    ) -> Option<(&'files [u8], RemakeWait)> {
        let [worker] = pending else { return None };
        if self.parallel_jobs.len() != 1
            || worker.modes.active()
            || worker.duplicate
            || !worker.active
            || worker.file.optional
            || worker.file.missing
            || worker.file.location.is_none()
        {
            return None;
        }
        let target = self.aliases.resolve(&worker.file.name);
        let job = self.parallel_jobs.primary.get(target)?;
        if self.running_groups.contains_key(target)
            || job.child.is_none()
            || job.before.is_none()
            || job.phony
            || job.recursive
            || job.continuation.branch.is_some()
            || job.continuation.one_shell
            || job.continuation.expanded.len() > 1
        {
            return None;
        }
        let wait = if let Some((index, bytes, recipe)) = job.continuation.expanded.front() {
            let (command, prefixes) = crate::oneshell::prefixes(bytes);
            if command.contains(&b'\n') || command.iter().all(u8::is_ascii_whitespace) {
                return None;
            }
            if prefixes.force && !has_direct_make_reference(&recipe.bytes) {
                RemakeWait::TerminalDryRecipe
            } else if !prefixes.force && !job.continuation.flow.recursive(*index) {
                RemakeWait::Command
            } else {
                return None;
            }
        } else {
            RemakeWait::Command
        };
        Some((worker.file.name.as_slice(), wait))
    }

    fn simple_named_assumed_dry_run(&self, file: &Makefile, name: &[u8]) -> bool {
        // An existing leaf with an active new time stays current. The caller
        // requires a later ordinary visit to restore the mode before reaping.
        file.location.is_some()
            && !file.missing
            && !name.contains(&b'/')
            && self.assumed_time(name).is_some()
            && !self.special(b".PHONY", name, false)
            && !self.database.loaded_modules.contains_key(name)
            && self.database.rules.get(name).is_some_and(|rule| {
                rule.dependencies.is_empty()
                    && rule.secondary.is_none()
                    && rule.group.is_none()
                    && rule.double_colon.is_none()
                    && matches!(rule.recipes.as_deref(),
                        Some([recipe]) if !recipe_is_recursive(recipe))
            })
    }

    fn simple_named_double_colon_dry_run(&self, file: &Makefile, name: &[u8]) -> bool {
        // One explicit declaration can be simulated while an earlier
        // required include's recipe is running.
        file.location.is_some()
            && !file.missing
            && !self.special(b".PHONY", name, false)
            && !self.database.loaded_modules.contains_key(name)
            && self
                .database
                .rules
                .get(name)
                .and_then(|rule| rule.double_colon.as_deref())
                .is_some_and(|branches| {
                    matches!(branches, [branch]
                        if branch.group.is_none()
                            && branch.secondary.is_none()
                            && matches!(branch.dependencies.as_slice(),
                                [dep] if !dep.order_only
                                    && self.simple_remake_prerequisite(dep, &[]))
                            && matches!(branch.recipes.as_deref(),
                                Some([recipe]) if !recipe_is_recursive(recipe)))
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

    fn unruled_primary_before_named_includes(
        &self,
        read_files: &[(Makefile, bool)],
        named_sources: Option<&HashSet<&[u8]>>,
    ) -> bool {
        use rmake_core::filenames::without_dot_prefix;

        let Some(((primary, false), includes)) = read_files.split_first() else {
            return false;
        };
        let name = without_dot_prefix(&primary.name);
        !self.no_remake_recipe_fallback()
            && !self.options.check_symlink_times
            && self.assumptions.is_none()
            && primary.location.is_none()
            && !primary.optional
            && !primary.missing
            && !self.is_makefile_goal(name, named_sources)
            && !self.database.rules.contains_key(name)
            && !self.database.rules.contains_key(b".DEFAULT".as_slice())
            && !self.database.loaded_modules.contains_key(name)
            && includes.iter().any(|(file, skip)| {
                !*skip && self.is_makefile_goal(without_dot_prefix(&file.name), named_sources)
            })
            && self.load_remakes_without_fallback(includes)
            && self
                .database
                .database_implicit_rules()
                .iter()
                .all(|(_, rule)| {
                    rule.recipes.as_ref().is_none_or(|recipes| {
                        rule.secondary.is_none() && recipes.iter().all(|recipe| recipe.builtin)
                    })
                })
    }

    fn load_remakes_without_fallback(&self, read_files: &[(Makefile, bool)]) -> bool {
        self.no_remake_recipe_fallback()
            || read_files.iter().all(|(file, skip)| {
                // An explicit recipe on every source keeps the built-in
                // implicit rules out of this remake chain.
                let name = rmake_core::filenames::without_dot_prefix(&file.name);
                *skip
                    || (!self.database.loaded_modules.contains_key(name)
                        && self.database.rules.get(name).is_some_and(|rule| {
                            rule.dependencies.is_empty()
                                && rule.secondary.is_none()
                                && rule.group.is_none()
                                && rule.double_colon.is_none()
                                && rule.recipes.as_ref().is_some_and(|recipes| {
                                    recipes.len() <= 2
                                        && recipes.iter().all(|recipe| !recipe_is_recursive(recipe))
                                })
                        }))
            })
    }

    fn optional_mode_sources_settle(
        &self,
        read_files: &[(Makefile, bool)],
        named_sources: Option<&HashSet<&[u8]>>,
    ) -> bool {
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
                || ((!name.contains(&b'/')
                    || self.qualified_directory_named_include(file, name, named_sources))
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
                                        rule.group.is_none()
                                            && rule.double_colon.is_none()
                                            && rule.recipes.is_some()
                                            && ((rule.dependencies.is_empty()
                                                && rule.secondary.is_none())
                                                || (file.optional
                                                    && rule.secondary.is_some()
                                                    && self.is_makefile_goal(name, named_sources)
                                                    && self.simple_named_remake(
                                                        file,
                                                        name,
                                                        &[],
                                                        MakefileModes::default(),
                                                    )))
                                    })
                            })
                    }
                    && (!file.optional
                        || self
                            .database
                            .rules
                            .get(name)
                            .map_or(no_recipe_fallback, |rule| {
                                (rule.secondary.is_none()
                                    || (rule.group.is_none()
                                        && self.options.load_limit.is_none()
                                        && rule.recipes.is_some()
                                        && self.is_makefile_goal(name, named_sources)))
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

    /// A job completion or a make-function file write advances the goal diagnostics.
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

    fn debug_was_considered(&self, target: &[u8]) -> bool {
        self.debug_considered
            .as_ref()
            .is_some_and(|files| files.contains(target))
            // A later MAKEFLAGS expansion can enable verbose output after
            // the file was considered in this same ordinary goal pass.
            || (!self.remaking
                && self.output.debug_mask().has(crate::debug::Mask::VERBOSE)
                && self
                    .file_traversals
                    .visit(target)
                    .and_then(|visit| visit.quiet_consideration)
                    .is_some_and(|pass| pass.get() == self.debug_pass.wrapping_add(1)))
    }

    fn debug_cached_prerequisite_visit(&mut self, target: &[u8], depth: usize, outcome: Outcome) {
        self.debug_cached_prerequisite_visit_with_question(target, depth, outcome, false);
    }

    fn debug_cached_prerequisite_visit_with_question(
        &mut self,
        target: &[u8],
        depth: usize,
        outcome: Outcome,
        include_question: bool,
    ) {
        use crate::debug::{Event, Mask, emit};

        if !self.output.debug_mask().has(Mask::VERBOSE)
            || outcome.fatal
            || (outcome.question_outdated && !include_question)
        {
            return;
        }
        let name = self.aliases.resolve(target);
        if self.debug_was_considered(name) {
            emit(&self.output, Event::Pruning(name, depth));
            return;
        }
        self.debug_considered
            .get_or_insert_default()
            .insert(name.to_vec());
        emit(&self.output, Event::Considering(name, depth));
        emit(
            &self.output,
            if self.pending(name) {
                Event::StillUpdating(name, depth)
            } else if outcome.failed || outcome.question_outdated {
                Event::RecentlyFailed(name, depth)
            } else {
                Event::Already(name, depth)
            },
        );
    }

    fn parallel_remake_debug_declarations(&self, name: &[u8]) -> bool {
        use crate::debug::Mask;

        let debug = self.output.debug_mask();
        self.parallel_makefile_remakes
            && debug.has(Mask::VERBOSE)
            && debug.has(Mask::MAKEFILES)
            && !MakefileModes::from_options(&self.options).active()
            && self.assumptions.is_none()
            && self.makefile_names.contains(name)
            && self.aliases.resolve(name) == name
            && !self.resolved_names.contains_key(name)
            && self
                .database
                .rules
                .get(name)
                .and_then(|rule| rule.double_colon.as_ref())
                .is_some_and(|branches| {
                    branches.len() > 1
                        && branches.iter().all(|branch| {
                            branch.recipes.is_some()
                                && branch.grouped_targets().is_none()
                                && branch.implicit_pattern.is_none()
                        })
                })
    }

    fn debug_double_colon_goal_prunes(&self, goal: &[u8], outcome: Outcome) {
        use crate::debug::{Event, Mask, emit};

        if !self.output.debug_mask().has(Mask::VERBOSE)
            || self.graph_abort.is_some()
            || self.shutdown.get() != Shutdown::Running
            || outcome.fatal
            || (!self.options.keep_going && outcome.failed)
            || outcome.replayed_double_colon_goal_failure
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
            let mut repeats = count.saturating_sub(1);
            if outcome.question_outdated && !self.options.keep_going {
                if !self.options.question
                    || self.remaking
                    || self.active_makefile_remake.is_some()
                    || self.options.goals.len() > 1
                    || self.ordinary_queries.first_verdict.is_some()
                    || self.forced_query_stop
                    || self.pending(name)
                {
                    return;
                }
                // Earlier declarations revisit the chain head before the
                // declaration with the first query verdict stops the goal.
                let mut key = (name.to_vec(), 0);
                let Some(ordinal) = (0..branches.len()).find(|ordinal| {
                    key.1 = *ordinal;
                    self.database_branch_outcomes
                        .get(&key)
                        .is_some_and(|branch| branch.question_outdated)
                }) else {
                    return;
                };
                repeats = ordinal;
            } else if self.remaking && self.optional_remake() && outcome.remake_group_peer_updated {
                // An optional source advances after its first declaration
                // already updated as another group's peer. Earlier entries
                // can still revisit and prune the fresh chain head.
                let mut key = (name.to_vec(), 0);
                for ordinal in 0..branches.len() {
                    key.1 = ordinal;
                    if self
                        .database_branch_outcomes
                        .get(&key)
                        .is_some_and(|branch| {
                            branch.database_result == DatabaseResult::ObservedGroup
                        })
                    {
                        repeats = repeats.min(ordinal);
                        break;
                    }
                }
            }
            for _ in 0..repeats {
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
        let Some(outcome) = self.file_traversals.outcome(name).copied() else {
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

    fn report_goal(&self, goal: &[u8], started_command: bool, outcome: Outcome, cached: bool) {
        if outcome.fatal || (outcome.failed && cached) {
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
            let detached_head = self.database.has_detached_file_head(identity);
            let merged_report = self.merged_goal_report(goal);
            if matches!(
                merged_report,
                Some(MergedGoalReport::UnattemptedDestination)
            ) || (detached_head && goal == identity)
            {
                // The destination kept an unattempted status. The queued
                // source head alone has a successful update to report.
                return;
            }
            let rule = self.database.rules.get(identity);
            // A live query can leave a verdict after makefile remaking restores
            // the invocation mode. A double-colon head can still be current
            // while a later declaration returns a query verdict.
            if outcome.question_outdated && rule.is_none_or(|rule| rule.double_colon.is_none()) {
                return;
            }
            let has_recipe = rule.is_some_and(|rule| {
                rule.recipes.is_some()
                    || rule
                        .double_colon
                        .as_ref()
                        .is_some_and(|branches| branches.iter().any(|rule| rule.recipes.is_some()))
            }) && !self.special(b".PHONY", goal, false);
            let moved_archive = if self.moved_archives.is_empty() {
                None
            } else {
                self.database
                    .file_records
                    .id(identity)
                    .and_then(|id| self.moved_archives.get(&id))
            };
            let completed_source = match merged_report {
                Some(
                    MergedGoalReport::CompletedSource(name)
                    | MergedGoalReport::CompletedVisitedSource(name),
                ) => Some(name.as_slice()),
                _ => None,
            };
            let reported_goal = if let Some(name) = completed_source {
                name
            } else if let Some(owner) = moved_archive {
                owner.original.as_slice()
            } else {
                self.resolved_names
                    .get(identity)
                    .map_or(identity, Vec::as_slice)
            };
            // Rehashing into a file already entered by a declaration or CLI
            // goal merges GNU's goal record; the old record has no summary.
            if reported_goal != goal
                && moved_archive.is_none()
                && completed_source.is_none()
                && !(detached_head && self.aliases.retains_goal_status(goal))
                && (if self.aliases.contains(goal) {
                    self.aliases.merged(goal)
                } else {
                    self.database.explicit_names.contains(reported_goal)
                        || self.has_requested_goal_name(reported_goal)
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
            if let Some(goals) = &self.requested_goal_names {
                goals.contains(target) || self.requested_goal_aliases.contains(target)
            } else if self.aliases.is_empty() {
                self.has_requested_goal_name(target)
            } else {
                self.options
                    .goals
                    .iter()
                    .any(|goal| self.aliases.resolve(goal) == target)
            }
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
                let detail = rmake_linux::filesystem::source_error_detail(&error);
                self.stderr(&crate::messages::system(Message::OsError {
                    action: b"\nunlink: ",
                    path: &target,
                    detail: &detail,
                }));
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
                && !self.file_traversals.has_outcome(target)
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
        if let Some(outcome) = self.file_traversals.outcome(target) {
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
            || self.file_traversals.has_outcome(target)
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
        let records = &self.database.file_records;
        self.file_traversals.remember_visit(target, || FileVisit {
            parent: Some(OwnedScopeParent::new(parent, records.id(parent.target))),
            quiet_consideration: None,
        });
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

    fn settle_optional_remake_pass(
        &mut self,
        previous_progress: u64,
        defer_load_refill: bool,
    ) -> (u64, u64) {
        let poll = |engine: &mut Self| {
            if defer_load_refill {
                engine.observe_interrupt();
                engine.poll_child_events(false);
            } else {
                engine.poll_jobs();
            }
        };
        let can_reap = |engine: &Self| {
            engine
                .parallel_jobs
                .values()
                .any(|job| !defer_load_refill || !job.waiting_load)
        };
        poll(self);
        // Directory generation counts make-function effects and all
        // completed children, including a synchronous shell expansion.
        if self.filesystem.borrow().generation() == previous_progress && can_reap(self) {
            let previous_jobs = self.parallel_jobs.len();
            while self.parallel_jobs.len() >= previous_jobs
                && self.shutdown.get() == Shutdown::Running
                && self.graph_abort.is_none()
                && can_reap(self)
            {
                self.reap_job();
            }
            poll(self);
        }
        self.advance_debug_pass();
        (self.recipe_reaps, self.filesystem.borrow().generation())
    }

    fn wait_makefile_remakes(
        &mut self,
        requests: &mut [PendingMakefileRemake<'_>],
        visited: &MakefileVisits<'_>,
        mut previous_progress: Option<u64>,
        track_effects: bool,
        wait_mode: RemakeWait,
        mut question_or_failure: bool,
    ) -> bool {
        let wait_for_completed_job = previous_progress.is_none();
        let start_load_at_pass = wait_mode == RemakeWait::LoadPass;
        let progress = |engine: &Self| {
            if track_effects {
                engine.filesystem.borrow().generation()
            } else {
                engine.recipe_reaps
            }
        };
        let mut incomplete_debug_pass = false;
        'passes: while requests.iter().any(|request| request.active) {
            if self.shutdown.get() != Shutdown::Running {
                return false;
            }
            // Reap against the previous pass's cursor. A pass with no newly
            // completed commands waits before changing which root is current.
            let previous_jobs = self.parallel_jobs.len();
            let previous_load_waiters = self.load_waiting.len();
            let wait = Some(progress(self)) == previous_progress;
            if start_load_at_pass {
                // These verbose named remakes refill the load queue at the
                // pass boundary, before collecting exited children.
                self.observe_interrupt();
                self.start_load_waiting();
                self.poll_child_events(false);
            } else if !wait || wait_mode != RemakeWait::TerminalDryRecipe {
                // A blocking terminal reap follows the sole recipe through its
                // forced continuation before another root can select a mode.
                self.poll_jobs();
            }
            if wait
                && (if wait_for_completed_job {
                    self.parallel_jobs.len() >= previous_jobs
                } else {
                    Some(progress(self)) == previous_progress
                })
                && self.parallel_jobs.values().any(|job| !job.waiting_load)
            {
                // Revisit an optional named touch only after a blocking reap
                // completes a recipe. Revisiting between commands can apply
                // touch mode to the final command of an ordinary sibling.
                let finish_recipe = wait_for_completed_job
                    || wait_mode != RemakeWait::Command
                    || (self.options.load_limit.is_none()
                        && requests.iter().any(|request| {
                            request.active
                                && request.file.optional
                                && request.modes.touch
                                && !request.modes.dry_run
                                && !request.modes.question
                                && self
                                    .parallel_jobs
                                    .contains_key(self.aliases.resolve(&request.file.name))
                        }));
                self.reap_job();
                while finish_recipe
                    && self.parallel_jobs.len() >= previous_jobs
                    && self.parallel_jobs.values().any(|job| !job.waiting_load)
                    && self.shutdown.get() == Shutdown::Running
                    && self.graph_abort.is_none()
                    && !self.forced_query_stop
                {
                    self.reap_job();
                }
                if start_load_at_pass {
                    self.observe_interrupt();
                    self.poll_child_events(false);
                } else {
                    self.poll_jobs();
                }
            }
            if self.forced_query_stop || self.shutdown.get() != Shutdown::Running {
                return false;
            }
            previous_progress = Some(progress(self));
            if std::mem::take(&mut incomplete_debug_pass) {
                // An optional root can end a pass before the considered set
                // advances, even if a child finishes before the next visit.
                self.debug_pass = self.debug_progress();
            } else {
                self.advance_debug_pass();
                if start_load_at_pass && self.load_waiting.len() < previous_load_waiters {
                    // A simulated load-delayed command can complete without a
                    // child reap. Its launch still starts a new considered pass.
                    if let Some(considered) = &mut self.debug_considered {
                        considered.clear();
                    }
                }
            }
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
                self.set_makefile_modes(request.modes);
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
                if let Some(worker) = request.trailing_optional_worker.as_deref()
                    && worker.only_synthetic_tail_remains
                    && needs_visit
                    && !request.active
                    && self.options.keep_going
                    && !MakefileModes::from_options(&self.options).active()
                    && request.initial.ran_refresh()
                    && request.initial.failed
                    && !request.initial.question_outdated
                    && match request.initial.remake_failure {
                        RemakeFailure::RequiredSyntheticInput => {
                            self.settled_required_worker_clock(worker).is_some()
                        }
                        RemakeFailure::RequiredDeclaredTailRecipe => self
                            .failed_declared_worker_reap(worker)
                            .is_some_and(|(ordinal, clock)| {
                                self.no_remake_recipe_fallback()
                                    && !self
                                        .database_branch_outcomes
                                        .contains_key(&(name.to_vec(), ordinal + 1))
                                    && Self::regular_remake_clock(name) == Some(clock)
                            }),
                        _ => false,
                    }
                    && self
                        .parallel_jobs
                        .get(name, worker.required_identity)
                        .is_none()
                    && self.parallel_jobs.len() == 1
                    && self.parallel_jobs.values().any(|optional| {
                        optional.identity == worker.optional_identity && optional.child.is_some()
                    })
                {
                    // The required branch has already reported its failure.
                    // Preserve that ordering for the actual optional reap.
                    worker.required_failure_settled.set(true);
                }
                let outcome = self
                    .file_traversals
                    .outcome(self.aliases.resolve(name))
                    .copied()
                    .unwrap_or(request.initial);
                if !request.active
                    && let Some(pair) = request.required_worker_pair.as_deref_mut()
                    && pair.first_reap.is_none()
                    && self.settled_required_worker_refresh(outcome)
                    && let Some(clock) = outcome.time
                    && clock != pair.first_before
                    && Self::regular_remake_clock(name) == Some(clock)
                    && self.parallel_jobs.get(name, pair.first_identity).is_none()
                    && self.required_remake_pair_has_expected_workers(pair)
                    && self
                        .parallel_jobs
                        .get(pair.second, pair.second_identity)
                        .is_some_and(|job| job.child.is_some())
                    && let Some(reap) =
                        self.required_remake_pair_first_reap(name, pair, outcome, clock)
                {
                    // This root settled while the same later child still ran.
                    pair.first_reap = Some(reap);
                    if self.options.keep_going {
                        pair.second_failure =
                            self.track_required_pair_failure(pair.second, pair.second_identity);
                        pair.third_failure = pair.third.as_ref().and_then(|third| {
                            self.track_required_pair_failure(third.name, third.identity)
                        });
                    }
                }
                if request.trace_synthetic_reap
                    && needs_visit
                    && !request.active
                    && self.graph_abort.is_none()
                    && !outcome.fatal
                    && !outcome.question_outdated
                    && (!outcome.failed || self.options.keep_going)
                {
                    // The completed child reopens both source entries for
                    // their cached checks, then the repeated head prunes.
                    self.debug_completed_double_colon_declarations(name, 0..2, 1);
                    crate::debug::emit(&self.output, crate::debug::Event::Pruning(name, 1));
                }
                if request.active
                    && let Some(child) = &request.trailing_prerequisite_child
                    && !self.pending(child.name)
                {
                    let time = self
                        .file_traversals
                        .outcome(child.name)
                        .filter(|result| {
                            self.parallel_jobs.get(child.name, child.identity).is_none()
                                && result.ran
                                && !result.failed
                                && !result.fatal
                                && !result.blocked
                                && !result.question_outdated
                                && result.completion == CompletionTime::Refresh
                        })
                        .and_then(|result| result.time);
                    match time {
                        Some(time) if self.qualify_synthetic_recipe_after_optional_read(name) => {
                            // Keep the optional source's completion clock
                            // through the later synthetic command.
                            request.trailing_prerequisite_changed = time != child.before;
                            request.trailing_prerequisite_child = None;
                        }
                        None => request.trailing_prerequisite_child = None,
                        Some(_) => {}
                    }
                }
                if !needs_visit {
                    if (wait_for_completed_job || start_load_at_pass)
                        && outcome.question_outdated
                        && !request.active
                    {
                        self.debug_consider(name, name, 1);
                        crate::debug::emit(
                            &self.output,
                            crate::debug::Event::RecentlyFailed(name, 1),
                        );
                    } else {
                        self.debug_cached_prerequisite_visit(name, 1, outcome);
                    }
                }
                question_or_failure |=
                    !request.active && (outcome.failed || outcome.question_outdated);
                // A named mode can give a successful child an assumed-new
                // timestamp. It ends the pass while the status is still open.
                if !request.active
                    && request.file.optional
                    && !outcome.failed
                    && !outcome.question_outdated
                    && match outcome.completion {
                        CompletionTime::AssumeNew => !question_or_failure,
                        CompletionTime::Refresh | CompletionTime::ArchiveUnknown => {
                            outcome.time != visited[name].0
                        }
                        _ => false,
                    }
                {
                    // An updated optional root ends this pass. Remaining roots
                    // are revisited after the next child event.
                    incomplete_debug_pass = true;
                    continue 'passes;
                }
            }
            if self.graph_epoch == pass_epoch
                && (self.parallel_jobs.is_empty()
                    || (self.defer_stopped_remake_load
                        && self.parallel_jobs.values().all(|job| job.waiting_load)
                        && (!start_load_at_pass || self.next_load_waiter().is_none())))
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
            self.stderr(&crate::messages::system(
                rmake_core::messages::Message::WaitForJobs,
            ));
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
        self.poll_child_events(true);
    }

    fn poll_child_events(&mut self, refill_load: bool) {
        loop {
            match self.child_events(false, None) {
                Ok(true) if refill_load => self.start_load_waiting(),
                Ok(true) => {}
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
        if self.failed_dispatch.is_some() {
            // Graph-only polls must leave the failed dispatch ahead of the
            // real worker, including when that worker has already exited.
            return Ok(block && self.complete_failed_dispatch());
        }
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
            return Some((target.to_vec(), identity, JobCompletion::Child(result)));
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
                use rmake_core::messages::Message;

                let body = if error.raw_os_error() == Some(0) {
                    crate::messages::system(Message::LoadLimitUnsupported)
                } else {
                    crate::messages::system(Message::OsError {
                        action: &crate::messages::system(Message::LoadLimitErrorPrefix),
                        path: b"getloadavg",
                        detail: &rmake_linux::filesystem::source_error_detail(error),
                    })
                };
                output.write_parts(true, &[program, b": ", &body, b"\n"]);
            } else if debug {
                crate::debug::load(event, limit, running);
            }
        })
    }

    fn start_load_waiting(&mut self) {
        while !self.load_waiting.is_empty() {
            // Dispatch can settle a launch failure synchronously. Recheck the
            // graph before giving its next reserved recipe a chance to run.
            if self.graph_abort.is_some()
                || self.forced_query_stop
                || self.shutdown.get() != Shutdown::Running
            {
                while let Some((target, identity)) = self.load_waiting.pop() {
                    let job = self
                        .parallel_jobs
                        .remove(&target, identity)
                        .expect("reserved load waiter");
                    let mut outcome = self
                        .file_traversals
                        .outcome(&target)
                        .copied()
                        .unwrap_or_default();
                    outcome.failed = true;
                    outcome.fatal |= self.graph_abort == Some(Status::Fatal);
                    outcome.ran = false;
                    self.file_traversals.set_outcome(&target, outcome);
                    drop(job); // Return the queued recipe's permit without launching it.
                }
                return;
            }
            if self.parallel_jobs.values().any(|job| !job.waiting_load) && self.load_high() {
                break;
            }
            let Some(index) = self.next_load_waiter() else {
                break;
            };
            let (target, identity) = self.load_waiting.remove(index);
            self.complete_job((target, identity, JobCompletion::LoadReady), false);
        }
    }

    fn next_load_waiter(&self) -> Option<usize> {
        if self.defer_stopped_remake_load
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
            self.load_waiting.len().checked_sub(1)
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

    fn complete_failed_dispatch(&mut self) -> bool {
        let Some((target, identity)) = self.failed_dispatch.take() else {
            return false;
        };
        let shutdown = self.shutdown.get();
        let drain_has_worker = shutdown == Shutdown::Draining
            && self.parallel_jobs.values().any(|job| job.child.is_some());
        if shutdown == Shutdown::Running || drain_has_worker {
            self.complete_job((target, identity, JobCompletion::FailedDispatch), false);
        } else {
            // A later graph error drains real workers only. The launch errno
            // has already been reported to this dispatch's output context.
            self.parallel_jobs
                .remove(&target, identity)
                .expect("reserved failed dispatch");
        }
        true
    }

    fn finish_parallel_job(
        &mut self,
        target: Vec<u8>,
        mut job: ParallelJob,
        completion: JobCompletion,
        blocking_reap: bool,
    ) {
        let recipe_branch = job.continuation.branch;
        let (result, failed_dispatch) = match completion {
            JobCompletion::Child(result) => (Some(result), false),
            JobCompletion::FailedDispatch => (None, true),
            JobCompletion::LoadReady => (None, false),
        };
        let already_draining = self.graph_abort.is_some();
        let output = job.continuation.output.clone();
        let debug_job = job.continuation.debug_job.clone();
        if let Some(result) = &result {
            crate::debug::Jobs::reaped(
                self.output.debug_mask(),
                debug_job.as_ref(),
                result.as_ref().is_ok_and(|status| status.success()),
            );
        }
        let launched = result.as_ref().is_some_and(Result::is_ok);
        if launched || failed_dispatch {
            self.recipe_reaps = self.recipe_reaps.wrapping_add(1);
        }
        if launched {
            self.load.reaped();
            if let Some(buffer) = &output {
                buffer.child_reaped(true);
            }
        }
        let report_error = failed_dispatch
            || result
                .as_ref()
                .is_some_and(|r| !r.as_ref().is_ok_and(|status| status.success()));
        if report_error {
            self.output
                .select(output.as_ref().filter(|buffer| buffer.enabled()));
        }
        if failed_dispatch || result.is_some() {
            self.filesystem.borrow_mut().child_completed();
        }
        let signaled = result
            .as_ref()
            .and_then(|status| status.as_ref().ok())
            .is_some_and(|status| status.signal().is_some());
        let error_code = match result.as_ref() {
            Some(Ok(status)) => status.code().filter(|code| *code != 0),
            Some(Err(_)) => Some(127),
            None if failed_dispatch => Some(127),
            None => None,
        };
        let command_question =
            self.options.question && job.recursive && !job.ignore && error_code == Some(1);
        let error = match result {
            None if failed_dispatch => Some("Error 127".to_owned()),
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
        let mut outcome = self
            .file_traversals
            .outcome(&target)
            .copied()
            .unwrap_or_default();
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
        let failed_source_recipe = launched
            && command_failed
            && error_code.is_some()
            && !job.recursive
            && !pending
            && !outcome.fatal
            && !outcome.blocked;
        if failed_source_recipe
            && self.required_declared_tail_recipe(target.as_slice(), recipe_branch)
        {
            outcome.remake_failure = RemakeFailure::RequiredDeclaredTailRecipe;
        } else if failed_source_recipe
            && self.required_synthetic_recipe(target.as_slice(), recipe_branch)
        {
            // The child error is reported and cached. The source pass uses the
            // declared result, so this child does not fail unrelated goals.
            outcome.remake_failure = RemakeFailure::RequiredSyntheticRecipe;
        } else if !optional_failure {
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
        if !failed_dispatch {
            self.record_graph_failure(outcome);
        }
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
        self.file_traversals.set_outcome(&target, cached);
        if let Some(failure) = job.required_pair_failure
            && failed_source_recipe
            && !optional_failure
            && cached.ran_refresh()
            && cached.failed
            && !cached.question_outdated
            && let Some(clock) = cached.time
            && Self::regular_remake_clock(&target) == Some(clock)
        {
            failure.set(Some(clock));
        }
        if let Some(worker) = job.trailing_optional_worker
            && launched
            && !pending
            && let Some(clock) = cached.time
        {
            if job.identity == worker.required_identity
                && cached.ran_refresh()
                && clock != worker.required_before
                && self.parallel_jobs.len() == 1
                && self.parallel_jobs.values().any(|optional| {
                    optional.identity == worker.optional_identity && optional.child.is_some()
                })
                && Self::regular_remake_clock(&target) == Some(clock)
            {
                if !report_error && cached.goal_verdict().is_none() {
                    worker
                        .required_reap
                        .set(Some(RequiredRemakeWorkerReap::Completed(clock)));
                } else if failed_source_recipe
                    && cached.failed
                    && matches!(
                        cached.remake_failure,
                        RemakeFailure::RequiredDeclaredTailRecipe
                    )
                    && let Some(ordinal) = recipe_branch
                {
                    worker
                        .required_reap
                        .set(Some(RequiredRemakeWorkerReap::FailedDeclaredTail {
                            ordinal,
                            clock,
                        }));
                }
            } else if job.identity == worker.optional_identity
                && (worker.required_failure_settled.get()
                    || self
                        .parallel_jobs
                        .get(&worker.required, worker.required_identity)
                        .is_some_and(|required| required.child.is_some())
                    || (worker.no_required_tail_remains
                        && match worker.required_reap.get() {
                            Some(_) if job.before == Some(clock) => {
                                !report_error || (failed_source_recipe && optional_failure)
                            }
                            Some(RequiredRemakeWorkerReap::Completed(_)) => {
                                failed_source_recipe && optional_failure
                            }
                            Some(RequiredRemakeWorkerReap::FailedDeclaredTail { .. }) => {
                                worker.direct_reread && failed_source_recipe && optional_failure
                            }
                            None => false,
                        }))
                && Self::regular_remake_clock(&target) == Some(clock)
            {
                if !report_error && Self::same_clock_optional_remake_outcome(Some(clock), cached) {
                    worker
                        .result
                        .set(Some(TrailingOptionalWorkerResult::Completed(clock)));
                } else if failed_source_recipe
                    && optional_failure
                    && self.cached_failed_optional_remake_at_clock(&target, clock, cached)
                {
                    // The optional failure remains cached for ordinary dependents.
                    worker
                        .result
                        .set(Some(TrailingOptionalWorkerResult::Failed(clock)));
                }
            }
        }
        if !pending {
            self.finish_recipe(&target, outcome);
        }
        if !outcome.fatal {
            let failed_dispatch_paid = failed_dispatch
                && completed_permit
                    .as_ref()
                    .is_some_and(|permit| !permit.is_implicit())
                && rmake_linux::signals::pending().is_none();
            self.release_completed_permit(completed_permit);
            if failed_dispatch_paid && let Some(record) = debug_job.as_ref() {
                crate::debug::Jobs::released(self.output.debug_mask(), record);
            }
        }
        self.finish_debug_job(debug_job.as_ref());
        self.reuse_implicit_job_slot(!outcome.fatal);
        if failed_dispatch {
            // Return the pidless dispatch's paid slot before starting a
            // stopping drain. That drain will scan the real worker.
            self.record_graph_failure(outcome);
        }
        // A blocking reap scans a continued command at its next wait. A
        // completed recipe or a token or poll wake still reports its scan here.
        let starts_dispatch_drain =
            failed_dispatch && !already_draining && self.graph_abort.is_some();
        if !starts_dispatch_drain && (!blocking_reap || !continued) {
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
            || self.file_traversals.has_outcome(target)
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
                && self.database.file_records.contains(archive)
                && self.database.file_records.logical_name(path) == Some(archive)
            {
                // enter_file recreated the original container key. Merging
                // it back into its own tentative record keeps its local name.
                self.database.file_records.rename(archive, path);
            }
            if let Some(path) = &location.path
                && (self.gpath_match(archive, path) || self.other_file_record(path, archive))
            {
                let name = [path.as_slice(), b"(", member, b")"].concat();
                let settled_container =
                    if location.container_time.is_some() && self.gpath_match(archive, path) {
                        self.settled_archive_record(archive)
                    } else {
                        None
                    };
                let merged_owners = settled_container
                    .and_then(|_| self.late_goal_merge_owners(archive, path))
                    .filter(|owners| owners.destination_unattempted);
                let retained_owner = if self.database.file_records.contains(path) {
                    merged_owners.map(|owners| owners.destination)
                } else {
                    settled_container
                };
                if retained_owner.is_some()
                    && let Some(history) = self.database.database_history()
                {
                    // VPATH interns the permanent container result before the
                    // member's rehash interns its matching spelling.
                    history.intern(path);
                    history.intern(&name);
                }
                self.rename_search_record(archive, path)?;
                if let Some(owner) = retained_owner
                    && let Some(mut outcome) = self.file_traversals.remove_outcome(archive)
                {
                    debug_assert_eq!(self.database.file_records.id(path), Some(owner));
                    // Both kinds of permanent rename retain the update and
                    // the observation of the searched container.
                    outcome.time = location.container_time;
                    if let Some(owners) = merged_owners {
                        outcome.database_result = DatabaseResult::MergedArchiveContainer;
                        self.record_merged_goal_reports(owners, path);
                    }
                    self.file_traversals.set_outcome(path, outcome);
                    self.archive_vpath_reads
                        .get_mut()
                        .as_mut()
                        .expect("successful archive search records the path")
                        .permanent_containers
                        .insert(owner, archive.to_vec());
                }
                self.path_observations
                    .entry(path.clone())
                    .or_insert(Some((None, location.container_time)));
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
                let name = [path.as_slice(), b"(", member, b")"].concat();
                if !self.has_file_record(path) {
                    if let Some(owner) = self.settled_archive_record(archive) {
                        if let Some(history) = self.database.database_history() {
                            // VPATH interns the container result before the
                            // archive rehash interns its member spelling.
                            history.intern(path);
                            history.intern(&name);
                        }
                        self.relocate_search_record(archive, path, Some(owner), None)?;
                    } else {
                        self.database.file_records.rehash(archive, path);
                    }
                }
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
                let moved_container = self.database.file_records.id(path).is_some_and(|id| {
                    self.moved_archives
                        .get(&id)
                        .is_some_and(|owner| owner.original == archive)
                });
                // The tentative member rehash itself searches its new slot.
                if moved_container || !self.has_file_record(&name) {
                    self.database.file_records.rehash(target, &name);
                }
            }
            let observation = self
                .archive_observation_at(target, &location, Some(target))
                .map_err(|error| self.expansion_error(None, &error))?;
            if location.container_time.is_some() || location.settled_missing_container {
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

    fn settled_archive_record(&self, archive: &[u8]) -> Option<FileRecordId> {
        if self.remaking
            || self.parallel_graph()
            || self.options.dry_run
            || self.options.question
            || self.options.touch
            || self.aliases.contains_source_or_destination(archive)
            || self.visiting.contains(archive)
            || self.pending(archive)
            || !self.database_contexts.contains_key(archive)
            || !self.target_contexts.contains_key(archive)
            || !self
                .database
                .rules
                .get(archive)
                .is_some_and(|rule| rule.double_colon.is_none() && rule.group.is_none())
            || !self
                .file_traversals
                .outcome(archive)
                .is_some_and(|outcome| {
                    outcome.ran && !outcome.failed && !outcome.search_after_update
                })
        {
            return None;
        }
        self.database.file_records.id(archive)
    }

    fn rename_search_record(&mut self, from: &[u8], to: &[u8]) -> Result<(), ()> {
        self.relocate_search_record(from, to, None, None)
    }

    fn relocate_search_record(
        &mut self,
        from: &[u8],
        to: &[u8],
        tentative_archive: Option<FileRecordId>,
        late_goal_owners: Option<MergedGoalOwners>,
    ) -> Result<(), ()> {
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
        let retained_ordinary = self.database_contexts.contains_key(from);
        let destination_has_scope = self.database.target_scopes.contains_key(to);
        let retained_head = !retained_ordinary
            && self.file_traversals.has_outcome(from)
            && self
                .database
                .rules
                .get(from)
                .is_some_and(|rule| rule.double_colon.is_some())
            && self
                .database_branch_contexts
                .contains_key(&(from.to_vec(), 0));
        let disjoint_file_head = retained_head
            && tentative_archive.is_none()
            && self.direct_named_head_reads()
            && self.gpath_match(from, to)
            && destination_has_scope
            && self.database.has_plain_double_colon_rules(from)
            && self
                .file_traversals
                .outcome(from)
                .is_some_and(|outcome| !outcome.failed)
            && self.file_traversals.visit(to).is_none()
            && !self.target_contexts.contains_key(to)
            && !self.database_contexts.contains_key(to)
            && self.database.rules.get(to).is_none_or(|rule| {
                rule.record_only && rule.group.is_none() && rule.double_colon.is_none()
            })
            && rmake_core::filenames::archive_parts(from).is_none()
            && rmake_core::filenames::archive_parts(to).is_none()
            && !self.aliases.contains_source_or_destination(from)
            && !self.aliases.contains_source_or_destination(to)
            && self.database.import_disjoint_file_head(
                from,
                to,
                &self.database_branch_contexts[&(from.to_vec(), 0)],
            );
        // An unseen destination without a declaration table can adopt the
        // completed file's ordinary table or declared double-colon head.
        let adopt_retained_scope = (retained_head
            || (retained_ordinary
                && self
                    .database
                    .rules
                    .get(from)
                    .is_none_or(|rule| rule.double_colon.is_none())))
            && !destination_has_scope
            && !self.target_contexts.contains_key(to)
            && !self.database_contexts.contains_key(to)
            && self.file_traversals.visit(to).is_none();
        // A recipe-less visit has no saved locals yet. Reconstruct its own
        // ancestry before merging or adopting the source file table.
        let uninitialized_visited_destination = late_goal_owners
            .is_some_and(|owners| !owners.destination_unattempted)
            && !self.database_contexts.contains_key(to)
            && !self.target_contexts.contains_key(to);
        let needs_visited_context = uninitialized_visited_destination
            && (!destination_has_scope
                || self
                    .file_traversals
                    .visit(to)
                    .is_some_and(|visit| visit.parent.is_some()));
        let visited_context = if needs_visited_context {
            self.with_target_read_expansion(None, |database, context| {
                database.variables_for_visited_file(to, &context)
            })
        } else {
            None
        };
        let uninitialized_visited_destination = uninitialized_visited_destination
            && (!needs_visited_context || visited_context.is_some());
        let uninitialized_named_destination = destination_has_scope
            && late_goal_owners.is_some_and(|owners| owners.destination_unattempted);
        let completed_scope_merge = !self.remaking
            && retained_ordinary
            && self
                .file_traversals
                .outcome(from)
                .is_some_and(|outcome| !outcome.failed)
            && self
                .database
                .rules
                .get(from)
                .is_some_and(|rule| rule.double_colon.is_none() && rule.group.is_none())
            && self
                .database
                .rules
                .get(to)
                .is_none_or(|rule| rule.double_colon.is_none() && rule.group.is_none())
            && if self.file_traversals.visit(to).is_some() {
                self.file_traversals
                    .outcome(to)
                    .is_some_and(|outcome| !outcome.failed && !outcome.search_after_update)
                    && (uninitialized_visited_destination
                        || (self.database_contexts.contains_key(to)
                            && self.target_contexts.contains_key(to)
                            && (destination_has_scope
                                || (rmake_core::filenames::archive_parts(from).is_none()
                                    && rmake_core::filenames::archive_parts(to).is_none()))))
            } else {
                destination_has_scope
                    && !self.database_contexts.contains_key(to)
                    && !self.target_contexts.contains_key(to)
            };
        let completion = self.archive_completion_times.remove(from);
        self.archive_completion_times.remove(to);
        if let Some(completion) = completion {
            self.archive_completion_times
                .insert(to.to_vec(), completion);
        }
        if let Some(owner) = tentative_archive {
            self.database.file_records.rehash(from, to);
            debug_assert_eq!(self.database.file_records.id(to), Some(owner));
        } else {
            self.database.file_records.rename(from, to);
        }
        let merged = self.database.explicit_names.contains(to)
            || destination_has_scope
            || self.has_requested_goal_name(to);
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
                        crate::messages::system(Message::AliasedRecipeSource {
                            path: from,
                            source_file: location.file.as_ref(),
                            line: location.line,
                        })
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
                        crate::messages::system(Message::AliasedRecipeSameFile { from, to }),
                        crate::messages::system(Message::AliasedRecipeIgnored { from, to }),
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
                if disjoint_file_head && let Some(branches) = &mut source.double_colon {
                    branches.truncate(1);
                }
                self.database.rules.insert(to.to_vec(), source);
            }
        }
        if completed_scope_merge
            && let Some(source) = self.database_contexts.get(from)
            && let Some((saved, base)) = self.database.merge_completed_file_scope(
                from,
                to,
                source,
                self.database_contexts.get(to).or(visited_context.as_ref()),
                self.target_contexts.get(to).or(visited_context.as_ref()),
                uninitialized_visited_destination || uninitialized_named_destination,
            )
        {
            self.database_contexts.insert(to.to_vec(), saved);
            self.target_contexts.insert(to.to_vec(), base);
        }
        self.database
            .rename_target_scope(from, to, adopt_retained_scope);
        self.file_traversals.rename_visit(from, to);
        if tentative_archive.is_some()
            && let Some(mut outcome) = self.file_traversals.remove_outcome(from)
        {
            // The member's successful VPATH hit already observed that the
            // original recipe spelling did not exist.
            outcome.time = None;
            self.file_traversals.set_outcome(to, outcome);
        }
        // The source identity is retired. An existing destination can keep
        // its own failed verdict and diagnostic edge.
        self.missing_rule_parents.remove(from);
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
            if let Some(mut context) = self.database_branch_contexts.remove(&key) {
                if adopt_retained_scope && retained_head && key.1 == 0 {
                    // A named eager declaration opens the head. The saved
                    // later branches still own their original runtime locals.
                    context.rename_retained_file_scope(
                        from,
                        to,
                        self.database.target_scopes.get(to),
                    );
                    self.database_contexts.insert(to.to_vec(), context.clone());
                }
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
        let implicit_keys: Vec<_> = self
            .database_branch_implicit
            .extract_if(|(name, _)| name.as_slice() == from)
            .map(|(_, ordinal)| (to.to_vec(), ordinal))
            .collect();
        self.database_branch_implicit.extend(implicit_keys);
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
            &mut self.ignored_vpath,
        ] {
            if flags.remove(from) {
                flags.insert(to.to_vec());
            }
        }
        let scope = self.database.target_scopes.get(to);
        if adopt_retained_scope {
            for context in self
                .target_contexts
                .get_mut(to)
                .into_iter()
                .chain(self.database_contexts.get_mut(to))
            {
                context.rename_retained_file_scope(from, to, scope);
            }
        } else if let Some(scope) = scope
            && let Some(context) = self.target_contexts.get_mut(to)
        {
            context.refresh_target_scope(scope);
        }
        if let Some(scope) = self.implicit_scopes.remove(from) {
            self.implicit_scopes.entry(to.to_vec()).or_insert(scope);
        }
        if self.database.explicit_names.contains(from) {
            if tentative_archive.is_some() {
                self.database.explicit_names.remove(from);
            }
            self.database.explicit_names.insert(to.to_vec());
        }
        if !merged && self.intermediates.remove(from) {
            self.intermediates.insert(to.to_vec());
        }
        self.intermediate_policy.renamed(from, to, merged);
        if let Some(owner) = tentative_archive {
            if let Some(reads) = self.archive_vpath_reads.get_mut()
                && reads.archives.remove(from)
            {
                reads.archives.insert(to.to_vec());
            }
            self.database_unchecked_times.borrow_mut().remove(to);
            self.database_times.borrow_mut().insert(to.to_vec(), None);
            let command_goal = self.database.file_records.moved_lookup_id_at_snapshot(from)
                == Some(owner)
                && self.has_requested_goal_name(from);
            self.moved_archives.insert(
                owner,
                MovedArchiveRecord {
                    lookup: to.to_vec(),
                    original: from.to_vec(),
                    command_goal,
                },
            );
            return Ok(());
        }
        let rebuild_goal_aliases = if let Some(goals) = &self.requested_goal_names {
            let previous = self.aliases.resolve(from);
            let destination = self.aliases.resolve(to);
            if previous == destination {
                false
            } else if previous != from {
                true
            } else {
                // An earlier requested alias can already resolve to this source.
                if self.requested_goal_aliases.remove(from) || goals.contains(from) {
                    self.requested_goal_aliases.insert(destination.to_vec());
                }
                false
            }
        } else {
            false
        };
        let kind = if disjoint_file_head {
            AliasKind::DetachedHead
        } else if merged {
            AliasKind::Merge
        } else {
            AliasKind::Rename
        };
        self.aliases.insert(from, to, kind);
        if rebuild_goal_aliases {
            // A redirected alias can leave other requested names at its old
            // destination. Recompute those resolutions for the new map.
            self.requested_goal_aliases = self
                .options
                .goals
                .iter()
                .filter_map(|goal| {
                    let destination = self.aliases.resolve(goal);
                    (destination != goal.as_slice()).then(|| destination.to_vec())
                })
                .collect();
        }
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
            let detail = rmake_linux::filesystem::source_error_detail(&issue.error);
            self.stderr(&crate::messages::system(Message::OsError {
                action: issue.operation,
                path: &issue.path,
                detail: &detail,
            }));
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
            return Err(ExpandError::system(Message::UnsupportedArchiveSymbol(
                target,
            )));
        }
        let member_time = self.name_time(member);
        let container = self
            .database
            .file_records
            .logical_name(archive)
            .unwrap_or(archive)
            .to_vec();
        let local_time = self.name_time(&container);
        let rebuilt_from = if refresh && local_time.is_some() && !self.moved_archives.is_empty() {
            self.archive_vpath_reads
                .get_mut()
                .as_ref()
                .and_then(|reads| reads.paths.get(target))
                .filter(|previous| {
                    self.database
                        .file_records
                        .id(previous)
                        .and_then(|id| self.moved_archives.get(&id))
                        .is_some_and(|owner| owner.original == archive)
                })
                .cloned()
        } else {
            None
        };
        if rebuilt_from.is_some() {
            if !self.database.file_records.contains(archive)
                && let Some(history) = self.database.database_history()
            {
                history.intern(archive);
            }
            self.database.file_records.enter_prerequisite(archive);
        } else {
            self.database.file_records.insert(archive);
        }
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
        // An unsuccessful container lookup updates the completed container's
        // physical time, but returns before setting the member's own mtime.
        let settled_missing_container = !refresh
            && path.is_none()
            && container_time.is_none()
            && archive == container
            && self.assumed_time(archive).is_none()
            && self.settled_archive_record(archive).is_some();
        if settled_missing_container {
            if let Some(outcome) = self.file_traversals.outcome_mut(archive) {
                outcome.time = None;
            }
            self.database_unchecked_times
                .borrow_mut()
                .insert(target.to_vec());
        }
        // The archive is a separate file observation even when the member
        // has already finished. Its first physical timestamp may be here.
        if self.clock_candidate(container_time).is_some()
            && !self
                .file_traversals
                .has_outcome(self.aliases.resolve(&container))
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
            let owner = self.database.file_records.id(path);
            let observed = if owner.is_some_and(|id| self.moved_archives.contains_key(&id)) {
                path.as_slice()
            } else {
                archive
            };
            reads.archives.insert(observed.to_vec());
            reads
                .paths
                .entry(target.to_vec())
                .or_insert_with(|| path.clone());
        } else if let Some(previous) = rebuilt_from
            && let Some(time) = local_time
        {
            // A member can recreate the raw archive after the earlier owner
            // was rehashed. Its local observation belongs to the new record.
            self.path_observations
                .entry(archive.to_vec())
                .or_insert(Some((None, Some(time))));
            if let Some(outcome) = self.file_traversals.outcome_mut(&previous) {
                outcome.time = Some(time);
            }
        }
        Ok(ArchiveLocation {
            refresh,
            member_time,
            container_time,
            settled_missing_container,
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
        if let Some(history) = self.database.database_history() {
            let tentative_rebuild = location.path.is_none()
                && previous.is_some()
                && self.database.file_records.id(path).is_some_and(|id| {
                    self.moved_archives
                        .get(&id)
                        .is_some_and(|owner| owner.original == archive)
                });
            if tentative_rebuild || self.retained_permanent_archive(path) {
                // ar_member_date looks up the searched container before reading
                // the member date, including after a local member rebuild.
                history.record_file_lookup(path);
            }
        }
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

    fn retained_permanent_archive(&self, path: &[u8]) -> bool {
        self.archive_vpath_reads
            .borrow()
            .as_ref()
            .is_some_and(|reads| {
                !reads.permanent_containers.is_empty()
                    && self
                        .database
                        .file_records
                        .id(path)
                        .is_some_and(|id| reads.permanent_containers.contains_key(&id))
            })
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
        if !self
            .file_traversals
            .has_outcome(self.aliases.resolve(spelling))
        {
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
                self.stderr(&crate::messages::system(Message::InvalidLibraryPattern(
                    pattern.literal_bytes(),
                )));
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
        let paths = self.database.ranked_vpath_directories(archive).chain(
            self.vpath_dirs
                .iter()
                .filter(|path| path.as_slice() != b".")
                .enumerate()
                .map(|(index, path)| ((self.database.vpath_group_count(), index), path.as_slice())),
        );
        let mut observed_group = None;
        for ((group, _), directory) in paths {
            if let Some(history) = self.database.database_history()
                && observed_group != Some(group)
            {
                history.record_file_lookup(archive);
                observed_group = Some(group);
            }
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
    fn implicit_presence(
        &mut self,
        target: &[u8],
        attempt: crate::debug::Event<'_>,
    ) -> Result<ImplicitPresence, ExpandError> {
        if rmake_core::filenames::archive_parts(target).is_none() {
            // The local lookup can announce an invalidated directory
            // before the prerequisite attempt. VPATH follows the attempt.
            let present = self.directory_member(target);
            crate::debug::emit(&self.output, attempt);
            return if present? {
                Ok(ImplicitPresence::Local)
            } else {
                Ok(match self.searched_file(target)? {
                    Some((Some(path), _)) => ImplicitPresence::Search(path),
                    Some((None, _)) => ImplicitPresence::Local,
                    None => ImplicitPresence::Absent,
                })
            };
        }
        crate::debug::emit(&self.output, attempt);
        // Viability uses the archive date alone. Standalone-member freshness
        // belongs to the later build timestamp phase. Existing containers enter
        // the filename table even if the candidate is eventually rejected.
        let (archive, member) =
            rmake_core::filenames::archive_parts(target).expect("archive reference");
        if member.starts_with(b"(") && member.ends_with(b")") {
            return Err(ExpandError::system(Message::UnsupportedArchiveSymbol(
                target,
            )));
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

    fn merged_goal_report(&self, operand: &[u8]) -> Option<&MergedGoalReport> {
        if self.merged_goal_reports.is_empty() {
            return None;
        }
        let records = &self.database.file_records;
        let owner = records
            .moved_lookup_id_at_snapshot(operand)
            .or_else(|| records.id(operand))?;
        self.merged_goal_reports.get(&owner)
    }

    fn record_merged_goal_reports(&mut self, owners: MergedGoalOwners, name: &[u8]) {
        let source = if owners.destination_unattempted {
            MergedGoalReport::CompletedSource(name.to_vec())
        } else {
            MergedGoalReport::CompletedVisitedSource(name.to_vec())
        };
        self.merged_goal_reports.insert(owners.source, source);
        if owners.destination_unattempted {
            self.merged_goal_reports
                .insert(owners.destination, MergedGoalReport::UnattemptedDestination);
        }
    }

    fn late_goal_merge_owners(&self, from: &[u8], to: &[u8]) -> Option<MergedGoalOwners> {
        // A prior terminal verdict can stop GNU's completion-time search
        // without stopping subsequent goals under keep-going.
        if self.remaking
            || self.parallel_graph()
            || self.ordinary_queries.first_verdict.is_some()
            || !self.gpath_match(from, to)
            || self.aliases.contains(to)
            || rmake_core::filenames::archive_parts(from).is_some()
            || rmake_core::filenames::archive_parts(to).is_some()
            || self.database.rules.get(to).is_some_and(|rule| {
                !rule.record_only || rule.double_colon.is_some() || rule.group.is_some()
            })
        {
            return None;
        }
        let rule = self.database.rules.get(from)?;
        if rule.group.is_some()
            || (rule.double_colon.is_some() && !self.database.has_plain_double_colon_rules(from))
        {
            return None;
        }
        let visited = self.file_traversals.visit(to).is_some();
        let outcome = self.file_traversals.outcome(to);
        let destination_unattempted = !visited && outcome.is_none();
        if !destination_unattempted
            && (!visited
                || rule.double_colon.is_some()
                || MakefileModes::from_options(&self.options).active()
                || self.assumed_time(to).is_some()
                || !outcome.is_some_and(|result| {
                    !result.failed
                        && !result.question_outdated
                        && !result.ran
                        && result.completion == CompletionTime::Unchanged
                        && result.database_result == DatabaseResult::Outcome
                        && result.time.is_some()
                }))
        {
            return None;
        }
        let source = self.database.file_records.id(from)?;
        let destination = self.database.file_records.id(to)?;
        if source == destination
            || self.merged_goal_reports.contains_key(&source)
            || self.merged_goal_reports.contains_key(&destination)
        {
            return None;
        }
        // The queued source keeps its success and adopts the new name. The
        // destination keeps its own update result, including an earlier
        // successful visit of an existing file without a recipe.
        Some(MergedGoalOwners {
            source,
            destination,
            destination_unattempted,
        })
    }

    fn defer_goal_timestamp_after_failure(
        &self,
        target: &[u8],
        completion: CompletionTime,
    ) -> bool {
        if self.ordinary_queries.first_verdict != Some(Status::Failure)
            || self.remaking
            || self.parallel_graph()
        {
            return false;
        }
        let modes = MakefileModes::from_options(&self.options);
        if !modes.active() {
            return true;
        }
        if !modes.touch || modes.dry_run || modes.question || completion != CompletionTime::Refresh
        {
            return false;
        }
        // Goal reporting can still hold the queued spelling after GPATH has
        // adopted the rule on its canonical file.
        let target = self.aliases.resolve(target);
        rmake_core::filenames::archive_parts(target).is_none()
            && self
                .database
                .rules
                .get(target)
                .is_some_and(|rule| rule.double_colon.is_none() && rule.grouped_targets().is_none())
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
        // A successful reopen leaves an earlier completed file unchecked
        // until this consumer actually needs its current timestamp.
        let reopened_include = outcome.time.is_none() && self.reopened_include_times.remove(target);
        if !outcome.search_after_update && !reopened_include {
            // This is the successful consumer/goal timestamp phase. Without
            // search paths, use the already-retained local observation; the
            // bookkeeping stat need not be repeated just for diagnostics.
            self.database_unchecked_times.borrow_mut().remove(target);
            return outcome;
        }
        if let Some(stamp) = self.archive_completion_times.get(target) {
            outcome.time = stamp.time;
            if stamp.known {
                self.database_unchecked_times.borrow_mut().remove(target);
            } else {
                outcome.completion = CompletionTime::ArchiveUnknown;
            }
            outcome.search_after_update = false;
            self.file_traversals.set_outcome(target, outcome);
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
                    let owners = self.late_goal_merge_owners(target, &name);
                    let mut retire_visited_source = false;
                    if self
                        .relocate_search_record(target, &name, None, owners)
                        .is_err()
                    {
                        outcome.failed = true;
                        outcome.fatal = true;
                    } else if let Some(owners) = owners
                        && !self.database.has_detached_file_head(&name)
                    {
                        if !owners.destination_unattempted {
                            outcome.database_result = DatabaseResult::MergedVisitedFile;
                            retire_visited_source = true;
                        }
                        self.record_merged_goal_reports(owners, &name);
                    }
                    self.file_traversals.set_outcome(&name, outcome);
                    if retire_visited_source || self.database.has_detached_file_head(&name) {
                        // A new record can reuse the old lookup key. Keep the
                        // completed update on its physical destination.
                        self.file_traversals.remove_outcome(target);
                        self.record_graph_failure(outcome);
                        return outcome;
                    }
                }
            }
            Err(error) => {
                self.expansion_error(None, &error);
                outcome.failed = true;
                outcome.fatal = true;
            }
        }
        self.file_traversals.set_outcome(target, outcome);
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
        // Keep prerequisites for default-recipe fallback before search can
        // run effects. Declared rules do not use that fallback.
        let (had_rule, default_dependencies) = match self.database.rules.get(target) {
            Some(rule) if rule.recipes.is_some() || rule.double_colon.is_some() => return true,
            Some(rule) => (true, rule.record_only.then(|| rule.dependencies.clone())),
            None => (false, Some(Vec::new())),
        };
        let Ok(plan) = self.plan_implicit_update(target, debug_depth, true, None) else {
            return false;
        };
        if plan.is_none()
            && !had_rule
            && self
                .database_times
                .borrow()
                .get(target)
                .is_some_and(Option::is_some)
        {
            self.database_secondary.insert(target.to_vec());
        }
        if let Some(plan) = plan {
            let _ = self.install_implicit_plan(plan, ImplicitPlacement::Root);
        } else if let Some(dependencies) = default_dependencies
            && let Some(mut rule) = self.default_update_rule(target, debug_depth, graph_visit)
        {
            rule.dependencies = dependencies;
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
        branch: Option<usize>,
    ) -> Result<Option<PendingImplicitRoot>, ()> {
        if self.terminal_prerequisites.contains(target) {
            return Ok(None);
        }
        if record_attempt {
            self.database_implicit.insert(target.to_vec());
        }
        // Retain the attempted lookup for the database. With no rules, only
        // implicit tracing needs to visit the empty search phases.
        if self.database.database_implicit_rules().is_empty()
            && !self.output.debug_mask().has(crate::debug::Mask::IMPLICIT)
        {
            return Ok(None);
        }
        crate::debug::emit(
            &self.output,
            crate::debug::Event::Looking(target, debug_depth + 1),
        );
        let mut session = ImplicitSearchSession {
            active: Vec::new(),
            budget: 100_000,
            debug_depth,
            root_children: Vec::new(),
            published: HashMap::new(),
            scope_revision: 0,
        };
        match self.implicit_plan(target, &mut session, false, branch) {
            Ok(None) if session.budget == 0 => {
                self.fatal(None, b"rmake: implicit rule search resource limit exceeded");
                Err(())
            }
            Ok(selection) => Ok(selection.map(|selection| PendingImplicitRoot {
                selection,
                children: session.root_children,
                published: session.published,
            })),
            Err(error) => {
                self.expansion_error(None, &error);
                Err(())
            }
        }
    }

    /// A detached double-colon root stays with its branch. Its children still
    /// enter the normal graph.
    fn install_implicit_plan(
        &mut self,
        mut plan: PendingImplicitRoot,
        placement: ImplicitPlacement,
    ) -> Option<Rule> {
        self.publish_implicit_children(
            &plan.selection,
            plan.children,
            &mut plan.published,
            placement,
        );
        self.retain_implicit_selection_history(&plan.selection, placement);
        let original = plan.published.get(&plan.selection.name);
        self.attach_implicit_selection(plan.selection, placement, original)
    }

    fn install_declared_implicit_plan(&mut self, mut plan: PendingImplicitRoot) -> Option<Rule> {
        if plan.selection.rule.group.is_none() {
            return self.install_implicit_plan(plan, ImplicitPlacement::DoubleColonRoot);
        }
        // A declared branch cannot take a grouped recipe. Its search still
        // accepted the child rules before returning the pending root.
        self.publish_implicit_children(
            &plan.selection,
            plan.children,
            &mut plan.published,
            ImplicitPlacement::Descendant,
        );
        None
    }

    fn publish_implicit_children(
        &mut self,
        parent: &ImplicitSelection,
        mut children: Vec<ImplicitSelection>,
        published: &mut HashMap<Vec<u8>, PublishedImplicit>,
        placement: ImplicitPlacement,
    ) {
        if children.is_empty() && placement == ImplicitPlacement::Descendant {
            return;
        }
        let inferred: HashMap<_, _> = children
            .iter()
            .map(|child| (child.name.as_slice(), &child.rule))
            .collect();
        for child in &children {
            self.retain_implicit_selection_history(child, ImplicitPlacement::Descendant);
        }
        // Process candidate inputs in reverse order when their owner accepts.
        for dependency in parent.rule.dependencies.iter().rev() {
            if let Some(child) = inferred.get(dependency.name.as_slice()) {
                self.database.file_records.insert(&dependency.name);
                for input in child.dependencies.iter().filter(|dep| dep.name != b".WAIT") {
                    self.database.file_records.insert(&input.name);
                }
            } else if placement != ImplicitPlacement::Descendant && dependency.name != b".WAIT" {
                if let Some(history) = self.database.database_history() {
                    history.intern(&dependency.name);
                }
                self.database
                    .file_records
                    .enter_prerequisite(&dependency.name);
            }
        }
        // Bind a literal child as explicit when it enters the graph. Other
        // inputs of a provisional parent wait until that parent is attached.
        self.database.explicit_names.extend(
            parent
                .explicit_names
                .iter()
                .filter(|name| {
                    placement != ImplicitPlacement::Descendant
                        || inferred.contains_key(name.as_slice())
                })
                .cloned(),
        );
        for child in &children {
            self.database
                .explicit_names
                .extend(child.explicit_names.iter().cloned());
            published.entry(child.name.clone()).or_insert_with(|| {
                let declaration = self.database.rules.get(&child.name).map(|rule| Rule {
                    dependencies: rule.dependencies.clone(),
                    secondary: rule.secondary.clone(),
                    ..Rule::default()
                });
                PublishedImplicit {
                    declaration,
                    ..PublishedImplicit::default()
                }
            });
        }
        // Candidate inputs are entered in reverse order. The first selection
        // supplies the pattern table after earlier entries have merged names.
        // Keep runtime metadata separately because it follows expansion order.
        for child in children.iter_mut().rev() {
            let Some(mut scope) = child.prepared_scope.take() else {
                continue;
            };
            let retained = published.get_mut(&child.name).expect("registered child");
            let declaration_revision = self.database.target_declaration_revision(&child.name);
            if retained.declaration_revision != declaration_revision {
                // A named declaration reconnects the file's active pattern
                // table, replacing the table used by previous merges.
                retained.declaration_revision = declaration_revision;
                retained.pattern_bindings.clear();
                if let Some(pattern) = self
                    .implicit_scopes
                    .get(&child.name)
                    .and_then(|active| active.pattern.as_ref())
                {
                    retained.pattern_bindings.extend(pattern.binding_names());
                }
            }
            if let Some(pattern) = &mut scope.pattern {
                pattern.retain_unmerged_bindings(&mut retained.pattern_bindings);
            }
            if scope.revision > retained.scope_revision {
                retained.scope_revision = scope.revision;
                self.implicit_scopes.insert(child.name.clone(), *scope);
            } else if let Some(active) = self.implicit_scopes.get_mut(&child.name) {
                active.pattern = scope.pattern;
            }
        }
        let mut attached = HashSet::new();
        for child in children {
            if attached.insert(child.name.clone()) {
                let retained = &published[&child.name];
                self.attach_implicit_selection(
                    child,
                    ImplicitPlacement::Descendant,
                    Some(retained),
                );
            }
        }
    }

    fn retain_implicit_selection_history(
        &mut self,
        selection: &ImplicitSelection,
        placement: ImplicitPlacement,
    ) {
        let group = selection.rule.group.as_ref();
        // Detached double-colon roots prepare their own stem at the branch.
        let retain_stem = group.is_some() || placement == ImplicitPlacement::Root;
        if let Some(history) = self.database.database_history() {
            if retain_stem && let Some(stem) = &selection.rule.stem {
                history.intern(stem);
            }
            if (retain_stem || placement == ImplicitPlacement::DoubleColonRoot)
                && let Some(pattern) = &selection.rule.implicit_pattern
            {
                history.record_file_lookup(pattern);
            }
        }
        if let Some(group) = group {
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

    fn attach_implicit_selection(
        &mut self,
        selection: ImplicitSelection,
        placement: ImplicitPlacement,
        original: Option<&PublishedImplicit>,
    ) -> Option<Rule> {
        let ImplicitSelection {
            name,
            mut rule,
            prepared_scope,
            explicit_names: _,
            intermediate_names,
        } = selection;
        let descendant = placement == ImplicitPlacement::Descendant;
        let detached = placement == ImplicitPlacement::DoubleColonRoot;
        if descendant {
            self.database_implicit.insert(name.clone());
        }
        if rule.terminal {
            // Explicit recipes and default recipes still apply to these inputs.
            for dependency in &rule.dependencies {
                if dependency.name != b".WAIT" {
                    self.terminal_prerequisites.insert(dependency.name.clone());
                    self.database_implicit.insert(dependency.name.clone());
                }
            }
        }
        // A selected rule promotes the file to a target, including previously
        // undeclared builtin records.
        rule.record_only = false;
        let implicit_prefix = rule.dependencies.len();
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
        if !detached {
            let declaration = match original {
                Some(original) => original.declaration.as_ref(),
                None => self.database.rules.get(&name),
            };
            if let Some(declaration) = declaration {
                rule.append_prerequisites(declaration);
            }
        }
        if descendant
            && !self.database.explicit_names.contains(&name)
            && !self.has_requested_goal_name(&name)
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
            Some(rule)
        } else {
            self.invalidate_phony_names(&name);
            self.database.rules.insert(name, rule);
            None
        }
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
        branch: Option<usize>,
    ) -> Result<Option<Rule>, ()> {
        let depth = depth + usize::from(self.remaking);
        Ok(
            match self.plan_implicit_update(target, depth, false, branch)? {
                Some(plan) => self.install_implicit_plan(plan, ImplicitPlacement::DoubleColonRoot),
                None => self.default_update_rule(target, depth, true),
            },
        )
    }

    /// A selection stays pending until its parent accepts. Descendants already
    /// attached by an accepting frame survive later rejection of its ancestors.
    /// Rule identities prevent unbounded chains such as %.x: %.x.x.
    fn implicit_plan(
        &mut self,
        target: &[u8],
        session: &mut ImplicitSearchSession,
        compatibility: bool,
        branch: Option<usize>,
    ) -> Result<Option<ImplicitSelection>, ExpandError> {
        if session.active.len() >= 128 {
            session.budget = 0;
        }
        if session.budget == 0 {
            return Ok(None);
        }
        let depth = session.debug_depth + session.active.len() + 1;
        let archive_name = rmake_core::filenames::archive_parts(target)
            .map(|(archive, _)| &target[archive.len()..]);
        let root = session.active.is_empty();
        let mut search = ImplicitSearch {
            compatibility,
            archive: false,
            intermediate_scope: None,
            branch,
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
            if let Some(selection) = self.implicit_search(target, session, &mut search)? {
                return Ok(Some(selection));
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
        session: &mut ImplicitSearchSession,
        search: &mut ImplicitSearch,
    ) -> Result<Option<ImplicitSelection>, ExpandError> {
        let debug_depth = session.debug_depth;
        let compatibility = search.compatibility;
        let archive_search = search.archive;
        let tracing = self.output.debug_mask().has(crate::debug::Mask::IMPLICIT);
        // Both archive search phases match without ignoring any directory.
        let trace_directory =
            tracing && !archive_search && rmake_core::filenames::archive_parts(target).is_none();
        if tracing && !session.active.is_empty() {
            let mut blocked = session.active.clone();
            blocked.sort_unstable();
            for identity in blocked {
                let label = crate::debug::rule_label(&self.database, identity);
                crate::debug::emit(
                    &self.output,
                    crate::debug::Event::AvoidingRecursion(
                        &label,
                        debug_depth + session.active.len() + 2,
                    ),
                );
            }
        }
        let mut candidates = if archive_search {
            self.database
                .implicit_archive_candidates_excluding(target, &session.active)
        } else {
            self.database
                .implicit_candidates_excluding(target, &session.active)
        };
        let mut compatibility_needed = false;
        let mut cancelled = Vec::new();
        let intermediate = !session.active.is_empty();
        let mut variables = None;
        for chain in [false, true] {
            if chain && !candidates.is_empty() {
                crate::debug::emit(
                    &self.output,
                    crate::debug::Event::TryingHarder(debug_depth + session.active.len() + 2),
                );
            }
            for (ordinal, candidate) in candidates.iter_mut().enumerate() {
                if session.budget == 0 {
                    return Ok(None);
                }
                session.budget -= 1;
                if cancelled.get(ordinal).copied().unwrap_or(false)
                    || session.active.contains(&candidate.identity)
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
                            debug_depth + session.active.len() + 2,
                        ),
                    );
                    if stem_too_long {
                        crate::debug::emit(
                            &self.output,
                            crate::debug::Event::StemTooLong(
                                directory,
                                printed_stem,
                                debug_depth + session.active.len() + 2,
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
                session.active.push(candidate.identity);
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
                                    search.branch,
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
                            if intermediate {
                                let scope = search
                                    .intermediate_scope
                                    .as_mut()
                                    .expect("initialized intermediate scope");
                                scope
                                    .runtime_metadata
                                    .retain_runtime_metadata(variables, true);
                                session.scope_revision += 1;
                                scope.revision = session.scope_revision;
                            } else if let Some(ordinal) = search.branch {
                                // Each declared branch owns its runtime metadata,
                                // including effects of a rejected implicit candidate.
                                let mut context = self.rule_variables(target, Some(ordinal))?;
                                context.retain_runtime_metadata(variables, true);
                                self.retain_database_context(target, Some(ordinal), &context);
                            } else {
                                let target = self.aliases.resolve(target);
                                if let Some(context) = self.target_contexts.get_mut(target) {
                                    context.retain_runtime_metadata(variables, true);
                                }
                                // The root's read table sees search automatics even
                                // if this candidate is rejected. Recipes and inputs
                                // keep their separately prepared variable contexts.
                                self.database_contexts
                                    .insert(target.to_vec(), variables.clone());
                            }
                            let value = value?;
                            ImplicitFilenames::Expanded(self.implicit_filenames(
                                &value,
                                &mut order_only,
                                if percent {
                                    candidate.secondary_directory.as_deref()
                                } else {
                                    None
                                },
                            )?)
                        } else {
                            // ONEWORD parsing preserves blanks and pipes from a
                            // stem, but still expands newly introduced globs.
                            let start = part
                                .iter()
                                .position(|byte| !matches!(byte, b' ' | b'\t'..=b'\r'))
                                .unwrap_or(part.len());
                            if start == part.len() {
                                ImplicitFilenames::Expanded(Vec::new())
                            } else {
                                self.implicit_filename(&part[start..], occurrence.order_only, None)?
                            }
                        };
                        let (single, expanded) = match names {
                            ImplicitFilenames::Single(name) => (Some(name), Vec::new()),
                            ImplicitFilenames::Expanded(names) => (None, names),
                        };
                        for mut dependency in single.into_iter().chain(expanded) {
                            dependency.hidden = occurrence.hidden;
                            let identity = self.aliases.resolve(&dependency.name);
                            if identity != dependency.name {
                                dependency.name = identity.to_vec();
                            }
                            let name = &dependency.name;
                            let event_depth = debug_depth + session.active.len() + 1;
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
                                if ordinal >= cancelled.len() {
                                    cancelled.resize(ordinal + 1, false);
                                }
                                cancelled[ordinal] = true;
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
                            let attempt =
                                crate::debug::Event::TryingPrerequisite(name, literal, event_depth);
                            let declaration = match session.published.get(target) {
                                Some(published) => published.declaration.as_ref(),
                                None => self.database.rules.get(target),
                            };
                            let explicit = (self.has_file_record(name)
                                && self.record_is_target(name))
                                || declaration.is_some_and(|rule| {
                                    rule.dependencies.iter().enumerate().any(|(index, dep)| {
                                        dep.name == *name
                                            && !rule.secondary.as_ref().is_some_and(|clauses| {
                                                clauses.iter().any(|clause| clause.index == index)
                                            })
                                    })
                                });
                            if name == b".WAIT" || explicit {
                                crate::debug::emit(&self.output, attempt);
                            }
                            if explicit {
                                crate::debug::emit(
                                    &self.output,
                                    crate::debug::Event::OughtToExist(
                                        name,
                                        debug_depth + session.active.len() + 1,
                                    ),
                                );
                            }
                            if name != b".WAIT" && !explicit {
                                let presence = self.implicit_presence(name, attempt)?;
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
                                    if let Some(child) =
                                        self.implicit_plan(name, session, compatibility, None)?
                                    {
                                        plan.push(child);
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
                session.active.pop();
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
                                debug_depth + session.active.len() + 1,
                            ),
                        );
                    }
                    let mut selected = std::mem::take(&mut candidate.rule);
                    // A pattern barrier cannot cross into the target's explicit prerequisites.
                    Self::trim_implicit_waits(&mut dependencies);
                    selected.dependencies = dependencies;
                    selected.secondary = None;
                    let selection = ImplicitSelection {
                        name: target.to_vec(),
                        rule: selected,
                        explicit_names,
                        intermediate_names,
                        prepared_scope: search.intermediate_scope.take().map(Box::new),
                    };
                    if intermediate {
                        self.publish_implicit_children(
                            &selection,
                            plan,
                            &mut session.published,
                            ImplicitPlacement::Descendant,
                        );
                    } else {
                        session.root_children = plan;
                    }
                    return Ok(Some(selection));
                }
            }
        }
        if compatibility_needed && !compatibility {
            crate::debug::emit(
                &self.output,
                crate::debug::Event::SearchingCompatibility(
                    target,
                    debug_depth + session.active.len() + 1,
                ),
            );
            search.compatibility = true;
            return self.implicit_search(target, session, search);
        }
        Ok(None)
    }

    fn implicit_variables(
        &mut self,
        target: &[u8],
        intermediate: bool,
        stem: Vec<u8>,
        intermediate_scope: &mut Option<ImplicitScope>,
        branch: Option<usize>,
    ) -> Result<VariableStore, ExpandError> {
        if intermediate && intermediate_scope.is_none() {
            let pattern = self.with_target_read_expansion(None, |database, context| {
                database.prepare_pattern_scope(target, context)
            })?;
            *intermediate_scope = Some(ImplicitScope {
                pattern,
                runtime_metadata: VariableStore::default(),
                revision: 0,
            });
        }
        let mut context = if let Some(prepared) = intermediate_scope.as_ref() {
            let mut context = match prepared.pattern.as_ref() {
                Some(scope) => self.database.variables.with_target_scopes(&[scope]),
                None => self.database.variables.with_target_scopes(&[]),
            };
            context.retain_runtime_metadata(&prepared.runtime_metadata, true);
            context
        } else {
            self.rule_variables(target, branch)?
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
    ) -> Result<ImplicitFilenames, ExpandError> {
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
                .with_shell(&|request| {
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
            return Ok(ImplicitFilenames::Single(Dependency {
                name,
                order_only,
                hidden: false,
            }));
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
        Ok(ImplicitFilenames::Expanded(
            bytes
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
                .collect(),
        ))
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
                match self.implicit_filename(&name, role, directory)? {
                    ImplicitFilenames::Single(name) => result.push(name),
                    ImplicitFilenames::Expanded(names) => result.extend(names),
                }
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
            let shell = |request: rmake_core::diagnostic::ShellRequest<'_>| {
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
                .with_shell(&shell)
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

    fn discard_missing_archive_name(&mut self, target: &[u8]) -> bool {
        if let Some(searched) = self.resolved_names.get(target)
            && rmake_core::filenames::archive_parts(target).is_some()
            && self.database.file_records.logical_name(searched) == Some(target)
        {
            // A tentative rehash keeps the member's searched key. Its
            // filename is promoted only when the member is current.
            self.resolved_names.remove(target);
            return true;
        }
        false
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
            if self.parallel_jobs.values().any(|job| job.child.is_some()) {
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

    fn prepare_missing_rule_visit(&mut self, target: &[u8], parent: Option<&[u8]>) {
        if self.missing_rule_parents.is_empty() {
            return;
        }
        let settled_failure =
            self.file_traversals.outcome(target).is_some_and(|outcome| {
                outcome.failed && !outcome.fatal && !outcome.question_outdated
            }) && !self.pending(target);
        if !settled_failure {
            self.missing_rule_parents.remove(target);
        } else if let Some(parent) = parent
            && let Some(previous) = self.missing_rule_parents.get_mut(target)
            && previous.as_deref() != Some(parent)
        {
            *previous = Some(parent.to_vec());
        }
    }

    fn missing_rule_parent<'a>(
        &'a self,
        target: &[u8],
        fallback: Option<&'a [u8]>,
    ) -> Option<&'a [u8]> {
        let target = self.aliases.resolve(target);
        if self.missing_rule_parents.is_empty()
            || !self.file_traversals.outcome(target).is_some_and(|outcome| {
                outcome.failed && !outcome.fatal && !outcome.question_outdated
            })
            || self.pending(target)
        {
            return fallback;
        }
        self.missing_rule_parents
            .get(target)
            .map_or(fallback, |parent| parent.as_deref())
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
                && self.file_traversals.outcome(&dependency.name).map_or_else(
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
                    // This direct failure belongs to the current prerequisite
                    // edge. The child's inherited variable scope can still
                    // belong to an earlier parent that visited the same leaf.
                    parent: target.to_vec(),
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
        self.report_missing_rule(target, self.missing_rule_parent(target, parent));
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
                match self.synthetic_double_colon_recipe(target, depth, None) {
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
                        self.file_traversals.set_outcome(target, outcome);
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
                    self.file_traversals.set_outcome(&key, head);
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
            self.file_traversals.set_outcome(target, outcome);
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
        self.file_traversals.remove_outcome(target);
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
                assumed_phony_input: false,
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

    fn replay_settled_double_colon_goal_failure(
        &mut self,
        target: &[u8],
        depth: usize,
    ) -> Option<Outcome> {
        if self.remaking
            || !self.options.keep_going
            || self.graph_abort.is_some()
            || self.shutdown.get() != Shutdown::Running
        {
            return None;
        }
        let cause = self.optional_remake_failures.get(target)?.as_ref()?.clone();
        let outcome = *self.file_traversals.outcome(target)?;
        if !outcome.failed
            || outcome.fatal
            || outcome.question_outdated
            || outcome.completion == CompletionTime::ArchiveUnknown
            || cause.target == target
            || !self.makefile_names.contains(target)
            || self.pending(target)
            || self
                .active_peers
                .get(target)
                .is_some_and(|owner| owner != target)
            || self.ordinary_queries.deferred_tails.contains_key(target)
            || self.cached_double_colon_input_due(target)
            || self.visited_cached_double_colon_input(target)
        {
            return None;
        }
        let declarations = self.serial_double_colon_declarations(target)?;
        let mut key = (target.to_vec(), 0);
        if !self
            .database_branch_outcomes
            .get(&key)
            .is_some_and(|head| head.failed)
            || !(1..declarations).all(|ordinal| {
                key.1 = ordinal;
                self.database_branch_outcomes.contains_key(&key)
            })
        {
            return None;
        }

        // Each ordinary goal entry revisits the settled declaration chain.
        // The head repeats its retained cause; the tails only report their
        // cached status. No prerequisites or recipes run again.
        for _ in 0..declarations {
            self.debug_completed_double_colon_declarations(target, 0..1, depth);
            self.show_retained_optional_failure(target, None, Some(Rc::clone(&cause)));
            self.debug_completed_double_colon_declarations(target, 1..declarations, depth);
        }
        Some(Outcome {
            replayed_double_colon_goal_failure: true,
            ..outcome
        })
    }

    fn show_retained_optional_failure(
        &mut self,
        target: &[u8],
        parent: Option<&[u8]>,
        cause: Option<Rc<FailedPrerequisite>>,
    ) {
        self.show_makefile_error();
        if let Some(cause) = cause {
            self.report_missing_rule(
                &cause.target,
                self.missing_rule_parent(&cause.target, Some(&cause.parent)),
            );
            self.optional_remake_failures
                .remove(cause.target.as_slice());
        } else {
            self.report_missing_rule(target, self.missing_rule_parent(target, parent));
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
                    if location.container_time.is_some() || location.settled_missing_container {
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
            self.file_traversals.remove_outcome(name);
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
        if visit == BuildVisit::NamedRemakeWithoutPoll {
            self.observe_interrupt();
        } else if !cached_wait || self.shutdown.get() != Shutdown::Running {
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
        let previously_done = archive_target && self.file_traversals.has_outcome(target);
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
        let mut reused_failed_outcome = false;
        let mark_optional_visit = self.remaking
            && !self.parallel_makefile_remakes
            && self.optional_remake()
            && self.output.database_requested();
        if mark_optional_visit {
            // Leaf updates and early expansion can bypass the cycle detection set.
            self.database_optional_visits.push(target.to_vec());
        }
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
                &mut reused_failed_outcome,
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
        if mark_optional_visit {
            self.database_optional_visits.pop();
        }
        if self.optional_remake()
            && !reused_failed_outcome
            && (outcome.failed || outcome.question_outdated)
            && !outcome.fatal
        {
            self.optional_remake_failures
                .entry(target.to_vec())
                .or_default();
        }
        if !deferred_peer_failure || !outcome.remake_failure.is_optional() || outcome.fatal {
            self.record_graph_failure(outcome);
        }
        outcome
    }

    #[allow(clippy::too_many_arguments)]
    fn build_inner(
        &mut self,
        target: &[u8],
        considering_name: &[u8],
        parent: Option<ScopeParent<&[u8]>>,
        depth: usize,
        initial_time: Option<&mut Option<Option<SystemTime>>>,
        visit: BuildVisit,
        reused_failed_outcome: &mut bool,
    ) -> Outcome {
        self.prepare_missing_rule_visit(target, parent.map(|parent| parent.target));
        if parent.is_none()
            && let Some(outcome) = self.replay_settled_double_colon_goal_failure(target, depth)
        {
            *reused_failed_outcome = true;
            return outcome;
        }
        let mut traced_optional_failure = false;
        if !self.optional_remake()
            && let Some(cause) = self.optional_remake_failures.get(target).cloned()
        {
            // A later required visit exposes the original failure hidden by
            // the optional remake, including the edge to a deeper failure.
            // Blocked parents retain their cause for later goal visits; the
            // failed leaf's own diagnostic is consumed only once.
            if cause.is_none() {
                self.optional_remake_failures.remove(target);
            }
            let query = self
                .file_traversals
                .outcome(target)
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
                && self
                    .file_traversals
                    .outcome(target)
                    .is_some_and(|outcome| outcome.failed)
                && self.trace_cached_unruled_makefile(considering_name, target, depth, true)
            {
                traced_optional_failure = true;
            }
            let cached_remake_parent = if (!self.remaking || !self.parallel_makefile_remakes)
                && parent.is_none()
                && self
                    .file_traversals
                    .outcome(target)
                    .is_some_and(|outcome| outcome.failed && !outcome.fatal)
            {
                self.file_traversals
                    .visit(target)
                    .and_then(|visit| visit.parent.as_ref())
                    .map(|parent| self.aliases.resolve(&parent.target).to_vec())
            } else {
                None
            };
            if !traced_optional_failure
                && self.remaking
                && ((self.parallel_makefile_remakes && parent.is_none())
                    || (!self.parallel_makefile_remakes
                        && self.options.keep_going
                        && parent.is_some())
                    || cached_remake_parent.is_some())
                && self
                    .file_traversals
                    .outcome(target)
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
            let diagnostic_parent = parent
                .map(|parent| parent.target)
                .or(cached_remake_parent.as_deref());
            self.show_retained_optional_failure(target, diagnostic_parent, cause);
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
            let mut outcome = self
                .file_traversals
                .outcome(owner)
                .copied()
                .unwrap_or_default();
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
        if let Some(mut outcome) = self.file_traversals.outcome(target).copied() {
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
            let pruned = self.debug_was_considered(target);
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
                        || (!self.remaking && !outcome.fatal && !outcome.question_outdated)
                    {
                        // An ordinary visit introduces cached remake results
                        // through the same consideration and failure policy.
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
                self.file_traversals.set_outcome(target, outcome);
            }
            // A settled failure keeps its existing reporting policy. Visiting
            // it through an optional source does not hide a failure already
            // reported by an earlier required source. Fresh branch dispatch
            // and pending work return above without marking this reuse.
            *reused_failed_outcome = outcome.failed && !outcome.question_outdated;
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
            self.file_traversals.set_outcome(target, outcome);
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
        let quiet_consideration =
            if self.remaking || self.output.debug_mask().has(crate::debug::Mask::VERBOSE) {
                None
            } else {
                NonZeroU64::new(self.debug_pass.wrapping_add(1))
            };
        let records = &self.database.file_records;
        self.file_traversals
            .remember_visit(target, || FileVisit {
                parent: parent
                    .map(|parent| OwnedScopeParent::new(parent, records.id(parent.target))),
                quiet_consideration,
            })
            .quiet_consideration = quiet_consideration;
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
                self.file_traversals.set_outcome(target, result);
                return result;
            }
            crate::debug::emit(
                &self.output,
                crate::debug::Event::MustRemake(target, debug_depth),
            );
            let tentative_member = self.discard_missing_archive_name(target);
            let optional = self.optional_remake();
            let keep_going = self.options.keep_going;
            let stopping_missing_rule = !optional
                && !keep_going
                && (tentative_member
                    || match rmake_core::filenames::archive_parts(target) {
                        Some((archive, _)) => {
                            self.database_unchecked_times.borrow().contains(target)
                                || self.retained_permanent_archive(archive)
                        }
                        None => !self.remaking && !self.parallel_graph(),
                    });
            self.missing_rule_parents
                .insert(target.to_vec(), parent.map(|parent| parent.target.to_vec()));
            if !optional {
                self.show_makefile_error();
                self.report_missing_rule(target, parent.map(|parent| parent.target));
            }
            if optional || keep_going {
                crate::debug::emit(
                    &self.output,
                    crate::debug::Event::Failed(target, debug_depth),
                );
            }
            let result = Outcome {
                failed: true,
                database_result: if stopping_missing_rule {
                    DatabaseResult::UnfinishedUpdate
                } else {
                    DatabaseResult::Outcome
                },
                ..Outcome::default()
            };
            self.file_traversals.set_outcome(target, result);
            return result;
        };
        if let Some(branches) = rule.double_colon.take() {
            let mut branches: std::collections::VecDeque<_> =
                branches.into_iter().map(Some).enumerate().collect();
            let mut synthetic_count = self.new_double_colon_entry_count(target);
            if synthetic_count == 1
                && depth == 0
                && self.remaking
                && !self.parallel_makefile_remakes
                && self.active_makefile_remake.is_some()
                && self.options.question
                && (self.options.touch || (!self.options.dry_run && self.options.goals.len() == 1))
                && !self.options.keep_going
                && self.has_requested_goal_name(rmake_core::filenames::without_dot_prefix(target))
                && let Some((_, count)) = self.assumed_new_double_colon_entries(target)
            {
                // A named query can complete successive synthetic entries.
                // A query verdict still stops the chain.
                synthetic_count = count;
            }
            for _ in 0..synthetic_count {
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
                    assumed_phony_input: false,
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
                return self
                    .file_traversals
                    .outcome(target)
                    .copied()
                    .unwrap_or(outcome);
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
            latest.packed_branch = OwnedScopeParent::pack_branch(parent.branch);
            latest.record = self.database.file_records.id(parent.target);
            // Keep the live edge if it returns to its original parent. A
            // retained eager read may have updated this file's local table.
            return;
        }
        if self.file_traversals.visit(target).is_some_and(|visit| {
            visit.parent.as_ref().map(|first| {
                let name = first
                    .record
                    .and_then(|id| self.moved_archives.get(&id))
                    .map_or(first.target.as_slice(), |owner| owner.lookup.as_slice());
                (self.aliases.resolve(name), first.branch())
            }) != Some((parent.target, parent.branch))
        }) {
            self.retained_scope_parents.insert(
                target.to_vec(),
                OwnedScopeParent::new(parent, self.database.file_records.id(parent.target)),
            );
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
        // The declared head alone inherits optional-source error suppression,
        // including when this visit will execute a grouped recipe itself.
        self.serial_double_colon_branch_count(target)
            .filter(|&count| count > 1)
    }

    fn serial_double_colon_branch_count(&self, target: &[u8]) -> Option<usize> {
        if self.parallel_makefile_remakes
            || self.options.dry_run
            || self.options.question
            || self.options.touch
        {
            return None;
        }
        let branches = self.database.rules.get(target)?.double_colon.as_ref()?;
        Some(branches.len())
    }

    fn unruled_leading_optional_source<'a>(
        &self,
        read_files: &'a [(Makefile, bool)],
    ) -> Option<&'a [u8]> {
        let [(primary, false), (neighbor, false), required @ ..] = read_files else {
            return None;
        };
        (!neighbor.missing
            && self.options.new_files.len() == 1
            && self.optional_remake_layout(primary, neighbor, required)
            && self.unruled_remake_source(&neighbor.name))
        .then_some(neighbor.name.as_slice())
    }

    fn leading_optional_remake_layout<'a>(
        &self,
        read_files: &'a [(Makefile, bool)],
    ) -> Option<LeadingOptionalRemake<'a>> {
        let [
            (primary, false),
            (neighbor, false),
            (required, false),
            trailing @ ..,
        ] = read_files
        else {
            return None;
        };
        if !self.optional_remake_layout(primary, neighbor, &read_files[2..3])
            || self
                .has_requested_goal_name(rmake_core::filenames::without_dot_prefix(&required.name))
        {
            return None;
        }
        let trailing = match trailing {
            [] => None,
            [(trailing, false)] if trailing.optional && trailing.location.is_some() => {
                let kind = if trailing.missing {
                    if !matches!(
                        trailing.read_error.as_ref(),
                        Some(IncludeError::Errno(error))
                            if io::Error::from_raw_os_error(*error).kind() == io::ErrorKind::NotFound
                    ) || !self.unruled_remake_source(&trailing.name)
                    {
                        return None;
                    }
                    TrailingOptionalKind::Missing
                } else if self.unruled_remake_source(&trailing.name) {
                    TrailingOptionalKind::Unruled
                } else if self.declared_optional_remake_source(&trailing.name) {
                    TrailingOptionalKind::Declared
                } else {
                    return None;
                };
                Some(TrailingOptionalRemake {
                    required,
                    neighbor: &trailing.name,
                    kind,
                })
            }
            _ => return None,
        };
        Some(LeadingOptionalRemake {
            required,
            neighbor,
            trailing,
        })
    }

    fn unassumed_direct_double_colon_remake(&self, target: &[u8]) -> bool {
        self.local_remake_source(target)
            && !self.special(b".PHONY", target, false)
            && self.database.implicit_candidates(target).is_empty()
            && self.database.rules.get(target).is_some_and(|rule| {
                let Some([head, tail]) = rule.double_colon.as_deref() else {
                    return false;
                };
                [head, tail].into_iter().all(|branch| {
                    branch.grouped_targets().is_none()
                        && branch.secondary.is_none()
                        && branch.dependencies.iter().all(|dep| {
                            dep.name != b".WAIT"
                                && !dep.hidden
                                && self.unruled_remake_source(&dep.name)
                        })
                })
            })
    }

    fn settle_inline_optional_remake_clock(
        &mut self,
        clock: InlineOptionalRemakeClock<'_>,
    ) -> bool {
        if self.shutdown.get() != Shutdown::Running
            || !self.parallel_jobs.is_empty()
            || MakefileModes::from_options(&self.options).active()
            || self.pending(clock.required)
            || !self
                .file_traversals
                .outcome(clock.required)
                .is_some_and(|outcome| {
                    outcome.ran
                        && !outcome.fatal
                        && !outcome.blocked
                        && outcome.goal_verdict().is_none()
                        && outcome.completion == CompletionTime::Refresh
                })
        {
            return clock.changed_on_visit;
        }
        let Some(result) = self.file_traversals.outcome(clock.source).copied() else {
            return clock.changed_on_visit;
        };
        if !Self::same_clock_optional_remake_outcome(Some(clock.inline), result) {
            return clock.changed_on_visit;
        }
        let Some(time) = Self::regular_remake_clock(clock.source) else {
            return clock.changed_on_visit;
        };
        self.publish_optional_remake_clock(clock.source, result, clock.before, time)
    }

    fn publish_optional_remake_clock(
        &mut self,
        name: &[u8],
        mut result: Outcome,
        before: SystemTime,
        time: SystemTime,
    ) -> bool {
        let changed = time != before;
        result.time = Some(time);
        result.debug_time_changed &= changed;
        self.file_traversals.set_outcome(name, result);
        self.retain_database_time(name, Some(time));
        changed
    }

    fn settled_required_worker_refresh(&self, outcome: Outcome) -> bool {
        outcome.ran_refresh()
            && (outcome.goal_verdict().is_none()
                || (self.options.keep_going
                    && outcome.failed
                    && !outcome.question_outdated
                    && matches!(
                        outcome.remake_failure,
                        RemakeFailure::RequiredDeclaredTailRecipe
                    )))
    }

    fn publish_restored_trailing_worker_clock(
        &mut self,
        leading: &LeadingOptionalRemake<'_>,
        visited: &MakefileVisits<'_>,
        pending: &[PendingMakefileRemake<'_>],
    ) {
        let Some(trailing) = leading.trailing.as_ref() else {
            return;
        };
        let name = trailing.neighbor;
        let Some(&(Some(before), _)) = visited.get(name) else {
            return;
        };
        let Some(worker) = pending
            .iter()
            .find(|request| !request.duplicate && std::ptr::eq(request.file, trailing.required))
            .and_then(|request| request.trailing_optional_worker.as_deref())
        else {
            return;
        };
        if self.options.keep_going
            && self.shutdown.get() == Shutdown::Running
            && self.parallel_jobs.is_empty()
            && !MakefileModes::from_options(&self.options).active()
            && worker.no_required_tail_remains
            && self
                .file_traversals
                .outcome(&worker.required)
                .is_some_and(|outcome| self.settled_required_worker_refresh(*outcome))
            && self.settled_leading_optional_visit_failure(leading, visited)
            && let Some(outcome) = self.restored_trailing_worker_outcome(worker, name, before)
        {
            // The earlier failure deferred the optional read's clock sample.
            // Ordinary consumers use that final sample after restoration.
            self.publish_optional_remake_clock(name, outcome, before, before);
        }
    }

    fn publish_restored_required_worker_clock(
        &mut self,
        leading: &LeadingOptionalRemake<'_>,
        visited: &MakefileVisits<'_>,
        pending: &[PendingMakefileRemake<'_>],
    ) {
        let name = leading.required.name.as_slice();
        if self.has_assumed_double_colon_entry(name)
            || !self.settled_leading_optional_visit_failure(leading, visited)
        {
            return;
        }
        let Some(trailing) = leading.trailing.as_ref() else {
            return;
        };
        if let Some(request) = pending.iter().find(|request| {
            !request.duplicate
                && !request.modes.active()
                && std::ptr::eq(request.file, trailing.required)
        }) && let Some(worker) = request.trailing_optional_worker.as_deref()
            && worker.no_required_tail_remains
            && let Some(mut outcome) = self.file_traversals.outcome(name).copied()
            && outcome.ran_refresh()
            && outcome.goal_verdict().is_none()
            && let Some(RequiredRemakeWorkerReap::Completed(clock)) = worker.required_reap.get()
            && outcome.time == Some(clock)
            && self.successful_explicit_source_refresh(&worker.required, clock)
            && self.required_worker_clock_was_restored(worker, trailing, visited, true)
        {
            // This pass continues with the original reads. Ordinary goals
            // compare the final physical time of the completed source.
            outcome.time = Some(worker.required_before);
            self.file_traversals.set_outcome(name, outcome);
        }
    }

    fn settled_leading_optional_visit_failure(
        &self,
        leading: &LeadingOptionalRemake<'_>,
        visited: &MakefileVisits<'_>,
    ) -> bool {
        visited
            .get(leading.neighbor.name.as_slice())
            .is_some_and(|(before, _)| {
                self.settled_leading_optional_failure(leading.neighbor, *before)
            })
    }

    fn same_clock_optional_remake_outcome(before: Option<SystemTime>, result: Outcome) -> bool {
        before.is_some()
            && result.time == before
            && result.ran
            && !result.failed
            && !result.fatal
            && !result.blocked
            && !result.question_outdated
            && result.completion == CompletionTime::Refresh
    }

    fn cached_failed_optional_remake_at_clock(
        &self,
        name: &[u8],
        clock: SystemTime,
        result: Outcome,
    ) -> bool {
        result.time == Some(clock)
            && result.ran_refresh()
            && result.failed
            && !result.question_outdated
            && self
                .optional_remake_failures
                .get(name)
                .is_some_and(Option::is_none)
    }

    fn settled_required_worker_clock(&self, worker: &TrailingOptionalWorker) -> Option<SystemTime> {
        let clock = Self::regular_remake_clock(&worker.required)?;
        (clock == worker.required_before
            || (worker.only_synthetic_tail_remains
                && self.successful_explicit_source_refresh(&worker.required, clock)))
        .then_some(clock)
    }

    fn settled_trailing_optional_worker(
        &self,
        request: &PendingMakefileRemake<'_>,
        trailing: &TrailingOptionalRemake<'_>,
        visited: &MakefileVisits<'_>,
    ) -> bool {
        if !self.options.keep_going {
            return false;
        }
        let Some(worker) = &request.trailing_optional_worker else {
            return false;
        };
        let Some(clock) = self.settled_optional_worker_clock(worker, trailing.neighbor) else {
            return false;
        };
        let Some(required_clock) = self.settled_required_worker_clock(worker) else {
            return false;
        };
        let current_clock = Self::regular_remake_clock(trailing.neighbor);
        // A later restoration leaves the optional failure in place. A
        // required rewrite supplies its own reread clock after either result.
        current_clock == Some(clock)
            || ((matches!(
                worker.result.get(),
                Some(TrailingOptionalWorkerResult::Failed(_))
            ) || required_clock != worker.required_before)
                && worker.only_synthetic_tail_remains
                && !worker.required_failure_settled.get()
                && visited
                    .get(trailing.neighbor)
                    .is_some_and(|(before, _)| before.is_some() && *before == current_clock))
    }

    fn settled_optional_worker_clock(
        &self,
        worker: &TrailingOptionalWorker,
        name: &[u8],
    ) -> Option<SystemTime> {
        let cached = self.file_traversals.outcome(name).copied()?;
        match worker.result.get()? {
            TrailingOptionalWorkerResult::Completed(clock) => {
                Self::same_clock_optional_remake_outcome(Some(clock), cached).then_some(clock)
            }
            TrailingOptionalWorkerResult::Failed(clock) => self
                .cached_failed_optional_remake_at_clock(name, clock, cached)
                .then_some(clock),
        }
    }

    fn restored_required_worker_clock(
        &self,
        request: &PendingMakefileRemake<'_>,
        trailing: &TrailingOptionalRemake<'_>,
        visited: &MakefileVisits<'_>,
    ) -> bool {
        let Some(worker) = request.trailing_optional_worker.as_deref() else {
            return false;
        };
        // Reread from the recorded explicit refresh when the second worker
        // restores the source clock before this pass reaches its final check.
        matches!(worker.required_reap.get(), Some(RequiredRemakeWorkerReap::Completed(clock))
            if self.successful_explicit_source_refresh(&worker.required, clock)
                && self.required_worker_clock_was_restored(worker, trailing, visited, worker.direct_reread))
    }

    fn failed_declared_worker_reap(
        &self,
        worker: &TrailingOptionalWorker,
    ) -> Option<(usize, SystemTime)> {
        let RequiredRemakeWorkerReap::FailedDeclaredTail { ordinal, clock } =
            worker.required_reap.get()?
        else {
            return None;
        };
        self.failed_declared_branch_at_clock(&worker.required, ordinal, clock)
            .then_some((ordinal, clock))
    }

    fn failed_declared_branch_at_clock(
        &self,
        target: &[u8],
        ordinal: usize,
        clock: SystemTime,
    ) -> bool {
        self.database_branch_outcomes
            .get(&(target.to_vec(), ordinal))
            .is_some_and(|branch| {
                branch.ran_refresh()
                    && branch.failed
                    && !branch.question_outdated
                    && branch.time == Some(clock)
                    && matches!(
                        branch.remake_failure,
                        RemakeFailure::RequiredDeclaredTailRecipe
                    )
            })
    }

    fn restored_failed_required_worker_clock(
        &self,
        request: &PendingMakefileRemake<'_>,
        trailing: &TrailingOptionalRemake<'_>,
        visited: &MakefileVisits<'_>,
    ) -> bool {
        request
            .trailing_optional_worker
            .as_deref()
            .is_some_and(|worker| {
                (worker.no_required_tail_remains
                    || (worker.only_synthetic_tail_remains
                        && worker.required_failure_settled.get()))
                    && self.failed_declared_worker_reap(worker).is_some()
                    && self.required_worker_clock_was_restored(
                        worker,
                        trailing,
                        visited,
                        worker.direct_reread,
                    )
            })
    }

    fn required_worker_clock_was_restored(
        &self,
        worker: &TrailingOptionalWorker,
        trailing: &TrailingOptionalRemake<'_>,
        visited: &MakefileVisits<'_>,
        allow_changed_optional_failure: bool,
    ) -> bool {
        let name = trailing.neighbor;
        let Some(&(Some(before), _)) = visited.get(name) else {
            return false;
        };
        Self::regular_remake_clock(&worker.required) == Some(worker.required_before)
            && self
                .settled_optional_worker_clock(worker, name)
                .is_some_and(|clock| {
                    Self::regular_remake_clock(name) == Some(clock)
                        && (clock == before
                            || (allow_changed_optional_failure
                                && matches!(
                                    worker.result.get(),
                                    Some(TrailingOptionalWorkerResult::Failed(_))
                                )))
                })
    }

    fn retained_trailing_worker_clock(
        &self,
        request: &PendingMakefileRemake<'_>,
        trailing: &TrailingOptionalRemake<'_>,
        visited: &MakefileVisits<'_>,
    ) -> bool {
        let Some(worker) = &request.trailing_optional_worker else {
            return false;
        };
        let name = trailing.neighbor;
        let Some(&(Some(before), _)) = visited.get(name) else {
            return false;
        };
        // The successful trailing clock can be read before the required
        // synthetic failure settles.
        self.options.keep_going
            && worker.only_synthetic_tail_remains
            && self
                .restored_trailing_worker_outcome(worker, name, before)
                .is_some()
    }

    fn restored_trailing_worker_outcome(
        &self,
        worker: &TrailingOptionalWorker,
        name: &[u8],
        before: SystemTime,
    ) -> Option<Outcome> {
        let Some(TrailingOptionalWorkerResult::Completed(reap_clock)) = worker.result.get() else {
            return None;
        };
        let outcome = self.file_traversals.outcome(name).copied()?;
        (!worker.required_failure_settled.get()
            && reap_clock != before
            && Self::same_clock_optional_remake_outcome(Some(reap_clock), outcome)
            && Self::regular_remake_clock(&worker.required) == Some(worker.required_before)
            && Self::regular_remake_clock(name) == Some(before))
        .then_some(outcome)
    }

    fn settled_trailing_optional_visit(
        &self,
        trailing: &TrailingOptionalRemake<'_>,
        visited: &MakefileVisits<'_>,
        same_clock_recipe_settled: bool,
    ) -> bool {
        let name = trailing.neighbor;
        self.options.keep_going
            && visited
                .get(name)
                .zip(self.file_traversals.outcome(name))
                .is_some_and(|((before, _), result)| {
                    let settled = if same_clock_recipe_settled {
                        Self::same_clock_optional_remake_outcome(*before, *result)
                    } else {
                        (trailing.kind != TrailingOptionalKind::Declared
                            || result.completion == CompletionTime::Unchanged)
                            && self
                                .settled_trailing_optional_remake(trailing, *before, *result, false)
                    };
                    // The missing outcome already checks that the path is absent.
                    settled
                        && (trailing.kind == TrailingOptionalKind::Missing
                            || fs::symlink_metadata(os(name)).is_ok_and(|file| {
                                file.is_file() && file.modified().ok() == *before
                            }))
                })
    }

    fn current_leading_optional_source(&self, name: &[u8]) -> bool {
        self.unruled_remake_source(name)
            || (self.declared_optional_remake_source(name)
                && !self.aliases.contains(name)
                && self.database.implicit_candidates(name).is_empty()
                && self
                    .database
                    .rules
                    .get(name)
                    .is_some_and(|rule| rule.recipes.is_some()))
    }

    fn settled_current_leading_optional(&self, name: &[u8], visited: &MakefileVisits<'_>) -> bool {
        visited
            .get(name)
            .zip(self.file_traversals.outcome(name))
            .is_some_and(|((before, _), outcome)| {
                before.is_some()
                    && outcome.time == *before
                    && outcome.completion == CompletionTime::Unchanged
                    && !outcome.ran
                    && !outcome.fatal
                    && !outcome.blocked
                    && outcome.goal_verdict().is_none()
                    && self.current_leading_optional_source(name)
                    && Self::regular_remake_clock(name) == *before
            })
    }

    fn settled_leading_optional_failure(
        &self,
        optional: &Makefile,
        before: Option<SystemTime>,
    ) -> bool {
        let name = optional.name.as_slice();
        if self.pending(name) {
            return false;
        }
        let Some(result) = self.file_traversals.outcome(name).copied() else {
            return false;
        };
        if !result.failed || result.fatal || result.question_outdated || result.ran {
            return false;
        }
        if optional.missing {
            return matches!(
                optional.read_error.as_ref(),
                Some(IncludeError::Errno(error))
                    if io::Error::from_raw_os_error(*error).kind() == io::ErrorKind::NotFound
            ) && self.settled_missing_optional_remake(name, before, result);
        }
        result.blocked
            && before.is_some()
            && result.time == before
            && self.declared_optional_remake_source(name)
            && fs::symlink_metadata(os(name))
                .is_ok_and(|file| file.is_file() && file.modified().ok() == before)
    }

    fn initial_synthetic_remake_trace_source<'a>(
        &self,
        read_files: &'a [(Makefile, bool)],
    ) -> Option<&'a [u8]> {
        let [_, (source, false), (neighbor, false)] = read_files else {
            return None;
        };
        let trailing = self.trailing_optional_remake(read_files)?;
        let name = source.name.as_slice();
        if trailing.kind != TrailingOptionalKind::Unruled
            || !fs::symlink_metadata(os(&neighbor.name)).is_ok_and(|entry| entry.is_file())
            || name.contains(&b'/')
            || name.starts_with(b"~")
            || name.starts_with(b"-l")
            || rmake_core::filenames::archive_parts(name).is_some()
            || self.database.loaded_modules.contains_key(name)
            || self.special(b".PHONY", name, false)
            || self
                .assumed_new_double_colon_entries(name)
                .is_none_or(|(_, count)| count != 1)
        {
            return None;
        }
        let [head] = self.database.rules.get(name)?.double_colon.as_deref()? else {
            return None;
        };
        let candidates = self.database.implicit_candidates(name);
        let [synthetic] = candidates.as_slice() else {
            return None;
        };
        let leaf_time = |rule: &Rule| {
            let [dependency] = rule.dependencies.as_slice() else {
                return None;
            };
            if rule.secondary.is_some()
                || rule.grouped_targets().is_some()
                || dependency.order_only
                || dependency.hidden
                || !rule.recipes.as_ref().is_some_and(|recipes| {
                    matches!(recipes.as_slice(), [recipe] if !recipe_is_recursive(recipe))
                })
                || !self.unruled_remake_source(&dependency.name)
                || self.special(b".PHONY", &dependency.name, false)
            {
                return None;
            }
            let entry = fs::symlink_metadata(os(&dependency.name)).ok()?;
            entry.is_file().then(|| entry.modified().ok()).flatten()
        };
        let entry = fs::symlink_metadata(os(name)).ok()?;
        let before = entry.modified().ok()?;
        (entry.is_file()
            && leaf_time(head).is_some_and(|time| time <= before)
            && leaf_time(&synthetic.rule).is_some_and(|time| time > before))
        .then_some(name)
    }

    fn trailing_optional_remake<'a>(
        &self,
        read_files: &'a [(Makefile, bool)],
    ) -> Option<TrailingOptionalRemake<'a>> {
        if self.options.new_files.len() == 1 {
            self.trailing_optional_remake_layout(read_files)
        } else {
            None
        }
    }

    fn trailing_optional_remake_layout<'a>(
        &self,
        read_files: &'a [(Makefile, bool)],
    ) -> Option<TrailingOptionalRemake<'a>> {
        let [(primary, false), required @ .., (neighbor, false)] = read_files else {
            return None;
        };
        let (source, _) = required.last()?;
        if !self.optional_remake_layout(primary, neighbor, required)
            || self.has_requested_goal_name(rmake_core::filenames::without_dot_prefix(&source.name))
        {
            return None;
        }
        let kind = if neighbor.missing {
            if required.len() != 1
                || !matches!(
                    neighbor.read_error.as_ref(),
                    Some(IncludeError::Errno(error))
                        if io::Error::from_raw_os_error(*error).kind() == io::ErrorKind::NotFound
                )
                || !self.unruled_remake_source(&neighbor.name)
            {
                return None;
            }
            TrailingOptionalKind::Missing
        } else if self.unruled_remake_source(&neighbor.name) {
            TrailingOptionalKind::Unruled
        } else if self.declared_optional_remake_source(&neighbor.name) {
            TrailingOptionalKind::Declared
        } else {
            return None;
        };
        Some(TrailingOptionalRemake {
            required: source,
            neighbor: &neighbor.name,
            kind,
        })
    }

    fn optional_remake_layout(
        &self,
        primary: &Makefile,
        neighbor: &Makefile,
        required: &[(Makefile, bool)],
    ) -> bool {
        if !(1..=2).contains(&required.len())
            || required.iter().any(|(source, skip)| {
                *skip || source.optional || source.missing || source.location.is_none()
            })
            || primary.optional
            || primary.missing
            || primary.location.is_some()
            || !neighbor.optional
            || neighbor.location.is_none()
            || !self.options.no_rules
            || self.options.load_limit.is_some()
            || self.options.check_symlink_times
            || !self.options.include_directories.is_empty()
            || !self.options.old_files.is_empty()
            || self.database.has_vpaths()
            || !self.vpath_dirs.is_empty()
            || !self.gpath_dirs.is_empty()
            || self.database.rules.contains_key(b".DEFAULT".as_slice())
        {
            return false;
        }
        self.unruled_remake_source(&primary.name)
    }

    fn local_remake_source(&self, name: &[u8]) -> bool {
        !name.contains(&b'/')
            && !name.starts_with(b"~")
            && !name.starts_with(b"-l")
            && rmake_core::filenames::archive_parts(name).is_none()
            && !self.has_requested_goal_name(name)
            && self.assumed_time(name).is_none()
            && !self.database.loaded_modules.contains_key(name)
    }

    fn unruled_remake_source(&self, name: &[u8]) -> bool {
        self.local_remake_source(name)
            && !self.database.rules.contains_key(name)
            && self.database.implicit_candidates(name).is_empty()
    }

    fn declared_optional_remake_source(&self, name: &[u8]) -> bool {
        self.local_remake_source(name)
            && !self.special(b".PHONY", name, false)
            && self.database.rules.get(name).is_some_and(|rule| {
                !rule.record_only
                    && !rule.default_recipe
                    && rule.implicit_pattern.is_none()
                    && rule.stem.is_none()
                    && rule.secondary.is_none()
                    && rule.double_colon.is_none()
                    && rule.grouped_targets().is_none()
                    && rule.dependencies.iter().all(|dep| {
                        dep.name != b".WAIT" && !dep.hidden && self.unruled_remake_source(&dep.name)
                    })
            })
    }

    fn settled_trailing_optional_remake(
        &self,
        trailing: &TrailingOptionalRemake<'_>,
        before: Option<SystemTime>,
        result: Outcome,
        inline_completion: bool,
    ) -> bool {
        if result.fatal || result.question_outdated {
            return false;
        }
        let name = trailing.neighbor;
        match trailing.kind {
            TrailingOptionalKind::Unruled => {
                before.is_some()
                    && result.time == before
                    && !result.ran
                    && !result.failed
                    && self.unruled_remake_source(name)
            }
            TrailingOptionalKind::Declared => {
                before.is_some()
                    && !result.failed
                    && !result.blocked
                    && self.declared_optional_remake_source(name)
                    // Inline expansion and a prepared ':' can refresh the
                    // physical clock without creating a recipe job.
                    && ((!result.ran && result.time == before)
                        || (inline_completion
                            && result.time.is_some()
                            && result.completion == CompletionTime::Refresh
                            && self.parallel_jobs.len() == 1
                            && !MakefileModes::from_options(&self.options).active()))
            }
            TrailingOptionalKind::Missing => {
                self.settled_missing_optional_remake(name, before, result)
            }
        }
    }

    fn settled_missing_optional_remake(
        &self,
        name: &[u8],
        before: Option<SystemTime>,
        result: Outcome,
    ) -> bool {
        // The optional file settled as a direct missing-rule visit.
        // Keep that failure available to ordinary prerequisites.
        before.is_none()
            && result.time.is_none()
            && result.failed
            && !result.ran
            && !result.blocked
            && self
                .missing_rule_parents
                .get(name)
                .is_some_and(Option::is_none)
            && self.unruled_remake_source(name)
            && fs::symlink_metadata(os(name))
                .is_err_and(|error| error.kind() == io::ErrorKind::NotFound)
    }

    fn regular_remake_clock(name: &[u8]) -> Option<SystemTime> {
        let file = fs::symlink_metadata(os(name)).ok()?;
        file.is_file().then(|| file.modified().ok()).flatten()
    }

    fn required_remake_worker_pair<'a>(
        &self,
        read_files: &[(Makefile, bool)],
        requests: &[PendingMakefileRemake<'a>],
        visited: &MakefileVisits<'_>,
    ) -> Option<Box<RequiredRemakeWorkerPair<'a>>> {
        let [
            (primary, false),
            (first_source, false),
            (second_source, false),
            trailing_sources @ ..,
        ] = read_files
        else {
            return None;
        };
        let [first, second, trailing_requests @ ..] = requests else {
            return None;
        };
        if !self.parallel_makefile_remakes
            || self.options.always
            || self.assumptions.is_some()
            || self.parallel_jobs.len() != requests.len()
            || !self.unruled_remake_source(&primary.name)
            || !std::ptr::eq(first.file, first_source)
            || !std::ptr::eq(second.file, second_source)
        {
            return None;
        }
        let (first_identity, first_before, first_branch) =
            self.held_direct_required_remake_job(first, visited)?;
        let (second_identity, second_before, second_branch) =
            self.held_direct_required_remake_job(second, visited)?;
        let third = match (trailing_sources, trailing_requests) {
            ([], []) => None,
            ([(source, false)], [request]) if std::ptr::eq(request.file, source) => {
                let (identity, before, branch) =
                    self.held_direct_required_remake_job(request, visited)?;
                if branch.is_some() {
                    return None;
                }
                Some(RemakeChild {
                    name: &request.file.name,
                    identity,
                    before,
                })
            }
            _ => return None,
        };
        second_branch.is_none().then(|| {
            Box::new(RequiredRemakeWorkerPair {
                first_identity,
                first_before,
                first_branch,
                first_reap: None,
                second: &second.file.name,
                second_identity,
                second_before,
                second_failure: None,
                third,
                third_failure: None,
            })
        })
    }

    fn held_direct_required_remake_job(
        &self,
        request: &PendingMakefileRemake<'_>,
        visited: &MakefileVisits<'_>,
    ) -> Option<(u64, SystemTime, Option<usize>)> {
        let file = request.file;
        let name = file.name.as_slice();
        if file.optional
            || file.missing
            || file.location.is_none()
            || request.duplicate
            || !request.active
            || request.modes.active()
            || self.aliases.resolve(name) != name
            || self.file_traversals.visit(name)?.parent.is_some()
        {
            return None;
        }
        let &(Some(before), _) = visited.get(name)? else {
            return None;
        };
        let job = self.parallel_jobs.primary.get(name)?;
        if job.child.is_none()
            || job.recursive
            || job.ignore
            || job.phony
            || !job.continuation.expanded.is_empty()
            || job
                .continuation
                .remake
                .as_ref()
                .is_none_or(|remake| remake.optional)
            || job.before != Some(before)
            || Self::regular_remake_clock(name) != Some(before)
        {
            return None;
        }
        let branch = job.continuation.branch;
        let direct = if branch.is_some() {
            self.unassumed_direct_double_colon_remake(name)
                && self.pending_explicit_remake_recipe(name)
                && self
                    .pending_double_colon
                    .get(name)
                    .is_some_and(|state| state.branches.is_empty())
        } else {
            self.declared_optional_remake_source(name)
                && self
                    .database
                    .rules
                    .get(name)
                    .is_some_and(|rule| rule.recipes.is_some())
        };
        direct.then_some((job.identity, before, branch))
    }

    fn required_remake_pair_has_expected_workers(
        &self,
        pair: &RequiredRemakeWorkerPair<'_>,
    ) -> bool {
        match &pair.third {
            Some(third)
                if self
                    .parallel_jobs
                    .get(third.name, third.identity)
                    .is_some_and(|job| job.child.is_some()) =>
            {
                self.parallel_jobs.len() == 2
            }
            Some(third) => {
                // A third worker that settled before the first clock was
                // observed must have succeeded.
                self.parallel_jobs.len() == 1
                    && self.settled_required_remake_worker_clock(third.name, third.before, None)
            }
            None => self.parallel_jobs.len() == 1,
        }
    }

    fn track_required_pair_failure(
        &mut self,
        name: &[u8],
        identity: u64,
    ) -> Option<Rc<Cell<Option<SystemTime>>>> {
        let job = self.parallel_jobs.primary.get_mut(name)?;
        if job.identity != identity || job.child.is_none() {
            return None;
        }
        let failure = Rc::new(Cell::new(None));
        job.required_pair_failure = Some(Rc::clone(&failure));
        Some(failure)
    }

    fn settled_required_remake_worker_clock(
        &self,
        name: &[u8],
        before: SystemTime,
        failure: Option<&Cell<Option<SystemTime>>>,
    ) -> bool {
        Self::regular_remake_clock(name) == Some(before)
            && self.file_traversals.outcome(name).is_some_and(|outcome| {
                outcome.ran_refresh()
                    && outcome.time == Some(before)
                    && (outcome.goal_verdict().is_none()
                        || (self.options.keep_going
                            && outcome.failed
                            && !outcome.question_outdated
                            && failure.is_some_and(|failure| failure.get() == Some(before))))
            })
    }

    fn required_remake_pair_first_reap(
        &self,
        name: &[u8],
        pair: &RequiredRemakeWorkerPair<'_>,
        outcome: Outcome,
        clock: SystemTime,
    ) -> Option<RequiredRemakeWorkerReap> {
        if outcome.goal_verdict().is_none() {
            return (pair.first_branch.is_none()
                || self.successful_explicit_source_refresh(name, clock))
            .then_some(RequiredRemakeWorkerReap::Completed(clock));
        }
        let ordinal = pair.first_branch?;
        // The current declared head can supply the reread clock after its
        // final recipe failed. The actual branch failure remains cached.
        self.failed_declared_branch_at_clock(name, ordinal, clock)
            .then_some(RequiredRemakeWorkerReap::FailedDeclaredTail { ordinal, clock })
    }

    fn restored_required_remake_pair(
        &self,
        first: &[u8],
        outcome: Outcome,
        pair: &RequiredRemakeWorkerPair<'_>,
    ) -> bool {
        let clock = match pair.first_reap {
            Some(RequiredRemakeWorkerReap::Completed(clock))
                if outcome.goal_verdict().is_none() =>
            {
                clock
            }
            Some(RequiredRemakeWorkerReap::FailedDeclaredTail { ordinal, clock })
                if outcome.failed
                    && self.failed_declared_branch_at_clock(first, ordinal, clock) =>
            {
                clock
            }
            _ => return false,
        };
        outcome.time == Some(clock)
            && self.settled_required_worker_refresh(outcome)
            && self.parallel_jobs.is_empty()
            && Self::regular_remake_clock(first) == Some(pair.first_before)
            && self.settled_required_remake_worker_clock(
                pair.second,
                pair.second_before,
                pair.second_failure.as_deref(),
            )
            && pair.third.as_ref().is_none_or(|third| {
                self.settled_required_remake_worker_clock(
                    third.name,
                    third.before,
                    pair.third_failure.as_deref(),
                )
            })
    }

    fn track_trailing_optional_worker(
        &mut self,
        trailing: &TrailingOptionalRemake<'_>,
        optional_before: SystemTime,
        retain_required_clock: bool,
        direct_reread: bool,
    ) -> Option<Rc<TrailingOptionalWorker>> {
        let required = &trailing.required.name;
        if !self.active_trailing_optional_remake(trailing)
            || !self.pending_explicit_remake_recipe(required)
            || Self::regular_remake_clock(trailing.neighbor) != Some(optional_before)
        {
            return None;
        }
        let required_before = Self::regular_remake_clock(required)?;
        let required_identity = self.parallel_jobs.primary.get(required)?.identity;
        let branches = &self.pending_double_colon.get(required)?.branches;
        let no_required_tail_remains = branches.is_empty();
        let only_synthetic_tail_remains =
            branches.len() == 1 && branches.front().is_some_and(|(_, branch)| branch.is_none());
        let optional = self.parallel_jobs.primary.get_mut(trailing.neighbor)?;
        if optional.before != Some(optional_before) {
            return None;
        }
        let worker = Rc::new(TrailingOptionalWorker {
            required: required.clone(),
            required_identity,
            optional_identity: optional.identity,
            required_before,
            direct_reread,
            no_required_tail_remains,
            only_synthetic_tail_remains,
            required_failure_settled: Cell::new(false),
            required_reap: Cell::new(None),
            result: Cell::new(None),
        });
        optional.trailing_optional_worker = Some(Rc::clone(&worker));
        if retain_required_clock
            && (no_required_tail_remains || only_synthetic_tail_remains)
            && let Some(job) = self.parallel_jobs.primary.get_mut(required)
        {
            job.trailing_optional_worker = Some(Rc::clone(&worker));
        }
        Some(worker)
    }

    fn active_trailing_optional_remake(&self, trailing: &TrailingOptionalRemake<'_>) -> bool {
        trailing.kind == TrailingOptionalKind::Declared
            && self.parallel_jobs.len() == 2
            && self.declared_optional_remake_source(trailing.neighbor)
            && self
                .parallel_jobs
                .primary
                .get(trailing.neighbor)
                .is_some_and(|job| {
                    job.child.is_some()
                        && !job.recursive
                        && job.continuation.branch.is_none()
                        && job.continuation.expanded.is_empty()
                        && job
                            .continuation
                            .remake
                            .as_ref()
                            .is_some_and(|remake| remake.optional)
                })
    }

    fn trailing_remake_prerequisite_child<'a>(
        &self,
        trailing: &TrailingOptionalRemake<'a>,
        earlier_sources: &[(Makefile, bool)],
    ) -> Option<Box<RemakeChild<'a>>> {
        if trailing.kind != TrailingOptionalKind::Declared || self.parallel_jobs.len() != 1 {
            return None;
        }
        let name = trailing.neighbor;
        let job = self.parallel_jobs.primary.get(name)?;
        let before = job.before?;
        let owner = self.file_traversals.visit(name)?.parent.as_ref()?;
        // The first required visit owns this worker even when its file is
        // also the final optional read in the pass.
        (job.child.is_some()
            && !job.recursive
            && job.continuation.branch.is_none()
            && job.continuation.expanded.is_empty()
            && job
                .continuation
                .remake
                .as_ref()
                .is_some_and(|remake| !remake.optional)
            && earlier_sources.iter().any(|(file, skip)| {
                !*skip && !file.optional && file.location.is_some() && file.name == owner.target
            }))
        .then(|| {
            Box::new(RemakeChild {
                name,
                identity: job.identity,
                before,
            })
        })
    }

    fn pending_explicit_remake_recipe(&self, target: &[u8]) -> bool {
        let Some(job) = self.parallel_jobs.primary.get(target) else {
            return false;
        };
        let Some(ordinal) = job.continuation.branch else {
            return false;
        };
        job.child.is_some()
            && !job.recursive
            && job.continuation.expanded.is_empty()
            && self.pending_double_colon.get(target).is_some_and(|state| {
                state.depth == 0 && state.active == Some(ordinal) && state.before.is_some()
            })
            && self
                .database
                .rules
                .get(target)
                .and_then(|rule| rule.double_colon.as_ref())
                .and_then(|declarations| declarations.get(ordinal))
                .is_some_and(|branch| {
                    branch.recipes.is_some()
                        && branch.grouped_targets().is_none()
                        && branch.implicit_pattern.is_none()
                        && !branch.default_recipe
                })
    }

    fn qualify_synthetic_recipe_after_optional_read(&mut self, target: &[u8]) -> bool {
        let Some(job) = self.parallel_jobs.primary.get(target) else {
            return false;
        };
        let Some(ordinal) = job.continuation.branch else {
            return false;
        };
        if job.child.is_some()
            && job.continuation.expanded.is_empty()
            && self.database.rules.get(target).is_some_and(|rule| {
                rule.double_colon
                    .as_ref()
                    .is_some_and(|branches| branches.len() == ordinal)
            })
            && let Some(state) = self.pending_double_colon.get_mut(target)
            && state.depth == 0
            && state.active == Some(ordinal)
            && let Some(remake) = state.remake.as_mut()
        {
            // The final synthetic command stayed pending through the optional
            // visit. Either child can now finish first.
            remake.use_declared_synthetic_head = true;
            return true;
        }
        false
    }

    fn required_declared_tail_recipe(&self, target: &[u8], ordinal: Option<usize>) -> bool {
        let Some(ordinal) = ordinal.filter(|&ordinal| ordinal != 0) else {
            return false;
        };
        if !self.options.keep_going
            || MakefileModes::from_options(&self.options).active()
            || self.running_groups.contains_key(target)
        {
            return false;
        }
        let Some(state) = self.pending_double_colon.get(target).filter(|state| {
            state.depth == 0
                && state.active == Some(ordinal)
                && state.before.is_some()
                && !state.phony
                && !state.combined.fatal
                && state
                    .remake
                    .as_ref()
                    .is_some_and(|context| context.use_declared_tail_head)
        }) else {
            return false;
        };
        let Some(declarations) = self
            .database
            .rules
            .get(target)
            .and_then(|rule| rule.double_colon.as_ref())
        else {
            return false;
        };
        if ordinal == declarations.len() {
            // The same read head owns the verdict when the sole assumed
            // declaration also runs and fails after the explicit tail.
            return state.branches.is_empty()
                && matches!(
                    state.combined.remake_failure,
                    RemakeFailure::RequiredDeclaredTailRecipe
                );
        }
        let synthetic_follows = state.branches.len() == 1
            && state
                .branches
                .front()
                .is_some_and(|(next, branch)| *next == ordinal + 1 && branch.is_none())
            && self
                .assumed_new_double_colon_entries(target)
                .is_some_and(|(_, count)| count == 1);
        ordinal + 1 == declarations.len()
            && (state.branches.is_empty() || synthetic_follows)
            && state.combined.time == state.before
            && !state.combined.ran
            && state.combined.terminal_verdict().is_none()
            && declarations.iter().all(|branch| {
                branch.grouped_targets().is_none()
                    && branch.implicit_pattern.is_none()
                    && !branch.default_recipe
                    && branch.recipes.is_some()
            })
    }

    fn required_synthetic_recipe(&self, target: &[u8], ordinal: Option<usize>) -> bool {
        let Some(ordinal) = ordinal else {
            return false;
        };
        self.parallel_makefile_remakes
            && self.options.keep_going
            && self.options.no_rules
            && !MakefileModes::from_options(&self.options).active()
            && !self.running_groups.contains_key(target)
            && !self.database.rules.contains_key(b".DEFAULT".as_slice())
            && self.pending_double_colon.get(target).is_some_and(|state| {
                state.depth == 0
                    && state.active == Some(ordinal)
                    && state.before.is_some()
                    && state.branches.is_empty()
                    && state.remake.as_ref().is_some_and(|context| {
                        context.use_declared_synthetic_head && !context.optional
                    })
                    && state.combined.terminal_verdict().is_none()
                    && !state.combined.fatal
            })
            && self
                .assumed_new_double_colon_entries(target)
                .is_some_and(|(_, count)| count == 1)
            && self.database.rules.get(target).is_some_and(|rule| {
                rule.double_colon.as_ref().is_some_and(|branches| {
                    branches.len() == ordinal
                        && branches.iter().all(|branch| {
                            branch.grouped_targets().is_none()
                                && branch.implicit_pattern.is_none()
                                && !branch.default_recipe
                                && branch.recipes.is_some()
                        })
                })
            })
    }

    fn settled_serial_source_declarations(
        &self,
        name: &[u8],
        declarations: usize,
    ) -> Option<(bool, bool)> {
        let mut key = (name.to_vec(), 0);
        let mut succeeded = true;
        let mut refreshed = false;
        for ordinal in 0..declarations {
            key.1 = ordinal;
            let branch = self.database_branch_outcomes.get(&key)?;
            if branch.fatal || branch.question_outdated {
                return None;
            }
            succeeded &= !branch.failed;
            refreshed |= branch.completion == CompletionTime::Refresh;
        }
        Some((succeeded, refreshed))
    }

    fn settled_assumed_new_makefile_clock(
        &self,
        source: &[u8],
        allow_stopping_repeats: bool,
        expected_physical_time: Option<SystemTime>,
    ) -> bool {
        let target = self.aliases.resolve(source);
        if self.assumed_new_double_colon_entries(target).is_none() {
            return false;
        }
        // Synthetic entries can settle on physical timestamps even when
        // their recipes did not need to run.
        self.settled_assumed_new_makefile_branches_match(
            target,
            allow_stopping_repeats,
            |assumed_time, branch| {
                !matches!(
                    branch.completion,
                    CompletionTime::AssumeNew | CompletionTime::AssumeNewPhony
                ) && branch.time.is_some_and(|time| {
                    time != assumed_time
                        && expected_physical_time.is_none_or(|before| time == before)
                })
            },
        )
    }

    fn assumed_new_makefile_branches(
        &self,
        target: &[u8],
        allow_stopping_repeats: bool,
    ) -> Option<(SystemTime, std::ops::Range<usize>)> {
        let (assumed_time, count) =
            self.assumed_new_double_colon_entries(target).or_else(|| {
                // A failed pass can still use the completed singleton NEW
                // entry when an OLD operand also marked this source.
                self.single_active_new_double_colon_time(target)
                    .map(|time| (time, 1))
            })?;
        if count > 1 && !self.options.keep_going && !allow_stopping_repeats {
            return None;
        }
        let declared = self
            .database
            .rules
            .get(target)?
            .double_colon
            .as_ref()?
            .len();
        Some((assumed_time, declared..declared + count))
    }

    fn settled_assumed_new_makefile_branches_match(
        &self,
        target: &[u8],
        allow_stopping_repeats: bool,
        qualifies: impl Fn(SystemTime, &Outcome) -> bool,
    ) -> bool {
        self.assumed_new_makefile_branches(target, allow_stopping_repeats)
            .is_some_and(|(assumed_time, mut ordinals)| {
                let mut key = (target.to_vec(), 0);
                ordinals.all(|ordinal| {
                    key.1 = ordinal;
                    self.database_branch_outcomes
                        .get(&key)
                        .is_some_and(|branch| {
                            branch.terminal_verdict().is_none()
                                && !branch.fatal
                                && qualifies(assumed_time, branch)
                        })
                })
            })
    }

    fn settled_parallel_assumed_source_has_new_clock(
        &self,
        read_files: &[(Makefile, bool)],
        visited: &MakefileVisits<'_>,
        named_sources: Option<&HashSet<&[u8]>>,
        ordinary_visits: bool,
    ) -> bool {
        // A source can settle on the second visit after its order-only
        // prerequisites finish. The synthetic entry retains the visit clock
        // even when settling the root refreshes its physical time.
        read_files.iter().any(|(source, skip)| {
            if *skip || source.missing {
                return false;
            }
            let name = source.name.as_slice();
            let target = self.aliases.resolve(name);
            let Some(before) = visited.get(name).and_then(|(before, _)| *before) else {
                return false;
            };
            let Some((assumed_time, 1)) = self.assumed_new_double_colon_entries(target) else {
                return false;
            };
            let Some(outcome) = self.file_traversals.outcome(target).filter(|outcome| {
                outcome.terminal_verdict().is_none()
                    && !outcome.fatal
                    && outcome.time.is_some_and(|time| time != assumed_time)
            }) else {
                return false;
            };
            match outcome.completion {
                CompletionTime::Unchanged | CompletionTime::Preserve => {
                    (ordinary_visits
                        || self.is_makefile_goal(
                            rmake_core::filenames::without_dot_prefix(name),
                            named_sources,
                        ))
                        && self.settled_assumed_new_makefile_clock(target, false, Some(before))
                }
                CompletionTime::Refresh if ordinary_visits => {
                    // The synthetic recipe can restore the real read time
                    // without writing the source. Its own result owns that clock.
                    self.settled_assumed_new_makefile_branches_match(
                        target,
                        false,
                        |assumed_time, branch| {
                            branch.time == Some(before)
                                && before != assumed_time
                                && (branch.completion == CompletionTime::Refresh
                                    || (!branch.ran
                                        && !branch.blocked
                                        && branch.completion == CompletionTime::Unchanged
                                        && !MakefileModes::from_options(&self.options).active()
                                        && self.successful_explicit_source_refresh(target, before)))
                        },
                    )
                }
                _ => false,
            }
        })
    }

    fn successful_explicit_source_refresh(&self, target: &[u8], clock: SystemTime) -> bool {
        let Some(declarations) = self
            .database
            .rules
            .get(target)
            .and_then(|rule| rule.double_colon.as_ref())
        else {
            return false;
        };
        // A completed explicit recipe can refresh the shared clock before
        // the current synthetic declaration propagates it to the chain.
        let mut key = (target.to_vec(), 0);
        let mut refreshed = false;
        for (ordinal, rule) in declarations.iter().enumerate() {
            if rule.recipes.is_none()
                || rule.implicit_pattern.is_some()
                || rule.default_recipe
                || rule.grouped_targets().is_some()
            {
                return false;
            }
            key.1 = ordinal;
            let Some(branch) = self.database_branch_outcomes.get(&key) else {
                return false;
            };
            if branch.fatal || branch.blocked || branch.terminal_verdict().is_some() {
                return false;
            }
            refreshed |= branch.ran
                && branch.completion == CompletionTime::Refresh
                && branch.database_result == DatabaseResult::Outcome
                && branch.time == Some(clock);
        }
        refreshed
    }

    fn settled_grouped_double_peer_time(&self, name: &[u8]) -> Option<SystemTime> {
        let [branch] = self.database.rules.get(name)?.double_colon.as_deref()? else {
            return None;
        };
        if branch.grouped_double_id().is_none() || branch.implicit_pattern.is_some() {
            return None;
        }
        let sampled = self.database_branch_outcomes.get(&(name.to_vec(), 0))?;
        // The cached file visit may observe a different clock than the group did.
        if sampled.database_result != DatabaseResult::ObservedGroup
            || sampled.completion != CompletionTime::Refresh
            || !sampled.ran
            || sampled.fatal
            || sampled.terminal_verdict().is_some()
        {
            return None;
        }
        sampled.time
    }

    fn follows_forced_named_dry_remake(
        &self,
        read_files: &[(Makefile, bool)],
        source: &Makefile,
        named_sources: Option<&HashSet<&[u8]>>,
    ) -> bool {
        use rmake_core::filenames::without_dot_prefix;

        let [(primary, false), (earlier, false), (later, false)] = read_files else {
            return false;
        };
        let earlier_name = earlier.name.as_slice();
        std::ptr::eq(later, source)
            && source.location.is_some()
            && !primary.optional
            && !primary.missing
            && primary.location.is_none()
            && !earlier.optional
            && !earlier.missing
            && earlier.location.is_some()
            && self.options.goals.len() == 1
            && !self.options.check_symlink_times
            && self.no_remake_recipe_fallback()
            && !self
                .database
                .rules
                .contains_key(without_dot_prefix(&primary.name))
            && self.is_makefile_goal(without_dot_prefix(earlier_name), named_sources)
            && self.aliases.resolve(earlier_name) == earlier_name
            && self.database.rules.get(earlier_name).is_some_and(|rule| {
                rule.double_colon.is_none()
                    && rule.grouped_targets().is_none()
                    && rule.implicit_pattern.is_none()
                    && !rule.default_recipe
                    && rule.recipes.is_some()
            })
            && self
                .file_traversals
                .outcome(earlier_name)
                .is_some_and(|outcome| {
                    // An ordinary simulated cleaner cannot replace the loaded clock.
                    outcome.ran
                        && outcome.completion == CompletionTime::Refresh
                        && outcome.database_result == DatabaseResult::Outcome
                        && !outcome.blocked
                        && !outcome.fatal
                        && outcome.terminal_verdict().is_none()
                })
    }

    fn removed_required_dry_declaration_count(
        &self,
        read_files: &[(Makefile, bool)],
        source: &Makefile,
    ) -> Option<usize> {
        let [(primary, false), (earlier, false), (named, false)] = read_files else {
            return None;
        };
        let name = source.name.as_slice();
        if !std::ptr::eq(named, source)
            || source.optional
            || source.missing
            || source.location.is_none()
            || primary.optional
            || primary.missing
            || primary.location.is_some()
            || earlier.optional
            || earlier.missing
            || earlier.location.is_none()
            || self.options.goals.len() != 1
            || self.options.check_symlink_times
            || self.assumptions.is_some()
            || !self.no_remake_recipe_fallback()
            || self.database.rules.contains_key(&primary.name)
            || self.aliases.resolve(name) != name
            || self.file_traversals.has_outcome(name)
            || !matches!(
                self.path_observations.get(name),
                Some(Some((None, Some(_))))
            )
            || !self
                .file_traversals
                .outcome(self.aliases.resolve(&earlier.name))
                .is_some_and(|outcome| {
                    outcome.ran
                        && outcome.completion == CompletionTime::Refresh
                        && outcome.database_result == DatabaseResult::Outcome
                        && !outcome.fatal
                        && outcome.terminal_verdict().is_none()
                })
        {
            return None;
        }
        self.explicit_named_dry_declaration_count(name)
    }

    fn present_required_dry_declaration_count(
        &self,
        read_files: &[(Makefile, bool)],
        source: &Makefile,
        before: SystemTime,
    ) -> Option<usize> {
        let [(primary, false), (named, false)] = read_files else {
            return None;
        };
        let name = source.name.as_slice();
        if !std::ptr::eq(named, source)
            || source.optional
            || source.missing
            || source.location.is_none()
            || primary.optional
            || primary.missing
            || primary.location.is_some()
            || self.options.goals.len() != 1
            || self.options.check_symlink_times
            || self.assumptions.is_some()
            || !self.no_remake_recipe_fallback()
            || self.database.rules.contains_key(&primary.name)
            || self.aliases.resolve(name) != name
            || self.file_traversals.has_outcome(name)
            || !matches!(
                self.path_observations.get(name),
                Some(Some((None, Some(time)))) if *time == before
            )
        {
            return None;
        }
        self.explicit_named_dry_declaration_count(name)
    }

    fn explicit_named_dry_declaration_count(&self, name: &[u8]) -> Option<usize> {
        let branches = self.database.rules.get(name)?.double_colon.as_deref()?;
        (branches.len() > 1
            && branches.iter().all(|rule| {
                rule.grouped_targets().is_none()
                    && rule.implicit_pattern.is_none()
                    && !rule.default_recipe
                    && rule.recipes.as_ref().is_some_and(|recipes| {
                        recipes.iter().all(|recipe| !recipe_is_recursive(recipe))
                    })
            }))
        .then_some(branches.len())
    }

    fn settled_named_dry_remake_has_new_clock(
        &self,
        read_files: &[(Makefile, bool)],
        visited: &MakefileVisits<'_>,
        named_sources: Option<&HashSet<&[u8]>>,
    ) -> bool {
        // A failed remake pass compares the logical clock of each successful
        // read source, including a named source that completed in dry-run.
        read_files.iter().any(|(source, skip)| {
            let name = source.name.as_slice();
            let target = self.aliases.resolve(name);
            let Some(before) = visited.get(name).and_then(|(before, _)| *before) else {
                return false;
            };
            !*skip
                && !source.optional
                && !source.missing
                && source.location.is_some()
                && self.is_makefile_goal(
                    rmake_core::filenames::without_dot_prefix(name),
                    named_sources,
                )
                && self.file_traversals.outcome(target).is_some_and(|outcome| {
                    outcome.terminal_verdict().is_none()
                        && !outcome.fatal
                        && outcome.time == Some(before)
                        && outcome.completion == CompletionTime::AssumeNew
                        && outcome.database_result == DatabaseResult::Simulated
                })
                && self.database.rules.get(target).is_some_and(|rule| {
                    let rule = match rule.double_colon.as_deref() {
                        None => rule,
                        Some([branch]) => branch,
                        Some(_) => return false,
                    };
                    rule.group.is_none()
                        && rule.implicit_pattern.is_none()
                        && !rule.default_recipe
                        && rule.recipes.as_ref().is_some_and(|recipes| {
                            recipes.iter().all(|recipe| !recipe_is_recursive(recipe))
                        })
                })
        })
    }

    fn failed_remake_has_updated_assumed_source(
        &self,
        read_files: &[(Makefile, bool)],
        visited: &MakefileVisits<'_>,
        named_sources: Option<&HashSet<&[u8]>>,
        completion: DatabaseResult,
        failed_source_optional: bool,
    ) -> bool {
        let failed = read_files.iter().find_map(|(source, skip)| {
            let name = source.name.as_slice();
            let target = self.aliases.resolve(name);
            (!*skip
                && source.optional == failed_source_optional
                && (source.optional || !source.missing)
                && visited.contains_key(name)
                && self.file_traversals.outcome(target).is_some_and(|outcome| {
                    outcome.failed && !outcome.fatal && !outcome.question_outdated
                }))
            .then_some(target)
        });
        let Some(failed) = failed else {
            return false;
        };

        // The failed remake pass compares successful source heads. A
        // synthetic entry can leave its newest time on the head. A settled
        // physical touch keeps that result if a later source restores its age.
        let succeeded = |outcome: &Outcome| outcome.terminal_verdict().is_none() && !outcome.fatal;
        read_files.iter().any(|(source, skip)| {
            let name = source.name.as_slice();
            let target = self.aliases.resolve(name);
            !*skip
                && !source.missing
                && target != failed
                && self.is_makefile_goal(
                    rmake_core::filenames::without_dot_prefix(name),
                    named_sources,
                )
                && self.settled_assumed_new_makefile_branches_match(target, false, |_, branch| {
                    branch.database_result == completion
                        && branch.completion == CompletionTime::AssumeNew
                })
                && self.file_traversals.outcome(target).is_some_and(succeeded)
                && self
                    .database_branch_outcomes
                    .get(&(target.to_vec(), 0))
                    .is_some_and(succeeded)
                && visited
                    .get(name)
                    .and_then(|(before, _)| *before)
                    .is_some_and(|before| {
                        completion == DatabaseResult::PhysicalTouch
                            || self.modified(target) == Some(before)
                    })
        })
    }

    fn resume_double_colon_state(&mut self, target: &[u8], mut state: DoubleColonState) -> Outcome {
        let fresh_parallel_remake_source = state.last_epoch.is_none()
            && state.before.is_some()
            && !state.phony
            && self.remaking
            && self.active_makefile_remake.is_some()
            && self.parallel_remake_debug_declarations(target);
        let trace_initial_synthetic = state.last_epoch.is_none()
            && state.depth == 0
            && state
                .remake
                .as_ref()
                .is_some_and(|context| context.trace_initial_synthetic);
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
                    if let Some(branch) = self.file_traversals.outcome(target).copied() {
                        merge_child(&mut outcome, branch);
                    }
                    self.pending_double_colon.insert(target.to_vec(), state);
                    return outcome;
                }
                let result = self
                    .file_traversals
                    .remove_outcome(target)
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
                if matches!(
                    result.remake_failure,
                    RemakeFailure::RequiredSyntheticRecipe
                        | RemakeFailure::RequiredDeclaredTailRecipe
                ) {
                    state.combined.remake_failure = result.remake_failure;
                }
                state.combined.time = assumed_time.filter(|_| !result.ran).or(result.time);
                state.combined.ran |= result.ran;
                state.combined.failed |= result.failed;
                state.combined.fatal |= result.fatal;
                state.combined.blocked |= result.blocked;
                state.combined.question_outdated |= result.question_outdated;
                state.combined.late_input_failed |= result.late_input_failed;
                state.combined.search_after_update |= result.search_after_update;
                state.combined.remake_group_peer_updated |= result.remake_group_peer_updated;
                state.combined.completion = state.combined.completion.max(result.completion);
                if result.database_result.has_simulated_time()
                    || result.database_result == DatabaseResult::ObservedDoubleColon
                {
                    state.combined.database_result = result.database_result;
                }
                if !group_observed
                    && (result.database_result.has_simulated_time()
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
                        && state.combined.database_result.has_simulated_time())
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
                state.combined.time = if state.phony && !state.assumed_phony_input {
                    None
                } else if state.assumed_phony_input
                    || state.combined.completion.preserves()
                    || state.combined.failed
                    || (!state.combined.ran && self.resolved_names.contains_key(target))
                {
                    // Current VPATH branches share the timestamp from their
                    // selected path. A rebuild discards that path before running.
                    state.combined.time
                } else {
                    self.modified(target)
                };
                state.combined.debug_time_changed =
                    self.output.debug_mask().has(crate::debug::Mask::BASIC)
                        && self.debug_timestamp_changed(
                            state
                                .synthetic_tail
                                .as_ref()
                                .and_then(|tail| tail.assumed_time)
                                .or(state.before),
                            state.combined,
                        );
                if let Some(tail) = state.synthetic_tail {
                    if !state.combined.failed {
                        if tail.assumed_time.is_some() {
                            self.forget_cached_double_colon_input(target);
                        }
                    } else if self.options.keep_going && !state.combined.fatal {
                        let mut head = tail.head;
                        head.time = state.combined.time;
                        head.search_after_update = false;
                        self.file_traversals.set_outcome(target, head);
                        self.remember_failed_double_colon_input(target);
                        state.combined.late_input_failed = true;
                        return state.combined;
                    }
                }
                self.file_traversals.set_outcome(target, state.combined);
                return state.combined;
            };
            let declared = entry.is_some();
            let assumed_time = state
                .synthetic_tail
                .as_ref()
                .and_then(|tail| tail.assumed_time);
            let implicit_branch = (ordinal != 0
                && !self.remaking
                && self.active_makefile_remake.is_none()
                && assumed_time.is_none())
            .then_some(ordinal);
            if !declared
                && ((!self.remaking && ordinal != 0 && assumed_time.is_none())
                    || (trace_initial_synthetic && ordinal == 1))
            {
                // A fresh synthetic branch announces its visit before
                // implicit search, even when no recipe can be found.
                crate::debug::emit(
                    &self.output,
                    crate::debug::Event::Considering(
                        target,
                        state.depth + usize::from(self.remaking),
                    ),
                );
            }
            let mut branch = if let Some(branch) = entry {
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
                match self.synthetic_double_colon_recipe(target, state.depth, implicit_branch) {
                    Ok(Some(rule)) => rule,
                    Ok(None) => {
                        if self.parallel_makefile_remakes
                            && state.depth == 0
                            && self.options.keep_going
                            && state.before.is_some()
                            && state.remake.as_ref().is_some_and(|context| {
                                !context.optional && context.open_error.is_none()
                            })
                            && state.combined.terminal_verdict().is_none()
                            && !state.combined.fatal
                            && self
                                .assumed_new_double_colon_entries(target)
                                .is_some_and(|(_, count)| count == 1)
                            && self.no_remake_recipe_fallback()
                            && self.database.rules.get(target).is_some_and(|rule| {
                                rule.double_colon.as_ref().is_some_and(|branches| {
                                    branches.iter().all(|branch| {
                                        branch.grouped_targets().is_none()
                                            && branch.implicit_pattern.is_none()
                                            && !branch.default_recipe
                                            && branch.recipes.is_some()
                                    })
                                })
                            })
                        {
                            state.combined.remake_failure = RemakeFailure::RequiredSyntheticInput;
                        }
                        self.fail_double_colon_input(
                            target,
                            state.depth + usize::from(self.remaking),
                            &mut state.combined,
                        );
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
                    self.mark_remade_group_peer(target, state.phony, &mut result);
                    if serial_remake_root && result.remake_group_peer_updated {
                        use crate::debug::{Event, emit};

                        // The root announced the first declaration; later
                        // completed declarations have their own cached visit.
                        let depth = state.depth + 1;
                        if ordinal != 0 {
                            emit(&self.output, Event::Considering(target, depth));
                        }
                        emit(&self.output, Event::Already(target, depth));
                    }
                    state.combined.retain_branch_verdict(result);
                    merge_child(&mut state.combined, result);
                    // This cached declaration is a completed read-peer occurrence,
                    // independently of prerequisite outcomes or physical timestamps.
                    if result.remake_group_peer_updated {
                        state.combined.remake_group_peer_updated = true;
                        state.combined.completion =
                            state.combined.completion.max(result.completion);
                    }
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
                && declared
                && (!self.remaking || serial_remake_root || fresh_parallel_remake_source)
                && assumed_time.is_none()
            {
                let depth = state.depth + usize::from(self.remaking);
                crate::debug::emit(
                    &self.output,
                    crate::debug::Event::Considering(target, depth),
                );
                if branch_before.is_none() {
                    crate::debug::emit(
                        &self.output,
                        crate::debug::Event::Missing(target, depth + 1),
                    );
                }
            }
            // A fresh assumed-new entry is appended after declarations
            // have been marked phony. Its recipe and dependents use the
            // synthetic entry's own timestamp.
            let synthetic_time = if state.phony
                && !declared
                && implicit_branch.is_some()
                && (branch.implicit_pattern.is_some() || branch.default_recipe)
                && branch.group.is_none()
            {
                self.assumed_new_double_colon_time(target)
            } else {
                None
            };
            state.assumed_phony_input = synthetic_time.is_some();
            let branch_phony = state.phony && !state.assumed_phony_input;
            let branch_before = synthetic_time.or(branch_before);
            let before = assumed_time.or(branch_before);
            let comparison = self.comparison_time(target, before);
            if declared
                && assumed_time.is_none()
                && !self.remaking
                && self.active_makefile_remake.is_none()
                && !state.phony
                && branch.recipes.is_none()
                && branch.group.is_none()
                && !self.shuffle.enabled()
                && !self.terminal_prerequisites.contains(target)
                && self
                    .database_branch_implicit
                    .insert((target.to_vec(), ordinal))
            {
                let Ok(plan) =
                    self.plan_implicit_update(target, state.depth, false, implicit_branch)
                else {
                    state.combined.failed = true;
                    state.combined.fatal = true;
                    state.branches.clear();
                    continue;
                };
                // Each declared branch retains its own prerequisites and
                // resolves a pattern recipe before visiting them.
                if let Some(plan) = plan
                    && let Some(mut selected) = self.install_declared_implicit_plan(plan)
                {
                    if let Some(history) = self.database.database_history()
                        && let Some(stem) = &selected.stem
                    {
                        history.intern(stem);
                    }
                    selected.append_prerequisites(&branch);
                    selected.prerequisite_location = branch.prerequisite_location;
                    if let Some(stored) = self
                        .database
                        .rules
                        .get_mut(target)
                        .and_then(|root| root.double_colon.as_mut())
                        .and_then(|branches| branches.get_mut(ordinal))
                    {
                        *stored = selected.clone();
                    }
                    branch = selected;
                }
            }
            let visit = |engine: &mut Self| {
                engine.build_rule(
                    target,
                    state.depth,
                    branch_before,
                    comparison,
                    branch_phony,
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
            self.file_traversals.set_outcome(target, result);
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
                query_skipped_dependencies: HashSet::new(),
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
            self.file_traversals
                .outcome(target)
                .copied()
                .unwrap_or(outcome)
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
        let mut question_in_scan = !group_frontier && state.outcome.question_outdated;
        let previous_query_skips = if group_frontier {
            state.next = 0;
            state.replay_cached_after_recipe |= replay_debug && !self.remaking;
            // A restarted scan reaches its query verdict in traversal order.
            // An earlier barrier can now admit prerequisites it skipped before.
            state
                .group
                .as_mut()
                .map(|group| std::mem::take(&mut group.query_skipped_dependencies))
        } else {
            None
        };
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
                && !previous_query_skips
                    .as_ref()
                    .is_some_and(|skipped| skipped.contains(&index))
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
            question_in_scan |= child.question_outdated;
            let deferred = child.deferred_spawn_failure && self.optional_remake();
            if question_in_scan
                && !child.fatal
                && !self.forced_query_stop
                && !self.options.keep_going
                && let Some(group) = state.group.as_mut()
                && group.ordinary_explicit()
            {
                // A query verdict ends this member's scan. A resumed frontier
                // can already have later members' prerequisites in the rule.
                let next_segment = group
                    .peer_segments
                    .partition_point(|segment| segment.start < state.next);
                let end = group
                    .peer_segments
                    .get(next_segment)
                    .map_or(state.rule.dependencies.len(), |segment| segment.start);
                group.query_skipped_dependencies.extend(
                    (state.next..end)
                        .map(|position| state.order.get(position).copied().unwrap_or(position)),
                );
                state.next = end;
            } else if child.fatal
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
                        .file_traversals
                        .outcome(name)
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
                let aborted_pending_recipe = self.graph_abort.is_some()
                    && (self.failed_dispatch.is_some()
                        || (self.remaking
                            && self.parallel_makefile_remakes
                            && !self.optional_remake()));
                if !aborted_pending_recipe {
                    crate::debug::emit(
                        &self.output,
                        crate::debug::Event::PrerequisitesRunning(target, debug_depth),
                    );
                }
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
        if state
            .group
            .as_ref()
            .is_some_and(|group| group.query_skipped_dependencies.contains(&index))
        {
            return (Outcome::default(), false);
        }
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
                let outcome = self
                    .file_traversals
                    .outcome(resolved)
                    .copied()
                    .unwrap_or_default();
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
                if !self.file_traversals.has_outcome(dependency) {
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
                && !self.file_traversals.has_outcome(dependency);
            let serial_observation = observe_change && !self.parallel_graph();
            let unknown_archive = self
                .file_traversals
                .outcome(dependency)
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
                let outcome = self.file_traversals.outcome(dependency).copied();
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
                    let include_question = outcome.question_outdated
                        && !self.forced_query_stop
                        && state
                            .group
                            .as_ref()
                            .is_some_and(|group| group.ordinary_explicit());
                    self.debug_cached_prerequisite_visit_with_question(
                        dependency,
                        state.depth + usize::from(self.remaking) + 1,
                        outcome,
                        include_question,
                    );
                }
                outcome.unwrap_or_default()
            };
            let pending = self.pending(dependency);
            let child = if pending {
                child
            } else if child.database_result == DatabaseResult::ContinuedAfterChildFailure
                && self.options.keep_going
                && !self.remaking
                && !self.parallel_graph()
                && state.branch.is_none()
                && state.group.is_none()
                && self.assumptions.is_none()
                && !dependency.starts_with(b"-l")
            {
                self.observe_failed_prerequisite_time(dependency, child)
            } else {
                self.observe_updated(dependency, child)
            };
            if observe_change
                && !pending
                && child.debug_time_changed
                && (serial_observation
                    || (self.parallel_jobs.is_empty() && self.debug_has_logical_time(child)))
            {
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

    fn observe_failed_prerequisite_time(&mut self, dependency: &[u8], child: Outcome) -> Outcome {
        let name = self.aliases.resolve(dependency);
        if !self.database_unchecked_times.get_mut().contains(name) {
            return child;
        }
        let alias = self.aliases.contains_source_or_destination(dependency);
        let time = if !alias && self.vpath_dirs.is_empty() && !self.database.has_vpaths() {
            self.name_time(name)
        } else if alias
            && self.gpath_dirs.is_empty()
            && !self.options.check_symlink_times
            && let Some(time) = modified(name)
        {
            // The retained VPATH name still exists, so its own timestamp
            // answers the query without a new search or file rename.
            self.retain_database_time(name, Some(time));
            Some(time)
        } else {
            return child;
        };
        self.database_unchecked_times.get_mut().remove(name);
        if let Some(cached) = self.file_traversals.outcome_mut(name) {
            cached.time = time;
        }
        Outcome { time, ..child }
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

    fn debug_has_logical_time(&self, outcome: Outcome) -> bool {
        // The pure dry-run path already records which grouped outcomes
        // preserve a physical timestamp instead of simulating an update.
        !self.remaking
            && outcome.completion == CompletionTime::AssumeNew
            && (outcome.database_result == DatabaseResult::Simulated
                || !self.options.dry_run
                || self.options.question
                || self.options.touch)
    }

    fn debug_timestamp_changed(&self, before: Option<SystemTime>, outcome: Outcome) -> bool {
        if self.debug_has_logical_time(outcome) {
            // A simulated recipe supplies a logical newest timestamp.
            return self
                .assumptions
                .as_ref()
                .is_none_or(|files| !files.is_new_time(before));
        }
        outcome.time != before
    }

    fn finish_rule(&mut self, target: &[u8], state: RuleState) -> Outcome {
        if self.parallel_graph()
            && let Some(id) = state.rule.grouped_double_id()
        {
            // Two peers may have suspended on the same prerequisite before
            // either selected a recipe. Recheck ownership after that wait.
            if let Some(mut result) = self.completed_group_branches.get(&id).copied() {
                result.ran = false;
                self.mark_remade_group_peer(target, state.phony, &mut result);
                result.time = if state.phony {
                    None
                } else {
                    self.modified(target)
                };
                self.visiting.remove(target);
                self.file_traversals.set_outcome(target, result);
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
        let all_normal = !capture_automatic
            && !rule
                .dependencies
                .iter()
                .any(|dependency| dependency.order_only);
        let normal_names: HashSet<_> = if all_normal {
            HashSet::new()
        } else if has_hidden || !capture_automatic {
            rule.normal_prerequisites().collect()
        } else {
            normal.iter().map(Vec::as_slice).collect()
        };
        let mut newer = Vec::new();
        for (index, occurrence) in rule.dependencies.iter().enumerate() {
            let dependency = &occurrence.name;
            if dependency == b".WAIT"
                || dropped.contains(dependency)
                || group
                    .as_ref()
                    .is_some_and(|group| group.query_skipped_dependencies.contains(&index))
            {
                continue;
            }
            if !self.remake_stopped() {
                self.wait_target(dependency);
            }
            let Some(child) = self.file_traversals.outcome(dependency).copied() else {
                continue;
            };
            merge_child(&mut outcome, child);
            if matches!(
                child.database_result,
                DatabaseResult::UnfinishedUpdate | DatabaseResult::StoppedAfterChildFailure
            ) && !self.remaking
                && !self.parallel_graph()
                && !self.options.keep_going
                && branch.is_none()
                && group.is_none()
                && rmake_core::filenames::archive_parts(target).is_none()
                && rmake_core::filenames::archive_parts(dependency).is_none()
            {
                // The stop interrupts each waiting ancestor before GNU
                // would finish its file update.
                outcome.database_result = DatabaseResult::UnfinishedUpdate;
            }
            let order_only = !all_normal && !normal_names.contains(dependency.as_slice());
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
        let stopping_dependency = (outcome.question_outdated && self.forced_query_stop)
            || (outcome.blocked
                && (outcome.fatal
                    || (!self.options.keep_going
                        && !self.optional_remake()
                        && (self.graph_abort.is_some() || !self.parallel_graph()))));
        if !stopping_dependency {
            crate::debug::emit(
                &self.output,
                crate::debug::Event::Finished(target, debug_depth),
            );
        }
        // A phony target normally has no timestamp. An explicit newest
        // assumption survives a recipe-less update.
        let assumed_phony = phony
            && !self.remaking
            && branch.is_none()
            && group.is_none()
            && rule.recipes.is_none()
            && self
                .assumptions
                .as_ref()
                .is_some_and(|files| files.is_new_time(before));
        let mask = self.output.debug_mask();
        let mut natural_remake = comparison.is_none();
        let mut dependencies_changed = false;
        if mask.has(crate::debug::Mask::BASIC)
            && before.is_some()
            && (!phony || assumed_phony)
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
                let child = self.file_traversals.outcome(dependency).copied();
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
                let logical_time = child.is_some_and(|child| self.debug_has_logical_time(child));
                let state = match child {
                    _ if occurrence.order_only => Prerequisite::OrderOnly,
                    Some(child) if child.time.is_some() || logical_time => {
                        if changed
                            || child.time > comparison
                            || (child.completion == CompletionTime::AssumeNew
                                && ((!logical_time && self.options.always)
                                    || self.prerequisite_timestamp_stale(comparison, child)))
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
            && (!phony || (assumed_phony && !stale))
            && !rule.record_only
            && rule.recipes.is_none()
            && !self.options.always
            && !dependencies_changed;
        let forced_current_target = self.options.always
            && mask.has(crate::debug::Mask::BASIC)
            && !self.remaking
            && branch.is_none()
            && group.is_none()
            && before.is_some()
            && (!phony || assumed_phony)
            && !rule.record_only
            && rule.recipes.is_none()
            && !natural_remake;
        // Always-make leaves a target without a recipe current when its
        // ordinary prerequisites do not require updating it.
        let report_remake = debug_remake && !no_recipe_no_changes && !forced_current_target;
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
            if report_remake {
                crate::debug::emit(
                    &self.output,
                    crate::debug::Event::MustRemake(target, debug_depth),
                );
            }
            if before.is_none() && rule.recipes.is_none() {
                self.discard_missing_archive_name(target);
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
                        record_suffix_stem_lookup: !implicit_group
                            && (rule.stem.is_none()
                                || (branch.is_some()
                                    && rule.implicit_pattern.is_some()
                                    && rule.stem.as_deref() == Some(b""))),
                        record_expansion_failure: branch.is_none() && rule.group.is_none(),
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
                } else if self.options.question
                    && !self.options.touch
                    && !execution.ran
                    && !execution.pending
                    && rule.grouped_targets().is_none()
                    && outcome.completion == CompletionTime::AssumeNew
                    && execution.status == Status::Success
                {
                    outcome.database_result = DatabaseResult::SimulatedQuery;
                } else if execution.touch != TouchCompletion::None
                    && !self.remaking
                    && !self.parallel_graph()
                    && branch.is_none()
                    && rule.grouped_targets().is_none()
                    && execution.status == Status::Success
                    && outcome.completion == CompletionTime::AssumeNew
                {
                    // A successful ordinary touch marks the logical file as
                    // new even when it also set a physical timestamp.
                    outcome.database_result = DatabaseResult::TouchedTarget;
                } else if execution.touch != TouchCompletion::None
                    && self.remaking
                    && !self.parallel_makefile_remakes
                    && !execution.pending
                    && rule.grouped_targets().is_none()
                    && execution.status == Status::Success
                    && outcome.completion == CompletionTime::AssumeNew
                    && let Some(ordinal) = branch
                    && self
                        .assumed_new_makefile_branches(target, false)
                        .is_some_and(|(_, ordinals)| ordinals.contains(&ordinal))
                {
                    outcome.database_result = match execution.touch {
                        TouchCompletion::None => DatabaseResult::Outcome,
                        TouchCompletion::Simulated => DatabaseResult::SimulatedTouch,
                        TouchCompletion::Physical => DatabaseResult::PhysicalTouch,
                    };
                }
                if execution.simulated_completion_time {
                    outcome.database_result = DatabaseResult::SimulatedRecipeCompletion;
                } else if execution.failed_serial_child && execution.status == Status::Failure {
                    outcome.database_result = if self.options.keep_going {
                        DatabaseResult::ContinuedAfterChildFailure
                    } else {
                        DatabaseResult::StoppedAfterChildFailure
                    };
                } else if execution.expansion_failed {
                    outcome.database_result = DatabaseResult::UnfinishedRecipeExpansion;
                } else if rule.grouped_double_id().is_some()
                    && execution.status == Status::Fatal
                    && !execution.ran
                {
                    outcome.database_result = DatabaseResult::RecipeUnstarted;
                }
                outcome.search_after_update =
                    self.update_search_pending(target, phony, execution.all_recursive);
                if !phony
                    && !execution.pending
                    && execution.status == Status::Success
                    && self.defer_goal_timestamp_after_failure(target, outcome.completion)
                {
                    // Recipe completion invalidates the logical file time
                    // even if expansion or an optimized ':' leaves no child.
                    self.database_unchecked_times
                        .borrow_mut()
                        .insert(target.to_vec());
                }
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
                        .file_traversals
                        .visit(target)
                        .and_then(|visit| visit.parent.as_ref())
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
            outcome.time = if phony && !assumed_phony {
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
                && self.debug_timestamp_changed(before, outcome);
        }
        if !debug_remake
            && !prerequisite_question
            && !outcome.failed
            && !self.special(b".NOTINTERMEDIATE", target, true)
        {
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
            } else if report_remake {
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
        self.file_traversals.set_outcome(target, outcome);
        if !self.parallel_jobs.contains_key(target) {
            self.finish_recipe(target, outcome)
        } else {
            outcome
        }
    }

    fn note_group_recipe_failure(&mut self, target: &[u8], location: &Location) {
        if let Some(group) = self.running_groups.get_mut(target)
            && group.implicit
        {
            // A stopping command keeps its line for the peer warning.
            group.location = Some(location.clone());
        }
    }

    fn finish_recipe(&mut self, target: &[u8], mut outcome: Outcome) -> Outcome {
        let group = self.running_groups.remove(target);
        let recipe_modes = MakefileModes::from_options(&self.options);
        let failed_serial_recipe = outcome.failed
            && !outcome.fatal
            && !outcome.blocked
            && outcome.ran
            && outcome.completion == CompletionTime::Refresh
            && self.remaking
            && !self.parallel_makefile_remakes
            && self.options.keep_going
            && self.assumptions.is_none();
        let failed_visit = failed_serial_recipe
            .then(|| self.file_traversals.visit(target))
            .flatten();
        let failed_prerequisite =
            !recipe_modes.active() && failed_visit.is_some_and(|visit| visit.parent.is_some());
        if failed_serial_recipe && self.makefile_names.contains(target) {
            let owner_failure = match &group {
                Some(group) if group.double_colon.is_none() => {
                    if failed_prerequisite {
                        // A consuming prerequisite check settles the owner.
                        Some(RemakeFailure::SettledPrerequisiteOwner)
                    } else if !group.implicit
                        && !recipe_modes.touch
                        && !outcome.question_outdated
                        && failed_visit.is_some_and(|visit| visit.parent.is_none())
                    {
                        // Explicit groups sample their peers after a real
                        // recipe, including a forced dry-run or query recipe.
                        Some(RemakeFailure::DirectExplicitGroupOwner)
                    } else {
                        None
                    }
                }
                None if failed_prerequisite
                    && self.database.rules.get(target).is_some_and(|rule| {
                        rule.double_colon.is_none()
                            && rule.grouped_targets().is_none()
                            && rule.recipes.is_some()
                            && !rule.default_recipe
                    }) =>
                {
                    Some(RemakeFailure::SettledPrerequisiteOwner)
                }
                _ => None,
            };
            if let Some(owner_failure) = owner_failure {
                outcome.remake_failure = owner_failure;
                if let Some(owner) = self.file_traversals.outcome_mut(target) {
                    owner.remake_failure = owner_failure;
                }
            }
        }
        let Some(group) = group else {
            return outcome;
        };
        let cached = self.cached_recipe_outcome(outcome);
        // A query can inspect an expanded recipe without starting it. Only
        // its owner has a verdict in that case, so peers can still be goals.
        let unstarted_query =
            outcome.question_outdated && outcome.completion == CompletionTime::Preserve;
        if let Some((id, owner_branch)) = group.double_colon {
            self.active_group_branches.remove(&id);
            if !(group.ordinary && unstarted_query) {
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
                if let Some(owner) = self.file_traversals.outcome_mut(target) {
                    owner.database_result = DatabaseResult::ObservedDoubleColon;
                }
            }
            if !outcome.question_outdated && !outcome.fatal && (!outcome.failed || outcome.ran) {
                let optional_serial_failure = self.remaking
                    && !self.parallel_makefile_remakes
                    && !MakefileModes::from_options(&self.options).active()
                    && self.optional_remake()
                    && outcome.failed;
                for (peer, before, phony) in group.members {
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
                    if optional_serial_failure && !owner && branches.len() == 1 {
                        completed.remake_failure = completed.remake_failure.with_optional();
                    }
                    completed.database_result = DatabaseResult::ObservedGroup;
                    completed.time = if phony {
                        None
                    } else {
                        self.observed_modified(&peer, false)
                    };
                    // A failed group can still sample a changed single read peer.
                    // Keep that observation without changing owner refresh policy.
                    if self.remaking
                        && !self.parallel_makefile_remakes
                        && outcome.completion == CompletionTime::Refresh
                        && (!outcome.failed
                            || (!owner && branches.len() == 1 && completed.time.is_some()))
                        && !phony
                        && self.makefile_names.contains(&peer)
                        && let Some(time) = before
                        && completed.time != before
                    {
                        self.serial_remake_peer_times
                            .entry(peer.clone())
                            .or_insert(time);
                    }
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
            return outcome;
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
        let retain_explicit_dry_time = group.ordinary
            && !group.implicit
            && observe_dry_group
            && outcome.completion == CompletionTime::AssumeNew;
        let retain_debug_time =
            retain_explicit_dry_time && self.output.debug_mask().has(crate::debug::Mask::BASIC);
        let debug_serial_group_peer = !self.remaking
            && self.active_makefile_remake.is_none()
            && !self.parallel_graph()
            && !outcome.question_outdated
            && !outcome.fatal;
        let serial_explicit_remake =
            self.remaking && !self.parallel_makefile_remakes && !group.implicit;
        let failed_required_remake = outcome.failed
            && self.options.keep_going
            && !self.optional_remake()
            && !outcome.blocked
            && !outcome.fatal;
        let track_read_peers = serial_explicit_remake
            && outcome.ran
            && outcome.completion == CompletionTime::Refresh
            && (!outcome.failed || failed_required_remake);
        let use_saved_peer_time = track_read_peers
            && (!outcome.failed || group.before.is_none())
            && self.assumptions.is_none()
            && !self.options.check_symlink_times
            && self.makefile_names.contains(target);
        let failed_explicit_owner = matches!(
            outcome.remake_failure,
            RemakeFailure::DirectExplicitGroupOwner
        );
        let track_failed_peer_clock =
            (group.implicit && failed_prerequisite) || failed_explicit_owner;
        let trace_failed_read_peers = serial_explicit_remake
            && (self.optional_remake() || self.options.keep_going)
            && outcome.failed
            && outcome.ran
            && !outcome.fatal;
        for (peer, before, phony) in group.members.into_iter().rev() {
            let observation = (observe_dry_group && !phony && (!group.implicit || peer != target))
                .then(|| {
                    // Only explicit groups include the owner in the physical
                    // refresh. Implicit owners keep their simulated timestamp.
                    if retain_explicit_dry_time {
                        // The grouped observation replaces a newest assumption.
                        self.release_assumed_new(&peer);
                    }
                    let time = self.modified(&peer);
                    self.database_unchecked_times.borrow_mut().remove(&peer);
                    time
                });
            let database_result = if observation.is_some() {
                DatabaseResult::ObservedGroup
            } else {
                DatabaseResult::Outcome
            };
            if peer == target {
                if let Some(time) = observation
                    && let Some(owner) = self.file_traversals.outcome_mut(target)
                {
                    owner.database_result = database_result;
                    if retain_explicit_dry_time {
                        owner.completion = CompletionTime::Preserve;
                        owner.debug_time_changed |= retain_debug_time && owner.time != time;
                        owner.time = time;
                        outcome = *owner;
                    }
                }
                continue;
            }
            self.active_peers.remove(&peer);
            if unstarted_query
                && (group.ordinary
                    || (self.remaking && !group.implicit && self.makefile_names.contains(&peer)))
            {
                // A read peer has its own remake visit, which may run without
                // the named owner's question mode.
                continue;
            }
            if group.implicit && (!outcome.failed || outcome.ran) {
                // Later implicit searches can mark an updated peer intermediate.
                self.cleanup_candidates.push(peer.clone());
            }
            self.pending_rules.remove(&peer);
            let mut completed = cached;
            completed.database_result = database_result;
            completed.debug_serial_group_peer = debug_serial_group_peer
                || (((track_read_peers && !outcome.failed) || trace_failed_read_peers)
                    && self.makefile_names.contains(&peer));
            self.mark_remade_group_peer(&peer, phony, &mut completed);
            completed.search_after_update = false;
            completed.time = if phony {
                None
            } else if outcome.completion.preserves() {
                before
            } else {
                self.modified(&peer)
            };
            if retain_explicit_dry_time && let Some(time) = observation {
                completed.completion = CompletionTime::Preserve;
                completed.debug_time_changed |= retain_debug_time && completed.time != time;
                completed.time = time;
            }
            // This failed group has already sampled the peer's physical time.
            // The required read keeps that observation through later changes.
            completed.remake_failure = if track_failed_peer_clock
                && !phony
                && self.makefile_names.contains(&peer)
                && !self.file_traversals.has_outcome(&peer)
                && let Some(Some((None, Some(loaded)))) = self.path_observations.get(&peer)
            {
                let changed = completed.time.is_some_and(|time| time != *loaded);
                match (failed_explicit_owner, changed) {
                    (true, true) => RemakeFailure::ExplicitRequiredClockChanged,
                    (true, false) => RemakeFailure::ExplicitRequiredClockUnchanged,
                    (false, true) => RemakeFailure::RequiredClockChanged,
                    (false, false) => RemakeFailure::RequiredClockUnchanged,
                }
            } else {
                RemakeFailure::None
            };
            if self.optional_remake() && outcome.failed && !outcome.fatal {
                completed.remake_failure = completed.remake_failure.with_optional();
            }
            if track_read_peers && !phony && self.makefile_names.contains(&peer) {
                // A missing group member can stop peer freshness checks.
                // The serial read pass can still establish the peer's time
                // before a successful recipe changed or removed it.
                let before = before.or_else(|| {
                    if use_saved_peer_time
                        && let Some(Some((None, Some(time)))) = self.path_observations.get(&peer)
                    {
                        Some(*time)
                    } else {
                        None
                    }
                });
                if let Some(time) = before
                    && completed.time != before
                {
                    self.serial_remake_peer_times
                        .entry(peer.clone())
                        .or_insert(time);
                }
            }
            if warn_missing && completed.time.is_none() {
                let message = crate::messages::system(Message::PatternPeerNotUpdated(&peer));
                emit_output(
                    &self.output,
                    &self.program,
                    Output::Warning {
                        location: group.location.as_ref(),
                        message: &message,
                    },
                );
            }
            self.file_traversals.set_outcome(&peer, completed);
        }
        outcome
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
        let parent = self.file_traversals.visit(target).and_then(|visit| {
            visit.parent.as_ref().map(|parent| {
                let name = parent
                    .record
                    .and_then(|id| self.moved_archives.get(&id))
                    .map_or(parent.target.as_slice(), |owner| owner.lookup.as_slice());
                (name.to_vec(), parent.branch())
            })
        });
        let inherited = if let Some((parent, branch)) = parent {
            self.rule_variables(&parent, branch)?
        } else {
            self.database.variables.clone()
        };
        let variables = if let Some(scope) = self.implicit_scopes.remove(target) {
            let mut variables = self.database.variables_for_target_prepared(
                target,
                &inherited,
                scope.pattern.as_ref(),
            );
            variables.retain_runtime_metadata(&scope.runtime_metadata, true);
            variables
        } else {
            self.with_target_read_expansion(None, |database, context| {
                database.variables_for_target_with(target, &inherited, context)
            })?
        };
        self.target_contexts
            .insert(target.to_vec(), variables.clone());
        Ok(variables)
    }

    fn direct_named_head_reads(&self) -> bool {
        !self.remaking && !self.parallel_graph()
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
            let mut scope = scope.clone();
            if ordinal != 0
                && self.direct_named_head_reads()
                && !self.aliases.contains_source_or_destination(&key.0)
                && self.database.has_plain_double_colon_rules(&key.0)
                && let Some(head) = self.database_branch_contexts.get(&(key.0, 0))
            {
                // A prerequisite can update the head after this branch's
                // secondary expansion prepared its own file scope.
                scope.refresh_double_colon_head_locals(head);
            }
            return Ok(scope);
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
            record_suffix_stem_lookup,
            record_expansion_failure,
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
            let script =
                crate::oneshell::collect(recipes.iter().map(|recipe| recipe.bytes.as_slice()));
            // A make reference can cross recipe slots in the collected script.
            let bytes = if recipes.iter().any(Recipe::has_physical_continuation)
                && script.bytes.contains(&b'$')
            {
                rmake_core::reader::fold_recipe_reference_continuations(&script.bytes)
            } else {
                script.bytes
            };
            combined = [Recipe {
                bytes,
                location: first.location.clone(),
                prefix: first.prefix,
                builtin: first.builtin,
                source: RecipeSource::Ordinary,
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
        // Secondary expansion can run before the next recipe starts. Consume a
        // failed launch here, before expanding any of the new recipe's lines.
        self.complete_failed_dispatch();
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
        if record_suffix_stem_lookup && !self.database_stem_prepared(target, branch) {
            self.record_explicit_stem_lookup();
            if let Some(history) = self.database.database_history()
                && let Some((_, stem)) = automatic.iter().find(|(name, _)| *name == b"*")
                && !stem.is_empty()
            {
                history.intern(stem);
            }
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
                source: RecipeSource::Ordinary,
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
                    // A single source line can also gain recipe prefixes
                    // during expansion.
                    if source.has_physical_continuation()
                        || (bytes.contains(&b'\n') && (one_shell || !source.bytes.contains(&b'\n')))
                    {
                        rmake_core::reader::strip_recipe_continuation_prefix(
                            &mut bytes,
                            source.prefix,
                        );
                    }
                    expanded.push_back((index, bytes, recipe));
                }
                Err(err) => {
                    self.expansion_error(Some(&recipe.location), &err);
                    let mut execution = Execution::fatal(false);
                    if record_expansion_failure
                        && !self.remaking
                        && !parallel
                        && self.output.database_requested()
                        && rmake_core::filenames::archive_parts(target).is_none()
                    {
                        // Earlier expansion effects belong to the file table
                        // even when no recipe command was dispatched.
                        self.retain_database_context(target, branch, &variables);
                        execution.expansion_failed = true;
                    }
                    return execution;
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
                                    && self.file_traversals.outcome(&dependency.name).is_some_and(
                                        |child| {
                                            child.time.is_none()
                                                && (child.completion != CompletionTime::AssumeNew
                                                    || self.special(
                                                        b".PHONY",
                                                        &dependency.name,
                                                        false,
                                                    ))
                                        },
                                    )
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
                    required_pair_failure: None,
                    trailing_optional_worker: None,
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
        let mut previous_executable = None;
        let mut failed_executable_attempt = false;
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
            let mut quiet = self.special(b".SILENT", target, false) || raw_flags.quiet;
            let mut command_noerror = raw_flags.ignore_errors;
            let mut force = flow.recursive(index);
            while let Some(prefix) = command.first() {
                match prefix {
                    b'@' => quiet = true,
                    b'-' => command_noerror = true,
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
            // A live MAKEFLAGS assignment can enable warnings inside configuration.
            // Restore the original policy before the next command or target.
            let configuration = self.with_scoped_expansion(
                Some(&recipe.location),
                true,
                |database, mut context| {
                    let shell = if variables.get(b"SHELL").is_some() {
                        database.expand_in(variables, b"$(SHELL)", context.reborrow())?
                    } else {
                        Vec::new()
                    };
                    // GNU rescans raw flag text even for a simple binding.
                    let flag_text = variables.database_lookup(b".SHELLFLAGS").map_or_else(
                        Vec::new,
                        |variable| {
                            if database.posix && variable.origin() == Origin::Default {
                                if command_noerror {
                                    b"-c".to_vec()
                                } else {
                                    b"-ec".to_vec()
                                }
                            } else {
                                variable.raw_value().to_vec()
                            }
                        },
                    );
                    let flags = database.expand_in(variables, &flag_text, context.reborrow())?;
                    let ifs = if variables.database_lookup(b"IFS").is_some() {
                        Some(database.expand_in(variables, b"$(IFS)", context.reborrow())?)
                    } else {
                        None
                    };
                    Ok((shell, flags, ifs))
                },
            );
            // MAKEFLAGS effects in configuration apply to this command.
            let ignore = self.options.ignore_errors
                || command_noerror
                || self.special(b".IGNORE", target, true);
            let (shell, flags, ifs) = match configuration {
                Ok(configuration) => configuration,
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
            if command.is_empty() {
                continue;
            }
            let transformed;
            let ordinary_invocation;
            if one_shell {
                if crate::process::empty_oneshell_continuation(
                    command,
                    shell,
                    &flags,
                    ifs.as_deref(),
                ) {
                    continue;
                }
                transformed = crate::oneshell::transform(command, shell);
                command = &transformed;
                ordinary_invocation = None;
            } else {
                let line = crate::process::prepare_line(command, shell, &flags, ifs.as_deref());
                if let Some(next) = line.next {
                    expanded.push_front((index, command[next..].to_vec(), recipe.clone()));
                }
                command = &command[..line.end];
                let Some(invocation) = line.invocation else {
                    continue;
                };
                if command.is_empty() {
                    continue;
                }
                ordinary_invocation = Some(invocation);
            }
            if self.options.question && !force {
                // Touch can still satisfy all ordinary goals after a
                // queried remake continuation has stopped.
                let record_question = if self.remaking {
                    !self.options.touch
                } else {
                    !self.optional_remake()
                };
                if record_question {
                    self.would_build = true;
                }
                // Once a command has started, stopping on a later ordinary
                // line gives a mixed recipe the simulated completion time.
                let simulated_completion_time = (previous_executable.is_some()
                    || failed_executable_attempt)
                    && !phony
                    && !parallel
                    && !self.remaking
                    && remake.is_none()
                    && branch.is_none()
                    && !self.running_groups.contains_key(target)
                    && rmake_core::filenames::archive_parts(target).is_none();
                return Execution {
                    simulated_completion_time,
                    ..Execution::question(ran, flow.continuation_cleanup())
                        .with_recursion(!flow.has_ordinary())
                };
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
                || !(quiet || self.globally_silent())
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
                let selected: Vec<_> = exports
                    .iter()
                    .map(|name| (name, environment_variables.select_environment_binding(name)))
                    .collect();
                let mut emitted_order = Vec::new();
                let mut inherited_mflags = false;
                let direct_heads = self.direct_named_head_reads();
                let mut target_reads = TargetReads {
                    visits: &self.file_traversals,
                    retained_parents: &self.retained_scope_parents,
                    moved_archives: &self.moved_archives,
                    aliases: &self.aliases,
                    base: &self.target_contexts,
                    recipes: &mut self.database_contexts,
                    branches: &mut self.database_branch_contexts,
                    direct_heads,
                };
                for (name, selection) in selected {
                    self.database.refresh_scoped_variables(variables);
                    let current = variables.clone();
                    if let Some(selection) = selection
                        && let Some(variable) = current.environment_binding(selection)
                    {
                        if !export_selected_binding(
                            &self.database.variables,
                            &self.database.unexports,
                            name,
                            variable,
                            variable.export_policy(),
                            export_all,
                        ) {
                            child.env_remove(os(name));
                            continue;
                        }
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
                        match self.database.expand_selected_environment_in(
                            variables,
                            selection,
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
                            .with_shell(&|request| {
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
                    if let Some(spool) = output.as_ref().and_then(|buffer| buffer.child_spool()) {
                        // Descriptor pressure can leave room for the spool but
                        // no room to clone it or open std's spawn error pipe.
                        if stdin_owner
                            && self.jobserver.is_none()
                            && rmake_linux::child::Child::wake_pipe_unavailable()
                            && !crate::native::has_modules()
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
                        if stdin_owner
                            && self.jobserver.as_ref().is_some_and(|server| {
                                server.auth().starts_with(b"fifo:")
                                    && server.owned_slots().is_some()
                            })
                        {
                            return rmake_linux::child::Child::spawn_ordered_fifo_spool(
                                &mut command,
                                image,
                                os(arg0),
                                &spool,
                            );
                        }
                        spool.configure_child(&mut command)?;
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
                    match rmake_linux::child::Child::spawn_ordered(&mut command, image, os(arg0)) {
                        Err(error)
                            if matches!(error.raw_os_error(), Some(23 | 24))
                                && stdin_owner
                                && output.as_ref().is_none_or(|buffer| !buffer.enabled())
                                && rmake_linux::signals::pending().is_none() =>
                        {
                            let Some(server) = &self.jobserver else {
                                return Err(error);
                            };
                            rmake_linux::child::Child::try_spawn_ordered_owned_pool(
                                &command,
                                image,
                                os(arg0),
                                server,
                                force,
                            )
                            .unwrap_or(Err(error))
                        }
                        result => result,
                    }
                },
            );
            self.retain_database_context(target, branch, variables);
            let executable_launch_failed = process.as_ref().is_err_and(|error| {
                matches!(
                    error.kind(),
                    io::ErrorKind::NotFound
                        | io::ErrorKind::PermissionDenied
                        | io::ErrorKind::NotADirectory
                )
            });
            // An ordinary parallel executable lookup failure still occupies its
            // recipe's graph frame until the next dispatch or goal sweep.
            // This first attempt can coexist with registered ordinary workers.
            let failed_dispatch = parallel
                && !self.remaking
                && remake.is_none()
                && !ignore
                && !flow.retained_owner()
                && !flow.failed_launch()
                && !(self.options.dry_run || self.options.question || self.options.touch)
                && executable_launch_failed
                && self
                    .parallel_jobs
                    .values()
                    .all(|job| job.child.is_some() && job.continuation.remake.is_none());
            if failed_dispatch && let Err(error) = &process {
                let detail = rmake_linux::filesystem::source_error_detail(error);
                self.stderr(&[program.as_slice(), b": ", &detail].concat());
            }
            self.output.select(None);
            if let Ok((child, _)) = &process {
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
            if parallel && (failed_dispatch || process.is_ok()) {
                let identity = self.next_job;
                self.next_job = self.next_job.wrapping_add(1);
                if failed_dispatch {
                    debug_assert!(self.failed_dispatch.is_none());
                    self.failed_dispatch = Some((target.to_vec(), identity));
                }
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
                        child: process.ok().map(|(child, _)| child),
                        signal_cleaned: false,
                        required_pair_failure: None,
                        trailing_optional_worker: None,
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
            failed_executable_attempt |= executable_launch_failed;
            let result = match process {
                Ok((mut child, path)) => {
                    previous_executable = Some(path);
                    self.wait_serial_child(
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
                    })
                }
                Err(error) => Err(error),
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
            let failed_child = result.as_ref().is_ok_and(|status| !status.success());
            let serial_ordinary_failure = !ignore
                && (executable_launch_failed || failed_child)
                && !parallel
                && !self.remaking
                && remake.is_none()
                && branch.is_none()
                && !self.running_groups.contains_key(target)
                && rmake_core::filenames::archive_parts(target).is_none();
            let serial_recipe_failure =
                serial_ordinary_failure && (executable_launch_failed || error_code.is_some());
            let simulated_completion_time = serial_recipe_failure
                && !phony
                && flow.has_ordinary()
                && MakefileModes::from_options(&self.options).active();
            let error = match result {
                Ok(status) if status.success() => None,
                // Recursive make uses exit 1 to report an out-of-date build in
                // query mode. GNU applies that convention to every forced
                // command; ignored errors retain their ordinary handling.
                Ok(status) if self.options.question && !ignore && status.code() == Some(1) => {
                    self.forced_query_completed();
                    return Execution {
                        simulated_completion_time,
                        ..Execution::question(ran, failure_cleanup)
                            .with_recursion(!flow.has_ordinary())
                    };
                }
                Ok(status) => Some(
                    status
                        .code()
                        .map_or_else(|| signal_status(status), |code| format!("Error {code}")),
                ),
                Err(err) => {
                    let detail = rmake_linux::filesystem::source_error_detail(&err);
                    self.stderr(&[program.as_slice(), b": ", &detail].concat());
                    // GNU rechecks the last successfully spawned command
                    // when the next executable lookup fails.
                    if executable_launch_failed
                        && err.kind() == io::ErrorKind::NotFound
                        && !parallel
                        && !ignore
                        && !self.remaking
                        && remake.is_none()
                        && branch.is_none()
                        && !MakefileModes::from_options(&self.options).active()
                        && self.output.mode() != crate::output::Mode::Recurse
                        && !self.running_groups.contains_key(target)
                        && rmake_core::filenames::archive_parts(target).is_none()
                        && let Some(path) = previous_executable.as_ref()
                        && let Err(error) = std::fs::metadata(path)
                    {
                        let detail = rmake_linux::filesystem::source_error_detail(&error);
                        self.stderr(&[path.as_os_str().as_bytes(), b": ", &detail].concat());
                    }
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
                    execution.simulated_completion_time = simulated_completion_time;
                    execution.failed_serial_child = failed_child && serial_ordinary_failure;
                    if executable_launch_failed && serial_recipe_failure {
                        self.record_delete_on_error_lookup();
                        if !phony && !simulated_completion_time {
                            // A failed executable launch completes the attempt
                            // without leaving a child that can clean up targets.
                            self.database_unchecked_times
                                .borrow_mut()
                                .insert(target.to_vec());
                        }
                    }
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
        // An expanded one-shell prefix forces execution, including queued
        // script tails. A touch completion uses the declared recursion.
        let all_recursive = if one_shell && self.options.touch {
            flow.has_declared_recursion()
        } else {
            !flow.has_ordinary()
        };
        if !self.options.touch || phony || all_recursive {
            return Execution::success(ran).with_recursion(all_recursive);
        }
        let passes = flow.completion_passes();
        let mut touch = TouchCompletion::None;
        for pass in 0..passes {
            if pass + 1 == passes
                && let Some(buffer) = &output
            {
                buffer.dump();
            }
            let mut touched = self.touch_target(target);
            ran |= touched.ran;
            touch = touched.touch;
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
        Execution {
            touch,
            ..Execution::success(ran).with_recursion(all_recursive)
        }
    }

    fn touch_target(&mut self, target: &[u8]) -> Execution {
        self.commands_started = self.commands_started.wrapping_add(1);
        if !self.globally_silent() {
            self.stdout(&[b"touch ".as_slice(), target].concat());
        }
        if self.options.dry_run {
            return Execution {
                touch: TouchCompletion::Simulated,
                ..Execution::success(true)
            };
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
                let detail = rmake_linux::filesystem::source_error_detail(&error);
                self.stderr(&crate::messages::system(Message::OsError {
                    action: b"touch: open: ",
                    path: target,
                    detail: &detail,
                }));
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
        Execution {
            touch: TouchCompletion::Physical,
            ..Execution::success(true)
        }
    }

    fn archive_recipe_fatal(&self, message: &[u8]) {
        self.fatal_with_owner(None, message, true);
    }

    fn record_delete_on_error_lookup(&self) {
        if !self.database_delete_policy.replace(true)
            && let Some(history) = self.database.database_history()
        {
            history.record_file_lookup(b".DELETE_ON_ERROR");
        }
    }

    fn cleanup_target(
        &self,
        target: &[u8],
        before: Option<SystemTime>,
        phony: bool,
        cleanup: Cleanup,
    ) -> bool {
        if matches!(cleanup, Cleanup::ChangedOnError | Cleanup::ChangedOnSignal) {
            self.record_delete_on_error_lookup();
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
                self.stderr(&crate::messages::system(
                    rmake_core::messages::Message::RecipeCleanup {
                        target,
                        owner,
                        archive: true,
                    },
                ));
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
        self.stderr(&crate::messages::system(
            rmake_core::messages::Message::RecipeCleanup {
                target,
                owner,
                archive: false,
            },
        ));
        if let Err(error) = fs::remove_file(os(target))
            && error.kind() != io::ErrorKind::NotFound
        {
            let detail = rmake_linux::filesystem::source_error_detail(&error);
            self.stderr(&crate::messages::system(Message::OsError {
                action: b"unlink: ",
                path: target,
                detail: &detail,
            }));
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
    has_force_prefix(&recipe.bytes) || has_direct_make_reference(&recipe.bytes)
}

fn has_direct_make_reference(bytes: &[u8]) -> bool {
    bytes
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

/// Select the first executable non-directory in the child's PATH before
/// spawning it. An exec failure for that file is a launch failure, even when
/// the same program name is executable in a later PATH directory.
fn recipe_program(program: &[u8], template: &Command) -> io::Result<PathBuf> {
    use rmake_linux::process::executable;

    if program.contains(&b'/') {
        let path = PathBuf::from(os(program));
        executable(&path)?;
        return Ok(path);
    }

    let path = template
        .get_envs()
        .find(|(key, _)| *key == "PATH")
        .and_then(|(_, value)| value)
        .unwrap_or_else(|| OsStr::new("/bin:/usr/bin"));
    let mut failure = io::Error::from_raw_os_error(2);
    for directory in std::env::split_paths(path) {
        let local = directory.as_os_str().is_empty() || directory.as_os_str() == ".";
        // The local PATH component tests the bare name before prepending ./
        // for launch. A quoted empty argv[0] therefore finds no local file.
        if local && program.is_empty() {
            continue;
        }
        let candidate = if local {
            std::path::Path::new(".").join(os(program))
        } else {
            directory.join(os(program))
        };
        match executable(&candidate) {
            Ok(()) => return Ok(candidate),
            Err(error) if error.raw_os_error() != Some(2) => failure = error,
            Err(_) => {}
        }
    }
    Err(failure)
}

fn launch_child<T>(
    program: &[u8],
    template: &Command,
    order: &[Vec<u8>],
    original_shell: Option<&[u8]>,
    run: impl FnMut(Command, &rmake_linux::environment::Image, &[u8]) -> io::Result<T>,
) -> io::Result<(T, PathBuf)> {
    let environment =
        rmake_linux::environment::Image::from_command(template, order, original_shell)?;
    launch_child_with_environment(program, template, &environment, run)
}

pub(crate) fn launch_child_with_environment<T>(
    program: &[u8],
    template: &Command,
    environment: &rmake_linux::environment::Image,
    mut run: impl FnMut(Command, &rmake_linux::environment::Image, &[u8]) -> io::Result<T>,
) -> io::Result<(T, PathBuf)> {
    let path = recipe_program(program, template)?;
    let mut command = Command::new(&path);
    command.arg0(os(program)).args(template.get_args());
    rmake_linux::signals::configure_child(&mut command);
    match run(command, environment, program) {
        Err(error) if error.raw_os_error() == Some(8) => {
            // Interpret the selected file through /bin/sh with its original
            // arguments and environment.
            let mut shell = Command::new("/bin/sh");
            shell.arg(&path).args(template.get_args());
            rmake_linux::signals::configure_child(&mut shell);
            run(shell, environment, b"/bin/sh")
        }
        result => result,
    }
    .map(|child| (child, path))
}

#[cfg(feature = "test-support")]
pub(crate) fn run_native_ownership_check(name: &str) -> bool {
    initial::run_native_ownership_check(name)
}

#[cfg(test)]
mod tests {
    #[test]
    fn file_traversals_keep_scope_and_outcome_lifetimes_independent() {
        let mut files = FileTraversals::default();
        let name = b"source";
        let alias = b"alias";
        files.set_outcome(
            name,
            Outcome {
                ran: true,
                ..Outcome::default()
            },
        );
        assert!(files.visit(name).is_none());

        files.remember_visit(name, || FileVisit {
            parent: Some(OwnedScopeParent::new(
                ScopeParent {
                    target: b"parent",
                    branch: Some(1),
                },
                None,
            )),
            quiet_consideration: NonZeroU64::new(2),
        });
        assert!(files.remove_outcome(name).expect("initial outcome").ran);
        assert!(files.visit(name).is_some());
        files.set_outcome(
            name,
            Outcome {
                failed: true,
                ..Outcome::default()
            },
        );
        files.set_outcome(alias, Outcome::default());

        files.rename_visit(name, alias);
        assert!(files.visit(name).is_none());
        assert!(files.outcome(name).expect("source outcome").failed);
        assert!(!files.outcome(alias).expect("alias outcome").failed);
        let moved = files.visit(alias).expect("moved visit");
        let parent = moved.parent.as_ref().expect("scope parent");
        assert_eq!(parent.target, b"parent");
        assert_eq!(parent.branch(), Some(1));
        assert_eq!(moved.quiet_consideration, NonZeroU64::new(2));

        files.set_outcome(b"third", Outcome::default());
        assert!(matches!(&files, FileTraversals::Map(_)));
        assert!(files.outcome(name).expect("retained source outcome").failed);
        assert_eq!(
            files
                .visit(alias)
                .expect("retained visit")
                .quiet_consideration,
            NonZeroU64::new(2)
        );
    }

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

    fn optional_failure_test_engine() -> Engine {
        let launch = Engine::invocation(
            ["-rR", "-k"].into_iter().map(OsString::from),
            b"rmake",
            OsStr::new("rmake"),
            None,
            None,
        )
        .unwrap();
        let mut engine =
            Engine::from_invocation(launch, b"rmake".to_vec(), OsStr::new("rmake")).unwrap();
        engine.remaking = true;
        engine.active_makefile_remake = Some(MakefileRemake {
            optional: true,
            retain_failure: true,
            use_declared_synthetic_head: false,
            use_declared_tail_head: false,
            trace_initial_synthetic: false,
            stopped: None,
            open_error: None,
        });
        engine
    }

    #[test]
    fn tentative_archive_member_keeps_the_raw_name_when_missing() {
        let target = b"test-archive-owner(absent)";
        let searched = b"staged/test-archive-owner(absent)";
        let present = Some(SystemTime::UNIX_EPOCH + Duration::from_secs(1_600_000_000));
        for (time, keep_going) in [(None, false), (None, true), (present, false)] {
            let mut engine = optional_failure_test_engine();
            engine.remaking = false;
            engine.active_makefile_remake = None;
            engine.options.keep_going = keep_going;
            engine.database.file_records.insert(target);
            let record = engine.database.file_records.id(target);
            engine.database.file_records.rehash(target, searched);
            engine
                .path_observations
                .insert(target.to_vec(), Some((Some(searched.to_vec()), time)));
            let outcome = engine.build_inner(
                target,
                target,
                None,
                0,
                None,
                BuildVisit::Ordinary,
                &mut false,
            );
            assert_eq!(outcome.failed, time.is_none());
            assert!(
                (outcome.database_result == DatabaseResult::UnfinishedUpdate)
                    == (time.is_none() && !keep_going),
            );
            assert_eq!(outcome.time, time);
            assert_eq!(engine.database.file_records.id(searched), record);
            assert_eq!(
                engine.resolved_name_ref(target),
                if time.is_some() {
                    searched.as_slice()
                } else {
                    target.as_slice()
                },
            );
        }
    }

    #[test]
    fn missing_rule_diagnostic_parent_changes_without_changing_variable_scope() {
        let mut engine = optional_failure_test_engine();
        let leaf = b"missing-diagnostic-leaf";
        engine.file_traversals.remember_visit(leaf, || FileVisit {
            parent: Some(OwnedScopeParent::new(
                ScopeParent {
                    target: b"first",
                    branch: None,
                },
                None,
            )),
            quiet_consideration: None,
        });
        engine.file_traversals.set_outcome(
            leaf,
            Outcome {
                failed: true,
                ..Outcome::default()
            },
        );
        engine
            .missing_rule_parents
            .insert(leaf.to_vec(), Some(b"first".to_vec()));
        engine
            .aliases
            .insert(b"leaf-alias", leaf, AliasKind::Rename);
        assert!(
            engine
                .build(
                    b"leaf-alias",
                    Some(ScopeParent {
                        target: b"latest",
                        branch: None,
                    }),
                    1
                )
                .failed
        );
        assert_eq!(
            engine.missing_rule_parent(leaf, None),
            Some(b"latest".as_slice())
        );
        assert_eq!(
            engine
                .file_traversals
                .visit(leaf)
                .unwrap()
                .parent
                .as_ref()
                .unwrap()
                .target,
            b"first"
        );
        // Replaying a blocked parent reads its leaf without visiting that leaf.
        assert_eq!(
            engine.missing_rule_parent(leaf, Some(b"blocked")),
            Some(b"latest".as_slice())
        );
        assert!(
            !engine
                .missing_rule_parents
                .contains_key(b"leaf-alias".as_slice())
        );
    }

    #[test]
    fn missing_rule_parent_is_retired_when_its_failed_verdict_is_replaced() {
        for replacement in [
            None,
            Some(Outcome::default()),
            Some(Outcome {
                failed: true,
                question_outdated: true,
                ..Outcome::default()
            }),
            Some(Outcome {
                failed: true,
                fatal: true,
                ..Outcome::default()
            }),
        ] {
            let mut engine = optional_failure_test_engine();
            let leaf = b"replaced-missing-leaf";
            engine.file_traversals.set_outcome(
                leaf,
                Outcome {
                    failed: true,
                    ..Outcome::default()
                },
            );
            engine
                .missing_rule_parents
                .insert(leaf.to_vec(), Some(b"old".to_vec()));
            if let Some(outcome) = replacement {
                engine.file_traversals.set_outcome(leaf, outcome);
            } else {
                engine.file_traversals.remove_outcome(leaf);
            }
            // A blocked replay can occur before the leaf is visited again.
            assert_eq!(
                engine.missing_rule_parent(leaf, Some(b"fallback")),
                Some(b"fallback".as_slice())
            );
            engine.prepare_missing_rule_visit(leaf, Some(b"new"));
            assert!(!engine.missing_rule_parents.contains_key(leaf.as_slice()));
        }
    }

    #[test]
    fn renamed_failure_retires_source_and_preserves_destination_verdict() {
        let mut engine = optional_failure_test_engine();
        for (name, parent) in [
            (b"old-name".as_slice(), b"old-parent".as_slice()),
            (b"new-name", b"new-parent"),
        ] {
            engine.file_traversals.set_outcome(
                name,
                Outcome {
                    failed: true,
                    ..Outcome::default()
                },
            );
            engine
                .missing_rule_parents
                .insert(name.to_vec(), Some(parent.to_vec()));
        }
        engine
            .rename_search_record(b"old-name", b"new-name")
            .unwrap();
        assert!(
            !engine
                .missing_rule_parents
                .contains_key(b"old-name".as_slice())
        );
        assert_eq!(
            engine.missing_rule_parent(b"new-name", None),
            Some(b"new-parent".as_slice())
        );
    }

    #[test]
    fn optional_cached_failure_keeps_reported_and_suppressed_states_distinct() {
        for question in [false, true] {
            for retained in [false, true] {
                for alias in [false, true] {
                    let mut engine = optional_failure_test_engine();
                    let target = b"cached-optional-input";
                    engine.file_traversals.set_outcome(
                        target,
                        Outcome {
                            failed: !question,
                            question_outdated: question,
                            ..Outcome::default()
                        },
                    );
                    if retained {
                        engine
                            .optional_remake_failures
                            .insert(target.to_vec(), None);
                    }
                    let name = if alias {
                        engine
                            .aliases
                            .insert(b"cached-alias", target, AliasKind::Rename);
                        b"cached-alias".as_slice()
                    } else {
                        target.as_slice()
                    };
                    let result = engine.build(name, None, 0);
                    assert_eq!(result.failed, !question);
                    assert_eq!(result.question_outdated, question);
                    assert_eq!(
                        engine
                            .optional_remake_failures
                            .contains_key(target.as_slice()),
                        retained || question,
                        "question={question}, retained={retained}, alias={alias}"
                    );
                    assert!(
                        !engine
                            .optional_remake_failures
                            .contains_key(b"cached-alias".as_slice())
                    );
                }
            }
        }
    }

    #[test]
    fn pending_optional_branch_retains_its_new_failure_after_a_cached_result() {
        let mut engine = optional_failure_test_engine();
        engine
            .database
            .read(b"later:: fresh-missing-input ;\n", b"Makefile")
            .unwrap();
        let branch = engine.database.rules[b"later".as_slice()]
            .double_colon
            .as_ref()
            .unwrap()[0]
            .clone();
        engine.file_traversals.set_outcome(
            b"later",
            Outcome {
                failed: true,
                ..Outcome::default()
            },
        );
        engine.pending_double_colon.insert(
            b"later".to_vec(),
            DoubleColonState {
                remake: engine.active_makefile_remake.clone(),
                synthetic_tail: None,
                depth: 0,
                before: None,
                phony: false,
                assumed_phony_input: false,
                branches: std::collections::VecDeque::from([(0, Some(branch))]),
                active: None,
                last_epoch: None,
                combined: Outcome::default(),
                awaiting_recipe: false,
            },
        );
        assert!(engine.build(b"later", None, 0).failed);
        let cause = engine.optional_remake_failures[b"later".as_slice()]
            .as_ref()
            .unwrap();
        assert_eq!(cause.target, b"fresh-missing-input");
        assert_eq!(cause.parent, b"later");
    }

    #[test]
    fn shared_implicit_child_keeps_first_selection_and_all_search_effects() {
        for (references, inputs, expected, visits) in [
            (
                "%.mid %.mid",
                "$$(word $$(words $$(VISITS)),first second third)",
                &["first"][..],
                b"2".as_slice(),
            ),
            (
                "%.mid %.mid %.mid",
                "$$(word $$(words $$(VISITS)),first second third)",
                &["first"][..],
                b"3".as_slice(),
            ),
            (
                "%.mid %.mid",
                "first first | second second",
                &["first", "first", "second", "second"][..],
                b"2".as_slice(),
            ),
            (
                "%.mid %.mid",
                "$$(shell exit $$(words $$(VISITS))) first",
                &["first"][..],
                b"2".as_slice(),
            ),
        ] {
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
            let source = format!(
                ".SECONDEXPANSION:\nfirst second third: ;\n\
                 %.out: {references} ;\n\
                 %.mid: $$(eval VISITS += x) {inputs} ;\n"
            );
            engine
                .database
                .read(source.as_bytes(), b"Makefile")
                .unwrap();
            assert!(engine.resolve_implicit_at(b"shared.out", 0, false));
            assert_eq!(
                engine
                    .database
                    .variables
                    .expand(b"$(words $(VISITS))")
                    .unwrap(),
                visits
            );
            let selected = &engine.database.rules[b"shared.mid".as_slice()];
            assert_eq!(
                selected
                    .dependencies
                    .iter()
                    .map(|dep| dep.name.as_slice())
                    .collect::<Vec<_>>(),
                expected
                    .iter()
                    .map(|name| name.as_bytes())
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                engine.database.rules[b"shared.out".as_slice()]
                    .dependencies
                    .len(),
                references.split_whitespace().count()
            );
            assert!(engine.file_traversals.outcome(b"shared.mid").is_none());
            if inputs.contains("shell") {
                assert_eq!(
                    engine
                        .target_variables(b"shared.mid")
                        .unwrap()
                        .expand(b"$(.SHELLSTATUS)")
                        .unwrap(),
                    b"2"
                );
            }
        }
    }

    #[test]
    fn process_pressure_lookup_does_not_retry_the_launch() {
        let mut missing = Command::new("unavailable");
        missing.env("PATH", "/dev/null:/dev/null");
        let result = launch_child::<()>(b"unavailable", &missing, &[], None, |_, _, _| {
            panic!("lookup failure must not attempt a launch");
        });
        assert_eq!(result.unwrap_err().raw_os_error(), Some(20));

        let mut available = Command::new("true");
        available.env("PATH", "/dev/null:/bin:/usr/bin");
        let mut attempts = 0;
        let result = launch_child(b"true", &available, &[], None, |command, _, _| {
            assert_eq!(command.get_program(), "/bin/true");
            attempts += 1;
            Err::<(), _>(io::Error::from_raw_os_error(11))
        });
        assert_eq!(attempts, 1);
        assert_eq!(result.unwrap_err().raw_os_error(), Some(11));
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

        engine
            .aliases
            .insert(b"goal-0000", b"renamed", AliasKind::Rename);
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
            PendingImplicitRoot {
                selection: ImplicitSelection {
                    name: b".PHONY".to_vec(),
                    rule,
                    explicit_names: Vec::new(),
                    intermediate_names: Vec::new(),
                    prepared_scope: None,
                },
                children: Vec::new(),
                published: HashMap::new(),
            },
            ImplicitPlacement::Root,
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
        engine
            .file_traversals
            .set_outcome(b".PHONY", Outcome::default());
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
        engine
            .aliases
            .insert(b"goal", b"resolved", AliasKind::Rename);

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
        assert!(!engine.ordinary_mode_remakes_settle(&reads, None, modes, false));
        reads[0].1 = true;
        assert!(engine.ordinary_mode_remakes_settle(&reads, None, modes, false));

        reads[0].1 = false;
        engine.options.goals = vec![b"named".to_vec(), b"all".to_vec()];
        assert!(engine.ordinary_mode_remakes_settle(&reads, None, modes, false));
        engine.vpath_dirs.push(b"search".to_vec());
        assert!(!engine.ordinary_mode_remakes_settle(&reads, None, modes, false));
        engine.vpath_dirs.clear();
        for goal in ["~/all", "-lwidget", "archive.a(member)"] {
            engine.options.goals = vec![goal.as_bytes().to_vec()];
            assert!(!engine.ordinary_mode_remakes_settle(&reads, None, modes, false));
        }
    }

    #[test]
    fn indexed_goal_aliases_follow_moves_and_independent_redirects() {
        let mut args = vec![
            OsString::from("-rR"),
            OsString::from("requested-a"),
            OsString::from("requested-b"),
        ];
        args.extend((0..32).map(|index| OsString::from(format!("goal-{index}"))));
        let launch = Engine::invocation(args, b"rmake", OsStr::new("rmake"), None, None).unwrap();
        let mut engine =
            Engine::from_invocation(launch, b"rmake".to_vec(), OsStr::new("rmake")).unwrap();

        engine
            .rename_search_record(b"requested-a", b"cache/a")
            .unwrap();
        engine
            .rename_search_record(b"requested-b", b"cache/a")
            .unwrap();
        engine.rename_search_record(b"cache/a", b"cache/b").unwrap();
        engine.intermediates.insert(b"cache/b".to_vec());
        assert!(!engine.intermediate(b"requested-a"));
        assert!(!engine.intermediate(b"cache/b"));

        engine
            .rename_search_record(b"requested-a", b"cache/c")
            .unwrap();
        engine.intermediates.insert(b"cache/c".to_vec());
        engine.intermediates.insert(b"unrequested".to_vec());
        assert!(!engine.intermediate(b"cache/b"));
        assert!(!engine.intermediate(b"cache/c"));
        assert!(engine.intermediate(b"unrequested"));
        assert!(engine.has_requested_goal_name(b"requested-a"));
        assert!(!engine.has_requested_goal_name(b"cache/c"));

        engine
            .rename_search_record(b"requested-b", b"requested-a")
            .unwrap();
        assert!(engine.intermediate(b"cache/b"));
        assert!(!engine.intermediate(b"cache/c"));
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
            engine.ordinary_mode_remakes_settle(&reads, Some(&indexed), modes, false),
            engine.ordinary_mode_remakes_settle(&reads, None, modes, false),
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
        assert!(engine.ordinary_mode_remakes_settle(&reads, None, modes, false));

        for name in [
            b"build/result".as_slice(),
            b"sub/other.gen",
            b"sub/../left.gen",
        ] {
            reads[1] = (leaf(name), false);
            assert!(!engine.ordinary_mode_remakes_settle(&reads, None, modes, false));
            reads[1].1 = true;
            assert!(!engine.ordinary_mode_remakes_settle(&reads, None, modes, false));
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
            assert!(!engine.ordinary_mode_remakes_settle(&reads, None, modes, false));
        }
        engine.options.goals[0] = b"build/result".to_vec();
        engine.gpath_dirs.insert(b"build".to_vec());
        assert!(!engine.ordinary_mode_remakes_settle(&reads, None, modes, false));
        engine.gpath_dirs.clear();
        engine
            .database
            .read(b"vpath %.gen build\n", b"Makefile")
            .unwrap();
        assert!(!engine.ordinary_mode_remakes_settle(&reads, None, modes, false));
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
        assert!(engine.ordinary_mode_remakes_settle(&reads, None, modes, false));
        engine.options.no_rules = false;
        assert!(!engine.ordinary_mode_remakes_settle(&reads, None, modes, false));
        engine.options.no_rules = true;

        let mut alias = source.clone();
        alias.name = b"./optional.gen".to_vec();
        alias.optional = false;
        reads.push((alias, false));
        assert!(!engine.ordinary_mode_remakes_settle(&reads, None, modes, false));
        reads.pop();

        engine.vpath_dirs.push(b"search".to_vec());
        assert!(!engine.ordinary_mode_remakes_settle(&reads, None, modes, false));
        engine.vpath_dirs.clear();
        engine
            .database
            .read(b"optional.gen: ; @:\n", b"Makefile")
            .unwrap();
        assert!(engine.ordinary_mode_remakes_settle(&reads, None, modes, false));
        engine.options.no_rules = false;
        assert!(engine.ordinary_mode_remakes_settle(&reads, None, modes, false));
        engine.options.no_rules = true;
        engine
            .database
            .read(b"optional.gen: prerequisite\n", b"Makefile")
            .unwrap();
        assert!(engine.ordinary_mode_remakes_settle(&reads, None, modes, false));
        engine.options.load_limit = Some(1.0);
        assert!(engine.ordinary_mode_remakes_settle(&reads, None, modes, false));
        let mut required = source;
        required.name = b"required.gen".to_vec();
        required.optional = false;
        engine
            .database
            .read(b"required.gen: ; @:\n", b"Makefile")
            .unwrap();
        reads.push((required, false));
        assert!(!engine.ordinary_mode_remakes_settle(&reads, None, modes, false));
        engine.options.load_limit = None;
        assert!(engine.ordinary_mode_remakes_settle(&reads, None, modes, false));
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
