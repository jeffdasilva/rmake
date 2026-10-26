// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Linux filesystem, dependency traversal and process execution boundary.
//! No persistent cache and no delegation to GNU Make are involved.

use crate::cli::Options;
use crate::file_aliases::Aliases;
use crate::makefiles::IncludeSearch;
use crate::recipe_flow::{Cleanup, Execution, Flow, Status};
use rmake_core::diagnostic::{Diagnostic, ExpansionContext, Output};
use rmake_core::parser::{
    Database, Dependency, Location, Makefile, Recipe, Rule, assignment, rule_filenames, trim, words,
};
use rmake_core::variables::{Assignment, ExpandError, Flavor, Origin, TargetScope, VariableStore};
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
use std::time::{Duration, SystemTime};

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
        .filter(|(d, ready)| **ready && !d.order_only && d.name != b".WAIT")
        .map(|(d, _)| d.name.as_slice());
    let first_normal = normal.clone().next().unwrap_or_default().to_vec();
    let plus = joined(normal.map(prerequisite_variable_name));
    // GNU computes < and + before promoting duplicate order-only
    // occurrences. Promotion survives into later clause expansions.
    let mut first = HashMap::new();
    let mut promotions = Vec::new();
    for (index, (dependency, ready)) in dependencies.iter().zip(ready).enumerate() {
        if !ready || dependency.name == b".WAIT" {
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
        .filter(|(d, ready)| **ready && d.name != b".WAIT" && seen.insert(d.name.as_slice()))
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
    // A real recipe invalidates its timestamp; a parent may search again.
    search_after_update: bool,
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
    identity: u64,
    program: Vec<u8>,
    location: Location,
    ignore: bool,
    before: Option<SystemTime>,
    phony: bool,
    continuation: RecipeState,
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
    fn values(&self) -> impl Iterator<Item = &ParallelJob> {
        self.primary.values().chain(self.overlapping.values())
    }
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

/// A dependency traversal can yield without holding a recipe slot. Only the
/// interpreter owns these frames; effects and secondary expansion run once.
struct RuleState {
    rule: Rule,
    group: Option<Box<GroupState>>,
    comparison: Option<SystemTime>,
    last_epoch: Option<u64>,
    depth: usize,
    before: Option<SystemTime>,
    phony: bool,
    next: usize,
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
    deferred_intermediates: Vec<Vec<u8>>,
}

/// Double-colon branches remain sequential for one target, while their
/// unfinished dependencies yield to unrelated targets in the enclosing graph.
struct DoubleColonState {
    depth: usize,
    before: Option<SystemTime>,
    phony: bool,
    branches: std::collections::VecDeque<Rule>,
    active: bool,
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

type JobResult = (Vec<u8>, u64, io::Result<std::process::ExitStatus>);

#[derive(Default)]
struct ArchiveVpathReads {
    members: HashSet<Vec<u8>>,
    archives: HashSet<Vec<u8>>,
}

struct ImplicitSelection {
    name: Vec<u8>,
    rule: Rule,
    pattern_scope: Option<Box<Option<TargetScope>>>,
    explicit_names: Vec<Vec<u8>>,
}

// A selected Makefile record can name a file that does not exist yet.
// Keep the search identity separate from its optional filesystem timestamp.
type PathObservation = Option<(Option<Vec<u8>>, Option<SystemTime>)>;

pub struct Engine {
    options: Options,
    filesystem: RefCell<FilesystemRuntime>,
    include_search: IncludeSearch,
    database: Database,
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
    completed_jobs: std::sync::mpsc::Receiver<JobResult>,
    job_sender: std::sync::mpsc::Sender<JobResult>,
    parallel_failed: bool,
    waiting_reported: bool,
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
    makefile_error: RefCell<Option<(Option<Location>, Vec<u8>)>>,
    initial_flags: Vec<u8>,
    initial_gnu_flags: Vec<u8>,
    intermediate_policy: crate::intermediates::Policy,
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
        database
            .variables
            .define_literal(b"MAKEFILES", b"", Origin::Default)
            .map_err(|error| error.diagnostic())?;
        database.variables.set_export_if_set(b"MAKEFILES");
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
        let mut next_include = 0;
        for index in 0..=options.assignments.len() {
            while options.include_assignment_positions.get(next_include) == Some(&index) {
                let directory = &mut options.include_directories[next_include];
                next_include += 1;
                *directory = crate::makefiles::prepare_argument(
                    directory.as_os_str().as_bytes(),
                    &mut database.variables,
                    ExpansionContext::new(None, &mut output)
                        .with_filesystem(&mut |effect| filesystem.borrow_mut().execute(effect))
                        .with_shell(&mut |request| {
                            let result = crate::shell::run(request, &program, startup_level);
                            filesystem.borrow_mut().child_completed();
                            result
                        }),
                )
                .map_err(|error| error.diagnostic())?;
            }
            let Some(arg) = options.assignments.get(index) else {
                break;
            };
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
        let mut seen_include_arguments = HashSet::new();
        options
            .include_directories
            .retain(|directory| seen_include_arguments.insert(directory.as_os_str().to_owned()));
        let include_search = IncludeSearch::new(&options.include_directories);
        database
            .variables
            .define_literal(b"MAKEFLAGS", &options.flags(), Origin::File)
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
                    ExpansionContext::new(None, &mut output)
                        .with_filesystem(&mut |effect| filesystem.borrow_mut().execute(effect))
                        .with_shell(&mut |request| {
                            let result = crate::shell::run(request, &program, startup_level);
                            filesystem.borrow_mut().child_completed();
                            result
                        }),
                )
                .map_err(|error| error.diagnostic())?;
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
        let mut initial_flags = options.flags();
        if command_replay {
            initial_flags.extend_from_slice(b" -- $(MAKEOVERRIDES)");
            database
                .variables
                .define(
                    b"MAKEFLAGS",
                    &initial_flags,
                    Assignment::Recursive,
                    Origin::File,
                )
                .map_err(|error| error.diagnostic())?;
        }
        let (job_sender, completed_jobs) = std::sync::mpsc::channel();
        Ok(Self {
            options,
            filesystem,
            include_search,
            database,
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
            completed_jobs,
            job_sender,
            parallel_failed: false,
            waiting_reported: false,
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
            makefile_error: RefCell::new(None),
            initial_flags,
            initial_gnu_flags: Vec::new(),
            intermediate_policy: crate::intermediates::Policy::default(),
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
                .with_includes(&mut |name| self.include_search.read(name)),
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
                .with_includes(&mut |name| self.include_search.read(name)),
        )
    }

    fn show_makefile_error(&self) {
        if let Some((location, message)) = self.makefile_error.borrow_mut().take() {
            emit_output(
                &self.program,
                Output::Warning {
                    location: location.as_ref(),
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

    fn read_startup_makefiles(&mut self) -> Result<bool, ()> {
        if self
            .database
            .variables
            .get(b"MAKEFILES")
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
                ExpansionContext::new(None, &mut |event| emit_output(&self.program, event))
                    .with_filesystem(&mut |effect| self.filesystem.borrow_mut().execute(effect))
                    .with_shell(&mut |request| {
                        let result = crate::shell::run(request, &self.program, self.makelevel);
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
            let file = match self.include_search.read(name) {
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
                    file: name.to_vec(),
                    line: 1,
                },
            ) {
                self.expansion_error(None, &error);
                return Err(());
            }
            if let Err(error) = self.database.read_preload_with(
                &file.bytes,
                name,
                ExpansionContext::new(None, &mut |event| emit_output(&self.program, event))
                    .with_filesystem(&mut |effect| self.filesystem.borrow_mut().execute(effect))
                    .with_shell(&mut |request| {
                        let result = crate::shell::run(request, &self.program, self.makelevel);
                        self.filesystem.borrow_mut().child_completed();
                        result
                    })
                    .with_includes(&mut |name| self.include_search.read(name)),
            ) {
                self.fatal(error.source_location(), error.message.as_bytes());
                return Err(());
            }
        }
        Ok(found)
    }

    pub fn run(mut self) -> u8 {
        let mut found_makefile;
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
                self.implicit_pattern_scopes.clear();
                self.impossible_prerequisites.clear();
                self.resolved_names.clear();
                self.aliases.clear();
                self.path_observations.clear();
                self.waiting_reported = false;
                *self.archive_vpath_reads.borrow_mut() = None;
                self.inferred_peer_flags.clear();
                self.completed_group_branches.clear();
                self.active_group_branches.clear();
                self.double_colon_before.clear();
                self.optional_remake_failures.clear();
            }
            let startup_found = match self.read_startup_makefiles() {
                Ok(found) => found,
                Err(()) => return 2,
            };
            let mut files = self.options.files.clone();
            if files.is_empty() {
                if let Some(file) = ["GNUmakefile", "makefile", "Makefile"]
                    .into_iter()
                    .find(|file| Path::new(file).is_file())
                {
                    files.push(file.into());
                } else {
                    // Preloaded rules may generate a default main makefile.
                    for name in [b"GNUmakefile".as_slice(), b"makefile", b"Makefile"] {
                        self.database.makefiles.push(Makefile {
                            name: name.to_vec(),
                            optional: true,
                            missing: true,
                            read_error: None,
                        });
                    }
                }
            }
            found_makefile = startup_found || !files.is_empty();
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
                        .with_includes(&mut |name| self.include_search.read(name)),
                ) {
                    self.fatal(err.source_location(), err.message.as_bytes());
                    return 2;
                }
            }
            for (name, initial) in [
                (b"MAKEFLAGS".as_slice(), self.initial_flags.clone()),
                (b"GNUMAKEFLAGS", self.initial_gnu_flags.clone()),
            ] {
                match self.database.variables.get(name) {
                    Some(current) if current.raw_value() == initial => {}
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
            self.intermediate_policy = match crate::intermediates::Policy::read(&self.database) {
                Ok(policy) => policy,
                Err(message) => {
                    self.fatal(None, &message);
                    return 2;
                }
            };
            self.database.freeze_rules();
            let vpath = match self.expand_global(b"$(VPATH)", None) {
                Ok(value) => value,
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
                Vec::new()
            };
            self.gpath_dirs = gpath
                .split(|byte| *byte == b':' || byte.is_ascii_whitespace() || *byte == 0x0b)
                .filter(|path| !path.is_empty())
                .map(|path| {
                    if path.len() > 1 && path.ends_with(b"/") {
                        &path[..path.len() - 1]
                    } else {
                        path
                    }
                })
                .filter(|path| *path != b".")
                .map(Vec::from)
                .collect();
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
            self.makefile_names = self
                .database
                .makefiles
                .iter()
                .map(|file| file.name.clone())
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
            let read_files = self.database.makefiles.clone();
            let required_names: HashSet<_> = read_files
                .iter()
                .filter(|file| !file.optional)
                .map(|file| file.name.clone())
                .collect();
            for mut makefile in read_files {
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
                let promoted_required =
                    makefile.optional && required_names.contains(&makefile.name);
                if promoted_required {
                    makefile.optional = false;
                }
                if !self.resolve_implicit(&makefile.name) {
                    self.remove_intermediates();
                    return 2;
                }
                if !self.database.rules.contains_key(&makefile.name)
                    && (!makefile.missing || makefile.optional)
                {
                    continue;
                }
                let before = self.modified(&makefile.name);
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
        if self.database.rules.contains_key(b".POSIX".as_slice()) {
            self.fatal(None, b"rmake: unsupported special target .POSIX");
            return 2;
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
                    // GNU first looks up the complete spelling. An interned
                    // filename from a glob can contain blanks without naming
                    // multiple default goals.
                    if self.database.rules.contains_key(&goal)
                        || self.database.explicit_names.contains(&goal)
                    {
                        vec![goal]
                    } else {
                        let goals: Vec<_> = words(&goal).map(Vec::from).collect();
                        if goals.len() != 1 {
                            self.fatal(None, b".DEFAULT_GOAL contains more than one target");
                            return 2;
                        }
                        goals
                    }
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
        let mut requested = Vec::new();
        for goal in goals {
            self.graph_epoch = self.graph_epoch.wrapping_add(1);
            let already_done =
                self.done.contains_key(self.aliases.resolve(&goal)) || self.pending(&goal);
            if already_done && !self.parallel_graph() {
                self.wait_target(&goal);
            }
            let outcome = self.build(&goal, None, 0);
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
            self.stderr(
                &[
                    b"Target '".as_slice(),
                    goal,
                    b"' not remade because of errors.",
                ]
                .concat(),
            );
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
            let body = if has_recipe {
                [b"'".as_slice(), goal, b"' is up to date."].concat()
            } else {
                [b"Nothing to be done for '".as_slice(), goal, b"'."].concat()
            };
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
            (empty_means_all && rule.dependencies.is_empty())
                || rule
                    .dependencies
                    .iter()
                    .any(|dependency| self.aliases.resolve(&dependency.name) == target)
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
        if self.intermediate_policy.all_secondary || self.intermediate_policy.no_intermediates {
            self.created_intermediates.clear();
            return;
        }
        let mut removed = Vec::new();
        for target in std::mem::take(&mut self.created_intermediates) {
            if removed.contains(&target)
                || self.special(b".SECONDARY", &target, true)
                || self.special_for_rule(b".PRECIOUS", &target)
                || self.special_for_rule(b".NOTINTERMEDIATE", &target)
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
        time: Option<SystemTime>,
        depth: usize,
    ) -> Inspection {
        if depth >= 512
            || self.done.contains_key(target)
            || self.pending(target)
            || !self.intermediate(target)
            || self.modified(target).is_some()
            || self.special(b".PHONY", target, false)
        {
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
        let mut dropped = false;
        while next < rule.dependencies.len() {
            let dep = &rule.dependencies[next];
            if dep.name == b".WAIT" {
                if waiting {
                    break;
                }
                next += 1;
                continue;
            }
            if self.visiting.contains(&dep.name) {
                self.stderr(
                    &[
                        b"Circular ".as_slice(),
                        target,
                        b" <- ",
                        &dep.name,
                        b" dependency dropped.",
                    ]
                    .concat(),
                );
                rule.dependencies.remove(next);
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

    fn notice_waiting_for_jobs(&mut self) {
        if !self.waiting_reported {
            self.stderr(b"*** Waiting for unfinished jobs....");
            self.waiting_reported = true;
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

    fn complete_job(&mut self, (target, identity, result): JobResult) {
        self.graph_epoch = self.graph_epoch.wrapping_add(1);
        let mut job = self
            .parallel_jobs
            .remove(&target, identity)
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
        if failed && self.parallel_jobs.contains_key(&target) {
            self.notice_waiting_for_jobs();
        }
        let mut outcome = self.done.get(&target).copied().unwrap_or_default();
        if !failed {
            let execution =
                self.continue_recipe(&target, job.continuation, job.phony, job.before, true);
            failed |= matches!(execution.status, Status::Failure | Status::Fatal);
            outcome.fatal |= execution.status == Status::Fatal;
            self.cleanup_target(&target, job.before, job.phony, execution.cleanup);
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
        self.parallel_failed |= failed;
        outcome.ran = true;
        outcome.failed |= failed;
        self.record_graph_failure(outcome);
        outcome.time = if job.phony {
            None
        } else {
            self.modified(&target)
        };
        outcome.search_after_update = self.update_search_pending(&target, job.phony, true);
        self.done.insert(target.clone(), outcome);
        if !self.parallel_jobs.contains_key(&target) {
            self.finish_group(&target, outcome);
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
        if (!self.database.has_vpaths() && self.vpath_dirs.is_empty())
            || self.makefile_names.contains(target)
            || self.path_observations.contains_key(target)
            || self.done.contains_key(target)
            || self.pending(target)
            || self.visiting.contains(target)
            || self.special(b".PHONY", target, false)
        {
            return Ok(None);
        }
        if let Some((archive, member)) = rmake_core::filenames::archive_parts(target)
            && let Some(path) = self
                .archive_vpath(archive)
                .map_err(|error| self.expansion_error(None, &error))?
            && (self.gpath_match(archive, &path) || self.has_file_record(&path))
        {
            let archive_time = self.modified(&path);
            self.rename_search_record(archive, &path)?;
            self.path_observations
                .entry(path.clone())
                .or_insert(Some((None, archive_time)));
            let name = [path.as_slice(), b"(", member, b")"].concat();
            let observation = self
                .find_vpath(&name)
                .map_err(|error| self.expansion_error(None, &error))?;
            self.rename_search_record(target, &name)?;
            self.path_observations.insert(name.clone(), observation);
            return Ok(Some(name));
        }
        let observation = self
            .find_vpath(target)
            .map_err(|error| self.expansion_error(None, &error))?;
        if let Some((Some(name), _)) = &observation
            && (self.gpath_match(target, name) || self.has_file_record(name))
        {
            let name = name.clone();
            self.rename_search_record(target, &name)?;
            self.path_observations
                .insert(name.clone(), observation.map(|(_, time)| (None, time)));
            return Ok(Some(name));
        }
        self.path_observations.insert(target.to_vec(), observation);
        Ok(None)
    }

    fn rename_search_record(&mut self, from: &[u8], to: &[u8]) -> Result<(), ()> {
        if from == to {
            return Ok(());
        }
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
                            location.file.as_slice(),
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
                            &self.program,
                            Output::Warning {
                                location,
                                message: &message,
                            },
                        );
                    }
                }
                if destination.double_colon.is_some() != source.double_colon.is_some() {
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
        self.database.explicit_names.contains(name)
            || self.database.rules.contains_key(name)
            || self.database.target_scopes.contains_key(name)
            || self.makefile_names.contains(name)
            || self.options.goals.iter().any(|goal| goal == name)
    }

    fn record_is_target(&self, name: &[u8]) -> bool {
        self.database.rules.contains_key(name) || self.special(b".PHONY", name, false)
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
        let observed = rmake_linux::timestamp::observe(name, true);
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

    fn modified(&self, name: &[u8]) -> Option<SystemTime> {
        if !self.options.check_symlink_times {
            return modified(name);
        }
        if let Some((archive, member)) = rmake_core::filenames::archive_parts(name) {
            self.archive_time(archive, member, self.name_time(member))
        } else {
            self.name_time(name)
        }
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
        for directory in self
            .database
            .vpath_directories(target)
            .chain(self.vpath_dirs.iter().map(Vec::as_slice))
        {
            let name = searched_name(directory, target);
            let (lookup, membership) = search_probe_names(directory, target, &name);
            let declared = self.search_record_matches(target, &lookup);
            if declared || self.directory_member(&membership)? {
                // A cached name gates the probe, but VPATH must still reject
                // a stale positive when a fresh stat no longer finds it.
                let physical = modified(&name);
                if declared || physical.is_some() {
                    let time = if self.options.check_symlink_times {
                        self.name_time(&name)
                    } else {
                        physical
                    };
                    return Ok(Some((Some(name), time)));
                }
            }
        }
        Ok(None)
    }

    fn find_vpath(&self, target: &[u8]) -> Result<PathObservation, ExpandError> {
        if let Some((archive, member)) = rmake_core::filenames::archive_parts(target) {
            // The standalone member is observed before container search.
            let member_time = self
                .options
                .check_symlink_times
                .then(|| self.name_time(member));
            let path = self.archive_vpath(archive)?;
            let archive = path.as_deref().unwrap_or(archive);
            let time = if let Some(member_time) = member_time {
                self.archive_time(archive, member, member_time)
            } else {
                rmake_linux::archive::modified(archive, member)
                    .ok()
                    .flatten()
            };
            return Ok(time.map(|time| {
                (
                    path.map(|path| [path.as_slice(), b"(", member, b")"].concat()),
                    Some(time),
                )
            }));
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
        self.searched_file(target)
    }

    /// Select the archive container before inspecting a member. Cached
    /// directory membership and a fresh stat gate non-declared candidates.
    fn archive_vpath(&self, archive: &[u8]) -> Result<Option<Vec<u8>>, ExpandError> {
        if !self.vpath_searchable(archive) {
            return Ok(None);
        }
        let local = if self.options.check_symlink_times {
            rmake_linux::timestamp::observe(archive, true)
                .time
                .is_some()
        } else {
            fs::metadata(os(archive)).is_ok()
        };
        if local {
            return Ok(None);
        }
        for directory in self
            .database
            .vpath_directories(archive)
            .chain(self.vpath_dirs.iter().map(Vec::as_slice))
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
    fn implicit_vpath_exists(&self, target: &[u8]) -> Result<bool, ExpandError> {
        if rmake_core::filenames::archive_parts(target).is_none() {
            return if self.directory_member(target)? {
                Ok(true)
            } else {
                Ok(self.searched_file(target)?.is_some())
            };
        }
        // Archive viability scans the local archive freshly, then searches
        // the complete literal member name (not the archive container).
        if self.modified(target).is_some() {
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
        if !outcome.search_after_update || outcome.fatal {
            return outcome;
        }
        outcome.search_after_update = false;
        match self.find_vpath(target) {
            Ok(observation) => {
                outcome.time = observation.as_ref().and_then(|(_, time)| *time);
                if let Some((Some(name), _)) = observation
                    && (self.gpath_match(target, &name) || self.has_file_record(&name))
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
        self.database.explicit_names.contains(target)
            || self.options.goals.iter().any(|goal| goal == target)
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
        let existing = match self.database.rules.get(target) {
            Some(rule) if rule.recipes.is_some() || rule.double_colon.is_some() => return true,
            rule => rule.cloned(),
        };
        let mut budget = 100_000;
        let plan = match self.implicit_plan(target, &mut Vec::new(), false, &mut budget) {
            Ok(plan) => plan,
            Err(error) => {
                self.expansion_error(None, &error);
                return false;
            }
        };
        if plan.is_none() && budget == 0 {
            self.fatal(None, b"rmake: implicit rule search resource limit exceeded");
            return false;
        }
        if let Some(plan) = plan {
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
                } = selection;
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
                self.database.rules.insert(name, rule);
            }
        } else if existing.is_none()
            && let Some(default) = self.database.rules.get(b".DEFAULT".as_slice())
            && default.recipes.is_some()
        {
            self.database.rules.insert(
                target.to_vec(),
                Rule {
                    recipes: default.recipes.clone(),
                    recipe_location: default.recipe_location.clone(),
                    default_recipe: true,
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
    ) -> Result<Option<Vec<ImplicitSelection>>, ExpandError> {
        if active.len() >= 128 {
            *budget = 0;
        }
        if *budget == 0 {
            return Ok(None);
        }
        for archive_search in [false, true] {
            if let Some(plan) =
                self.implicit_search(target, active, compatibility, archive_search, budget)?
            {
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
                let mut plan = Vec::new();
                let mut explicit_names = Vec::new();
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
                            }
                            let explicit = self.database.rules.contains_key(name)
                                || self.database.rules.get(target).is_some_and(|rule| {
                                    rule.dependencies.iter().enumerate().any(|(index, dep)| {
                                        dep.name == *name
                                            && !rule.secondary.as_ref().is_some_and(|clauses| {
                                                clauses.iter().any(|clause| clause.index == index)
                                            })
                                    })
                                });
                            if name != b".WAIT" && !explicit && !self.implicit_vpath_exists(name)? {
                                if self.implicit_known_file(name) {
                                    if compatibility {
                                        dependencies.push(dependency);
                                        continue;
                                    }
                                    compatibility_needed = true;
                                }
                                if chain {
                                    if let Some(child) =
                                        self.implicit_plan(name, active, compatibility, budget)?
                                    {
                                        plan.extend(child);
                                    } else {
                                        if !self.implicit_known_file(name) {
                                            self.refresh_impossible_prerequisites();
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
                    let mut selected = candidate.rule.clone();
                    selected.dependencies = dependencies;
                    selected.secondary = None;
                    plan.push(ImplicitSelection {
                        name: target.to_vec(),
                        rule: selected,
                        explicit_names,
                        pattern_scope: (intermediate && variables.is_some())
                            .then(|| Box::new(pattern_scope.clone())),
                    });
                    return Ok(Some(plan));
                }
            }
        }
        if compatibility_needed && !compatibility {
            return self.implicit_search(target, active, true, archive_search, budget);
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
                ExpansionContext::new(None, &mut |event| emit_output(&self.program, event))
                    .with_filesystem(&mut |effect| self.filesystem.borrow_mut().execute(effect))
                    .with_shell(&mut |request| {
                        let result = crate::shell::run(request, &self.program, self.makelevel);
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
        &self,
        name: &[u8],
        order_only: bool,
        directory: Option<&[u8]>,
    ) -> Result<Vec<Dependency>, ExpandError> {
        let name = rmake_core::filenames::without_dot_prefix(name);
        let name = directory.map_or_else(|| name.to_vec(), |dir| [dir, name].concat());
        if !name.iter().any(|byte| matches!(byte, b'*' | b'?' | b'[')) {
            return Ok(vec![Dependency { name, order_only }]);
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
            .map(|name| Dependency {
                name: name.to_vec(),
                order_only,
            })
            .collect())
    }

    fn implicit_filenames(
        &self,
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
            let mut context = ExpansionContext::pure().with_filesystem(&mut filesystem);
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
                for name in rule_filenames(text, &mut context)? {
                    expanded.push(Dependency {
                        name: name.into_owned(),
                        order_only: role,
                    });
                }
            }
            original_next = clause.index + 1;
            expanded_next = index + expanded.len();
            ready_count += expanded.len();
            ready.splice(index..=index, std::iter::repeat_n(true, expanded.len()));
            rule.dependencies.splice(index..=index, expanded);
        }
        Ok(())
    }

    fn resolved_name(&self, name: &[u8]) -> Vec<u8> {
        let name = self.aliases.resolve(name);
        self.resolved_names
            .get(name)
            .cloned()
            .unwrap_or_else(|| name.to_vec())
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
            if !self.parallel_jobs.is_empty() {
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
    }

    fn archive_metadata_error(&self, target: &[u8], search: bool) -> bool {
        let Some((archive, member)) = rmake_core::filenames::archive_parts(target) else {
            return false;
        };
        let symbol = member.starts_with(b"(") && member.ends_with(b")");
        if !symbol
            && (self.special(b".PHONY", target, false)
                || (!search
                    && self
                        .archive_vpath_reads
                        .borrow()
                        .as_ref()
                        .is_some_and(|reads| reads.members.contains(target))))
        {
            return false;
        }
        let message = if symbol {
            Some(
                [
                    b"attempt to use unsupported feature: '".as_slice(),
                    target,
                    b"'",
                ]
                .concat(),
            )
        } else {
            let path = if search {
                match self.archive_vpath(archive) {
                    Ok(path) => path,
                    Err(error) => {
                        self.expansion_error(None, &error);
                        return true;
                    }
                }
            } else {
                None
            };
            if path.is_some() {
                // Reading a VPATH member observes the original archive as
                // locally absent. GNU's original archive record keeps that
                // observation if it is later traversed as a prerequisite.
                let mut reads = self.archive_vpath_reads.borrow_mut();
                let reads = reads.get_or_insert_with(Default::default);
                reads.members.insert(target.to_vec());
                reads.archives.insert(archive.to_vec());
            }
            rmake_linux::archive::modified(path.as_deref().unwrap_or(archive), member)
                .err()
                .map(|error| error.into_message())
        };
        if let Some(message) = message {
            self.fatal(None, &message);
            true
        } else {
            false
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
            && outcome.ran
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
        if let Some(outcome) = self.done.get(target) {
            return *outcome;
        }
        if self.graph_abort.is_some() {
            let mut outcome = Outcome::default();
            self.apply_graph_abort(&mut outcome);
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
        // Record first ownership before implicit secondary expansion can ask
        // for this context. Pattern-scope activation itself remains lazy.
        self.scope_parents
            .entry(target.to_vec())
            .or_insert_with(|| parent.map(Vec::from));
        let phony = self.special(b".PHONY", target, false);
        // Search-time expansion can create or remove this target. GNU decides
        // freshness from the observation made before those effects occur.
        let observed = self.path_observations.remove(target);
        let resolved = if phony {
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
        if !phony && !self.resolve_implicit(target) {
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
            if before.is_some() {
                let result = Outcome {
                    time: before,
                    ..Outcome::default()
                };
                self.done.insert(target.to_vec(), result);
                return result;
            }
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
            self.pending_double_colon.insert(
                target.to_vec(),
                DoubleColonState {
                    depth,
                    before,
                    phony,
                    branches: branches.into(),
                    active: false,
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
        self.build_rule(target, depth, before, comparison, phony, rule, false)
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
            if state.active {
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
                state.combined.time = result.time;
                state.combined.ran |= result.ran;
                state.combined.failed |= result.failed;
                state.combined.fatal |= result.fatal;
                state.combined.blocked |= result.blocked;
                state.combined.question_outdated |= result.question_outdated;
                state.combined.search_after_update |= result.search_after_update;
                self.record_graph_failure(result);
                state.active = false;
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
            let Some(branch) = state.branches.pop_front() else {
                state.combined.time = if state.phony {
                    None
                } else {
                    self.modified(target)
                };
                self.done.insert(target.to_vec(), state.combined);
                return state.combined;
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
                    state.branches.push_front(branch);
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
                true,
            );
            state.active = true;
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
        double_colon: bool,
    ) -> Outcome {
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
            || phony
            || self.options.always
            || (double_colon && rule.dependencies.is_empty());
        let serial_phase = !rule.dependencies.is_empty()
            && rule.grouped_double_id().is_none()
            && self.special(b".NOTPARALLEL", target, false);
        let state = RuleState {
            rule,
            group,
            comparison,
            last_epoch: None,
            depth,
            before,
            phony,
            next: 0,
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
            if (self.database.has_vpaths() || !self.vpath_dirs.is_empty())
                && state.rule.dependencies[state.next].name != b".WAIT"
            {
                match self.prepare_search_name(&state.rule.dependencies[state.next].name) {
                    Ok(Some(name)) => state.rule.dependencies[state.next].name = name,
                    Ok(None) => {}
                    Err(()) => {
                        state.outcome.failed = true;
                        state.outcome.fatal = true;
                        state.stopped = true;
                        break;
                    }
                }
            }
            let occurrence = &state.rule.dependencies[state.next];
            let dependency = &occurrence.name;
            if dependency == b".WAIT" {
                for index in 0..state.next {
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
            let index = state.next - 1;
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
        // Revisit suspended children in declaration order without replaying the
        // parent's dependency expansion, circular diagnostics or scope setup.
        for index in 0..state.next {
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
        let inspect = state.inspections.contains_key(&index)
            || (first && self.intermediate(dependency) && self.modified(dependency).is_none());
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
            let child = if first || self.graph_pending(dependency) {
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
            (child, pending)
        }
    }

    fn prerequisite_stale(&self, before: Option<SystemTime>, child: Outcome) -> bool {
        self.options.always
            || before.is_none()
            || child.time.is_none()
            || child.time > before
            || (self.options.dry_run && child.ran)
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
            mut rule,
            mut group,
            comparison,
            before,
            phony,
            mut stale,
            mut outcome,
            dropped,
            intermediate_stale,
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
        let canonical = recipe_rule.canonical_dependencies();
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
        let normal_names: HashSet<_> = normal.iter().map(Vec::as_slice).collect();
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
                    newer.push(dependency.clone());
                }
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
                let repeated: Vec<_> = recipe_rule
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
                ];
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
                let execution = self.execute(target, recipes, &autos, phony, before);
                outcome.ran |= execution.ran;
                outcome.search_after_update = execution.ran
                    && self.update_search_pending(target, phony, execution.all_recursive);
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
                self.modified(target)
            } else {
                before
            };
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
            && self
                .modified(target)
                .is_some_and(|time| Some(time) > group.before);
        for (peer, _, phony) in group.members.into_iter().rev() {
            if peer == target {
                continue;
            }
            self.active_peers.remove(&peer);
            self.pending_rules.remove(&peer);
            let mut completed = outcome;
            completed.search_after_update = false;
            completed.time = if phony { None } else { self.modified(&peer) };
            if warn_missing && completed.time.is_none() {
                emit_output(
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
                ExpansionContext::new(None, &mut |event| emit_output(&self.program, event))
                    .with_filesystem(&mut |effect| self.filesystem.borrow_mut().execute(effect))
                    .with_shell(&mut |request| {
                        let result = crate::shell::run(request, &self.program, self.makelevel);
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
        if self.graph_abort == Some(Status::Fatal) {
            return Execution::fatal(false);
        }
        if self.graph_abort.is_some() || (self.parallel_failed && !self.options.keep_going) {
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
        if self.graph_abort == Some(Status::Fatal) {
            return Execution::fatal(false);
        }
        if self.graph_abort.is_some() || (self.parallel_failed && !self.options.keep_going) {
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
        let stdin_owner = !self
            .parallel_jobs
            .values()
            .any(|job| job.continuation.stdin_owner);
        self.continue_recipe(
            target,
            RecipeState {
                expanded,
                variables,
                flow,
                ran: false,
                one_shell,
                stdin_owner,
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
                    return Execution::fatal(ran).with_recursion(!flow.has_ordinary());
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
                    return Execution::fatal(ran).with_recursion(!flow.has_ordinary());
                }
            };
            let ifs = if variables.get(b"IFS").is_some() {
                match self.expand(variables, b"$(IFS)", Some(&recipe.location)) {
                    Ok(value) => Some(value),
                    Err(err) => {
                        self.expansion_error(Some(&recipe.location), &err);
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
                        return Execution::failure(ran, Cleanup::None)
                            .with_recursion(!flow.has_ordinary());
                    }
                    Err(crate::oneshell::PrepareError::QuotedMultilineDirect) => {
                        self.fatal(
                            Some(&recipe.location),
                            b"rmake: .ONESHELL direct commands with quoted newlines are not implemented yet",
                        );
                        return Execution::failure(ran, Cleanup::None)
                            .with_recursion(!flow.has_ordinary());
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
                            .with_includes(&mut |name| self.include_search.read(name)),
                        ) {
                            Ok(value) => {
                                child.env(os(name), OsString::from_vec(value));
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
                            self.expansion_error(Some(&recipe.location), &error);
                            return Execution::fatal(ran).with_recursion(!flow.has_ordinary());
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
                let identity = self.next_job;
                self.next_job = self.next_job.wrapping_add(1);
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
                            let _ = sender.send((name, identity, result));
                        });
                if let Err(error) = worker {
                    self.stderr(os_error(&error).as_bytes());
                    return Execution::failure(ran, Cleanup::None)
                        .with_recursion(!flow.has_ordinary());
                }
                self.parallel_jobs.insert(
                    target.to_vec(),
                    ParallelJob {
                        identity,
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
                    return Execution::question(ran, failure_cleanup)
                        .with_recursion(!flow.has_ordinary());
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
                if (ignore || !self.optional_remake) && (!ignore || !self.globally_silent()) {
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
                    return Execution::failure(ran, failure_cleanup)
                        .with_recursion(!flow.has_ordinary());
                }
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
                Err(message) => {
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

    fn cleanup_target(
        &self,
        target: &[u8],
        before: Option<SystemTime>,
        phony: bool,
        cleanup: Cleanup,
    ) {
        self.cleanup_one(target, before, phony, cleanup, None);
        if let Some(group) = self.running_groups.get(target) {
            for (peer, time, peer_phony) in group.members.iter().rev() {
                if peer != target {
                    self.cleanup_one(peer, *time, *peer_phony, cleanup, Some(target));
                }
            }
        }
    }

    fn cleanup_one(
        &self,
        target: &[u8],
        before: Option<SystemTime>,
        phony: bool,
        cleanup: Cleanup,
        owner: Option<&[u8]>,
    ) {
        if cleanup == Cleanup::None || phony || self.special_for_rule(b".PRECIOUS", target) {
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
        if let Some((archive, member)) = rmake_core::filenames::archive_parts(target) {
            let date = rmake_linux::archive::date(archive, member)
                .ok()
                .flatten()
                .and_then(|seconds| {
                    SystemTime::UNIX_EPOCH.checked_add(std::time::Duration::from_secs(seconds))
                });
            if date != before {
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
        let mut engine = Engine::new(
            Options {
                no_rules: true,
                no_variables: true,
                ..Options::default()
            },
            b"rmake".to_vec(),
            OsStr::new("rmake"),
        )
        .unwrap();
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
