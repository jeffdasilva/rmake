// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Incremental reading and immediate evaluation of the initial language subset.
//!
//! This module has no filesystem or process access. Recipes remain unexpanded
//! until the executor selects a target. Unsupported rule families fail explicitly.

pub use crate::diagnostic::Location;
use crate::diagnostic::SourceName;
use crate::diagnostic::{
    Diagnostic, ExpansionContext, FilesystemEffect, FilesystemError, IncludeError, IncludeFailure,
    Output,
};
use crate::messages::Message;
use crate::reader::{Input, ParserReader, strip_comment};
use crate::variables::{
    Assignment, ConditionalFlagBinding, ExpandError, Flavor, MAX_OUTPUT, Origin, Pattern,
    PatternVariable, TargetScope, VariableStore, reference_end,
};
use std::borrow::Cow;
use std::cell::{Cell, OnceCell};
use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

fn source_read_error_at(
    file: SourceName,
    detail: Vec<u8>,
    context: &mut ExpansionContext<'_>,
) -> ParseError {
    let diagnostic =
        context.freeze_diagnostic(Diagnostic::unlocated_system(Message::SourceError {
            path: file.as_ref(),
            detail: &detail,
        }));
    ParseError {
        location: Location { file, line: 1 },
        unlocated: true,
        message: diagnostic,
    }
}

struct IncludeWork {
    paths: std::vec::IntoIter<Vec<u8>>,
    retained: usize,
    optional: bool,
    location: Location,
}
struct ReadFrame<'a> {
    reader: ParserReader<'a>,
    source_name: SourceName,
    evaluated_at: Option<usize>,
    declaration: RuleDeclaration,
    conditions: Vec<Conditional>,
    last_line: usize,
    pending: Option<IncludeWork>,
    extra: bool,
    charged: usize,
}
impl<'a> ReadFrame<'a> {
    fn new(
        reader: ParserReader<'a>,
        file: SourceName,
        evaluated_at: Option<usize>,
        extra: bool,
    ) -> Self {
        Self {
            reader,
            source_name: file,
            evaluated_at,
            declaration: RuleDeclaration::default(),
            conditions: Vec::new(),
            last_line: 0,
            pending: None,
            extra,
            charged: 0,
        }
    }
    fn account(&mut self, used: &mut usize) -> Result<(), ParseError> {
        if !self.extra {
            return Ok(());
        }
        let cost = 1024usize
            .saturating_add(self.reader.retained_bytes())
            .saturating_add(self.conditions.capacity() * std::mem::size_of::<Conditional>())
            .saturating_add(self.pending.as_ref().map_or(0, |p| p.retained))
            .saturating_add(self.declaration.prerequisites.capacity())
            .saturating_add(
                self.declaration
                    .targets
                    .iter()
                    .map(Vec::capacity)
                    .sum::<usize>(),
            );
        let next = used.saturating_sub(self.charged).saturating_add(cost);
        if next > MAX_OUTPUT {
            return Err(ParseError {
                unlocated: false,
                location: Location {
                    file: self.source_name.clone(),
                    line: self.last_line.max(1),
                },
                message: "rmake: include continuation storage limit exceeded".into(),
            });
        }
        *used = next;
        self.charged = cost;
        Ok(())
    }
}

struct LoadHost<'a, 'b> {
    database: &'a mut Database,
    context: ExpansionContext<'b>,
}

impl crate::native::NativeHost for LoadHost<'_, '_> {
    fn loader_event(&mut self, event: crate::native::LoaderEvent<'_>) {
        self.context.loader_event(event);
    }

    fn guile_callback(&mut self, callback: crate::native::GuileCallback<'_>) {
        self.context.guile_callback(callback);
    }

    fn registered_function_name(&mut self, name: &[u8]) {
        if let Some(history) = self.database.database_history() {
            history.intern(name);
        }
    }
    fn expand(&mut self, input: &[u8]) -> Result<Vec<u8>, ExpandError> {
        self.database.expand_with(input, self.context.reborrow())
    }

    fn evaluate(&mut self, input: &[u8], location: Option<&Location>) -> Result<(), ExpandError> {
        let location = location.or(self.context.location);
        self.database
            .read_evaluated(input, location, self.context.reborrow())
            .map_err(crate::variables::evaluation_error)
    }

    fn emit(&mut self, event: Output<'_>) -> bool {
        self.context.emit(event)
    }
}

/// The initial suffix order of the pinned GNU Make 4.4.1 Linux profile.
/// This metadata does not install implicit rules or suffix-rule conversions.
pub const DEFAULT_SUFFIXES: &[u8] = b".out .a .ln .o .c .cc .C .cpp .p .f .F .m .r .y .l .ym .yl .s .S .mod .sym .def .h .info .dvi .tex .texinfo .texi .txinfo .w .ch .web .sh .elc .el";

#[derive(Clone, Debug)]
pub struct Recipe {
    pub bytes: Vec<u8>,
    pub location: Location,
    /// Remember the prefix used to read this recipe; .ONESHELL needs it after
    /// expansion even if a later assignment changes .RECIPEPREFIX.
    pub prefix: u8,
    /// Installed declarative defaults, independent of source-name spelling.
    pub builtin: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Rule {
    /// Ordered occurrences, including duplicates and each declaration's original
    /// normal/order-only classification. Splitting these into two lists loses
    /// traversal order when multiple rule declarations are merged.
    pub dependencies: Vec<Dependency>,
    /// None means no recipe; Some(empty) is an explicit empty recipe.
    pub recipes: Option<Vec<Recipe>>,
    pub recipe_location: Option<Location>,
    pub stem: Option<Vec<u8>>,
    pub prerequisite_location: Option<Location>,
    /// Only deferred occurrences need clause metadata. Ordinary dependencies
    /// retain their compact representation and are never expanded again.
    pub secondary: Option<Box<Vec<SecondaryPrerequisite>>>,
    pub double_colon: Option<Vec<Rule>>,
    /// Double-colon pattern rules cannot infer missing prerequisites.
    pub terminal: bool,
    /// Runtime-selected .DEFAULT commands use the target itself as $<.
    pub default_recipe: bool,
    /// A filename carrying metadata/dependencies without a target declaration.
    pub record_only: bool,
    /// The selected implicit pattern controls pattern-level special flags;
    /// explicit/static/default rules do not inherit arbitrary matching flags.
    pub implicit_pattern: Option<Vec<u8>>,
    /// Rare multi-output metadata stays out of ordinary rule clones.
    pub group: Option<Box<RuleGroup>>,
}

#[derive(Clone, Debug, Default)]
pub struct RuleGroup {
    /// Declaration order, containing patterns until implicit selection.
    pub targets: Vec<Vec<u8>>,
    /// One explicit &:: declaration shared by its per-target branches.
    pub double_id: Option<usize>,
    /// Distinguish repeated occurrences of one file inside a declaration.
    pub ordinal: usize,
    /// A multi-output implicit rule cannot recur through a sibling pattern.
    pub implicit_identity: Option<usize>,
    pub implicit_peer_patterns: Vec<(Vec<u8>, Vec<u8>)>,
}

#[derive(Clone, Debug)]
pub struct ImplicitCandidate {
    /// Stable within this database; a rule cannot recur in its own chain.
    pub identity: usize,
    pub match_anything: bool,
    pub rule: Rule,
    /// Only deferred prerequisites need directory reconstruction at runtime.
    pub secondary_directory: Option<Vec<u8>>,
    /// Immediate prerequisites which contain a stem pattern, before replacement.
    pub stem_dependencies: Vec<usize>,
}

/// A declaration stays open across blank/comment and conditional lines. Rule
/// diagnostics become observable only when the next statement closes it.
#[derive(Default)]
struct RuleDeclaration {
    targets: Vec<Vec<u8>>,
    offsets: HashMap<Vec<u8>, usize>,
    location: Option<Location>,
    recipe_started: bool,
    ignore_recipes: bool,
    branches: HashMap<Vec<u8>, Vec<usize>>,
    patterns: HashMap<Vec<u8>, Pattern>,
    target_patterns: Vec<bool>,
    implicit: HashMap<Vec<u8>, usize>,
    implicit_group_identity: Option<usize>,
    warnings: Vec<(Location, Diagnostic)>,
    cancel_default: bool,
    grouped: bool,
    group_warnings: Vec<(Location, Diagnostic)>,
    grouped_double_id: Option<usize>,
    header: Option<RuleHeader>,
    prerequisites: Vec<u8>,
    recipes: Vec<Recipe>,
}

struct RuleHeader {
    static_pattern: Option<Pattern>,
    double_colon: bool,
}

impl RuleDeclaration {
    fn has_recipe_context(&self) -> bool {
        self.ignore_recipes || !self.targets.is_empty()
    }

    fn finish(
        &mut self,
        rules: &mut HashMap<Vec<u8>, Rule>,
        warnings: &mut Vec<(Location, Diagnostic)>,
        context: &mut ExpansionContext<'_>,
    ) -> Result<(), ParseError> {
        if self.grouped
            && !self.recipe_started
            && self.recipes.is_empty()
            && !self.targets.is_empty()
        {
            return Err(ParseError {
                unlocated: false,
                location: self.location.clone().expect("group header location"),
                message: "grouped targets must provide a recipe".into(),
            });
        }
        if self.cancel_default
            && !self.recipe_started
            && let Some(rule) = rules.get_mut(b".DEFAULT".as_slice())
        {
            rule.recipes = None;
            rule.recipe_location = None;
        }
        self.cancel_default = false;
        for (location, message) in self.warnings.drain(..) {
            record_warning(warnings, context, location, message);
        }
        for (location, message) in self.group_warnings.drain(..).rev() {
            record_warning(warnings, context, location, message);
        }
        self.grouped = false;
        self.grouped_double_id = None;
        self.targets.clear();
        self.offsets.clear();
        self.branches.clear();
        self.patterns.clear();
        self.target_patterns.clear();
        self.implicit.clear();
        self.implicit_group_identity = None;
        self.location = None;
        self.recipe_started = false;
        self.ignore_recipes = false;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dependency {
    pub name: Vec<u8>,
    pub order_only: bool,
    /// Participates in traversal and freshness, but not recipe/secondary autos.
    pub hidden: bool,
}

impl Dependency {
    /// A wait marks the next prerequisite in the same normal or order-only
    /// partition. A marker left at the end of a partition is discarded.
    pub fn append_to_partition(
        self,
        dependencies: &mut Vec<Self>,
        pending_wait: &mut Option<Self>,
    ) {
        if self.name == b".WAIT" {
            *pending_wait = Some(self);
            return;
        }
        if let Some(wait) = pending_wait.take()
            && wait.order_only == self.order_only
        {
            dependencies.push(wait);
        }
        dependencies.push(self);
    }
}

#[derive(Clone, Debug)]
pub struct SecondaryPrerequisite {
    pub index: usize,
    /// Replacing a deferred occurrence drops the targeted wait attached to it.
    pub discard_targeted_wait: bool,
    /// A static declaration's stem can differ from the target's final stem.
    pub stem: Option<Vec<u8>>,
}

impl Rule {
    fn insert_targeted_waits(&mut self) {
        if self.dependencies.len() < 2 {
            return;
        }
        let original = std::mem::take(&mut self.dependencies);
        self.dependencies.reserve(original.len() * 2);
        let mut deferred = self
            .secondary
            .iter_mut()
            .flat_map(|clauses| clauses.iter_mut())
            .peekable();
        let mut explicit_wait = false;
        let mut has_predecessor = false;
        for (index, dependency) in original.into_iter().enumerate() {
            if dependency.name == b".WAIT" {
                explicit_wait = true;
                continue;
            }
            if has_predecessor || explicit_wait {
                self.dependencies.push(Dependency {
                    name: b".WAIT".to_vec(),
                    order_only: dependency.order_only,
                    hidden: false,
                });
            }
            if let Some(clause) = deferred.next_if(|clause| clause.index == index) {
                clause.index = self.dependencies.len();
                clause.discard_targeted_wait = has_predecessor && !explicit_wait;
            }
            self.dependencies.push(dependency);
            explicit_wait = false;
            has_predecessor = true;
        }
    }

    pub fn append_prerequisites(&mut self, other: &Self) {
        let offset = self.dependencies.len();
        self.dependencies.extend(other.dependencies.iter().cloned());
        if let Some(secondary) = &other.secondary {
            self.secondary.get_or_insert_with(Default::default).extend(
                secondary.iter().cloned().map(|mut clause| {
                    clause.index += offset;
                    clause
                }),
            );
        }
    }
    pub fn grouped_targets(&self) -> Option<&[Vec<u8>]> {
        self.group.as_ref().map(|group| group.targets.as_slice())
    }

    pub fn grouped_double_id(&self) -> Option<usize> {
        self.group.as_ref().and_then(|group| group.double_id)
    }

    pub fn grouped_double_ordinal(&self) -> usize {
        self.group.as_ref().map_or(0, |group| group.ordinal)
    }

    pub fn normal_prerequisites(&self) -> impl Iterator<Item = &[u8]> {
        self.dependencies
            .iter()
            .filter(|dependency| !dependency.order_only && dependency.name != b".WAIT")
            .map(|dependency| dependency.name.as_slice())
    }

    /// Automatic variables ^, ? and | use first-occurrence order, with any
    /// normal occurrence promoting the name from order-only to normal. The raw
    /// occurrences remain available for <, + and repeated circular diagnostics.
    pub fn canonical_dependencies(&self) -> Vec<Dependency> {
        self.canonical(false)
    }

    pub fn automatic_dependencies(&self) -> Vec<Dependency> {
        self.canonical(true)
    }

    fn canonical(&self, visible_only: bool) -> Vec<Dependency> {
        let normal: HashSet<_> = self
            .dependencies
            .iter()
            .filter(|dep| !dep.order_only && (!visible_only || !dep.hidden))
            .map(|dep| dep.name.as_slice())
            .collect();
        let mut seen = HashSet::new();
        self.dependencies
            .iter()
            .filter(|dependency| {
                dependency.name != b".WAIT"
                    && (!visible_only || !dependency.hidden)
                    && seen.insert(dependency.name.as_slice())
            })
            .map(|dependency| Dependency {
                name: dependency.name.clone(),
                order_only: !normal.contains(dependency.name.as_slice()),
                hidden: dependency.hidden,
            })
            .collect()
    }
}

#[derive(Debug)]
pub struct ParseError {
    pub location: Location,
    pub message: Diagnostic,
    /// Keep syntax provenance while allowing GNU's program-only fatal messages.
    pub unlocated: bool,
}

impl ParseError {
    fn system(
        location: Location,
        message: Message<'_>,
        context: &mut ExpansionContext<'_>,
    ) -> Self {
        Self {
            unlocated: false,
            location,
            message: context.freeze_diagnostic(Diagnostic::system(message)),
        }
    }

    pub fn source_location(&self) -> Option<&Location> {
        (!self.unlocated).then_some(&self.location)
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.unlocated {
            return write!(f, "{}", self.message);
        }
        write!(
            f,
            "{}:{}: {}",
            String::from_utf8_lossy(&self.location.file),
            self.location.line,
            self.message
        )
    }
}

impl std::error::Error for ParseError {}

/// One pattern-variable declaration. Equal-specificity patterns retain their
/// original interleaved order, including repeated appearances of a pattern.
#[derive(Clone, Debug)]
pub struct PatternAssignment {
    pattern: Pattern,
    variable: PatternVariable,
}

#[derive(Clone, Debug)]
pub struct Makefile {
    pub name: Vec<u8>,
    pub optional: bool,
    pub missing: bool,
    pub location: Option<Location>,
    pub read_error: Option<IncludeError>,
}

#[derive(Clone, Default)]
pub struct Database {
    database_history: Option<crate::database::History>,
    pub file_records: crate::file_records::FileRecords,
    pub variables: VariableStore,
    /// Target-only variable declarations never create rules or select a goal.
    pub target_scopes: HashMap<Vec<u8>, TargetScope>,
    target_scope_revision: u64,
    pub pattern_assignments: Vec<PatternAssignment>,
    pub rules: HashMap<Vec<u8>, Rule>,
    /// Names mentioned by explicit declarations, distinct from inferred graph
    /// nodes. Maintained incrementally so implicit search never scans the graph.
    pub explicit_names: HashSet<Vec<u8>>,
    pub exports: HashSet<Vec<u8>>,
    pub unexports: HashSet<Vec<u8>>,
    pub export_all: bool,
    /// Sticky reader mode selected by an actual .POSIX rule declaration.
    pub posix: bool,
    pub warnings: Vec<(Location, Diagnostic)>,
    pub makefiles: Vec<Makefile>,
    /// Module identities remain distinct while sharing makefile remake traversal.
    pub loaded_modules: HashMap<Vec<u8>, Location>,
    pub(crate) implicit_rules: Vec<(Pattern, Rule)>,
    implicit_rule_shadow: OnceCell<Option<Vec<bool>>>,
    implicit_rule_suffixes: OnceCell<Option<ImplicitRuleSuffixes>>,
    implicit_rule_index_visits: Cell<u8>,
    vpaths: Vec<Vpath>,
    include_depth: usize,
    next_group_id: usize,
    evaluation_depth: usize,
    pub(crate) evaluation_scope: Option<VariableStore>,
    target_read: Option<TargetRead>,
    suspended_target_reads: Vec<VariableStore>,
    rules_frozen: bool,
    suppress_default_goal: bool,
    second_expansion: bool,
    // One warning per category across includes/eval: prerequisites=1, commands=2.
    wait_warnings: u8,
    pub(crate) evaluation_bindings: Vec<crate::variables::LocalFrame>,
    pub(crate) evaluation_active: Option<std::sync::Arc<crate::variables::ActiveContext>>,
    // Active only while parsing a potentially recursive HOME filename. Owned
    // bytes preserve shared-name overwrites without aliased or dangling pointers.
    filename_scratch: Option<Vec<u8>>,
    filename_session_depth: usize,
    runtime_metadata_live: bool,
    canonical_flags_published: bool,
    /// Global bindings being expanded for an environment vector. Target-local
    /// bindings do not refer to the global object removed by undefine.
    pub(crate) exporting: Vec<Vec<u8>>,
}

#[derive(Clone, Default)]
struct ImplicitRuleSuffixes {
    unfiltered: Vec<usize>,
    by_last_byte: HashMap<u8, Vec<usize>>,
}

struct ImplicitRuleIndices<'a> {
    unfiltered: &'a [usize],
    matching: &'a [usize],
}

impl Iterator for ImplicitRuleIndices<'_> {
    type Item = usize;

    fn next(&mut self) -> Option<usize> {
        let source = match (self.unfiltered.first(), self.matching.first()) {
            (Some(left), Some(right)) if left < right => &mut self.unfiltered,
            (_, Some(_)) => &mut self.matching,
            (_, None) => &mut self.unfiltered,
        };
        let (&index, remaining) = source.split_first()?;
        *source = remaining;
        Some(index)
    }
}

// Replacement compares ordered prerequisite names independently of edge flags.
struct ImplicitRuleKey<'a> {
    pattern: &'a Pattern,
    prerequisites: &'a [Dependency],
}

impl PartialEq for ImplicitRuleKey<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.pattern == other.pattern
            && self
                .prerequisites
                .iter()
                .map(|dep| &dep.name)
                .eq(other.prerequisites.iter().map(|dep| &dep.name))
    }
}

impl Eq for ImplicitRuleKey<'_> {}

impl Hash for ImplicitRuleKey<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.pattern.hash(state);
        self.prerequisites.len().hash(state);
        for dep in self.prerequisites {
            dep.name.hash(state);
        }
    }
}

#[derive(Clone)]
struct TargetRead {
    target: Vec<u8>,
    global_lexical: crate::variables::DeclarationLexical,
}

enum TargetReadOwner {
    Current,
    Suspended(usize),
    Retained,
}

/// Compiled pure lookup policy, kept consistent by the directive reader.
/// Filesystem observations and mutable Make variables are never stored here.
#[derive(Clone)]
struct Vpath {
    key: Vec<u8>,
    pattern: Pattern,
    directories: Vec<Vec<u8>>,
}

impl Database {
    /// Start before imports/defaults; enabling after observation would lose
    /// history and cannot produce a compatible database dump.
    pub fn enable_database_history(&mut self) -> Result<(), &'static str> {
        if self.database_history.is_some() {
            return Ok(());
        }
        if self.variables.definitions().next().is_some()
            || self.file_records.names().next().is_some()
        {
            return Err("database history must start before definitions and file records");
        }
        let history = crate::database::History::default();
        self.variables
            .enable_database_history(history.variable_table());
        self.file_records.enable_database_history(history.clone());
        self.database_history = Some(history);
        Ok(())
    }

    pub fn database_history(&self) -> Option<&crate::database::History> {
        self.database_history.as_ref()
    }

    pub fn activate_runtime_metadata(&mut self) {
        self.runtime_metadata_live = true;
    }

    /// Activate a target's local bindings over its inherited execution context.
    /// Descendant visibility and append layers are preserved by the store.
    pub fn variables_for_target_with(
        &mut self,
        target: &[u8],
        parent: &VariableStore,
        context: ExpansionContext<'_>,
    ) -> Result<VariableStore, ExpandError> {
        let pattern = self.prepare_pattern_scope(target, context)?;
        Ok(self.variables_for_target_prepared(target, parent, pattern.as_ref()))
    }

    /// Activate matching pattern bindings separately from parent inheritance.
    /// A temporary implicit-chain search has no parent or explicit bindings;
    /// a selected chain reuses this activation when it later acquires a parent.
    pub fn prepare_pattern_scope(
        &mut self,
        target: &[u8],
        mut context: ExpansionContext<'_>,
    ) -> Result<Option<TargetScope>, ExpandError> {
        let matches_target = |assignment: &PatternAssignment| {
            assignment
                .pattern
                .matches(target)
                .is_some_and(|stem| !stem.is_empty())
        };
        let mut matching = self
            .pattern_assignments
            .iter()
            .enumerate()
            .filter_map(|(index, assignment)| matches_target(assignment).then_some(index))
            .collect::<Vec<_>>();
        // More fixed bytes mean a shorter stem, hence higher precedence.
        // Pop in increasing specificity, with declaration order at each rank.
        let order = |assignments: &[PatternAssignment], index: usize| {
            (assignments[index].pattern.fixed_len(), index)
        };
        matching.sort_unstable_by_key(|&index| Reverse(order(&self.pattern_assignments, index)));
        // Most targets have no matching pattern bindings. Do not allocate an
        // empty scope for them; matching declarations still run in full order.
        let mut pattern_scope = None;
        while let Some(index) = matching.pop() {
            let scope = pattern_scope.get_or_insert_with(TargetScope::default);
            let prepared = &self.pattern_assignments[index].variable;
            let aliases_global = if prepared.needs_database_expansion(scope) {
                // A deferred expression can reenter the reader and add
                // pattern declarations. Keep the current binding independent
                // of storage that can grow during that expansion.
                let prepared = prepared.clone();
                let before = self.pattern_assignments.len();
                let mut globals = std::mem::take(&mut self.variables);
                let result = scope.apply_pattern_with_using(
                    &mut globals,
                    &prepared,
                    context.reborrow(),
                    |globals, scope, bytes, shell, context| {
                        std::mem::swap(globals, &mut self.variables);
                        let result = if shell {
                            crate::variables::shell_scoped_database(self, scope, bytes, context)
                        } else {
                            crate::variables::expand_scoped_database::<false>(
                                self, scope, bytes, context,
                            )
                        };
                        std::mem::swap(globals, &mut self.variables);
                        result
                    },
                );
                self.variables = globals;
                let aliases_global = result?;
                if before != self.pattern_assignments.len() {
                    // Declarations inserted before the active binding can
                    // only affect later file activations.
                    let current = order(&self.pattern_assignments, index);
                    matching.extend((before..self.pattern_assignments.len()).filter(|&added| {
                        matches_target(&self.pattern_assignments[added])
                            && order(&self.pattern_assignments, added) > current
                    }));
                    matching.sort_unstable_by_key(|&index| {
                        Reverse(order(&self.pattern_assignments, index))
                    });
                }
                aliases_global
            } else {
                scope.apply_pattern_with(&mut self.variables, prepared, context.reborrow())?
            };
            if aliases_global {
                // A suppressed conditional can return the global binding. Its
                // modifiers then replace global export policy too.
                let name = self.pattern_assignments[index].variable.name();
                match self
                    .variables
                    .get(name)
                    .and_then(|variable| variable.export_policy())
                {
                    Some(true) => {
                        self.unexports.remove(name);
                        self.exports.insert(name.to_vec());
                    }
                    Some(false) => {
                        self.exports.remove(name);
                        self.unexports.insert(name.to_vec());
                    }
                    None => {
                        self.exports.remove(name);
                        self.unexports.remove(name);
                    }
                }
            }
        }
        Ok(pattern_scope)
    }

    pub fn variables_for_target_prepared(
        &self,
        target: &[u8],
        parent: &VariableStore,
        pattern: Option<&TargetScope>,
    ) -> VariableStore {
        let mut parent = parent.clone();
        self.refresh_scoped_variables(&mut parent);
        let empty;
        let scope = if let Some(scope) = self.target_scopes.get(target) {
            scope
        } else {
            empty = TargetScope::for_target(target);
            &empty
        };
        if let Some(pattern) = pattern {
            parent.with_target_scopes(&[pattern, scope])
        } else {
            parent.with_target_scopes(&[scope])
        }
    }

    /// Refresh a cached file context without rerunning its pattern effects.
    pub fn refresh_scoped_variables(&self, variables: &mut VariableStore) {
        variables.refresh_globals(&self.variables);
        variables.refresh_declared_scopes(&self.target_scopes, self.target_scope_revision);
    }

    pub fn merge_target_scope(&mut self, from: &[u8], to: &[u8]) {
        if let Some(scope) = self.target_scopes.remove(from) {
            self.target_scopes
                .entry(to.to_vec())
                .or_insert_with(|| TargetScope::for_target(to))
                .merge_missing(&scope);
            self.target_scope_revision = self.target_scope_revision.wrapping_add(1);
        }
    }

    pub fn has_vpaths(&self) -> bool {
        !self.vpaths.is_empty()
    }

    pub fn vpath_directories<'a>(&'a self, target: &'a [u8]) -> impl Iterator<Item = &'a [u8]> {
        self.vpaths
            .iter()
            .filter(move |path| path.pattern.matches(target).is_some())
            .flat_map(|path| path.directories.iter().map(Vec::as_slice))
    }

    /// Library selection compares path positions across different patterns.
    /// Keep the index of nonmatching declarations so ranks remain comparable.
    pub fn ranked_vpath_directories<'a>(
        &'a self,
        target: &'a [u8],
    ) -> impl Iterator<Item = ((usize, usize), &'a [u8])> {
        self.vpaths
            .iter()
            .enumerate()
            .filter(move |(_, path)| path.pattern.matches(target).is_some())
            .flat_map(|(group, path)| {
                path.directories
                    .iter()
                    .filter(|directory| directory.as_slice() != b".")
                    .enumerate()
                    .map(move |(index, directory)| ((group, index), directory.as_slice()))
            })
    }

    /// Borrow declaration data for diagnostics without searching or expanding.
    pub fn database_pattern_variables(
        &self,
    ) -> impl Iterator<Item = (&Pattern, crate::database::PatternVariableView<'_>)> {
        self.pattern_assignments
            .iter()
            .map(|declaration| (&declaration.pattern, declaration.variable.database_view()))
    }

    pub fn database_implicit_rules(&self) -> &[(Pattern, Rule)] {
        &self.implicit_rules
    }

    pub fn database_vpaths(&self) -> impl Iterator<Item = crate::database::VpathView<'_>> {
        self.vpaths.iter().map(|path| crate::database::VpathView {
            pattern: &path.key,
            directories: &path.directories,
        })
    }

    pub fn vpath_group_count(&self) -> usize {
        self.vpaths.len()
    }

    /// Produce candidates in stem-length and declaration order, without I/O.
    /// The runtime checks prerequisites and commits only a viable candidate.
    pub fn implicit_candidates(&self, target: &[u8]) -> Vec<ImplicitCandidate> {
        self.implicit_candidates_excluding(target, &[])
    }

    /// Source syntax for an existing implicit candidate; no expansion or search.
    pub fn implicit_rule_source(&self, identity: usize) -> Option<(&Pattern, &Rule)> {
        self.implicit_rules
            .get(identity)
            .map(|(pattern, rule)| (pattern, rule))
    }

    pub fn implicit_candidates_excluding(
        &self,
        target: &[u8],
        active: &[usize],
    ) -> Vec<ImplicitCandidate> {
        self.implicit_candidates_in(
            target,
            active,
            crate::filenames::archive_parts(target).is_some(),
        )
    }

    /// The second archive search omits the archive path, but retains directory
    /// bytes within the member stem. Search identities remain the originals.
    pub fn implicit_archive_candidates_excluding(
        &self,
        target: &[u8],
        active: &[usize],
    ) -> Vec<ImplicitCandidate> {
        let Some((_, member)) = crate::filenames::archive_parts(target) else {
            return Vec::new();
        };
        let name = [b"(".as_slice(), member, b")"].concat();
        // GNU keeps peer spellings from the member-only pattern. Inserting an
        // archive prefix here would incorrectly share different member goals.
        self.implicit_candidates_in(&name, active, true)
    }

    pub(crate) fn invalidate_implicit_rule_indexes(&mut self) {
        self.implicit_rule_shadow.take();
        self.implicit_rule_suffixes.take();
        self.implicit_rule_index_visits.set(0);
    }

    fn compile_implicit_rule_suffixes(&self) -> Option<ImplicitRuleSuffixes> {
        let mut suffixes = ImplicitRuleSuffixes::default();
        suffixes.by_last_byte.try_reserve(256).ok()?;
        for (index, (pattern, _)) in self.implicit_rules.iter().enumerate() {
            let indices = match pattern.trailing_literal_byte() {
                Some(byte) => suffixes.by_last_byte.entry(byte).or_default(),
                None => &mut suffixes.unfiltered,
            };
            indices.try_reserve(1).ok()?;
            indices.push(index);
        }
        Some(suffixes)
    }

    fn use_implicit_rule_indexes(&self) -> bool {
        let count = self.implicit_rules.len();
        if count >= 128 {
            return true;
        }
        if count < 64 {
            return false;
        }
        // Stock builtins and other medium tables amortize the indexes only
        // when the invocation makes repeated searches.
        const WARMUP: u8 = 16;
        let visits = self.implicit_rule_index_visits.get();
        if visits == WARMUP {
            return true;
        }
        self.implicit_rule_index_visits.set(visits + 1);
        false
    }

    fn implicit_rule_indices(&self, target: &[u8]) -> Option<ImplicitRuleIndices<'_>> {
        let count = self.implicit_rules.len();
        let suffixes = self
            .implicit_rule_suffixes
            .get_or_init(|| self.compile_implicit_rule_suffixes())
            .as_ref()?;
        // Removing a directory prefix leaves any literal suffix byte unchanged.
        // Patterns without a literal suffix always keep their declaration position.
        let matching = target
            .last()
            .and_then(|byte| suffixes.by_last_byte.get(byte))
            .map_or(&[][..], Vec::as_slice);
        let unfiltered = suffixes.unfiltered.as_slice();
        // Dense selections are cheaper to visit directly.
        (unfiltered.len() + matching.len() <= count / 2).then_some(ImplicitRuleIndices {
            unfiltered,
            matching,
        })
    }

    fn compile_implicit_rule_shadow(&self) -> Option<Vec<bool>> {
        let count = self.implicit_rules.len();
        let mut shadowed = Vec::new();
        let mut later = HashSet::new();
        shadowed.try_reserve_exact(count).ok()?;
        later.try_reserve(count).ok()?;
        for (pattern, rule) in self.implicit_rules.iter().rev() {
            let duplicate = !later.insert(ImplicitRuleKey {
                pattern,
                prerequisites: &rule.dependencies,
            });
            shadowed.push(
                duplicate
                    && rule.grouped_targets().is_none_or(|peers| {
                        peers.iter().all(|peer| peer == pattern.literal_bytes())
                    }),
            );
        }
        shadowed.reverse();
        Some(shadowed)
    }

    fn implicit_candidates_in(
        &self,
        target: &[u8],
        active: &[usize],
        archive: bool,
    ) -> Vec<ImplicitCandidate> {
        let mut result = Vec::new();
        let mut specific = false;
        let directory = if archive {
            b"".as_slice()
        } else {
            crate::implicit::directory_prefix(target)
        };
        let use_indexes = self.use_implicit_rule_indexes();
        let shadow_cache = use_indexes.then_some(&self.implicit_rule_shadow);
        let mut visit = |(index, (pattern, rule)): (usize, &(Pattern, Rule))| {
            let identity = rule
                .group
                .as_ref()
                .and_then(|group| group.implicit_identity)
                .unwrap_or(index);
            if active.contains(&identity) {
                return;
            }
            let name = pattern.literal_bytes();
            let stripped = !name.contains(&b'/');
            let word = if stripped {
                &target[directory.len()..]
            } else {
                target
            };
            let Some(stem) = pattern.matches(word).filter(|stem| !stem.is_empty()) else {
                return;
            };
            let shadowed = shadow_cache.and_then(|cache| {
                cache
                    .get_or_init(|| self.compile_implicit_rule_shadow())
                    .as_ref()
            });
            let replaced = shadowed.map_or_else(
                || {
                    self.implicit_rules[index + 1..]
                        .iter()
                        .any(|(later_pattern, later_rule)| {
                            // Distinct-output groups retain their declaration priority.
                            pattern == later_pattern
                                && rule
                                    .grouped_targets()
                                    .is_none_or(|peers| peers.iter().all(|peer| peer == name))
                                && rule
                                    .dependencies
                                    .iter()
                                    .map(|dep| &dep.name)
                                    .eq(later_rule.dependencies.iter().map(|dep| &dep.name))
                        })
                },
                |shadowed| shadowed[index],
            );
            if replaced {
                return;
            }
            specific |= name != b"%" && (rule.recipes.is_some() || rule.dependencies.is_empty());
            if rule.recipes.is_none() {
                return;
            }
            let mut candidate = rule.clone();
            candidate.implicit_pattern = Some(name.to_vec());
            candidate.stem = Some(if stripped {
                [directory, stem].concat()
            } else {
                stem.to_vec()
            });
            if let Some(group) = &mut candidate.group {
                for peer in &mut group.targets {
                    let pattern = peer.clone();
                    *peer = Pattern::new(peer)
                        .replace(candidate.stem.as_deref().unwrap_or_default())
                        .expect("a parsed pattern can substitute its selected stem");
                    group.implicit_peer_patterns.push((peer.clone(), pattern));
                }
            }
            let mut valid = true;
            let mut stem_dependencies = Vec::new();
            for index in 0..candidate.dependencies.len() {
                if candidate
                    .secondary
                    .as_ref()
                    .is_some_and(|clauses| clauses.iter().any(|clause| clause.index == index))
                {
                    continue;
                }
                let original = &candidate.dependencies[index].name;
                let percent = original.iter().position(|byte| *byte == b'%');
                if percent.is_some() {
                    stem_dependencies.push(index);
                }
                // Implicit prerequisites substitute their first literal %;
                // target-pattern backslash quoting does not apply here. Keep
                // unchanged literals borrowed until a real rewrite is needed.
                let substituted = if let Some(percent) = percent {
                    let directory = if stripped { directory } else { b"" };
                    let Some(capacity) = original
                        .len()
                        .checked_sub(1)
                        .and_then(|n| n.checked_add(stem.len()))
                        .and_then(|n| n.checked_add(directory.len()))
                        .filter(|n| *n <= MAX_OUTPUT)
                    else {
                        valid = false;
                        break;
                    };
                    let mut value = Vec::with_capacity(capacity);
                    value.extend_from_slice(directory);
                    value.extend_from_slice(&original[..percent]);
                    value.extend_from_slice(stem);
                    value.extend_from_slice(&original[percent + 1..]);
                    Cow::Owned(value)
                } else {
                    Cow::Borrowed(original.as_slice())
                };
                let normalized = crate::filenames::without_dot_prefix(&substituted);
                if normalized != original.as_slice() {
                    candidate.dependencies[index].name = if normalized.len() == substituted.len() {
                        substituted.into_owned()
                    } else {
                        normalized.to_vec()
                    };
                }
            }
            if valid {
                result.push(ImplicitCandidate {
                    identity,
                    match_anything: name == b"%",
                    rule: candidate,
                    secondary_directory: (rule.secondary.is_some()
                        && stripped
                        && !directory.is_empty())
                    .then(|| directory.to_vec()),
                    stem_dependencies,
                });
            }
        };
        if use_indexes && let Some(indices) = self.implicit_rule_indices(target) {
            for index in indices {
                visit((index, &self.implicit_rules[index]));
            }
        } else {
            self.implicit_rules.iter().enumerate().for_each(visit);
        }
        result
            .retain(|candidate| !specific || !candidate.match_anything || candidate.rule.terminal);
        result.sort_by_key(|candidate| candidate.rule.stem.as_ref().map_or(0, Vec::len));
        result
    }

    /// Initialize suffix metadata before reading user Makefiles. SUFFIXES is an
    /// independent default variable, defined even when builtin rules are off.
    /// Existing higher-priority variables and explicit rules are preserved.
    pub fn initialize_suffixes(&mut self, builtin_rules: bool) -> Result<(), ExpandError> {
        if let Some(history) = &self.database_history {
            history.intern(b".SUFFIXES");
        }
        self.file_records.insert_builtin(b".SUFFIXES");
        let suffixes = if builtin_rules { DEFAULT_SUFFIXES } else { b"" };
        for name in words(suffixes) {
            if let Some(history) = &self.database_history {
                history.intern(name);
            }
            if let Some(history) = &self.database_history {
                history.record_file_lookup(name);
            }
            if !self.file_records.contains(name) {
                self.file_records.insert_builtin(name);
            }
        }
        self.variables
            .define_literal(b"SUFFIXES", suffixes, Origin::Default)?;
        if builtin_rules {
            self.rules
                .entry(b".SUFFIXES".to_vec())
                .or_insert_with(|| Rule {
                    record_only: true,
                    dependencies: words(suffixes)
                        .map(|name| Dependency {
                            name: name.to_vec(),
                            order_only: false,
                            hidden: false,
                        })
                        .collect(),
                    ..Rule::default()
                });
        }
        Ok(())
    }

    /// Explicit targets use the first matching suffix in the final raw list.
    /// Matching preserves the complete target path and arbitrary filename bytes;
    /// order-only suffix entries participate just like normal entries.
    pub fn explicit_stem<'a>(&self, target: &'a [u8]) -> &'a [u8] {
        self.rules
            .get(b".SUFFIXES".as_slice())
            .and_then(|rule| {
                rule.dependencies
                    .iter()
                    .find_map(|dependency| target.strip_suffix(dependency.name.as_slice()))
            })
            .unwrap_or_default()
    }

    /// Default-goal selection skips single- and double-suffix rule names using
    /// the suffix metadata already present when the declaration is read. This
    /// recognition does not convert the declaration into an implicit rule.
    fn is_suffix_rule_name(&self, target: &[u8]) -> bool {
        self.rules.get(b".SUFFIXES".as_slice()).is_some_and(|rule| {
            rule.dependencies.iter().any(|suffix| {
                target == suffix.name
                    || target
                        .strip_prefix(suffix.name.as_slice())
                        .is_some_and(|remaining| {
                            rule.dependencies
                                .iter()
                                .any(|other| remaining == other.name)
                        })
            })
        })
    }
}

fn stem_words(bytes: &[u8]) -> impl Iterator<Item = &[u8]> {
    bytes
        .split(|byte| matches!(byte, b' ' | b'\t'..=b'\r'))
        .filter(|word| !word.is_empty())
}

/// GNU validates the whole target against the static pattern, then derives its
/// stem by substituting each word. Embedded blanks therefore matter twice.
fn static_stem(pattern: &Pattern, target: &[u8]) -> Vec<u8> {
    let mut stem = Vec::new();
    for word in stem_words(target) {
        let part = pattern.matches(word).unwrap_or(word);
        if part.is_empty() {
            continue;
        }
        if !stem.is_empty() {
            stem.push(b' ');
        }
        stem.extend_from_slice(part);
    }
    stem
}

fn static_prerequisite(name: &[u8], stem: &[u8]) -> Result<Vec<u8>, ExpandError> {
    let pattern = Pattern::new(name);
    if !pattern.has_wildcard() {
        return Ok(name.to_vec());
    }
    if stem.is_empty() {
        return pattern.replace(stem);
    }
    let mut result = Vec::new();
    for word in stem_words(stem) {
        let name = pattern.replace(word)?;
        if name.is_empty() {
            continue;
        }
        if result
            .len()
            .saturating_add(name.len() + usize::from(!result.is_empty()))
            > MAX_OUTPUT
        {
            return Err(ExpandError::ExpansionLimit);
        }
        if !result.is_empty() {
            result.push(b' ');
        }
        result.extend_from_slice(&name);
    }
    Ok(result)
}

/// GNU's deferred static prerequisite pass rewrites the first percent in each
/// whitespace-delimited word to a stem reference, even inside surviving syntax.
fn static_secondary_prerequisites(bytes: &[u8]) -> Vec<u8> {
    let mut rewritten = Vec::with_capacity(bytes.len());
    let mut replaced = false;
    for &byte in bytes {
        if byte.is_ascii_whitespace() || byte == 0x0b {
            replaced = false;
        }
        if byte == b'%' && !replaced {
            rewritten.extend_from_slice(b"$*");
            replaced = true;
        } else {
            rewritten.push(byte);
        }
    }
    rewritten
}

#[cfg(test)]
mod include_tests {
    use super::*;
    use crate::diagnostic::IncludedFile;

    #[test]
    fn evaluated_append_reads_selected_file_scope_then_defines_globally() {
        let mut database = Database::default();
        database.enable_database_history().unwrap();
        database
            .read(
                b"X := global\nall: X = scope\nall: ONLY = present\n",
                b"Makefile",
            )
            .unwrap();
        let context = database
            .variables
            .with_target_scopes(&[&database.target_scopes[b"all".as_slice()]]);
        let file_before = context.database_stats().unwrap();
        let global_before = database.variables.database_stats().unwrap();
        database.evaluation_scope = Some(context);
        database
            .define_variable_with(
                AssignmentSpec {
                    name: b"X",
                    value: b"suffix",
                    kind: Assignment::Append,
                },
                Origin::File,
                None,
                ExpansionContext::pure(),
            )
            .unwrap();
        assert_eq!(
            database.variables.get(b"X").unwrap().raw_value(),
            b"scope suffix"
        );
        let scope = database.evaluation_scope.as_ref().unwrap();
        assert_eq!(scope.get(b"X").unwrap().raw_value(), b"scope");
        assert_eq!(
            scope.database_stats().unwrap().lookups - file_before.lookups,
            1
        );
        assert_eq!(
            database.variables.database_stats().unwrap().lookups - global_before.lookups,
            1
        );
        let file_before = scope.database_stats().unwrap();
        let global_before = database.variables.database_stats().unwrap();
        database
            .define_variable_with(
                AssignmentSpec {
                    name: b"ONLY",
                    value: b"replacement",
                    kind: Assignment::Conditional,
                },
                Origin::File,
                None,
                ExpansionContext::pure(),
            )
            .unwrap();
        assert!(database.variables.get(b"ONLY").is_none());
        assert_eq!(
            database
                .evaluation_scope
                .as_ref()
                .unwrap()
                .database_stats()
                .unwrap()
                .lookups
                - file_before.lookups,
            1
        );
        assert_eq!(database.variables.database_stats().unwrap(), global_before);
    }

    #[test]
    fn argument_rhs_has_database_effects_without_makefile_comment_parsing() {
        let mut database = Database::default();
        let mut output = Vec::new();
        let mut emit = |event: Output<'_>| {
            if let Output::Info(bytes) = event {
                output.push(bytes.to_vec());
            }
        };
        database
            .define_argument_with(
                b"X:=$(eval Y:=inner)$(info rhs)value#literal",
                Origin::File,
                ExpansionContext::new(None, &mut emit),
            )
            .unwrap();
        let variable = database.variables.get(b"X").unwrap();
        assert_eq!(variable.raw_value(), b"value#literal");
        assert_eq!(variable.origin(), Origin::File);
        assert!(variable.definition().is_none());
        assert_eq!(database.variables.expand(b"$(Y)").unwrap(), b"inner");
        assert_eq!(output, [b"rhs".to_vec()]);
    }

    #[test]
    fn argument_name_effects_do_not_reparse_semicolons_or_comments() {
        let mut database = Database::default();
        let name = database
            .define_argument_with(
                b"$(eval SIDE:=yes)X=foo;bar #baz",
                Origin::Environment,
                ExpansionContext::pure(),
            )
            .unwrap()
            .unwrap();
        assert_eq!(name, b"X");
        assert_eq!(database.variables.expand(b"$(SIDE)").unwrap(), b"yes");
        assert_eq!(
            database.variables.get(b"X").unwrap().raw_value(),
            b"foo;bar #baz"
        );
        assert!(database.rules.is_empty());
    }

    #[test]
    fn flag_warning_steps_keep_argument_order_active_location_and_final_policy() {
        use crate::diagnostic::{
            FlagAssignment, MakeflagsReceiver, MakeflagsResult, WarningOutput,
        };
        use std::cell::{Cell, RefCell};
        use std::rc::Rc;
        struct Receiver {
            enabled: Rc<Cell<bool>>,
            policies: Rc<RefCell<Vec<bool>>>,
        }
        impl MakeflagsReceiver for Receiver {
            fn decode(
                &mut self,
                request: crate::diagnostic::MakeflagsRequest<'_>,
            ) -> Result<MakeflagsResult, Vec<u8>> {
                let first = !request.value.is_empty();
                if first {
                    // A host may already have decoded the final option state.
                    self.enabled.set(true);
                }
                Ok(MakeflagsResult {
                    eval_reference: false,
                    value: b"--warn-undefined-variables".to_vec(),
                    origin: Origin::File,
                    environment_override: false,
                    include_dirs: Vec::new(),
                    prepare_includes: None,
                    publish_includes: None,
                    assignments: if first {
                        vec![
                            FlagAssignment {
                                argument: b"A:=$(MISSING_A)".to_vec(),
                                environment_override: false,
                                position: 1,
                            },
                            FlagAssignment {
                                argument: b"B:=$(MISSING_B)".to_vec(),
                                environment_override: false,
                                position: 3,
                            },
                        ]
                    } else {
                        Vec::new()
                    },
                    warning_positions: if first { vec![2] } else { Vec::new() },
                    notices: Vec::new(),
                    warning_entry: false,
                    next_argument: 4,
                })
            }
            fn set_warning_policy(&mut self, enabled: bool) {
                self.enabled.set(enabled);
                self.policies.borrow_mut().push(enabled);
            }
        }
        let enabled = Rc::new(Cell::new(false));
        let policies = Rc::new(RefCell::new(Vec::new()));
        let mut receiver = Receiver {
            enabled: enabled.clone(),
            policies: policies.clone(),
        };
        let mut warnings = Vec::new();
        Database::default()
            .read_with(
                b"MAKEFLAGS=seed\nNEXT:=$(MISSING_NEXT)\n",
                b"case.mk",
                ExpansionContext::with_receiver(
                    None,
                    &mut WarningOutput::new(
                        &mut |event| {
                            if let Output::Warning { location, message } = event {
                                warnings.push((location.cloned(), message.to_vec()));
                            }
                        },
                        &enabled,
                    ),
                )
                .with_makeflags(&mut receiver),
            )
            .unwrap();
        assert_eq!(*policies.borrow(), [false, true]);
        assert!(enabled.get());
        assert_eq!(warnings.len(), 2);
        for ((location, message), (line, name)) in
            warnings.iter().zip([(1, "MISSING_B"), (2, "MISSING_NEXT")])
        {
            let location = location.as_ref().unwrap();
            assert_eq!(
                (location.file.as_ref(), location.line),
                (b"case.mk".as_slice(), line)
            );
            assert_eq!(
                message,
                format!("warning: undefined variable '{name}'").as_bytes()
            );
        }
    }

    #[test]
    fn include_metadata_uses_full_database_before_publication_and_flag_normalization() {
        use std::cell::Cell;
        use std::rc::Rc;
        let published = Rc::new(Cell::new(false));
        let pending = published.clone();
        let mut database = Database::default();
        let mut receiver = move |_: crate::diagnostic::MakeflagsRequest<'_>| {
            let pending = pending.clone();
            Ok(crate::diagnostic::MakeflagsResult {
                eval_reference: false,
                value: b"normalized".to_vec(),
                origin: Origin::File,
                environment_override: false,
                include_dirs: vec![
                    b"dir-$(eval include probe.mk)$(eval INSIDE := $(value MAKEFLAGS))".to_vec(),
                ],
                prepare_includes: None,
                publish_includes: Some(Box::new(move || {
                    pending.set(true);
                    crate::diagnostic::MakeflagsMetadata {
                        eval_reference: false,
                        value: b"normalized".to_vec(),
                        origin: Origin::File,
                        environment_override: false,
                    }
                })),
                assignments: Vec::new(),
                warning_positions: Vec::new(),
                notices: Vec::new(),
                warning_entry: false,
                next_argument: 1,
            })
        };
        let mut read = |name: &[u8]| {
            assert_eq!(name, b"probe.mk");
            assert!(!published.get());
            Ok(crate::diagnostic::IncludedFile {
                name: name.to_vec(),
                input: crate::reader::Input::Bytes(b"SEEN := old-search\n".to_vec()),
            })
        };
        database
            .read_with(
                b"MAKEFLAGS = original\nAFTER := $(SEEN):$(INSIDE):$(MAKEFLAGS)\n",
                b"Makefile",
                ExpansionContext::pure()
                    .with_makeflags(&mut receiver)
                    .with_includes(&mut read),
            )
            .unwrap();
        assert!(published.get());
        assert_eq!(
            database.variables.get(b"AFTER").unwrap().raw_value(),
            b"old-search:original:normalized"
        );
    }

    #[test]
    fn failed_search_home_drops_owned_preparation_before_metadata_publication() {
        use crate::diagnostic::{IncludePreparation, MakeflagsResult, PreparedIncludes};
        use std::cell::Cell;
        use std::rc::Rc;
        struct Pending(Rc<Cell<usize>>);
        impl Drop for Pending {
            fn drop(&mut self) {
                self.0.set(self.0.get() + 1);
            }
        }
        impl IncludePreparation for Pending {
            fn next(&mut self) -> Option<Vec<u8>> {
                Some(b"~/inc".to_vec())
            }
            fn directory(&mut self, _: Vec<u8>) {
                panic!("failed HOME reached a directory observation");
            }
            fn finish(self: Box<Self>) -> PreparedIncludes {
                panic!("failed HOME published a search");
            }
        }
        let dropped = Rc::new(Cell::new(0));
        let host_dropped = dropped.clone();
        let mut receiver = move |_: crate::diagnostic::MakeflagsRequest<'_>| {
            Ok(MakeflagsResult {
                eval_reference: false,
                value: b"normalized".to_vec(),
                origin: Origin::File,
                environment_override: false,
                include_dirs: Vec::new(),
                prepare_includes: Some(Box::new(Pending(host_dropped.clone()))),
                publish_includes: None,
                assignments: Vec::new(),
                warning_positions: Vec::new(),
                notices: Vec::new(),
                warning_entry: false,
                next_argument: 1,
            })
        };
        let mut database = Database::default();
        let error = database.read_with(
            b".INCLUDE_DIRS := prior\nHOME = $(eval SIDE:=yes)$(error search-failed)\nMAKEFLAGS = raw\n",
            b"case.mk", ExpansionContext::pure().with_makeflags(&mut receiver),
        ).unwrap_err();
        assert!(
            error
                .message
                .as_bytes()
                .windows(b"search-failed".len())
                .any(|p| p == b"search-failed")
        );
        assert_eq!(dropped.get(), 1);
        assert_eq!(database.variables.get(b"SIDE").unwrap().raw_value(), b"yes");
        assert_eq!(
            database
                .variables
                .get(b".INCLUDE_DIRS")
                .unwrap()
                .raw_value(),
            b"prior"
        );
        assert_eq!(
            database.variables.get(b"MAKEFLAGS").unwrap().raw_value(),
            b"raw"
        );
    }

    #[test]
    fn failed_include_metadata_keeps_prior_search_and_uncanonicalized_flags() {
        use std::cell::Cell;
        use std::rc::Rc;
        let published = Rc::new(Cell::new(false));
        let pending = published.clone();
        let mut database = Database::default();
        let mut receiver = move |_: crate::diagnostic::MakeflagsRequest<'_>| {
            let pending = pending.clone();
            Ok(crate::diagnostic::MakeflagsResult {
                eval_reference: false,
                value: b"normalized".to_vec(),
                origin: Origin::File,
                environment_override: false,
                include_dirs: vec![b"dir-$(eval SIDE:=yes)$(error metadata-failed)".to_vec()],
                prepare_includes: None,
                publish_includes: Some(Box::new(move || {
                    pending.set(true);
                    crate::diagnostic::MakeflagsMetadata {
                        eval_reference: false,
                        value: b"normalized".to_vec(),
                        origin: Origin::File,
                        environment_override: false,
                    }
                })),
                assignments: Vec::new(),
                warning_positions: Vec::new(),
                notices: Vec::new(),
                warning_entry: false,
                next_argument: 1,
            })
        };
        let error = database
            .read_with(
                b"MAKEFLAGS = original\n",
                b"Makefile",
                ExpansionContext::pure().with_makeflags(&mut receiver),
            )
            .unwrap_err();
        assert_eq!(error.message.as_bytes(), b"metadata-failed");
        assert!(!published.get());
        assert_eq!(database.variables.get(b"SIDE").unwrap().raw_value(), b"yes");
        assert_eq!(
            database.variables.get(b"MAKEFLAGS").unwrap().raw_value(),
            b"original"
        );
    }

    #[test]
    fn nested_include_publication_finishes_outer_search_with_current_inner_flags() {
        use std::cell::RefCell;
        use std::rc::Rc;
        let search = Rc::new(RefCell::new(b"initial".to_vec()));
        let options = Rc::new(RefCell::new(Vec::<u8>::new()));
        let mut database = Database::default();
        let host_search = search.clone();
        let mut receiver = move |request: crate::diagnostic::MakeflagsRequest<'_>| {
            let selection = request.value.to_vec();
            *options.borrow_mut() = selection.clone();
            let directories = if selection == b"outer" {
                vec![b"dir-$(eval MAKEFLAGS = inner)$(eval include probe.mk)".to_vec()]
            } else {
                Vec::new()
            };
            let pending_search = host_search.clone();
            let pending_options = options.clone();
            Ok(crate::diagnostic::MakeflagsResult {
                eval_reference: false,
                value: selection.clone(),
                origin: Origin::File,
                environment_override: false,
                include_dirs: directories,
                prepare_includes: None,
                publish_includes: Some(Box::new(move || {
                    *pending_search.borrow_mut() = selection;
                    crate::diagnostic::MakeflagsMetadata {
                        eval_reference: false,
                        value: pending_options.borrow().clone(),
                        origin: Origin::File,
                        environment_override: false,
                    }
                })),
                assignments: Vec::new(),
                warning_positions: Vec::new(),
                notices: Vec::new(),
                warning_entry: false,
                next_argument: 1,
            })
        };
        let mut read = |name: &[u8]| {
            assert_eq!(&*search.borrow(), b"inner");
            Ok(crate::diagnostic::IncludedFile {
                name: name.to_vec(),
                input: crate::reader::Input::Bytes(b"SEEN := inner\n".to_vec()),
            })
        };
        database
            .read_with(
                b"MAKEFLAGS = outer\n",
                b"Makefile",
                ExpansionContext::pure()
                    .with_makeflags(&mut receiver)
                    .with_includes(&mut read),
            )
            .unwrap();
        assert_eq!(&*search.borrow(), b"outer");
        assert_eq!(
            database.variables.get(b"SEEN").unwrap().raw_value(),
            b"inner"
        );
        assert_eq!(
            database.variables.get(b"MAKEFLAGS").unwrap().raw_value(),
            b"inner"
        );
    }

    #[test]
    fn flag_value_eval_runs_before_the_host_callback_and_following_lines() {
        let mut database = Database::default();
        let mut values = Vec::new();
        let mut receiver = |request: crate::diagnostic::MakeflagsRequest<'_>| {
            values.push(request.value.to_vec());
            Ok(crate::diagnostic::MakeflagsResult {
                eval_reference: false,
                value: request.value.to_vec(),
                origin: Origin::File,
                environment_override: false,
                include_dirs: Vec::new(),
                prepare_includes: None,
                publish_includes: None,
                assignments: Vec::new(),
                warning_positions: Vec::new(),
                notices: Vec::new(),
                warning_entry: false,
                next_argument: 1,
            })
        };
        database
            .read_with(
                b"MAKEFLAGS = $(eval SEEN := yes)-k\nLATER := $(SEEN)\n",
                b"Makefile",
                ExpansionContext::pure().with_makeflags(&mut receiver),
            )
            .unwrap();
        assert_eq!(values, [b"-k".to_vec()]);
        assert_eq!(
            database.variables.get(b"LATER").unwrap().raw_value(),
            b"yes"
        );
    }

    #[test]
    fn special_flag_append_reconstructs_command_overrides_after_option_words() {
        let mut database = Database::default();
        database
            .variables
            .define(
                b"MAKEFLAGS",
                b"-j2 -- $(MAKEOVERRIDES)",
                Assignment::Recursive,
                Origin::File,
            )
            .unwrap();
        database
            .variables
            .define_literal(b"MAKEOVERRIDES", b"X=foo", Origin::Default)
            .unwrap();
        let mut values = Vec::new();
        let mut receiver = |request: crate::diagnostic::MakeflagsRequest<'_>| {
            values.push(request.value.to_vec());
            Ok(crate::diagnostic::MakeflagsResult {
                eval_reference: false,
                value: request.value.to_vec(),
                origin: Origin::File,
                environment_override: false,
                include_dirs: Vec::new(),
                prepare_includes: None,
                publish_includes: None,
                assignments: Vec::new(),
                warning_positions: Vec::new(),
                notices: Vec::new(),
                warning_entry: false,
                next_argument: 1,
            })
        };
        database
            .read_with(
                b"MAKEFLAGS += -k\n",
                b"Makefile",
                ExpansionContext::pure().with_makeflags(&mut receiver),
            )
            .unwrap();
        assert_eq!(values, [b"-j2 -k".to_vec()]);
        assert_eq!(
            database.variables.expand(b"$(MAKEFLAGS)").unwrap(),
            b"-j2 -k -- X=foo"
        );
    }

    #[test]
    fn cancellation_stops_reader_before_a_later_definition_or_effect() {
        use std::sync::atomic::{AtomicBool, Ordering};
        let flag = AtomicBool::new(false);
        let mut receiver = |_: Output<'_>| {
            flag.store(true, Ordering::Relaxed);
        };
        let mut database = Database::default();
        let error = database
            .read_with(
                b"$(info before)\nLATER := changed\n$(file >forbidden)\n",
                b"Makefile",
                ExpansionContext::new(None, &mut receiver).with_cancellation(&flag),
            )
            .unwrap_err();
        assert_eq!(error.message.as_bytes(), b"interrupted");
        assert!(database.variables.get(b"LATER").is_none());
    }

    #[test]
    fn nested_read_uses_shared_database_and_records_only_successful_list_entries() {
        let mut db = Database::default();
        let mut output = Vec::new();
        let mut emit = |event: Output<'_>| {
            if let Output::Info(bytes) = event {
                output.push(bytes.to_vec());
            }
        };
        let mut read = |name: &[u8]| match name {
            b"child.mk" => Ok(IncludedFile {
                name: name.to_vec(),
                input: crate::reader::Input::Bytes(b"V=child\n-include absent.mk\n".to_vec()),
            }),
            _ => Err(b"No such file or directory".to_vec().into()),
        };
        db.read_with(
            b"include child.mk\n$(info $(V):$(MAKEFILE_LIST))\nall:;\n",
            b"Makefile",
            ExpansionContext::new(None, &mut emit).with_includes(&mut read),
        )
        .unwrap();
        assert_eq!(output, [b"child:child.mk".to_vec()]);
        assert_eq!(
            db.variables.get(b"MAKEFILE_LIST").unwrap().definition(),
            Some(&Location {
                file: b"child.mk".as_slice().into(),
                line: 1
            }),
        );
        assert_eq!(db.makefiles.len(), 2);
        assert!(db.makefiles[1].optional && db.makefiles[1].missing);
        assert!(db.rules.contains_key(b"all".as_slice()));
    }

    #[test]
    fn absent_required_include_is_retained_for_remake_and_has_no_early_warning() {
        let mut db = Database::default();
        let mut events = 0;
        let mut emit = |_: Output<'_>| events += 1;
        let mut read = |_: &[u8]| Err(b"No such file or directory".to_vec().into());
        db.read_with(
            b"include generated.mk\ngenerated.mk:;\n",
            b"Makefile",
            ExpansionContext::new(None, &mut emit).with_includes(&mut read),
        )
        .unwrap();
        assert_eq!(events, 0);
        let file = &db.makefiles[0];
        assert!(file.missing && !file.optional);
        assert!(file.read_error.is_some());
        assert_eq!(file.location.as_ref().unwrap().line, 1);
        assert!(db.rules.contains_key(b"generated.mk".as_slice()));
    }
}

#[cfg(test)]
mod evaluation_tests {
    use super::*;

    #[test]
    fn evaluation_changes_are_visible_in_the_same_expansion_and_next_read() {
        let mut db = Database::default();
        db.read(b"V=before\n", b"Makefile").unwrap();
        assert_eq!(
            db.expand_with(b"$(V):$(eval V=after)$(V)", ExpansionContext::pure())
                .unwrap(),
            b"before:after"
        );
        db.read(b"X:=$(V)\n", b"later.mk").unwrap();
        assert_eq!(db.variables.expand(b"$(X)").unwrap(), b"after");
    }

    #[test]
    fn evaluation_reads_call_bindings_but_assignments_modify_the_global_binding() {
        let mut db = Database::default();
        db.read(
            b"V=global\n$(foreach V,a b,$(eval X:=$$(V))$(eval V=changed))\n",
            b"Makefile",
        )
        .unwrap();
        assert_eq!(db.variables.expand(b"$(X):$(V)").unwrap(), b"b:changed");
    }

    #[test]
    fn generated_rule_keeps_caller_line_for_recipe_diagnostics() {
        let mut db = Database::default();
        db.read(b"\n\n$(eval target:;echo command)\n", b"outer.mk")
            .unwrap();
        let recipe = &db.rules[b"target".as_slice()].recipes.as_ref().unwrap()[0];
        assert_eq!(
            recipe.location,
            Location {
                file: b"outer.mk".as_slice().into(),
                line: 3
            }
        );
    }
}

pub fn trim(bytes: &[u8]) -> &[u8] {
    let start = bytes
        .iter()
        .position(|b| !b.is_ascii_whitespace())
        .unwrap_or(bytes.len());
    let end = bytes
        .iter()
        .rposition(|b| !b.is_ascii_whitespace())
        .map_or(start, |i| i + 1);
    &bytes[start..end]
}

pub fn words(bytes: &[u8]) -> impl Iterator<Item = &[u8]> {
    bytes
        .split(u8::is_ascii_whitespace)
        .filter(|word| !word.is_empty())
}

impl Database {
    pub(crate) fn begin_filename_session(&mut self, needed: bool) -> bool {
        if self.filename_scratch.is_none() {
            if !needed {
                return false;
            }
            self.filename_scratch = Some(Vec::new());
        }
        self.filename_session_depth += 1;
        true
    }

    pub(crate) fn end_filename_session(&mut self, active: bool) {
        if active {
            self.filename_session_depth -= 1;
            if self.filename_session_depth == 0 {
                self.filename_scratch = None;
            }
        }
    }

    pub(crate) fn write_filename(&mut self, name: &[u8], offset: usize) -> Vec<u8> {
        let Some(scratch) = &mut self.filename_scratch else {
            return name.to_vec();
        };
        if scratch.len() <= name.len() {
            scratch.resize(name.len() + 1, 0);
        }
        let offset = offset.min(name.len());
        scratch[offset..name.len()].copy_from_slice(&name[offset..]);
        scratch[name.len()] = 0;
        let end = scratch
            .iter()
            .position(|&byte| byte == 0)
            .unwrap_or(scratch.len());
        scratch[..end].to_vec()
    }

    pub(crate) fn filename_suffix(&self, fallback: &[u8]) -> Vec<u8> {
        let name = self.filename_scratch.as_deref().unwrap_or(fallback);
        let end = name
            .iter()
            .position(|&byte| byte == 0)
            .unwrap_or(name.len());
        name.get(1..end).unwrap_or_default().to_vec()
    }

    /// Tilde expansion belongs to filename processing, after ordinary make
    /// expansion. Use global/eval bindings here, never a target's later scope.
    pub fn rule_tilde<'a>(
        &mut self,
        name: &'a [u8],
        context: &mut ExpansionContext<'_>,
    ) -> Result<Cow<'a, [u8]>, ExpandError> {
        if !name.starts_with(b"~") {
            return Ok(Cow::Borrowed(name));
        }
        let mut observed = None;
        let home = if name.len() == 1 || name[1] == b'/' {
            let session = self.begin_filename_session(true);
            self.write_filename(name, 0);
            let result = self.expand_variable_with(b"HOME", context.reborrow());
            observed = Some(self.filename_suffix(name));
            self.end_filename_session(session);
            result.map_err(|error| {
                if context.location.is_some() {
                    return error;
                }
                // This expansion finished outside a file context. Freeze its
                // selected diagnostic before a recipe/search caller handles it.
                let diagnostic = error.diagnostic();
                let message = diagnostic.as_bytes().to_vec();
                let selected = match error.diagnostic_location(None) {
                    Some(location) => ExpandError::Located {
                        location: std::sync::Arc::new(location.clone()),
                        error: Box::new(ExpandError::FileError(message)),
                    },
                    None => ExpandError::UnlocatedError(message),
                };
                selected.preserve_frozen(diagnostic)
            })?
        } else {
            Vec::new()
        };
        let suffix = observed.as_deref().unwrap_or(&name[1..]);
        let fallback;
        let opaque_suffix = observed.is_some() && !suffix.is_empty() && suffix[0] != b'/';
        let fallback_name = if opaque_suffix {
            b"~".as_slice()
        } else if observed.is_some() {
            fallback = [b"~".as_slice(), suffix].concat();
            fallback.as_slice()
        } else {
            name
        };
        let expanded = if home.is_empty() {
            let mut expanded = context
                .perform_filesystem(FilesystemEffect::Tilde(fallback_name))
                .ok_or_else(|| ExpandError::Unsupported(b"tilde expansion".to_vec()))?
                .map_err(|error| match error {
                    FilesystemError::Caller(message) => ExpandError::FileError(message),
                    FilesystemError::Unlocated(message) => ExpandError::UnlocatedError(message),
                })?;
            if opaque_suffix && !expanded.is_empty() {
                if expanded.len().saturating_add(suffix.len()) > MAX_OUTPUT {
                    return Err(ExpandError::ExpansionLimit);
                }
                expanded.extend_from_slice(suffix);
            }
            expanded
        } else {
            if home.len().saturating_add(suffix.len()) > MAX_OUTPUT {
                return Err(ExpandError::ExpansionLimit);
            }
            [home.as_slice(), suffix].concat()
        };
        if expanded.len() > MAX_OUTPUT {
            return Err(ExpandError::ExpansionLimit);
        }
        if expanded.is_empty() {
            Ok(Cow::Borrowed(name))
        } else {
            Ok(Cow::Owned(expanded))
        }
    }

    /// Rule names retain unmatched globs and use NUL-separated filesystem results
    /// so a matched filename never becomes another list of words. Ordinary names
    /// stay borrowed and never cross the filesystem-effect boundary.
    pub fn rule_filenames<'a>(
        &mut self,
        bytes: &'a [u8],
        context: &mut ExpansionContext<'_>,
    ) -> Result<impl Iterator<Item = Cow<'a, [u8]>> + use<'a>, ExpandError> {
        self.prepared_filenames::<true>(bytes, context, true)
    }

    pub fn secondary_prerequisite_filenames<'a>(
        &mut self,
        bytes: &'a [u8],
        context: &mut ExpansionContext<'_>,
    ) -> Result<impl Iterator<Item = Cow<'a, [u8]>> + use<'a>, ExpandError> {
        self.prepared_filenames::<true>(bytes, context, false)
    }

    // Module setup symbols use parentheses, so load filenames bypass archive
    // grouping and member lookup while sharing quoting, tilde and glob policy.
    fn prepared_filenames<'a, const ARCHIVES: bool>(
        &mut self,
        bytes: &'a [u8],
        context: &mut ExpansionContext<'_>,
        intern_wait: bool,
    ) -> Result<impl Iterator<Item = Cow<'a, [u8]>> + use<'a, ARCHIVES>, ExpandError> {
        let scan = bytes
            .iter()
            .any(|byte| matches!(byte, b'*' | b'?' | b'[' | b'~') || (ARCHIVES && *byte == b'('));
        let database_history = self.database_history.clone();
        let session = self.begin_filename_session(scan && bytes.contains(&b'~'));
        let result = (|| {
            if !scan && self.filename_scratch.is_some() {
                for name in crate::filenames::words(bytes) {
                    self.write_filename(crate::filenames::without_dot_prefix(&name), 0);
                }
            }
            let names = if scan {
                let mut names = Vec::new();
                let mut size = 0usize;
                for (mut name, emit, offset) in crate::filenames::sequence_events(bytes, ARCHIVES) {
                    let skip = name.len() - crate::filenames::without_dot_prefix(&name).len();
                    name.drain(..skip);
                    if session {
                        name = self.write_filename(&name, offset.saturating_sub(skip));
                    }
                    if !emit {
                        continue;
                    }
                    if name.starts_with(b"~") {
                        name = self.rule_tilde(&name, context)?.into_owned();
                    }
                    let glob = name.iter().any(|b| matches!(b, b'*' | b'?' | b'['));
                    if glob {
                        let effect = if ARCHIVES && crate::filenames::archive_parts(&name).is_some()
                        {
                            FilesystemEffect::ArchiveGlob(&name)
                        } else {
                            FilesystemEffect::RuleGlob(&name)
                        };
                        let result = context
                            .perform_filesystem(effect)
                            .ok_or_else(|| ExpandError::Unsupported(b"rule wildcard".to_vec()))?
                            .map_err(|error| match error {
                                FilesystemError::Caller(message) => ExpandError::FileError(message),
                                FilesystemError::Unlocated(message) => {
                                    ExpandError::UnlocatedError(message)
                                }
                            })?;
                        for name in result.split(|&b| b == 0).filter(|name| !name.is_empty()) {
                            size = size.saturating_add(name.len() + 1);
                            if size > MAX_OUTPUT {
                                return Err(ExpandError::ExpansionLimit);
                            }
                            names.push(name.to_vec());
                        }
                    } else {
                        size = size.saturating_add(name.len() + 1);
                        if size > MAX_OUTPUT {
                            return Err(ExpandError::ExpansionLimit);
                        }
                        names.push(name);
                    }
                }
                Some(names)
            } else {
                None
            };
            if let Some(history) = &database_history {
                if let Some(names) = &names {
                    for name in names {
                        if intern_wait || name != b".WAIT" {
                            history.intern(name);
                        }
                    }
                } else {
                    for name in crate::filenames::words(bytes) {
                        let name = crate::filenames::without_dot_prefix(&name);
                        if intern_wait || name != b".WAIT" {
                            history.intern(name);
                        }
                    }
                }
            }
            Ok(names
                .into_iter()
                .flatten()
                .map(Cow::Owned)
                .chain(crate::filenames::words(if scan { b"" } else { bytes }))
                .map(move |name| {
                    if scan {
                        name
                    } else {
                        match name {
                            Cow::Borrowed(bytes) => {
                                Cow::Borrowed(crate::filenames::without_dot_prefix(bytes))
                            }
                            Cow::Owned(mut bytes) => {
                                let normalized = crate::filenames::without_dot_prefix(&bytes);
                                if bytes.ends_with(normalized) {
                                    let skip = bytes.len() - normalized.len();
                                    bytes.drain(..skip);
                                } else {
                                    bytes = normalized.to_vec();
                                }
                                Cow::Owned(bytes)
                            }
                        }
                    }
                }))
        })();
        self.end_filename_session(session);
        result
    }

    fn parsed_rule_filenames<'a>(
        &mut self,
        bytes: &'a [u8],
        location: &Location,
        context: &mut ExpansionContext<'_>,
    ) -> Result<impl Iterator<Item = Cow<'a, [u8]>> + use<'a>, ParseError> {
        self.rule_filenames(bytes, &mut context.at(Some(location)))
            .map_err(|error| ParseError {
                unlocated: error.is_unlocated(),
                location: error
                    .diagnostic_location(Some(location))
                    .unwrap_or(location)
                    .clone(),
                message: error.diagnostic(),
            })
    }
}

/// Find a separator outside nested make references. It is deliberately not a
/// shell parser: quotes have no special meaning in make rule/assignment syntax.
fn separator(bytes: &[u8], needle: u8) -> Option<usize> {
    let mut closes = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' if i + 1 < bytes.len() => i += 2,
            b'$' if i + 1 < bytes.len() => {
                match bytes[i + 1] {
                    b'(' => closes.push(b')'),
                    b'{' => closes.push(b'}'),
                    _ => {}
                }
                i += 2;
            }
            b'(' if closes.last() == Some(&b')') => {
                closes.push(b')');
                i += 1;
            }
            b'{' if closes.last() == Some(&b'}') => {
                closes.push(b'}');
                i += 1;
            }
            b if closes.last() == Some(&b) => {
                closes.pop();
                i += 1;
            }
            b if closes.is_empty() && b == needle => return Some(i),
            _ => i += 1,
        }
    }
    None
}

/// References that survive immediate expansion are literal rule bytes.
/// Backslash parity still quotes separators.
fn expanded_separator(bytes: &[u8], separator: u8) -> Option<usize> {
    let mut escaped = false;
    for (index, byte) in bytes.iter().copied().enumerate() {
        if escaped {
            escaped = false;
        } else if byte == b'\\' {
            escaped = true;
        } else if byte == separator {
            return Some(index);
        }
    }
    None
}

/// Remove one level of semicolon quoting before the known boundary. Raw rule
/// references remain intact until expansion, and recipe bytes are outside the
/// scan in both phases.
fn unquote_semicolons<const SOURCE: bool>(bytes: &[u8], end: usize) -> (Cow<'_, [u8]>, usize) {
    if !bytes[..end].contains(&b'\\') {
        return (Cow::Borrowed(bytes), end);
    }
    let mut normalized = None;
    let mut copied = 0;
    let mut offset = 0;
    while offset < end {
        match bytes[offset] {
            b'$' if SOURCE => offset = rule_reference_end(&bytes[..end], offset),
            b'\\' => {
                let start = offset;
                while offset < end && bytes[offset] == b'\\' {
                    offset += 1;
                }
                if bytes.get(offset) == Some(&b';') {
                    let output = normalized.get_or_insert_with(|| Vec::with_capacity(bytes.len()));
                    output.extend_from_slice(&bytes[copied..start]);
                    output.extend(std::iter::repeat_n(b'\\', (offset - start) / 2));
                    copied = offset;
                }
            }
            _ => offset += 1,
        }
    }
    if let Some(mut output) = normalized {
        let end = output.len() + (end - copied);
        output.extend_from_slice(&bytes[copied..]);
        (Cow::Owned(output), end)
    } else {
        (Cow::Borrowed(bytes), end)
    }
}

fn rule_reference_end(bytes: &[u8], start: usize) -> usize {
    match bytes.get(start + 1) {
        Some(b'(' | b'{') => reference_end(bytes, start + 1).map_or(bytes.len(), |close| close + 1),
        Some(_) => start + 2,
        None => start + 1,
    }
}

/// A generated delimiter can appear before the end of its source word. The
/// rest of that word expands with it, up to the next raw token boundary.
fn rule_word_tail(bytes: &[u8]) -> &[u8] {
    let mut end = 0;
    while let Some(&byte) = bytes.get(end) {
        let next = bytes.get(end + 1).copied();
        match byte {
            b' ' | b'\t'..=b'\r' | b':' | b'=' | b'#' => break,
            b'&' if next == Some(b':') => break,
            b'+' | b'?' if next == Some(b'=') => break,
            b'\\' if matches!(next, Some(b':' | b';' | b'=' | b'\\')) => end += 2,
            b'$' => end = rule_reference_end(bytes, end),
            _ => end += 1,
        }
    }
    &bytes[..end]
}

pub struct AssignmentSpec<'a> {
    pub name: &'a [u8],
    pub value: &'a [u8],
    pub kind: Assignment,
}

struct TargetAssignment<'a> {
    spec: AssignmentSpec<'a>,
    origin: Origin,
    private: bool,
    export: Option<bool>,
}

struct RulePrefix {
    expanded: Vec<u8>,
    consumed: usize,
    delimiter: Option<RuleDelimiter>,
}

#[derive(Clone, Copy)]
struct RuleDelimiter {
    targets_end: usize,
    tail_start: usize,
    grouped: bool,
    double_colon: bool,
}

impl RuleDelimiter {
    fn new(colon: usize, grouped: bool, double_colon: bool) -> Self {
        Self {
            targets_end: colon - usize::from(grouped),
            tail_start: colon + 1 + usize::from(double_colon),
            grouped,
            double_colon,
        }
    }
}

/// Modifiers are keywords only when the complete remaining text is not itself
/// an assignment: `target: private = value` names the variable `private`.
fn target_assignment(tail: &[u8], syntax_end: usize) -> Option<TargetAssignment<'_>> {
    let mut bytes = assignment_start(&tail[..syntax_end]);
    let mut origin = Origin::File;
    let mut private = false;
    let mut export = None;
    loop {
        if let Some(mut spec) = assignment(bytes) {
            // A physical inline delimiter can end syntax lookahead, but a
            // recognized assignment keeps that delimiter in its raw value.
            spec.value = &tail[syntax_end - spec.value.len()..];
            return Some(TargetAssignment {
                spec,
                origin,
                private,
                export,
            });
        }
        if let Some(rest) = directive(bytes, b"override") {
            origin = Origin::Override;
            bytes = rest;
        } else if let Some(rest) = directive(bytes, b"private") {
            private = true;
            bytes = rest;
        } else if let Some(rest) = directive(bytes, b"export") {
            export = Some(true);
            bytes = rest;
        } else if let Some(rest) = directive(bytes, b"unexport") {
            export = Some(false);
            bytes = rest;
        } else {
            return None;
        }
    }
}

// Assignment syntax uses C whitespace at the start of names/values, but only
// space and tab at the end of raw names. Other trailing bytes name the variable.
// Directive tokenization shares this leading rule; body keyword boundaries and
// physical-line handling remain separate.
fn assignment_start(bytes: &[u8]) -> &[u8] {
    &bytes[bytes
        .iter()
        .position(|byte| !(byte.is_ascii_whitespace() || *byte == 0x0b))
        .unwrap_or(bytes.len())..]
}

/// Parse an assignment without expanding either side. The caller chooses the
/// origin and expands the name; the variable store controls RHS timing.
///
/// Make references consume complete name tokens before operator recognition.
/// Quotes and backslashes are literal; only top-level SP/TAB separate the raw
/// name from its operator. This lexer is separate from rule-separator escaping.
pub fn assignment(bytes: &[u8]) -> Option<AssignmentSpec<'_>> {
    let bytes = assignment_start(bytes);
    let mut cursor = 0;
    let mut name_end = 0;
    let mut trailing = false;
    while cursor < bytes.len() {
        let rest = &bytes[cursor..];
        let operator = match rest {
            [b'=', ..] => Some((1, Assignment::Recursive)),
            [b'+', b'=', ..] => Some((2, Assignment::Append)),
            [b'?', b'=', ..] => Some((2, Assignment::Conditional)),
            [b'!', b'=', ..] => Some((2, Assignment::Shell)),
            [b':', b'=', ..] => Some((2, Assignment::Simple)),
            [b':', b':', b'=', ..] => Some((3, Assignment::Simple)),
            [b':', b':', b':', b'=', ..] => Some((4, Assignment::ImmediateRecursive)),
            _ => None,
        };
        if let Some((length, kind)) = operator {
            return Some(AssignmentSpec {
                name: &bytes[..name_end],
                value: assignment_start(&rest[length..]),
                kind,
            });
        }
        if matches!(bytes[cursor], b' ' | b'\t') {
            trailing = true;
            cursor += 1;
            continue;
        }
        if trailing || matches!(bytes[cursor], b':' | b'#') {
            return None;
        }
        match bytes[cursor] {
            b'$' if cursor + 1 < bytes.len() => {
                let open = bytes[cursor + 1];
                cursor += 2;
                if matches!(open, b'(' | b'{') {
                    let close = if open == b'(' { b')' } else { b'}' };
                    let mut depth = 1usize;
                    while cursor < bytes.len() && depth != 0 {
                        if bytes[cursor] == open {
                            depth += 1;
                        }
                        if bytes[cursor] == close {
                            depth -= 1;
                        }
                        cursor += 1;
                    }
                    if depth != 0 {
                        return None;
                    }
                }
            }
            _ => cursor += 1,
        }
        name_end = cursor;
    }
    None
}

fn directive<'a>(bytes: &'a [u8], keyword: &[u8]) -> Option<&'a [u8]> {
    bytes
        .strip_prefix(keyword)
        .filter(|rest| rest.is_empty() || is_c_space(rest[0]))
        .map(assignment_start)
}

fn is_c_space(byte: u8) -> bool {
    byte.is_ascii_whitespace() || byte == 0x0b
}

fn directive_keyword(bytes: &[u8]) -> &[u8] {
    bytes
        .split(|byte| is_c_space(*byte))
        .next()
        .unwrap_or_default()
}

/// Define bodies skip leading C whitespace, but their nesting keywords require
/// space/tab boundaries. Other whitespace after the keyword remains body data.
fn body_directive<'a>(bytes: &'a [u8], keyword: &[u8]) -> Option<&'a [u8]> {
    assignment_start(bytes)
        .strip_prefix(keyword)
        .filter(|rest| rest.is_empty() || matches!(rest[0], b' ' | b'\t'))
        .map(assignment_start)
}

#[derive(Clone, Copy)]
struct Conditional {
    parent: bool,
    active: bool,
    taken: bool,
    otherwise: bool,
}

fn record_warning(
    warnings: &mut Vec<(Location, Diagnostic)>,
    context: &mut ExpansionContext<'_>,
    location: Location,
    message: Diagnostic,
) {
    context.emit_diagnostic_warning(&location, &message);
    warnings.push((location, message));
}

impl Database {
    fn expand(
        &mut self,
        bytes: &[u8],
        location: &Location,
        context: &mut ExpansionContext<'_>,
    ) -> Result<Vec<u8>, ParseError> {
        self.expand_with(bytes, context.at(Some(location)))
            .map_err(|err| ParseError {
                unlocated: err.is_unlocated(),
                location: err
                    .diagnostic_location(Some(location))
                    .unwrap_or(location)
                    .clone(),
                message: err.diagnostic(),
            })
    }

    fn expand_directive_name(
        &mut self,
        bytes: &[u8],
        location: &Location,
        context: &mut ExpansionContext<'_>,
    ) -> Result<Vec<u8>, ParseError> {
        let mut name = self.expand(bytes, location, context)?;
        let start = name.len() - assignment_start(&name).len();
        name.drain(..start);
        // The reader skips leading C whitespace but only removes trailing blanks.
        while name.last().is_some_and(|byte| matches!(byte, b' ' | b'\t')) {
            name.pop();
        }
        if name.is_empty() {
            return Err(ParseError::system(
                location.clone(),
                Message::EmptyVariableName,
                context,
            ));
        }
        Ok(name)
    }

    /// Expand only the target portion. A colon generated by a reference fixes
    /// the assignment/prerequisite boundary before the untouched tail runs.
    /// Keep the consumed raw offset so ordinary rules can resume expansion
    /// without emitting any prefix effects a second time.
    fn expand_rule_prefix(
        &mut self,
        bytes: &[u8],
        limit: usize,
        location: &Location,
        context: &mut ExpansionContext<'_>,
    ) -> Result<RulePrefix, ParseError> {
        let scan_expanded_inline = limit == bytes.len();
        let expansion_limit = || ParseError {
            unlocated: false,
            location: location.clone(),
            message: ExpandError::ExpansionLimit.diagnostic(),
        };
        let mut expanded = Vec::new();
        let mut consumed = 0;
        while consumed < limit {
            let start = consumed;
            let expansion = bytes[start] == b'$';
            let part = if expansion {
                consumed = match bytes.get(start + 1) {
                    Some(b'(' | b'{') => {
                        reference_end(bytes, start + 1).map_err(|error| {
                            // Retain structural reader balancing on success;
                            // an incomplete recognized function has its own
                            // diagnostic rather than a generic variable error.
                            let error =
                                crate::variables::expansion_reference_span(bytes, start + 1)
                                    .err()
                                    .unwrap_or(error);
                            ParseError {
                                unlocated: error.is_unlocated(),
                                location: location.clone(),
                                message: error.diagnostic(),
                            }
                        })? + 1
                    }
                    Some(_) => start + 2,
                    None => start + 1,
                };
                Cow::Owned(self.expand(&bytes[start..consumed], location, context)?)
            } else {
                consumed += 1;
                while consumed < limit
                    && !matches!(bytes[consumed - 1], b':')
                    && bytes[consumed] != b'$'
                {
                    consumed += 1;
                }
                let raw = &bytes[start..consumed];
                if !raw.iter().any(|byte| matches!(byte, b' ' | b'\t'..=b'\r')) {
                    // The common target prefix needs no temporary allocation.
                    Cow::Borrowed(raw)
                } else {
                    let mut part = Vec::with_capacity(raw.len());
                    let mut blank = false;
                    for &byte in raw {
                        if matches!(byte, b' ' | b'\t'..=b'\r') {
                            if !blank {
                                part.push(b' ');
                            }
                            blank = true;
                        } else {
                            part.push(byte);
                            blank = false;
                        }
                    }
                    Cow::Owned(part)
                }
            };
            if expanded.len().saturating_add(part.len()) > MAX_OUTPUT {
                return Err(expansion_limit());
            }
            let has_colon = part.contains(&b':');
            let has_semicolon = scan_expanded_inline && part.contains(&b';');
            let part_start = expanded.len();
            expanded.extend_from_slice(&part);
            if has_colon || has_semicolon {
                let colon = expanded_separator(&expanded, b':');
                if scan_expanded_inline
                    && separator(&expanded, b';')
                        .is_some_and(|semicolon| colon.is_none_or(|colon| semicolon < colon))
                {
                    return Ok(RulePrefix {
                        expanded,
                        consumed,
                        delimiter: None,
                    });
                }
                if let Some(colon) = colon {
                    if expansion {
                        let suffix = rule_word_tail(&bytes[consumed..limit]);
                        if !suffix.is_empty() {
                            consumed += suffix.len();
                            let tail = self.expand(suffix, location, context)?;
                            if expanded.len().saturating_add(tail.len()) > MAX_OUTPUT {
                                return Err(expansion_limit());
                            }
                            expanded.extend_from_slice(&tail);
                        }
                    }
                    // Raw delimiter tokens cannot pick up an ampersand from a
                    // preceding expansion or a colon from a following one.
                    let grouped = colon != 0
                        && expanded[colon - 1] == b'&'
                        && (expansion || colon > part_start);
                    let double_colon = if expansion {
                        expanded.get(colon + 1) == Some(&b':')
                    } else if bytes.get(consumed) == Some(&b':') {
                        if expanded.len() == MAX_OUTPUT {
                            return Err(expansion_limit());
                        }
                        expanded.push(b':');
                        consumed += 1;
                        true
                    } else {
                        false
                    };
                    return Ok(RulePrefix {
                        expanded,
                        consumed,
                        delimiter: Some(RuleDelimiter::new(colon, grouped, double_colon)),
                    });
                }
            }
        }
        Ok(RulePrefix {
            expanded,
            consumed,
            delimiter: None,
        })
    }

    fn define(
        &mut self,
        spec: AssignmentSpec<'_>,
        origin: Origin,
        location: &Location,
        evaluation_location: &Location,
        context: &mut ExpansionContext<'_>,
    ) -> Result<(Vec<u8>, Option<ConditionalFlagBinding>), ParseError> {
        let name = self.expand(spec.name, location, context)?;
        if name.is_empty() {
            return Err(ParseError::system(
                location.clone(),
                Message::EmptyVariableName,
                context,
            ));
        }
        let binding = self.define_expanded(
            AssignmentSpec {
                name: &name,
                value: spec.value,
                kind: spec.kind,
            },
            origin,
            location,
            evaluation_location,
            context,
        )?;
        Ok((name, binding))
    }

    /// Store a definition after its name expansion, without evaluating it again.
    fn define_expanded(
        &mut self,
        spec: AssignmentSpec<'_>,
        origin: Origin,
        location: &Location,
        evaluation_location: &Location,
        context: &mut ExpansionContext<'_>,
    ) -> Result<Option<ConditionalFlagBinding>, ParseError> {
        self.define_with_flag_binding(
            spec,
            origin,
            Some(location),
            context.at(Some(evaluation_location)),
        )
        .map_err(|err| ParseError {
            unlocated: err.is_unlocated(),
            location: err
                .diagnostic_location(Some(evaluation_location))
                .unwrap_or(evaluation_location)
                .clone(),
            message: err.diagnostic(),
        })
    }

    /// Define an already-expanded name with complete database effects. The
    /// assignment engine selects RHS timing and priority; definition provenance
    /// remains distinct from the immediate evaluation context.
    pub fn define_variable_with(
        &mut self,
        spec: AssignmentSpec<'_>,
        origin: Origin,
        definition: Option<&Location>,
        context: ExpansionContext<'_>,
    ) -> Result<(), ExpandError> {
        self.define_with_flag_binding(spec, origin, definition, context)
            .map(|_| ())
    }

    fn define_with_flag_binding(
        &mut self,
        spec: AssignmentSpec<'_>,
        origin: Origin,
        definition: Option<&Location>,
        mut context: ExpansionContext<'_>,
    ) -> Result<Option<ConditionalFlagBinding>, ExpandError> {
        if spec.kind == Assignment::Conditional
            && self
                .evaluation_bindings
                .iter()
                .any(|frame| frame.contains_key(spec.name))
        {
            return Ok(None);
        }
        let lookup_scope = if matches!(spec.kind, Assignment::Append | Assignment::Conditional) {
            self.evaluation_scope.as_mut().map(|scope| {
                scope.refresh_globals(&self.variables);
                scope.refresh_declared_scopes(&self.target_scopes, self.target_scope_revision);
                scope.clone()
            })
        } else {
            None
        };
        let live_makeflags = spec.name == b"MAKEFLAGS" && context.has_makeflags();
        let flag_binding = if live_makeflags && spec.kind == Assignment::Conditional {
            Some(
                lookup_scope
                    .as_ref()
                    .unwrap_or(&self.variables)
                    .conditional_flag_binding(self.canonical_flags_published),
            )
        } else {
            None
        };
        let append_base = if spec.kind == Assignment::Append {
            lookup_scope.as_ref().and_then(|scope| {
                scope
                    .scoped_append(spec.name, live_makeflags)
                    .map(std::cell::RefCell::new)
            })
        } else {
            None
        };
        if live_makeflags && spec.kind == Assignment::Append && !spec.value.is_empty() {
            self.variables.strip_makeflags_assignments(origin);
        }
        // Literal RHS bytes cannot invoke eval or read scoped bindings. Keep
        // their assignment in the store, but still run special setters below.
        // Shell definitions retain the full database for environment effects.
        let result = if append_base.is_none()
            && lookup_scope.is_none()
            && spec.kind != Assignment::Shell
            && !spec.value.contains(&b'$')
        {
            self.variables.define_with(
                spec.name,
                spec.value,
                spec.kind,
                origin,
                definition,
                context.reborrow(),
            )
        } else {
            let mut variables = std::mem::take(&mut self.variables);
            let result = variables.define_with_using(
                spec.name,
                spec.value,
                spec.kind,
                origin,
                definition,
                context.reborrow(),
                append_base.as_ref(),
                lookup_scope.as_ref(),
                |variables, bytes, shell, context| {
                    std::mem::swap(variables, &mut self.variables);
                    let result = if shell {
                        crate::variables::shell_database(self, bytes, context)
                    } else {
                        self.expand_with(bytes, context)
                    };
                    if let Some(base) = &append_base
                        && let Some(value) = self
                            .evaluation_scope
                            .as_ref()
                            .and_then(|scope| scope.scoped_append(spec.name, live_makeflags))
                    {
                        *base.borrow_mut() = value;
                    }
                    std::mem::swap(variables, &mut self.variables);
                    result
                },
            );
            self.variables = variables;
            result
        };
        result?;
        // A conditional can return an ordinary file or pattern binding.
        // The live setter runs only when that selected binding is special.
        let replay_flags = live_makeflags
            && if spec.kind == Assignment::Conditional {
                !matches!(flag_binding, Some(ConditionalFlagBinding::Ordinary))
            } else {
                !self.canonical_flags_published || self.variables.has_special_makeflags()
            };
        if replay_flags {
            self.reset_assigned_flags(origin, None, context.reborrow())?;
            return Ok(flag_binding);
        }
        Ok(None)
    }

    /// An option payload is one assignment token, not a Makefile statement:
    /// comments/directives are data and the definition has no file location.
    pub fn define_argument_with(
        &mut self,
        argument: &[u8],
        origin: Origin,
        mut context: ExpansionContext<'_>,
    ) -> Result<Option<Vec<u8>>, ExpandError> {
        let Some(spec) = assignment(argument) else {
            return Ok(None);
        };
        let name = self.expand_with(spec.name, context.reborrow())?;
        // Option definitions strip the encoded suffix before activation too.
        if name == b"MAKEFLAGS"
            && spec.kind == Assignment::Append
            && !context.has_makeflags()
            && !spec.value.is_empty()
        {
            self.variables.strip_makeflags_assignments(origin);
        }
        self.define_variable_with(
            AssignmentSpec {
                name: &name,
                ..spec
            },
            origin,
            None,
            context,
        )?;
        Ok(Some(name))
    }

    pub fn define_canonical_makeflags(
        &mut self,
        flags: &[u8],
        origin: Origin,
    ) -> Result<(), ExpandError> {
        self.variables
            .define_runtime_metadata(b"MAKEFLAGS", flags, origin)?;
        self.canonical_flags_published = true;
        Ok(())
    }

    pub(crate) fn update_mflags(
        &mut self,
        flags: &[u8],
        environment_override: bool,
        named_target: Option<&[u8]>,
    ) -> Result<(), ExpandError> {
        let variables = self
            .evaluation_scope
            .as_mut()
            .unwrap_or(&mut self.variables);
        variables.with_flag_destination(named_target, |variables| {
            variables.update_mflags(flags, environment_override)
        })
    }

    fn reset_assigned_flags(
        &mut self,
        origin: Origin,
        named_target: Option<&[u8]>,
        mut context: ExpansionContext<'_>,
    ) -> Result<(), ExpandError> {
        let encoded_overrides = self
            .variables
            .get(b"MAKEFLAGS")
            .is_some_and(|variable| Self::has_command_overrides_reference(variable.raw_value()));
        let value = self.expand_with(b"$(MAKEFLAGS)", context.reborrow())?;
        context.update_makeflags_in(self, origin, value, encoded_overrides, named_target)
    }

    fn define_target_assignments(
        &mut self,
        targets: &[u8],
        assignment: TargetAssignment<'_>,
        location: &Location,
        context: &mut ExpansionContext<'_>,
    ) -> Result<bool, ParseError> {
        let targets = crate::filenames::target_colons(targets);
        let targets = self
            .parsed_rule_filenames(&targets, location, context)?
            .collect::<Vec<_>>();
        let has_targets = !targets.is_empty();
        for target in targets {
            let target = target.as_ref();
            let pattern = Pattern::new(target);
            if pattern.has_wildcard() {
                let name = self.expand(assignment.spec.name, location, context)?;
                let value = if assignment.spec.kind == Assignment::Simple && !name.is_empty() {
                    self.expand(assignment.spec.value, location, context)?
                } else {
                    assignment.spec.value.to_vec()
                };
                let variable = PatternVariable::prepare_read_value(
                    &self.variables,
                    &name,
                    value,
                    assignment.spec.kind,
                    assignment.origin,
                    assignment.private,
                    assignment.export,
                    Some(location),
                )
                .map_err(|error| ParseError {
                    unlocated: error.is_unlocated(),
                    location: error
                        .diagnostic_location(Some(location))
                        .unwrap_or(location)
                        .clone(),
                    message: error.diagnostic(),
                })?;
                self.pattern_assignments
                    .push(PatternAssignment { pattern, variable });
                continue;
            }
            let target = pattern.literal_bytes();
            if let Some(history) = &self.database_history
                && !self.file_records.contains(target)
            {
                // A target-specific assignment reuses an existing file's name.
                history.intern(target);
            }
            self.file_records.enter_prerequisite(target);
            let raw_name = assignment.spec.name;
            let scope = self
                .target_scopes
                .entry(target.to_vec())
                .or_insert_with(|| {
                    let mut scope = TargetScope::for_target(target);
                    if let Some(history) = self
                        .evaluation_scope
                        .as_ref()
                        .and_then(|active| active.file_scope_history(target))
                        .or_else(|| {
                            context
                                .target_reads
                                .as_deref()?
                                .variables(target)?
                                .file_scope_history(target)
                        })
                    {
                        scope.set_file_history(history);
                    }
                    scope
                });
            let (scope, name) = if raw_name.contains(&b'$') {
                let mut parse_variables = scope.context(&self.variables);
                let name = self.expand_target_read(
                    target,
                    &mut parse_variables,
                    raw_name,
                    false,
                    context.at(Some(location)),
                );
                // The name may have changed this target's own table. Finish
                // those effects before selecting the RHS definition owner.
                let scope = self
                    .target_scopes
                    .get_mut(target)
                    .expect("target declaration");
                scope.retain_read_bindings(&parse_variables);
                (scope, name)
            } else {
                let name = scope
                    .context(&self.variables)
                    .expand_with(raw_name, context.at(Some(location)));
                (scope, name)
            };
            let name = name.map_err(|error| ParseError {
                unlocated: error.is_unlocated(),
                location: error
                    .diagnostic_location(Some(location))
                    .unwrap_or(location)
                    .clone(),
                message: error.diagnostic(),
            })?;
            if name.is_empty() {
                return Err(ParseError::system(
                    location.clone(),
                    Message::EmptyVariableName,
                    context,
                ));
            }
            let parse_error = |error: ExpandError| ParseError {
                unlocated: error.is_unlocated(),
                location: error
                    .diagnostic_location(Some(location))
                    .unwrap_or(location)
                    .clone(),
                message: error.diagnostic(),
            };
            let rhs = assignment.spec.value;
            let kind = assignment.spec.kind;
            let scope = if kind == Assignment::Conditional
                && name == b"MAKEFLAGS"
                && context.has_makeflags()
            {
                let mut declarations = scope.context(&self.variables);
                let replayed = self
                    .replay_named_flags(
                        target,
                        &mut declarations,
                        &assignment,
                        context.at(Some(location)),
                    )
                    .map_err(parse_error)?;
                let scope = self
                    .target_scopes
                    .get_mut(target)
                    .expect("target declaration");
                if let Some(owner) = replayed {
                    scope.retain_read_bindings(&declarations);
                    if matches!(owner, ConditionalFlagBinding::Global) {
                        self.variables
                            .qualify_global_target_flags(assignment.private, assignment.export);
                    }
                    self.publish_target_scope(target);
                    continue;
                }
                scope
            } else {
                scope
            };
            let scope = if kind == Assignment::Conditional
                && (self.evaluation_scope.is_some()
                    || self.target_read.is_some()
                    || context.target_reads.is_some())
            {
                let declarations = scope.context(&self.variables);
                if let Some((owner, mut read)) = self.existing_target_read(
                    target,
                    &declarations,
                    self.evaluation_scope.as_ref(),
                    context,
                ) {
                    self.refresh_target_read(&owner, &mut read, &declarations);
                    if read.qualify_ancestor_or_pattern(
                        &name,
                        assignment.private,
                        assignment.export,
                    ) {
                        self.publish_target_scope(target);
                        continue;
                    }
                }
                self.target_scopes
                    .get_mut(target)
                    .expect("target declaration")
            } else {
                scope
            };
            let immediate = matches!(kind, Assignment::Simple | Assignment::ImmediateRecursive)
                || (kind == Assignment::Append
                    && scope
                        .get(&name)
                        .is_some_and(|binding| binding.flavor() == Flavor::Simple));
            let result = if kind == Assignment::Shell || (immediate && rhs.contains(&b'$')) {
                let mut updated = scope.clone();
                let mut globals = std::mem::take(&mut self.variables);
                let result = updated.define_before_origin_lookup(
                    &mut globals,
                    &name,
                    rhs,
                    kind,
                    assignment.origin,
                    assignment.private,
                    assignment.export,
                    Some(location),
                    context.at(Some(location)),
                    |globals, scope, bytes, shell, context| {
                        std::mem::swap(globals, &mut self.variables);
                        let result = self.expand_target_read(target, scope, bytes, shell, context);
                        std::mem::swap(globals, &mut self.variables);
                        result
                    },
                );
                self.variables = globals;
                if result.is_ok() {
                    self.target_scopes.insert(target.to_vec(), updated);
                }
                result
            } else {
                scope.define_before_origin_lookup(
                    &mut self.variables,
                    &name,
                    rhs,
                    kind,
                    assignment.origin,
                    assignment.private,
                    assignment.export,
                    Some(location),
                    context.at(Some(location)),
                    |_, scope, bytes, _, context| scope.expand_mut_with(bytes, context),
                )
            };
            let lookup_origin = result.map_err(parse_error)?;
            self.publish_target_scope(target);
            if lookup_origin
                && !self
                    .evaluation_bindings
                    .iter()
                    .rev()
                    .any(|binding| binding.contains_key(&name))
                && let Some(value) = self
                    .evaluation_scope
                    .as_ref()
                    .unwrap_or(&self.variables)
                    .target_command_value(&name)
                    .map_err(parse_error)?
                && self
                    .target_scopes
                    .get_mut(target)
                    .expect("target declaration")
                    .adopt_target_command_value(&name, value)
            {
                self.publish_target_scope(target);
            }
        }
        Ok(has_targets)
    }

    fn replay_named_flags(
        &mut self,
        target: &[u8],
        declarations: &mut VariableStore,
        assignment: &TargetAssignment<'_>,
        context: ExpansionContext<'_>,
    ) -> Result<Option<ConditionalFlagBinding>, ExpandError> {
        let active = self
            .evaluation_scope
            .as_ref()
            .is_some_and(|scope| scope.is_file_read_context(target));
        self.with_target_read(target, declarations, context, |database, read, context| {
            let owner = read.conditional_flag_binding(database.canonical_flags_published);
            if read.get(b"MAKEFLAGS").is_none()
                || matches!(owner, ConditionalFlagBinding::Ordinary)
                || (!active && !matches!(owner, ConditionalFlagBinding::Global))
            {
                return Ok(None);
            }
            let destination = (!matches!(owner, ConditionalFlagBinding::Current)).then_some(target);
            database.evaluation_scope = Some(std::mem::take(read));
            let result = database.reset_assigned_flags(assignment.origin, destination, context);
            *read = database.evaluation_scope.take().expect("named flag scope");
            result?;
            read.qualify_named_flags(owner, assignment.private, assignment.export);
            Ok(Some(owner))
        })
    }

    fn publish_target_scope(&mut self, target: &[u8]) {
        self.target_scope_revision = self.target_scope_revision.wrapping_add(1);
        if let Some(active) = self.evaluation_scope.as_mut() {
            active.refresh_globals(&self.variables);
            active.refresh_declared_scopes(&self.target_scopes, self.target_scope_revision);
            if self
                .target_read
                .as_ref()
                .is_some_and(|read| read.target == target)
            {
                self.target_scopes[target].refresh_read_bindings(active);
            }
        }
    }

    fn retained_target_read(
        &self,
        target: &[u8],
        declaration: &VariableStore,
        reads: &dyn crate::diagnostic::TargetReadReceiver,
        caller: Option<&VariableStore>,
    ) -> Option<VariableStore> {
        if reads.has_parent_changes()
            && let Some(read) = self.reparented_target_read(target, declaration, reads, caller)
        {
            return Some(read);
        }
        if let Some(variables) = reads.variables(target) {
            return variables.active_target_read(target, declaration);
        }

        // A visited file can have a parent before any recipe or secondary
        // expansion has needed its variable table. Reading a declaration
        // does not activate any pattern scopes along that path.
        let mut path = vec![target];
        let mut parent = reads.parent(target)?;
        let mut inherited = loop {
            let Some(edge) = parent else {
                break self.variables.clone();
            };
            if let Some(variables) = reads.parent_variables(edge) {
                break variables.clone();
            }
            if path.contains(&edge.target) {
                return None;
            }
            path.push(edge.target);
            parent = reads.parent(edge.target).flatten();
        };
        for name in path.into_iter().rev() {
            inherited = self.variables_for_target_prepared(name, &inherited, None);
        }
        inherited.active_target_read(target, declaration)
    }

    fn reparented_target_read(
        &self,
        target: &[u8],
        declaration: &VariableStore,
        reads: &dyn crate::diagnostic::TargetReadReceiver,
        caller: Option<&VariableStore>,
    ) -> Option<VariableStore> {
        let mut path: Vec<crate::diagnostic::ScopeParent<&[u8]>> = Vec::new();
        let mut changed = false;
        let mut name = target;
        loop {
            let replacement = reads.changed_parent(name);
            changed |= replacement.is_some();
            let Some(parent) = replacement.or_else(|| reads.parent(name).flatten()) else {
                break;
            };
            if parent.target == target || path.iter().any(|edge| edge.target == parent.target) {
                return None;
            }
            path.push(parent);
            name = parent.target;
        }
        if !changed {
            return None;
        }

        let mut read = match reads.variables(target) {
            Some(variables) => variables.active_target_read(target, declaration)?,
            None => self
                .variables_for_target_prepared(target, &self.variables, None)
                .active_target_read(target, declaration)?,
        };
        read.replace_file_read_ancestors(
            path.into_iter().map(|parent| {
                let variables = caller
                    .filter(|scope| scope.is_file_read_parent(parent))
                    .or_else(|| {
                        self.suspended_target_reads
                            .iter()
                            .rev()
                            .find(|scope| scope.is_file_read_parent(parent))
                    })
                    .or_else(|| {
                        reads.parent_variables(parent).filter(|scope| {
                            // An uninitialized branch can use its head's table.
                            scope.is_file_read_parent(parent)
                                || scope.is_file_read_context(parent.target)
                        })
                    });
                crate::variables::ReadAncestor {
                    name: parent.target,
                    variables,
                    declarations: self.target_scopes.get(parent.target),
                }
            }),
            &self.variables,
        );
        Some(read)
    }

    fn existing_target_read(
        &self,
        target: &[u8],
        scope: &VariableStore,
        caller: Option<&VariableStore>,
        context: &ExpansionContext<'_>,
    ) -> Option<(TargetReadOwner, VariableStore)> {
        caller
            .and_then(|caller| caller.active_target_read(target, scope))
            .map(|read| (TargetReadOwner::Current, read))
            .or_else(|| {
                self.suspended_target_reads
                    .iter()
                    .enumerate()
                    .rev()
                    .find_map(|(index, caller)| {
                        caller
                            .active_target_read(target, scope)
                            .map(|read| (TargetReadOwner::Suspended(index), read))
                    })
            })
            .or_else(|| {
                self.retained_target_read(target, scope, context.target_reads.as_deref()?, caller)
                    .map(|read| (TargetReadOwner::Retained, read))
            })
    }

    fn refresh_target_read(
        &self,
        owner: &TargetReadOwner,
        read: &mut VariableStore,
        declarations: &VariableStore,
    ) {
        if matches!(owner, TargetReadOwner::Current) {
            read.refresh_globals(&self.variables);
        } else {
            self.refresh_scoped_variables(read);
            read.replace_target_read_declarations(declarations);
        }
    }

    fn expand_target_read(
        &mut self,
        target: &[u8],
        scope: &mut VariableStore,
        bytes: &[u8],
        shell: bool,
        context: ExpansionContext<'_>,
    ) -> Result<Vec<u8>, ExpandError> {
        self.with_target_read(target, scope, context, |database, read, context| {
            if shell {
                crate::variables::shell_scoped_database(database, read, bytes, context)
            } else {
                crate::variables::expand_scoped_database::<false>(database, read, bytes, context)
            }
        })
    }

    fn with_target_read<T>(
        &mut self,
        target: &[u8],
        scope: &mut VariableStore,
        mut context: ExpansionContext<'_>,
        read_target: impl FnOnce(
            &mut Self,
            &mut VariableStore,
            ExpansionContext<'_>,
        ) -> Result<T, ExpandError>,
    ) -> Result<T, ExpandError> {
        let owns_reader_lexical = self.target_read.is_none() && self.evaluation_scope.is_none();
        let global_lexical = if let Some(read) = &self.target_read {
            read.global_lexical.clone()
        } else if owns_reader_lexical {
            crate::variables::DeclarationLexical::capture(&self.evaluation_bindings)
        } else {
            crate::variables::DeclarationLexical::default()
        };
        let mut previous_scope = self.evaluation_scope.take();
        let mut file_read =
            self.existing_target_read(target, scope, previous_scope.as_ref(), &context);
        // Crossing into another file can re-enter this caller in a nested
        // declaration. Keep its live local table available until we return.
        let suspended = previous_scope.is_some()
            && file_read
                .as_ref()
                .is_none_or(|(owner, _)| !matches!(owner, TargetReadOwner::Current));
        if suspended {
            self.suspended_target_reads
                .push(previous_scope.take().expect("active caller"));
        }
        let retained_before = file_read.as_ref().and_then(|(owner, read)| {
            matches!(owner, TargetReadOwner::Retained).then(|| read.clone())
        });
        let read = if let Some((owner, read)) = file_read.as_mut() {
            self.refresh_target_read(owner, read, scope);
            read
        } else {
            scope.add_declaration_lexical(&global_lexical);
            &mut *scope
        };
        let parent = self.target_read.replace(TargetRead {
            target: target.to_vec(),
            global_lexical,
        });
        let previous_bindings = std::mem::take(&mut self.evaluation_bindings);
        let result = read_target(self, read, context.reborrow());
        if let Some((owner, read)) = file_read {
            scope.retain_target_read_declarations(&read);
            match owner {
                TargetReadOwner::Current => previous_scope
                    .as_mut()
                    .expect("active caller")
                    .retain_file_locals(read),
                TargetReadOwner::Suspended(index) => {
                    self.suspended_target_reads[index].retain_file_locals(read);
                }
                TargetReadOwner::Retained => {
                    if retained_before.is_some_and(|before| !read.same_file_locals(&before)) {
                        context
                            .target_reads
                            .as_deref_mut()
                            .expect("retained target")
                            .retain_locals(target, read);
                    }
                }
            }
        }
        if suspended {
            previous_scope = self.suspended_target_reads.pop();
        }
        if owns_reader_lexical {
            self.target_read
                .as_ref()
                .expect("active target declaration")
                .global_lexical
                .restore_export_policies(&previous_bindings);
        }
        self.evaluation_bindings = previous_bindings;
        self.evaluation_scope = previous_scope;
        self.target_read = parent;
        result
    }

    fn condition(
        &mut self,
        bytes: &[u8],
        location: &Location,
        context: &mut ExpansionContext<'_>,
    ) -> Result<Option<bool>, ParseError> {
        for (keyword, negate) in [(b"ifdef".as_slice(), false), (b"ifndef", true)] {
            if let Some(name) = directive(bytes, keyword) {
                let name = self.expand(name, location, context)?;
                let end = name
                    .iter()
                    .position(|byte| is_c_space(*byte))
                    .unwrap_or(name.len());
                if !assignment_start(&name[end..]).is_empty() {
                    return Err(ParseError::system(
                        location.clone(),
                        Message::InvalidConditional,
                        context,
                    ));
                }
                return Ok(Some(
                    self.evaluation_bindings
                        .iter()
                        .rev()
                        .find_map(|bindings| bindings.get(&name[..end]))
                        .or_else(|| {
                            self.evaluation_scope
                                .as_ref()
                                .and_then(|scope| scope.get(&name[..end]))
                        })
                        .or_else(|| self.variables.get(&name[..end]))
                        .is_some_and(|var| !var.raw_value().is_empty())
                        ^ negate,
                ));
            }
        }
        for (keyword, negate) in [(b"ifeq".as_slice(), false), (b"ifneq", true)] {
            if let Some(args) = directive(bytes, keyword) {
                let invalid = |context: &mut ExpansionContext<'_>| {
                    ParseError::system(location.clone(), Message::InvalidConditional, context)
                };
                let (left, right, extra) = if matches!(args.first(), Some(b'(' | b',')) {
                    // The first operand can include literal closing parentheses.
                    // A comma ends it once no unmatched opener precedes it.
                    let mut depth = 0isize;
                    let mut comma = None;
                    for (index, byte) in args.iter().copied().enumerate().skip(1) {
                        match byte {
                            b'(' => depth += 1,
                            b')' => depth -= 1,
                            b',' if depth <= 0 => {
                                comma = Some(index);
                                break;
                            }
                            _ => {}
                        }
                    }
                    let comma = comma.ok_or_else(|| invalid(context))?;
                    // GNU evaluates the complete left operand before it checks
                    // that the right operand has a closing delimiter.
                    let left = &args[1..comma];
                    let left_end = left
                        .iter()
                        .rposition(|byte| !matches!(byte, b' ' | b'\t'))
                        .map_or(0, |i| i + 1);
                    let left = self.expand(&left[..left_end], location, context)?;
                    let mut depth = 1usize;
                    let mut closing = None;
                    for (index, byte) in args.iter().copied().enumerate().skip(comma + 1) {
                        match byte {
                            b'(' => depth += 1,
                            b')' => {
                                depth -= 1;
                                if depth == 0 {
                                    closing = Some(index);
                                    break;
                                }
                            }
                            _ => {}
                        }
                    }
                    let closing = closing.ok_or_else(|| invalid(context))?;
                    (
                        left,
                        assignment_start(&args[comma + 1..closing]),
                        &args[closing + 1..],
                    )
                } else {
                    let quote = *args.first().ok_or_else(|| invalid(context))?;
                    if quote != b'\'' && quote != b'"' {
                        return Err(invalid(context));
                    }
                    let end = args[1..]
                        .iter()
                        .position(|b| *b == quote)
                        .ok_or_else(|| invalid(context))?
                        + 1;
                    let left = self.expand(&args[1..end], location, context)?;
                    let rest = assignment_start(&args[end + 1..]);
                    let (right, extra) = match *rest.first().ok_or_else(|| invalid(context))? {
                        b')' => (&b""[..], &rest[1..]),
                        quote @ (b'\'' | b'"') => {
                            let end = rest[1..]
                                .iter()
                                .position(|b| *b == quote)
                                .ok_or_else(|| invalid(context))?
                                + 1;
                            (&rest[1..end], &rest[end + 1..])
                        }
                        _ => return Err(invalid(context)),
                    };
                    (left, right, extra)
                };
                if !assignment_start(extra).is_empty() {
                    record_warning(
                        &mut self.warnings,
                        context,
                        location.clone(),
                        Diagnostic::system(Message::DirectiveTail(keyword)),
                    );
                }
                return Ok(Some(
                    (left == self.expand(right, location, context)?) ^ negate,
                ));
            }
        }
        Ok(None)
    }

    /// Pure/test reader: output functions require an explicit receiver.
    pub fn read(&mut self, bytes: &[u8], file: &[u8]) -> Result<(), ParseError> {
        self.read_with(bytes, file, ExpansionContext::pure())
    }

    pub fn read_with(
        &mut self,
        bytes: &[u8],
        file: &[u8],
        context: ExpansionContext<'_>,
    ) -> Result<(), ParseError> {
        self.read_at(ParserReader::memory(bytes), file, None, context)
    }

    pub fn read_input(
        &mut self,
        input: Input,
        file: &[u8],
        context: ExpansionContext<'_>,
    ) -> Result<(), ParseError> {
        match input {
            Input::Bytes(bytes) => self.read_with(&bytes, file, context),
            Input::Stream(source) => {
                self.read_at(ParserReader::stream(source), file, None, context)
            }
        }
    }

    pub fn read_preload_input(
        &mut self,
        input: Input,
        file: &[u8],
        context: ExpansionContext<'_>,
    ) -> Result<(), ParseError> {
        let previous = std::mem::replace(&mut self.suppress_default_goal, true);
        let result = self.read_input(input, file, context);
        self.suppress_default_goal = previous;
        result
    }

    /// MAKEFILES preloads and their includes do not select an implicit default
    /// goal. Explicit .DEFAULT_GOAL assignments retain their ordinary effects.
    pub fn read_preload_with(
        &mut self,
        bytes: &[u8],
        file: &[u8],
        context: ExpansionContext<'_>,
    ) -> Result<(), ParseError> {
        let previous = std::mem::replace(&mut self.suppress_default_goal, true);
        let result = self.read_with(bytes, file, context);
        self.suppress_default_goal = previous;
        result
    }

    /// Expand a selected raw extra-prerequisite binding in global context.
    /// Even simple values are rescanned at this phase, as GNU does.
    pub fn extra_prerequisites(
        &mut self,
        raw: Option<&[u8]>,
        context: &mut ExpansionContext<'_>,
    ) -> Result<Vec<Dependency>, ExpandError> {
        let Some(raw) = raw else {
            return Ok(Vec::new());
        };
        let value = self.expand_with(raw, context.reborrow())?;
        let (normal, order) = crate::filenames::split_order(&value);
        let mut dependencies = Vec::new();
        for (text, order_only) in [(normal.as_ref(), false), (order.unwrap_or_default(), true)] {
            for name in self.rule_filenames(text, context)? {
                dependencies.push(Dependency {
                    name: name.into_owned(),
                    order_only,
                    hidden: true,
                });
            }
        }
        for dependency in &dependencies {
            if dependency.name != b".WAIT" {
                self.file_records.enter_prerequisite(&dependency.name);
            }
        }
        Ok(dependencies)
    }

    pub fn snap_extra_prerequisites(
        &mut self,
        capture_records: bool,
        context: &mut ExpansionContext<'_>,
    ) -> Result<Vec<Vec<u8>>, ExpandError> {
        let raw = self
            .variables
            .database_lookup(b".EXTRA_PREREQS")
            .map(|variable| variable.raw_value().to_vec());
        for scope in self.target_scopes.values() {
            let _ = scope.database_lookup(b".EXTRA_PREREQS");
        }
        if raw.is_none()
            && !self
                .target_scopes
                .values()
                .any(|scope| scope.get(b".EXTRA_PREREQS").is_some())
        {
            return Ok(Vec::new());
        }
        let global = self.extra_prerequisites(raw.as_deref(), context)?;
        // Capture special flags before appending hidden inputs. A bare phony
        // prerequisite is a target record even without a normal rule header.
        let phony: HashSet<_> = self
            .rules
            .get(b".PHONY".as_slice())
            .into_iter()
            .flat_map(|root| {
                root.double_colon
                    .as_deref()
                    .unwrap_or_else(|| std::slice::from_ref(root))
            })
            .flat_map(|rule| &rule.dependencies)
            .map(|dep| dep.name.clone())
            .collect();
        let mut captured = Vec::new();
        let slots = self.file_records.slots_len();
        for index in 0..slots {
            let Some(name) = self.file_records.name_in_slot(index).map(Vec::from) else {
                continue;
            };
            if capture_records {
                captured.push(name.clone());
            }
            let extras = if let Some(scope) = self.target_scopes.get(&name) {
                let raw = scope
                    .get(b".EXTRA_PREREQS")
                    .map(|variable| variable.raw_value().to_vec());
                self.extra_prerequisites(raw.as_deref(), context)?
            } else if self.rules.get(&name).is_some_and(|rule| !rule.record_only)
                || phony.contains(&name)
            {
                global.clone()
            } else {
                continue;
            };
            // Any self occurrence cancels the entire extra list. Names were
            // already entered by expansion, preserving observable table order.
            if extras.is_empty() || extras.iter().any(|dep| dep.name == name) {
                continue;
            }
            let root = self.rules.entry(name).or_insert_with(|| Rule {
                record_only: true,
                ..Rule::default()
            });
            let head = if let Some(branches) = &mut root.double_colon {
                branches.first_mut().expect("double-colon head")
            } else {
                root
            };
            head.dependencies.extend(extras);
        }
        Ok(captured)
    }

    pub fn snap_implicit_extra_prerequisites(
        &mut self,
        context: &mut ExpansionContext<'_>,
    ) -> Result<(), ExpandError> {
        let raw = self
            .variables
            .database_lookup(b".EXTRA_PREREQS")
            .map(|variable| variable.raw_value().to_vec());
        let global = self.extra_prerequisites(raw.as_deref(), context)?;
        self.invalidate_implicit_rule_indexes();
        for (_, rule) in &mut self.implicit_rules {
            rule.dependencies.extend(global.iter().cloned());
        }
        Ok(())
    }

    pub fn command_overrides_name(&self) -> &'static [u8] {
        if self.posix {
            b"-*-command-variables-*-"
        } else {
            b"MAKEOVERRIDES"
        }
    }

    pub fn command_overrides_reference(&self) -> &'static [u8] {
        if self.posix {
            b" -- $(-*-command-variables-*-)"
        } else {
            b" -- $(MAKEOVERRIDES)"
        }
    }

    pub fn has_command_overrides_reference(value: &[u8]) -> bool {
        [
            b"$(MAKEOVERRIDES)".as_slice(),
            b"$(-*-command-variables-*-)",
        ]
        .iter()
        .any(|reference| {
            value
                .windows(reference.len())
                .any(|part| part == *reference)
        })
    }

    pub(crate) fn export_all_for_shell(&self) -> bool {
        self.export_all
    }

    fn notparallel_prerequisites(&self) -> Option<&[Dependency]> {
        let rule = self.rules.get(b".NOTPARALLEL".as_slice())?;
        let head = rule
            .double_colon
            .as_ref()
            .and_then(|branches| branches.first())
            .unwrap_or(rule);
        Some(&head.dependencies)
    }

    pub fn globally_serial(&self) -> bool {
        self.notparallel_prerequisites()
            .is_some_and(|prerequisites| prerequisites.iter().all(|dependency| dependency.hidden))
    }

    /// After the initial dependency snapshot, global setup expansions share
    /// recipe expansion's restriction on introducing new rules. A fresh
    /// database on Makefile restart restores the reader phase.
    pub fn freeze_rules(&mut self) {
        if !self.rules_frozen {
            let named = self
                .rules
                .get(b".EXPORT_ALL_VARIABLES".as_slice())
                .is_some_and(|rule| !rule.record_only);
            let phony = self.rules.get(b".PHONY".as_slice()).is_some_and(|rule| {
                rule.dependencies
                    .iter()
                    .any(|dependency| dependency.name == b".EXPORT_ALL_VARIABLES")
            });
            self.export_all |= named || phony;
            let serial_targets: HashSet<_> = self
                .notparallel_prerequisites()
                .into_iter()
                .flatten()
                .filter(|dependency| !dependency.hidden && dependency.name != b".WAIT")
                .map(|dependency| dependency.name.clone())
                .collect();
            for target in serial_targets {
                let Some(rule) = self.rules.get_mut(&target) else {
                    continue;
                };
                if let Some(branches) = &mut rule.double_colon {
                    for branch in branches {
                        branch.insert_targeted_waits();
                    }
                } else {
                    rule.insert_targeted_waits();
                }
            }
        }
        self.rules_frozen = true;
    }

    /// Load one prepared module, retaining raw metadata and its first source.
    /// Filename expansion/list preparation belongs to the directive reader.
    pub fn load_native_with(
        &mut self,
        name: &[u8],
        optional: bool,
        location: &Location,
        context: &mut ExpansionContext<'_>,
    ) -> Result<crate::native::LoadOutcome, ExpandError> {
        use crate::native::NativeLoadStatus;
        let runtime = context.native_runtime().ok_or_else(|| {
            ExpandError::FileError(
                [
                    b"rmake: unsupported directive '".as_slice(),
                    if optional { b"-load" } else { b"load" },
                    b"'",
                ]
                .concat(),
            )
        })?;
        // The requested setup suffix is not part of the physical module's
        // file identity. Observe the cache and prior-file query before setup:
        // constructors and setup callbacks can themselves request a dump.
        let physical_name = if let Some(open) = name.iter().position(|byte| *byte == b'(')
            && let Some(close) = name[open + 1..]
                .iter()
                .position(|byte| *byte == b')')
                .map(|offset| open + 1 + offset)
            && close + 1 == name.len()
            && close != open + 1
        {
            &name[..open]
        } else {
            name
        };
        if let Some(history) = &self.database_history {
            history.intern(physical_name);
            history.record_file_lookup(physical_name);
        }
        let loaded = runtime.load(
            name,
            optional,
            location,
            &mut LoadHost {
                database: self,
                context: context.at(Some(location)),
            },
        );
        let loaded = loaded.map_err(|error| {
            if error.system_message().is_some() {
                context.freeze_error(error)
            } else {
                error
            }
        })?;
        if loaded.status == NativeLoadStatus::Failed && !optional {
            return Err(context.freeze_error(ExpandError::system(Message::LoadFailed(name))));
        }
        self.loaded_modules
            .entry(loaded.name.clone())
            .or_insert_with(|| location.clone());
        if matches!(
            loaded.status,
            NativeLoadStatus::Loaded | NativeLoadStatus::LoadedWithoutRemake
        ) {
            self.variables
                .append_literal_at(b".LOADED", &loaded.name, Origin::File, location)?;
        }
        // A successful or optional attempt retains its file identity even
        // when the runtime has already loaded the module. Only a genuinely
        // absent record performs a logical enter.
        self.file_records.enter_prerequisite(&loaded.name);
        if matches!(
            loaded.status,
            NativeLoadStatus::Loaded | NativeLoadStatus::Failed
        ) {
            self.makefiles.push(Makefile {
                name: loaded.name.clone(),
                optional: false,
                missing: loaded.status == NativeLoadStatus::Failed,
                location: Some(location.clone()),
                read_error: None,
            });
        }
        Ok(loaded)
    }

    /// Service accepted worker callbacks against the actual paused interpreter.
    pub fn service_waiting_native_workers_with(
        &mut self,
        context: &mut ExpansionContext<'_>,
    ) -> Result<(), ExpandError> {
        let runtime = context
            .native_runtime()
            .ok_or_else(|| ExpandError::Unsupported(b"native worker callbacks".to_vec()))?;
        runtime.service_waiting_workers(&mut LoadHost {
            database: self,
            context: context.reborrow(),
        })
    }

    /// Keep interpreter state available while native process finalizers run.
    pub fn terminate_native_with(
        &mut self,
        status: u8,
        context: &mut ExpansionContext<'_>,
    ) -> Result<(), ExpandError> {
        let runtime = context
            .native_runtime()
            .ok_or_else(|| ExpandError::Unsupported(b"native process termination".to_vec()))?;
        runtime.terminate(
            status,
            &mut LoadHost {
                database: self,
                context: context.reborrow(),
            },
        )
    }

    /// Preserve the same actual interpreter host during platform signal exit.
    pub fn terminate_native_signal_with(
        &mut self,
        number: i32,
        context: &mut ExpansionContext<'_>,
    ) -> Result<(), ExpandError> {
        let runtime = context
            .native_runtime()
            .ok_or_else(|| ExpandError::Unsupported(b"native signal termination".to_vec()))?;
        runtime.terminate_signal(
            number,
            &mut LoadHost {
                database: self,
                context: context.reborrow(),
            },
        )
    }

    /// Unload a module before rebuilding its file. Destructor effects use the
    /// same scoped interpreter host as loading and ordinary callbacks.
    pub fn unload_native_with(
        &mut self,
        name: &[u8],
        context: &mut ExpansionContext<'_>,
    ) -> Result<bool, ExpandError> {
        let Some(runtime) = context.native_runtime() else {
            return Ok(false);
        };
        runtime.unload(
            name,
            &mut LoadHost {
                database: self,
                context: context.reborrow(),
            },
        )
    }

    pub fn expand_with(
        &mut self,
        bytes: &[u8],
        context: ExpansionContext<'_>,
    ) -> Result<Vec<u8>, ExpandError> {
        self.expand_entry_with::<false>(bytes, context)
    }

    /// Select an internal binding silently, while evaluating its value with
    /// ordinary effects and diagnostics in the complete active context.
    pub fn expand_variable_with(
        &mut self,
        name: &[u8],
        context: ExpansionContext<'_>,
    ) -> Result<Vec<u8>, ExpandError> {
        self.expand_entry_with::<true>(name, context)
    }

    fn expand_entry_with<const NAMED: bool>(
        &mut self,
        bytes: &[u8],
        context: ExpansionContext<'_>,
    ) -> Result<Vec<u8>, ExpandError> {
        if let Some(mut scope) = self.evaluation_scope.take() {
            self.refresh_scoped_variables(&mut scope);
            let result =
                crate::variables::expand_scoped_database::<NAMED>(self, &mut scope, bytes, context);
            self.evaluation_scope = Some(scope);
            result
        } else {
            crate::variables::expand_database::<NAMED>(self, bytes, context)
        }
    }

    pub fn expand_in(
        &mut self,
        variables: &mut VariableStore,
        bytes: &[u8],
        context: ExpansionContext<'_>,
    ) -> Result<Vec<u8>, ExpandError> {
        self.refresh_scoped_variables(variables);
        let previous = std::mem::replace(&mut self.rules_frozen, true);
        let result =
            crate::variables::expand_scoped_database::<false>(self, variables, bytes, context);
        self.rules_frozen = previous;
        result
    }

    pub fn expand_environment_in(
        &mut self,
        variables: &mut VariableStore,
        name: &[u8],
        context: ExpansionContext<'_>,
    ) -> Result<Vec<u8>, ExpandError> {
        self.refresh_scoped_variables(variables);
        let previous = std::mem::replace(&mut self.rules_frozen, true);
        let result = crate::variables::expand_environment_database(self, variables, name, context);
        self.rules_frozen = previous;
        result
    }

    pub(crate) fn evaluate_in(
        &mut self,
        bytes: &[u8],
        location: Option<&Location>,
        scope: Option<&mut VariableStore>,
        bindings: &mut Vec<crate::variables::LocalFrame>,
        active: Option<std::sync::Arc<crate::variables::ActiveContext>>,
        context: ExpansionContext<'_>,
    ) -> Result<(), ParseError> {
        // Nested declarations share the active target's table. Publish any
        // preceding changes before re-entering the reader; it refreshes the
        // saved scope when another declaration commits to the same owner.
        if let (Some(read), Some(scope)) = (&self.target_read, scope.as_deref()) {
            self.target_scopes
                .get_mut(&read.target)
                .expect("active target declaration")
                .retain_read_bindings(scope);
        }
        let previous_scope =
            std::mem::replace(&mut self.evaluation_scope, scope.as_deref().cloned());
        std::mem::swap(&mut self.evaluation_bindings, bindings);
        let previous_active = std::mem::replace(&mut self.evaluation_active, active);
        let result = self.read_evaluated(bytes, location, context);
        self.evaluation_active = previous_active;
        if let Some(scope) = scope {
            *scope = self.evaluation_scope.take().expect("evaluation scope");
        }
        self.evaluation_scope = previous_scope;
        std::mem::swap(&mut self.evaluation_bindings, bindings);
        result
    }

    /// Evaluate a buffer without appending a source filename to MAKEFILE_LIST.
    /// None denotes an anonymous command-line buffer; includes retain locations.
    pub fn read_evaluated(
        &mut self,
        bytes: &[u8],
        location: Option<&Location>,
        context: ExpansionContext<'_>,
    ) -> Result<(), ParseError> {
        if let Some(stack) = context.evaluation_stack {
            let mut context = Some(context);
            return stack
                .run(&mut || {
                    self.read_evaluated_inner(
                        bytes,
                        location,
                        context
                            .take()
                            .expect("evaluation stack callback invoked once"),
                    )
                })
                .unwrap_or_else(|| {
                    Err(ParseError {
                        location: location.cloned().unwrap_or(Location {
                            file: SourceName::null(),
                            line: 1,
                        }),
                        unlocated: location.is_none(),
                        message: "rmake: evaluation stack storage limit exceeded".into(),
                    })
                });
        }
        self.read_evaluated_inner(bytes, location, context)
    }

    fn read_evaluated_inner(
        &mut self,
        bytes: &[u8],
        location: Option<&Location>,
        context: ExpansionContext<'_>,
    ) -> Result<(), ParseError> {
        let (file, first_line) = location.map_or_else(
            || (SourceName::null(), 1),
            |location| (location.file.clone(), location.line),
        );
        if context.evaluation_stack.is_none() && self.evaluation_depth >= 128 {
            return Err(ParseError {
                location: Location {
                    file: file.clone(),
                    line: first_line,
                },
                unlocated: location.is_none(),
                message: "rmake: evaluation nesting limit exceeded".into(),
            });
        }
        self.evaluation_depth += 1;
        // GNU eval buffers select defaults even inside a MAKEFILES preload.
        let suppress_default = std::mem::replace(&mut self.suppress_default_goal, false);
        let result = self
            .read_at_source(
                ParserReader::memory(
                    &bytes[..bytes
                        .iter()
                        .position(|byte| *byte == 0)
                        .unwrap_or(bytes.len())],
                ),
                file,
                Some(first_line),
                context,
            )
            .map_err(|mut error| {
                if location.is_none() && error.location.file.is_null() {
                    error.unlocated = true;
                }
                error
            });
        self.evaluation_depth -= 1;
        self.suppress_default_goal = suppress_default;
        result
    }

    fn read_at(
        &mut self,
        reader: ParserReader<'_>,
        file: &[u8],
        evaluated_at: Option<usize>,
        context: ExpansionContext<'_>,
    ) -> Result<(), ParseError> {
        self.read_at_source(reader, file.into(), evaluated_at, context)
    }

    fn read_at_source(
        &mut self,
        reader: ParserReader<'_>,
        file: SourceName,
        evaluated_at: Option<usize>,
        mut context: ExpansionContext<'_>,
    ) -> Result<(), ParseError> {
        let entry_depth = self.include_depth;
        let result = self.read_frames(reader, file, evaluated_at, &mut context);
        self.include_depth = entry_depth;
        result
    }

    fn read_frames(
        &mut self,
        reader: ParserReader<'_>,
        file: SourceName,
        evaluated_at: Option<usize>,
        context: &mut ExpansionContext<'_>,
    ) -> Result<(), ParseError> {
        let mut frames = vec![ReadFrame::new(
            reader,
            file,
            evaluated_at,
            self.include_depth >= 128,
        )];
        let mut extra = 0;
        while !frames.is_empty() {
            let frame = frames.last_mut().expect("active reader frame");
            if context.is_cancelled() {
                return Err(ParseError {
                    unlocated: true,
                    location: Location {
                        file: frame.source_name.clone(),
                        line: frame.last_line.max(1),
                    },
                    message: "interrupted".into(),
                });
            }
            frame.account(&mut extra)?;
            if let Some(pending) = &mut frame.pending {
                let location = pending.location.clone();
                let optional = pending.optional;
                if let Some(path) = pending.paths.next() {
                    context.reader_event(crate::diagnostic::ReaderEvent::Reading {
                        name: &path,
                        search: true,
                        optional,
                        no_default_goal: self.suppress_default_goal,
                        no_tilde: true,
                    });
                    let result = context.read_include(&path).ok_or_else(|| ParseError {
                        unlocated: false,
                        location: location.clone(),
                        message: "rmake: include requires a makefile reader".into(),
                    })?;
                    let result = match result {
                        Ok(included) => Ok(included),
                        Err(IncludeFailure::Deferred {
                            resolved_name,
                            detail,
                        }) => Err((resolved_name, detail)),
                        Err(IncludeFailure::Fatal(detail)) => {
                            return Err(ParseError {
                                unlocated: false,
                                location,
                                message: context.freeze_diagnostic(detail.into()),
                            });
                        }
                    };
                    let file_identity = match &result {
                        Ok(included) => included.name.as_slice(),
                        Err((resolved_name, _)) => resolved_name.as_deref().unwrap_or(&path),
                    };
                    if let Some(history) = &self.database_history {
                        history.intern(file_identity);
                    }
                    self.file_records.enter_prerequisite(file_identity);
                    match result {
                        Ok(included) => {
                            self.makefiles.push(Makefile {
                                name: included.name.clone(),
                                optional,
                                missing: false,
                                location: Some(location.clone()),
                                read_error: None,
                            });
                            self.variables
                                .append_literal_at(
                                    b"MAKEFILE_LIST",
                                    &included.name,
                                    Origin::File,
                                    &Location {
                                        file: path.as_slice().into(),
                                        line: 1,
                                    },
                                )
                                .map_err(|e| ParseError {
                                    unlocated: false,
                                    location: location.clone(),
                                    message: e.diagnostic(),
                                })?;
                            self.include_depth += 1;
                            frames.push(ReadFrame::new(
                                ParserReader::owned(included.input),
                                path.as_slice().into(),
                                None,
                                self.include_depth >= 128,
                            ));
                        }
                        Err((resolved_name, detail)) => {
                            self.makefiles.push(Makefile {
                                name: resolved_name.unwrap_or(path),
                                optional,
                                missing: true,
                                location: Some(location),
                                read_error: Some(detail),
                            });
                        }
                    }
                    continue;
                }
                frame.pending = None;
            }
            let ReadFrame {
                reader,
                source_name,
                evaluated_at,
                declaration,
                conditions,
                last_line,
                ..
            } = frame;
            let evaluated_at = *evaluated_at;
            let prefix = self
                .variables
                .get(b".RECIPEPREFIX")
                .and_then(|v| v.raw_value().first().copied())
                .unwrap_or(b'\t');
            reader.set_posix(self.posix);
            if evaluated_at == Some(1) {
                reader.skip_eval_bom();
            }
            let Some(mut line) = reader
                .next_line(prefix, declaration.has_recipe_context())
                .map_err(|message| source_read_error_at(source_name.clone(), message, context))?
            else {
                if !conditions.is_empty() {
                    return Err(ParseError::system(
                        Location {
                            file: source_name.clone(),
                            line: evaluated_at.unwrap_or(*last_line + 1),
                        },
                        Message::MissingEndif,
                        context,
                    ));
                }
                self.finish_declaration(
                    declaration,
                    &Location {
                        file: source_name.clone(),
                        line: evaluated_at.unwrap_or(*last_line + 1),
                    },
                    context,
                )?;
                reader.finish();
                let finished = frames.pop().expect("finished source");
                extra -= finished.charged;
                if !frames.is_empty() {
                    self.include_depth -= 1;
                }
                continue;
            };
            if context.is_cancelled() {
                return Err(ParseError {
                    unlocated: true,
                    location: Location {
                        file: source_name.clone(),
                        line: line.line,
                    },
                    message: "interrupted".into(),
                });
            }
            // GNU strips one UTF-8 BOM from the first observed logical line,
            // after file-stream reads have already counted its bytes.
            if evaluated_at.is_none() && line.line == 1 && line.bytes.starts_with(b"\xef\xbb\xbf") {
                context.reader_event(crate::diagnostic::ReaderEvent::Bom(source_name.as_ref()));
                line.bytes.drain(..3);
            }
            *last_line = line.end_line;
            let location = Location {
                file: source_name.clone(),
                line: evaluated_at.unwrap_or(line.line),
            };
            for _ in 0..reader.take_nul_warnings() {
                record_warning(
                    &mut self.warnings,
                    context,
                    location.clone(),
                    "warning: NUL character seen; rest of line ignored".into(),
                );
            }
            let active = conditions.last().is_none_or(|c| c.active);
            if line.recipe {
                if active && !declaration.ignore_recipes {
                    self.add_recipe(
                        declaration,
                        Recipe {
                            bytes: line.bytes[1..].to_vec(),
                            location,
                            prefix,
                            builtin: false,
                        },
                    );
                }
                continue;
            }
            // Assignment values may contain semicolons. Rule inline recipes
            // retain shell comments, so comment stripping happens separately.
            let normal = line
                .bytes
                .contains(&b'#')
                .then(|| strip_comment(&line.bytes));
            let text = assignment_start(normal.as_deref().unwrap_or(&line.bytes));
            if text.is_empty() {
                continue;
            }
            // Assignment syntax wins over directive keywords. In particular,
            // `ifdef = value` must not open a conditional, even in an ignored
            // branch. Recognition itself has no expansion effects.
            if let Some(spec) = assignment(text) {
                if active {
                    self.finish_declaration(declaration, &location, context)?;
                    self.define(spec, Origin::File, &location, &location, context)?;
                }
                continue;
            }
            let keyword = directive_keyword(text);
            if [b"ifdef".as_slice(), b"ifndef", b"ifeq", b"ifneq"].contains(&keyword) {
                let yes = active && self.condition(text, &location, context)?.unwrap_or(false);
                conditions.push(Conditional {
                    parent: active,
                    active: yes,
                    taken: yes,
                    otherwise: false,
                });
                continue;
            }
            if let Some(rest) = directive(text, b"else") {
                let Some(top) = conditions.last_mut() else {
                    return Err(ParseError {
                        unlocated: false,
                        location,
                        message: "extraneous 'else'".into(),
                    });
                };
                if top.otherwise {
                    return Err(ParseError {
                        unlocated: false,
                        location,
                        message: "only one 'else' per conditional".into(),
                    });
                }
                let eligible = top.parent && !top.taken;
                top.active = if rest.is_empty() {
                    top.otherwise = true;
                    eligible
                } else if [b"ifdef".as_slice(), b"ifndef", b"ifeq", b"ifneq"]
                    .contains(&directive_keyword(rest))
                {
                    eligible && self.condition(rest, &location, context)?.unwrap_or(false)
                } else {
                    record_warning(
                        &mut self.warnings,
                        context,
                        location.clone(),
                        Diagnostic::system(Message::DirectiveTail(b"else")),
                    );
                    top.otherwise = true;
                    eligible
                };
                top.taken |= top.active;
                continue;
            }
            if let Some(rest) = directive(text, b"endif") {
                if conditions.pop().is_none() {
                    return Err(ParseError {
                        unlocated: false,
                        location,
                        message: "extraneous 'endif'".into(),
                    });
                }
                if !rest.is_empty() {
                    record_warning(
                        &mut self.warnings,
                        context,
                        location.clone(),
                        Diagnostic::system(Message::DirectiveTail(b"endif")),
                    );
                }
                continue;
            }
            let mut variable_text = text;
            let mut origin = Origin::File;
            let mut export = None;
            let mut private = false;
            let variable_assignment = loop {
                // A modifier can itself be a variable name (`private = x`).
                if let Some(spec) = assignment(variable_text) {
                    break Some(spec);
                } else if let Some(rest) = directive(variable_text, b"override") {
                    origin = Origin::Override;
                    variable_text = rest;
                } else if let Some(rest) = directive(variable_text, b"private") {
                    private = true;
                    variable_text = rest;
                } else if let Some(rest) = directive(variable_text, b"export") {
                    export = Some(true);
                    variable_text = rest;
                } else if let Some(rest) = directive(variable_text, b"unexport") {
                    export = Some(false);
                    variable_text = rest;
                } else {
                    break None;
                }
            };
            if variable_assignment.is_none()
                && let Some(header) = directive(variable_text, b"define")
            {
                let definition = if active {
                    self.finish_declaration(declaration, &location, context)?;
                    let spec = assignment(header);
                    if spec.as_ref().is_some_and(|spec| !spec.value.is_empty()) {
                        record_warning(
                            &mut self.warnings,
                            context,
                            location.clone(),
                            Diagnostic::system(Message::DefineTail),
                        );
                    }
                    let (raw_name, kind) = spec.map_or((header, Assignment::Recursive), |spec| {
                        (spec.name, spec.kind)
                    });
                    let name = self.expand_directive_name(raw_name, &location, context)?;
                    Some((name, kind))
                } else {
                    None
                };
                let mut body = Vec::new();
                let mut depth = 1;
                let mut warning_line = location.line;
                loop {
                    if context.is_cancelled() {
                        return Err(ParseError {
                            unlocated: true,
                            location: location.clone(),
                            message: "interrupted".into(),
                        });
                    }
                    let Some(part) = reader.next_line(prefix, false).map_err(|message| {
                        source_read_error_at(source_name.clone(), message, context)
                    })?
                    else {
                        if !active {
                            // Ignored definitions are skipped by the outer
                            // reader, whose EOF error is the open conditional.
                            break;
                        }
                        return Err(ParseError::system(location, Message::MissingEndef, context));
                    };
                    if context.is_cancelled() {
                        return Err(ParseError {
                            unlocated: true,
                            location: location.clone(),
                            message: "interrupted".into(),
                        });
                    }
                    for _ in 0..reader.take_nul_warnings() {
                        record_warning(
                            &mut self.warnings,
                            context,
                            Location {
                                file: source_name.clone(),
                                line: if active { warning_line } else { part.line },
                            },
                            "warning: NUL character seen; rest of line ignored".into(),
                        );
                    }
                    warning_line += part.end_line - part.line + 1;
                    *last_line = part.end_line;
                    if !active {
                        // Ignored definitions have no nesting counter: the
                        // first standalone endef resumes conditional reading.
                        let normal = strip_comment(&part.bytes);
                        if (part.bytes.first() != Some(&prefix)
                            || !declaration.has_recipe_context())
                            && directive(assignment_start(&normal), b"endef")
                                .is_some_and(|rest| rest.is_empty())
                        {
                            break;
                        }
                        continue;
                    }
                    if part.bytes.first() != Some(&prefix) {
                        if body_directive(&part.bytes, b"define").is_some() {
                            depth += 1;
                        }
                        if let Some(rest) = body_directive(&part.bytes, b"endef") {
                            if !assignment_start(&strip_comment(rest)).is_empty() {
                                record_warning(
                                    &mut self.warnings,
                                    context,
                                    Location {
                                        file: source_name.clone(),
                                        line: part.end_line,
                                    },
                                    Diagnostic::system(Message::EndefTail),
                                );
                            }
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                    }
                    body.extend_from_slice(&part.bytes);
                    body.push(b'\n');
                }
                if body.last() == Some(&b'\n') {
                    body.pop();
                }
                if let Some((name, kind)) = definition {
                    let binding = self.define_expanded(
                        AssignmentSpec {
                            name: &name,
                            value: &body,
                            kind,
                        },
                        origin,
                        &location,
                        &Location {
                            file: source_name.clone(),
                            line: *last_line,
                        },
                        context,
                    )?;
                    self.apply_assignment_qualifiers(name, kind, private, export, binding);
                }
                continue;
            }
            if !active {
                continue;
            }
            if let Some(spec) = variable_assignment {
                self.finish_declaration(declaration, &location, context)?;
                let kind = spec.kind;
                let (name, binding) = self.define(spec, origin, &location, &location, context)?;
                self.apply_assignment_qualifiers(name, kind, private, export, binding);
                continue;
            }
            if let Some(rest) = directive(variable_text, b"undefine") {
                self.finish_declaration(declaration, &location, context)?;
                let name = self.expand_directive_name(rest, &location, context)?;
                let name = name.as_slice();
                if self.variables.can_undefine(name, origin) {
                    let safety = if self.runtime_metadata_live && name == b".DEFAULT_GOAL" {
                        Some(crate::safety::SafetyError::DefaultGoalDeletion)
                    } else if self.exporting.iter().any(|active| active == name) {
                        Some(crate::safety::SafetyError::ActiveExportDeletion)
                    } else {
                        None
                    };
                    if let Some(error) = safety {
                        return Err(ParseError {
                            location,
                            message: error.message().into(),
                            unlocated: false,
                        });
                    }
                    self.variables.remove(name);
                    self.exports.remove(name);
                    self.unexports.remove(name);
                }
                continue;
            }
            // Modifiers only belong to a recognized variable operation. If
            // there was none, parse the original statement as a directive or
            // rule. Names such as `private` remain literal export-list names.
            let export_list = directive(text, b"export")
                .map(|names| (true, names))
                .or_else(|| directive(text, b"unexport").map(|names| (false, names)));
            if let Some((export, text)) = export_list {
                self.finish_declaration(declaration, &location, context)?;
                // Only a syntactically empty directive changes export-all.
                // An expression that expands to no names still has its effects,
                // but does not become an empty directive after evaluation.
                let is_list_space = |byte: &u8| byte.is_ascii_whitespace() || *byte == 0x0b;
                let empty_directive = text.iter().all(is_list_space);
                let names = self.expand(text, &location, context)?;
                if empty_directive {
                    self.export_all = export;
                }
                for name in names.split(is_list_space).filter(|name| !name.is_empty()) {
                    // A standalone directive updates the selected binding.
                    // Only a name absent from every scope creates a global.
                    if self
                        .evaluation_bindings
                        .iter()
                        .rev()
                        .any(|frame| frame.set_export_policy(name, export))
                        || self.apply_scoped_assignment_qualifiers(name, false, Some(export))
                    {
                        continue;
                    }
                    if self.variables.get(name).is_none() {
                        self.variables
                            .define_at(name, b"", Assignment::Simple, Origin::File, &location)
                            .map_err(|error| ParseError {
                                unlocated: error.is_unlocated(),
                                location: location.clone(),
                                message: error.diagnostic(),
                            })?;
                    }
                    self.set_global_export(name.to_vec(), export);
                }
                continue;
            }
            if [b"include".as_slice(), b"-include", b"sinclude"].contains(&keyword) {
                self.finish_declaration(declaration, &location, context)?;
                let expanded =
                    self.expand(directive(text, keyword).unwrap(), &location, context)?;
                let mut paths = Vec::new();
                // Filename interpretation completes before the first child is
                // read. Child effects cannot alter this prepared path list.
                for name in crate::filenames::words(&expanded) {
                    let mut name = crate::filenames::without_dot_prefix(&name).to_vec();
                    if self.filename_scratch.is_some() {
                        self.write_filename(&name, 0);
                    }
                    if name.starts_with(b"~") {
                        name = self
                            .rule_tilde(&name, &mut context.at(Some(&location)))
                            .map_err(|e| ParseError {
                                unlocated: e.is_unlocated(),
                                location: e
                                    .diagnostic_location(Some(&location))
                                    .unwrap_or(&location)
                                    .clone(),
                                message: e.diagnostic(),
                            })?
                            .into_owned();
                    }
                    let matches = if name.iter().any(|b| matches!(b, b'*' | b'?' | b'[')) {
                        context
                            .perform_filesystem(FilesystemEffect::IncludeGlob(&name))
                            .and_then(Result::ok)
                            .unwrap_or_default()
                    } else {
                        Vec::new()
                    };
                    if matches.is_empty() {
                        paths.push(name);
                    } else {
                        paths.extend(
                            matches
                                .split(|&byte| byte == 0)
                                .filter(|name| !name.is_empty())
                                .map(Vec::from),
                        );
                    }
                }
                if let Some(history) = &self.database_history {
                    for path in &paths {
                        history.intern(path);
                    }
                }
                let retained = paths.iter().map(Vec::capacity).sum::<usize>()
                    + paths.capacity() * std::mem::size_of::<Vec<u8>>();
                frame.pending = Some(IncludeWork {
                    paths: paths.into_iter(),
                    retained,
                    optional: keyword != b"include",
                    location,
                });
                continue;
            }
            if keyword == b"vpath" {
                self.finish_declaration(declaration, &location, context)?;
                let value = self.expand(directive(text, b"vpath").unwrap(), &location, context)?;
                let mut names = words(&value);
                if let Some(pattern) = names.next() {
                    let operands = names.collect::<Vec<_>>();
                    if operands.is_empty() {
                        self.vpaths.retain(|path| path.key != pattern);
                    } else {
                        let mut directories = Vec::new();
                        for directory in operands
                            .iter()
                            .flat_map(|name| name.split(|byte| *byte == b':'))
                            .filter(|path| !path.is_empty())
                        {
                            let original_directory = directory;
                            let directory = if directory.len() > 1 && directory.ends_with(b"/") {
                                &directory[..directory.len() - 1]
                            } else {
                                directory
                            };
                            if directory == b"." {
                                continue;
                            }
                            if let Some(history) = &self.database_history {
                                history.intern(directory);
                            }
                            if let Some(result) = context
                                .perform_filesystem(FilesystemEffect::DirectoryName(directory))
                            {
                                result.map_err(|error| ParseError {
                                    unlocated: matches!(&error, FilesystemError::Unlocated(_)),
                                    location: location.clone(),
                                    message: error.into_message().into(),
                                })?;
                            }
                            directories.push(original_directory.to_vec());
                        }
                        if !directories.is_empty() {
                            if let Some(history) = &self.database_history {
                                history.intern(pattern);
                            }
                            self.vpaths.push(Vpath {
                                key: pattern.to_vec(),
                                pattern: Pattern::new(pattern),
                                directories,
                            });
                        }
                    }
                } else {
                    self.vpaths.clear();
                }
                continue;
            }
            if matches!(keyword, b"load" | b"-load") {
                self.finish_declaration(declaration, &location, context)?;
                let expanded =
                    self.expand(directive(text, keyword).unwrap(), &location, context)?;
                let paths = self
                    .prepared_filenames::<false>(&expanded, &mut context.at(Some(&location)), true)
                    .map_err(|error| ParseError {
                        unlocated: error.is_unlocated(),
                        location: error
                            .diagnostic_location(Some(&location))
                            .unwrap_or(&location)
                            .clone(),
                        message: error.diagnostic(),
                    })?;
                // Prepare every operand before attempting the first module.
                // Even an optional load rejects an explicitly empty setup name.
                for name in paths {
                    if let Some(open) = name.iter().position(|&byte| byte == b'(')
                        && name.get(open + 1..) == Some(b")")
                    {
                        return Err(ParseError::system(
                            location,
                            Message::EmptyLoadSymbol(&name),
                            context,
                        ));
                    }
                    self.load_native_with(&name, keyword == b"-load", &location, context)
                        .map_err(|error| ParseError {
                            unlocated: error.is_unlocated(),
                            location: error
                                .diagnostic_location(Some(&location))
                                .unwrap_or(&location)
                                .clone(),
                            message: error.diagnostic(),
                        })?;
                }
                continue;
            }
            self.finish_declaration(declaration, &location, context)?;
            if line.bytes.first() == Some(&prefix) && prefix == b'\t' {
                return Err(ParseError::system(location, Message::OrphanRecipe, context));
            }
            let raw_rule_end = separator(text, b';').unwrap_or(text.len());
            let (text, rule_end) = unquote_semicolons::<true>(text, raw_rule_end);
            // Keep the raw assignment tail until a source word has produced
            // an inline recipe boundary during expansion.
            let mut prefix_expansion =
                self.expand_rule_prefix(&text, rule_end, &location, context)?;
            let dequoted_targets = prefix_expansion.delimiter.and_then(|delimiter| {
                let targets = &prefix_expansion.expanded[..delimiter.targets_end];
                if rule_end != text.len() || !targets.contains(&b';') {
                    return None;
                }
                // Keep the expanded delimiter offsets and prepare the target
                // spelling separately for assignment and rule filenames.
                match unquote_semicolons::<false>(targets, targets.len()).0 {
                    Cow::Owned(targets) => Some(targets),
                    Cow::Borrowed(_) => None,
                }
            });
            let mut expanded_recipe_tail = None;
            if let Some(delimiter) = prefix_expansion.delimiter {
                let prefix_tail = &prefix_expansion.expanded[delimiter.tail_start..];
                let raw_tail = &text[prefix_expansion.consumed..];
                let (mut tail, expanded_inline, rescan_prefix) =
                    if rule_end == text.len() && prefix_tail.contains(&b';') {
                        let semicolon = expanded_separator(prefix_tail, b';');
                        let (normalized, end) = unquote_semicolons::<false>(
                            prefix_tail,
                            semicolon.unwrap_or(prefix_tail.len()),
                        );
                        let rescan_prefix = semicolon.is_none()
                            && !raw_tail.is_empty()
                            && matches!(normalized, Cow::Owned(_));
                        let mut tail = normalized.into_owned();
                        tail.truncate(end);
                        (tail, semicolon.is_some(), rescan_prefix)
                    } else {
                        (prefix_tail.to_vec(), false, false)
                    };
                if expanded_inline {
                    // An inline recipe exposed in the prefix expands its
                    // remaining text before target filenames are observed.
                    expanded_recipe_tail = Some(self.expand(raw_tail, &location, context)?);
                }
                let targets = dequoted_targets
                    .as_deref()
                    .unwrap_or(&prefix_expansion.expanded[..delimiter.targets_end]);
                // Prefix effects already ran. A declaration with no names
                // has no prerequisite expansion or target assignments.
                if targets
                    .iter()
                    .all(|byte| matches!(byte, b' ' | b'\t'..=b'\r'))
                {
                    declaration.ignore_recipes = true;
                    continue;
                }
                let prefix_tail_len = tail.len();
                let mut syntax_end = prefix_tail_len;
                if !expanded_inline {
                    syntax_end += rule_end.saturating_sub(prefix_expansion.consumed);
                    tail.extend_from_slice(raw_tail);
                }
                if let Some(assignment) = target_assignment(&tail, syntax_end) {
                    declaration.ignore_recipes =
                        !self.define_target_assignments(targets, assignment, &location, context)?;
                    continue;
                }
                if rescan_prefix {
                    // The later prerequisite expansion scans again only when
                    // source text remains. Keep the first scan's dequoting.
                    prefix_expansion.expanded.truncate(delimiter.tail_start);
                    prefix_expansion
                        .expanded
                        .extend_from_slice(&tail[..prefix_tail_len]);
                }
            }
            // GNU observes target globs before expanding the prerequisite
            // tail. A shell/file effect in that tail must not change which
            // targets this declaration names. Ordinary words remain borrowed.
            let observed_targets = if let Some(delimiter) = prefix_expansion.delimiter
                && prefix_expansion.expanded[..delimiter.targets_end]
                    .iter()
                    .any(|byte| matches!(byte, b'(' | b'*' | b'?' | b'[' | b'~'))
            {
                let targets = dequoted_targets
                    .as_deref()
                    .unwrap_or(&prefix_expansion.expanded[..delimiter.targets_end]);
                let targets = crate::filenames::target_colons(targets);
                Some(
                    self.parsed_rule_filenames(&targets, &location, context)?
                        .map(Cow::into_owned)
                        .collect::<Vec<_>>(),
                )
            } else {
                None
            };
            if observed_targets.as_ref().is_some_and(Vec::is_empty) {
                declaration.ignore_recipes = true;
                continue;
            }
            let source_inline = separator(&line.bytes, b';')
                .filter(|i| separator(&line.bytes[..*i], b'#').is_none());
            let tail = match expanded_recipe_tail {
                Some(tail) => tail,
                None => self.expand(
                    &text[prefix_expansion.consumed..rule_end],
                    &location,
                    context,
                )?,
            };
            let mut expanded_line = prefix_expansion.expanded;
            if expanded_line.len().saturating_add(tail.len()) > MAX_OUTPUT {
                return Err(ParseError {
                    unlocated: false,
                    location,
                    message: ExpandError::ExpansionLimit.diagnostic(),
                });
            }
            expanded_line.extend_from_slice(&tail);
            // A semicolon produced by immediate expansion is also an inline
            // recipe separator. Its text has already undergone that expansion.
            let expanded_inline = if source_inline.is_some() {
                None
            } else if let Some(delimiter) = prefix_expansion.delimiter {
                let suffix = &expanded_line[delimiter.tail_start..];
                let semicolon = expanded_separator(suffix, b';');
                let (normalized, end) =
                    unquote_semicolons::<false>(suffix, semicolon.unwrap_or(suffix.len()));
                if let Cow::Owned(normalized) = normalized {
                    expanded_line.truncate(delimiter.tail_start);
                    expanded_line.extend_from_slice(&normalized);
                }
                semicolon.map(|_| delimiter.tail_start + end)
            } else {
                separator(&expanded_line, b';')
            };
            let expanded =
                expanded_inline.map_or(expanded_line.as_slice(), |i| &expanded_line[..i]);
            if trim(expanded).is_empty() {
                continue;
            }
            let delimiter = prefix_expansion.delimiter.or_else(|| {
                expanded_separator(expanded, b':').map(|colon| {
                    RuleDelimiter::new(
                        colon,
                        expanded[..colon].ends_with(b"&"),
                        expanded.get(colon + 1) == Some(&b':'),
                    )
                })
            });
            let Some(delimiter) = delimiter else {
                let message = if prefix == b'\t' && line.bytes.starts_with(b"        ") {
                    Message::MissingSeparator { tabs: true }
                } else if [b"ifeq".as_slice(), b"ifneq"].iter().any(|word| {
                    text.strip_prefix(*word)
                        .and_then(|suffix| suffix.first())
                        .is_some_and(|byte| !matches!(*byte, b' ' | b'\t'))
                }) {
                    Message::MissingSeparatorConditional
                } else {
                    Message::MissingSeparator { tabs: false }
                };
                return Err(ParseError::system(location, message, context));
            };
            declaration.location = Some(location.clone());
            declaration.grouped = delimiter.grouped;
            let early_targets = observed_targets.is_some();
            let targets = dequoted_targets
                .as_deref()
                .unwrap_or(&expanded[..delimiter.targets_end]);
            let target_text = crate::filenames::target_colons(targets);
            let targets = observed_targets
                .into_iter()
                .flatten()
                .map(Cow::Owned)
                .chain(self.parsed_rule_filenames(
                    if early_targets { b"" } else { &target_text },
                    &location,
                    context,
                )?);
            for target in targets {
                let target = target.as_ref();
                let pattern = Pattern::new(target);
                declaration.target_patterns.push(pattern.has_wildcard());
                if pattern.has_wildcard() {
                    declaration
                        .patterns
                        .insert(pattern.literal_bytes().to_vec(), pattern.clone());
                }
                declaration.targets.push(pattern.literal_bytes().to_vec());
            }
            if declaration.targets.is_empty() {
                declaration.ignore_recipes = true;
                continue;
            }
            let mut prereqs = &expanded[delimiter.tail_start..];
            let static_pattern = if let Some(second) = expanded_separator(prereqs, b':') {
                let pattern_text = crate::filenames::target_colons(&prereqs[..second]);
                let patterns: Vec<_> = crate::filenames::words(&pattern_text).collect();
                if patterns.len() != 1 || !Pattern::new(&patterns[0]).has_wildcard() {
                    let message = if patterns.is_empty() {
                        Message::MissingTargetPattern
                    } else if patterns.len() > 1 {
                        Message::MultipleTargetPatterns
                    } else {
                        Message::TargetPatternWithoutPercent
                    };
                    return Err(ParseError::system(location, message, context));
                }
                let pattern = Pattern::new(&patterns[0]);
                prereqs = &prereqs[second + 1..];
                Some(pattern)
            } else {
                None
            };
            // Check the complete declaration before merging any target: a
            // .SUFFIXES target on this same line has not changed the list yet.
            if !self.suppress_default_goal
                && self
                    .variables
                    .get(b".DEFAULT_GOAL")
                    .is_none_or(|v| v.raw_value().is_empty())
                && let Some(target) = declaration
                    .targets
                    .iter()
                    .take_while(|target| !target.contains(&b'%'))
                    .find(|target| {
                        (!target.starts_with(b".") || target.contains(&b'/'))
                            && !self.is_suffix_rule_name(target)
                            && !declaration.patterns.contains_key(target.as_slice())
                    })
            {
                self.variables
                    .define_literal(b".DEFAULT_GOAL", target, Origin::File)
                    .map_err(|err| ParseError {
                        unlocated: err.is_unlocated(),
                        location: location.clone(),
                        message: err.diagnostic(),
                    })?;
            }
            if declaration
                .targets
                .iter()
                .any(|target| target == b".ONESHELL")
            {
                self.variables.enable_oneshell();
            }
            if !self.posix && declaration.targets.iter().any(|target| target == b".POSIX") {
                self.posix = true;
                for (name, value) in [
                    (b".SHELLFLAGS".as_slice(), b"-ec".as_slice()),
                    (b"CC", b"c99"),
                    (b"CFLAGS", b"-O1"),
                    (b"FC", b"fort77"),
                    (b"FFLAGS", b"-O1"),
                    (b"SCCSGETFLAGS", b"-s"),
                    (b"ARFLAGS", b"-rv"),
                ] {
                    self.variables
                        .define_literal(name, value, Origin::Default)
                        .map_err(|err| ParseError {
                            unlocated: err.is_unlocated(),
                            location: location.clone(),
                            message: err.diagnostic(),
                        })?;
                }
            }
            self.second_expansion |= declaration
                .targets
                .iter()
                .any(|target| target == b".SECONDEXPANSION");
            declaration.prerequisites.extend_from_slice(prereqs);
            declaration.header = Some(RuleHeader {
                static_pattern,
                double_colon: delimiter.double_colon,
            });
            let recipe = source_inline
                .map(|i| line.bytes[i + 1..].to_vec())
                .or_else(|| expanded_inline.map(|i| expanded_line[i + 1..].to_vec()));
            if let Some(bytes) = recipe {
                self.add_recipe(
                    declaration,
                    Recipe {
                        bytes,
                        location,
                        prefix,
                        builtin: false,
                    },
                );
            }
        }
        Ok(())
    }

    #[inline]
    fn finish_declaration(
        &mut self,
        declaration: &mut RuleDeclaration,
        location: &Location,
        context: &mut ExpansionContext<'_>,
    ) -> Result<(), ParseError> {
        if declaration.header.is_some() && self.rules_frozen {
            return Err(ParseError {
                unlocated: false,
                location: declaration.location.clone().expect("rule header location"),
                message: "prerequisites cannot be defined in recipes".into(),
            });
        }
        let missing_group = declaration.grouped
            && declaration.recipes.is_empty()
            && !declaration.targets.is_empty();
        if declaration.header.is_some() && !missing_group {
            self.record_declaration(declaration, location, context)?;
        }
        declaration.finish(&mut self.rules, &mut self.warnings, context)
    }

    fn record_declaration(
        &mut self,
        declaration: &mut RuleDeclaration,
        closing_location: &Location,
        context: &mut ExpansionContext<'_>,
    ) -> Result<(), ParseError> {
        let header = declaration.header.take().expect("open rule header");
        let location = declaration.location.clone().expect("rule header location");
        let double_colon = header.double_colon;
        let grouped = declaration.grouped;
        let static_pattern = header.static_pattern;
        let mut prerequisite_text = std::mem::take(&mut declaration.prerequisites);
        let prereqs = crate::filenames::unquote_colons(&prerequisite_text);
        let prereqs = prereqs.as_ref();
        let secondary = self.second_expansion && prereqs.contains(&b'$');
        let (normal, order) = crate::filenames::split_order(prereqs);
        let normal = normal.as_ref();
        let order = order.unwrap_or_default();
        let empty_dependencies = crate::filenames::words(normal).next().is_none()
            && crate::filenames::words(order).next().is_none();
        // The expanded text is frozen at the header. Filename observations
        // wait for the complete declaration, including conditional effects.
        let prerequisite_names = if secondary
            || !prereqs
                .iter()
                .any(|byte| matches!(byte, b'(' | b'*' | b'?' | b'[' | b'~'))
        {
            None
        } else {
            let mut names = Vec::new();
            for (text, order_only) in [(normal, false), (order, true)] {
                for name in self.parsed_rule_filenames(text, closing_location, context)? {
                    names.push((name, order_only));
                }
            }
            Some(names)
        };
        let implicit = declaration.target_patterns[0];
        if implicit && static_pattern.is_some() {
            return Err(ParseError {
                unlocated: false,
                location,
                message: "mixed implicit and static pattern rules".into(),
            });
        }
        if implicit && declaration.target_patterns.iter().any(|pattern| !pattern) {
            return Err(ParseError {
                unlocated: false,
                location,
                message: "mixed implicit and normal rules".into(),
            });
        }
        // GNU classifies the declaration by its first target. Subsequent
        // percent names in an explicit declaration are literal file records.
        if !implicit {
            declaration.patterns.clear();
        }
        if grouped && double_colon && declaration.patterns.is_empty() {
            declaration.grouped_double_id = Some(self.next_group_id);
            self.next_group_id += 1;
        }
        let mut recipe_targets = HashSet::new();
        for (ordinal, target) in declaration.targets.iter().enumerate() {
            // The targets share prerequisite spellings. Static patterns
            // derive separate names for each target.
            let bind_prerequisites = ordinal == 0 || static_pattern.is_some();
            if ordinal != 0 && !implicit && declaration.target_patterns[ordinal] {
                record_warning(
                    &mut self.warnings,
                    context,
                    location.clone(),
                    "*** mixed implicit and normal rules: deprecated syntax".into(),
                );
            }
            if target == b".DEFAULT" && empty_dependencies {
                declaration.cancel_default = true;
            }
            let mut unmatched = false;
            let stem = if let Some(pattern) = &static_pattern {
                if pattern.matches(target).is_none() {
                    record_warning(
                        &mut self.warnings,
                        context,
                        location.clone(),
                        [
                            b"target '".as_slice(),
                            target,
                            b"' doesn't match the target pattern",
                        ]
                        .concat()
                        .into(),
                    );
                    unmatched = true;
                }
                Some(static_stem(pattern, target))
            } else {
                None
            };
            let record_start = if !implicit {
                self.rules
                    .get(target)
                    .map_or(0, |rule| rule.dependencies.len())
            } else {
                0
            };
            let mut group_ordinal = 0;
            let rule = if let Some(pattern) = declaration.patterns.get(target) {
                self.invalidate_implicit_rule_indexes();
                let index = *declaration
                    .implicit
                    .entry(target.clone())
                    .or_insert_with(|| {
                        self.implicit_rules.push((pattern.clone(), Rule::default()));
                        self.implicit_rules.len() - 1
                    });
                self.implicit_rules[index].1.terminal = double_colon;
                if declaration.patterns.len() > 1 {
                    declaration.implicit_group_identity.get_or_insert(index);
                }
                &mut self.implicit_rules[index].1
            } else {
                self.explicit_names.insert(target.clone());
                let occupied = self.rules.contains_key(target);
                let root = self.rules.entry(target.clone()).or_default();
                root.record_only = false;
                if occupied && root.double_colon.is_some() != double_colon {
                    return Err(ParseError {
                        unlocated: false,
                        location: location.clone(),
                        message: [
                            b"target file '".as_slice(),
                            target,
                            b"' has both : and :: entries",
                        ]
                        .concat()
                        .into(),
                    });
                }
                if double_colon {
                    let branches = root.double_colon.get_or_insert_with(Vec::new);
                    let index = branches.len();
                    branches.push(Rule::default());
                    let occurrences = declaration.branches.entry(target.clone()).or_default();
                    group_ordinal = occurrences.len();
                    occurrences.push(index);
                    &mut branches[index]
                } else {
                    root
                }
            };
            if grouped || (declaration.patterns.len() > 1) {
                if grouped && !double_colon && rule.group.is_some() {
                    declaration.group_warnings.push((
                        location.clone(),
                        [
                            b"warning: overriding group membership for target '".as_slice(),
                            target,
                            b"'",
                        ]
                        .concat()
                        .into(),
                    ));
                }
                let group = rule.group.get_or_insert_with(Default::default);
                group.targets = declaration.targets.clone();
                group.double_id = declaration.grouped_double_id;
                group.ordinal = group_ordinal;
                group.implicit_identity = declaration.implicit_group_identity;
            }
            rule.prerequisite_location = Some(location.clone());
            if let Some(stem) = &stem {
                rule.stem = Some(stem.clone());
            }
            if unmatched {
                declaration
                    .offsets
                    .entry(target.clone())
                    .or_insert(rule.dependencies.len());
            }
            if !unmatched {
                if secondary {
                    rule.secondary.get_or_insert_with(Default::default).push(
                        SecondaryPrerequisite {
                            index: rule.dependencies.len(),
                            discard_targeted_wait: false,
                            stem: stem.clone(),
                        },
                    );
                    rule.dependencies.push(Dependency {
                        name: if stem.is_some() {
                            static_secondary_prerequisites(prereqs)
                        } else {
                            prereqs.to_vec()
                        },
                        order_only: false,
                        hidden: false,
                    });
                    declaration
                        .offsets
                        .entry(target.clone())
                        .or_insert(rule.dependencies.len() - 1);
                } else {
                    if target == b".SUFFIXES" && empty_dependencies {
                        // Clearing suffix metadata retains its recipe and location.
                        // Record offsets only after clearing, including repeated
                        // target names in the same declaration.
                        rule.dependencies.clear();
                        rule.secondary = None;
                    }
                    declaration
                        .offsets
                        .entry(target.clone())
                        .or_insert(rule.dependencies.len());
                    let observed_prereqs = prerequisite_names.is_some();
                    let names = prerequisite_names
                        .as_ref()
                        .into_iter()
                        .flatten()
                        .map(|(name, order_only)| (Cow::Borrowed(name.as_ref()), *order_only))
                        .chain(
                            crate::filenames::words(if observed_prereqs { b"" } else { normal })
                                .map(|name| (name, false)),
                        )
                        .chain(
                            crate::filenames::words(if observed_prereqs { b"" } else { order })
                                .map(|name| (name, true)),
                        );
                    let mut pending_wait = None;
                    for (name, order_only) in names {
                        let name = if let Some(stem) = &stem {
                            static_prerequisite(&name, stem).map_err(|error| ParseError {
                                unlocated: false,
                                location: location.clone(),
                                message: error.diagnostic(),
                            })?
                        } else {
                            name.into_owned()
                        };
                        // Static replacement can erase the entire prerequisite.
                        if name.is_empty() {
                            continue;
                        }
                        let name = if !observed_prereqs && name.starts_with(b"./") {
                            crate::filenames::without_dot_prefix(&name).to_vec()
                        } else {
                            name
                        };
                        if !declaration.patterns.contains_key(target) && stem.is_none() {
                            self.explicit_names.insert(name.clone());
                        }
                        if bind_prerequisites
                            && !observed_prereqs
                            && let Some(history) = &self.database_history
                        {
                            history.intern(&name);
                        }
                        Dependency {
                            name,
                            order_only,
                            hidden: false,
                        }
                        .append_to_partition(&mut rule.dependencies, &mut pending_wait);
                    }
                }
            }
            if !implicit {
                if bind_prerequisites && !secondary && !unmatched {
                    // Clearing .SUFFIXES can shorten the rule below its old length.
                    let start = record_start.min(rule.dependencies.len());
                    for dependency in rule.dependencies[start..]
                        .iter()
                        .filter(|dep| dep.name != b".WAIT")
                    {
                        self.file_records.enter_prerequisite(&dependency.name);
                    }
                }
                if let Some(history) = &self.database_history {
                    history.intern(target);
                    if double_colon {
                        // Classify the existing record before creating this
                        // independent branch, including the first declaration.
                        history.record_file_lookup(target);
                    }
                }
                self.file_records.insert(target);
            }
            let duplicate = !declaration.recipes.is_empty()
                && !double_colon
                && !implicit
                && !recipe_targets.insert(target.as_slice());
            if duplicate && !declaration.recipes.is_empty() {
                declaration.warnings.push((
                    location.clone(),
                    [
                        b"target '".as_slice(),
                        target,
                        b"' given more than once in the same rule",
                    ]
                    .concat()
                    .into(),
                ));
            } else {
                let offset = declaration.offsets[target];
                for (index, recipe) in declaration.recipes.iter().enumerate() {
                    Self::attach_recipe(
                        rule,
                        target,
                        offset,
                        index != 0,
                        &mut declaration.warnings,
                        recipe,
                    );
                }
            }
            if target == b".WAIT" {
                for (mask, present, message) in [
                    (
                        1,
                        !rule.dependencies.is_empty(),
                        ".WAIT should not have prerequisites",
                    ),
                    (2, rule.recipes.is_some(), ".WAIT should not have commands"),
                ] {
                    if present && self.wait_warnings & mask == 0 {
                        self.wait_warnings |= mask;
                        declaration
                            .warnings
                            .push((location.clone(), message.into()));
                    }
                }
            }
            // A later target can fail after this one has emitted warnings.
            for (location, message) in declaration.warnings.drain(..) {
                record_warning(&mut self.warnings, context, location, message);
            }
        }
        drop(prerequisite_names);
        prerequisite_text.clear();
        declaration.prerequisites = prerequisite_text;
        declaration.recipe_started = !declaration.recipes.is_empty();
        declaration.recipes.clear();
        Ok(())
    }

    fn apply_scoped_assignment_qualifiers(
        &self,
        name: &[u8],
        private: bool,
        export: Option<bool>,
    ) -> bool {
        self.evaluation_scope
            .as_ref()
            .is_some_and(|scope| scope.apply_scoped_assignment_qualifiers(name, private, export))
    }

    fn apply_assignment_qualifiers(
        &mut self,
        name: Vec<u8>,
        kind: Assignment,
        private: bool,
        export: Option<bool>,
        flag_binding: Option<ConditionalFlagBinding>,
    ) {
        if !private && export.is_none() {
            return;
        }
        // Flag replay can publish a new current binding. Apply qualifiers to
        // the outer binding selected before that publication.
        if let Some(ConditionalFlagBinding::Outer(layer)) = flag_binding {
            if let Some(scope) = &self.evaluation_scope {
                scope.qualify_outer_flags(layer, private, export);
            }
            return;
        }
        // A suppressed conditional can return an existing lexical or scoped binding.
        // A normal definition applies its modifiers to the global binding.
        if kind == Assignment::Conditional
            && !matches!(flag_binding, Some(ConditionalFlagBinding::Global))
            && (self
                .evaluation_bindings
                .iter()
                .rev()
                .any(|frame| frame.apply_assignment_qualifiers(&name, private, export))
                || self.apply_scoped_assignment_qualifiers(&name, private, export))
        {
            return;
        }
        if private {
            self.variables.set_private(&name, true);
        }
        if let Some(export) = export {
            self.set_global_export(name, export);
        }
    }

    fn set_global_export(&mut self, name: Vec<u8>, export: bool) {
        self.variables.set_export_policy(&name, Some(export));
        if export {
            self.unexports.remove(&name);
            self.exports.insert(name);
        } else {
            self.exports.remove(&name);
            self.unexports.insert(name);
        }
    }

    fn add_recipe(&mut self, declaration: &mut RuleDeclaration, recipe: Recipe) {
        declaration.recipes.push(recipe);
    }

    fn attach_recipe(
        rule: &mut Rule,
        target: &[u8],
        offset: usize,
        recipe_started: bool,
        warnings: &mut Vec<(Location, Diagnostic)>,
        recipe: &Recipe,
    ) {
        if !recipe_started {
            let count = rule.dependencies.len() - offset;
            rule.dependencies.rotate_right(count);
            if let Some(secondary) = &mut rule.secondary {
                let length = rule.dependencies.len();
                for clause in secondary.iter_mut() {
                    clause.index = (clause.index + count) % length;
                }
                secondary.sort_by_key(|clause| clause.index);
            }
            if let Some(previous) = &rule.recipe_location {
                warnings.push((
                    recipe.location.clone(),
                    Diagnostic::system(Message::DuplicateRecipe {
                        ignoring_previous: false,
                        target,
                    }),
                ));
                warnings.push((
                    previous.clone(),
                    Diagnostic::system(Message::DuplicateRecipe {
                        ignoring_previous: true,
                        target,
                    }),
                ));
            }
            rule.recipes = Some(Vec::new());
            rule.recipe_location = Some(recipe.location.clone());
        }
        rule.recipes
            .as_mut()
            .expect("recipe initialized")
            .push(recipe.clone());
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn load_prepares_every_glob_without_interpreting_setup_names_as_archives() {
        let mut db = Database::default();
        let mut requests = Vec::new();
        let mut filesystem = |effect: FilesystemEffect<'_>| {
            let FilesystemEffect::RuleGlob(name) = effect else {
                panic!("module filename used an archive or function effect")
            };
            requests.push(name.to_vec());
            Ok(b"module one()\0module two()\0".to_vec())
        };
        let error = db
            .read_with(
                b"-load ./unit*() second*()\n",
                b"module.mk",
                ExpansionContext::pure().with_filesystem(&mut filesystem),
            )
            .unwrap_err();
        assert_eq!(requests, [b"unit*()".to_vec(), b"second*()".to_vec()]);
        assert_eq!(error.location.file.as_ref(), b"module.mk");
        assert_eq!(error.location.line, 1);
        assert_eq!(
            error.message.as_bytes(),
            b"Empty symbol name for load: module one()"
        );
    }

    #[test]
    fn empty_load_finishes_the_previous_rule_before_expanding_its_operand() {
        let mut db = Database::default();
        let mut output = Vec::new();
        let error = db
            .read_with(
                b"all: dep\n\t@echo previous\nload $(info visited)\n\t@echo orphan\n",
                b"module.mk",
                ExpansionContext::new(None, &mut |event| {
                    if let Output::Info(bytes) = event {
                        output.push(bytes.to_vec());
                    }
                }),
            )
            .unwrap_err();
        assert_eq!(output, [b"visited".to_vec()]);
        assert_eq!(error.location.line, 4);
        assert_eq!(
            error.message.as_bytes(),
            b"recipe commences before first target"
        );
        let all = db.rules.get(b"all".as_slice()).unwrap();
        assert_eq!(all.dependencies[0].name, b"dep");
        assert_eq!(all.recipes.as_ref().unwrap()[0].bytes, b"@echo previous");
    }

    #[test]
    fn anonymous_eval_preserves_real_included_file_errors() {
        let mut db = Database::default();
        let mut include = |_: &[u8]| {
            Ok(crate::diagnostic::IncludedFile {
                name: b"child.mk".to_vec(),
                input: crate::reader::Input::Bytes(b"$(error included)\n".to_vec()),
            })
        };
        let error = db
            .read_evaluated(
                b"include child.mk\n",
                None,
                ExpansionContext::pure().with_includes(&mut include),
            )
            .unwrap_err();
        assert!(!error.unlocated);
        assert_eq!(error.source_location().unwrap().file.as_ref(), b"child.mk");
        let error = Database::default()
            .read_evaluated(b"broken\n", None, ExpansionContext::pure())
            .unwrap_err();
        assert!(error.unlocated);
    }
    use super::*;

    #[test]
    fn named_binding_selection_is_silent_but_its_body_preserves_warning_context() {
        let enabled = std::cell::Cell::new(true);
        let mut db = Database::default();
        db.read(b"VALUE = $(INNER)\n", b"m").unwrap();
        let mut warnings = Vec::new();
        let mut output = |event: Output<'_>| {
            if let Output::Warning { location, message } = event {
                warnings.push((location.cloned(), message.to_vec()));
            }
        };
        assert_eq!(
            db.expand_variable_with(
                b"ABSENT",
                ExpansionContext::with_receiver(
                    None,
                    &mut crate::diagnostic::WarningOutput::new(&mut output, &enabled)
                )
            )
            .unwrap(),
            b""
        );
        assert_eq!(
            db.expand_variable_with(
                b"VALUE",
                ExpansionContext::with_receiver(
                    None,
                    &mut crate::diagnostic::WarningOutput::new(&mut output, &enabled)
                )
            )
            .unwrap(),
            b""
        );
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].0.as_ref().unwrap().file.as_ref(), b"m");
        assert_eq!(warnings[0].0.as_ref().unwrap().line, 1);
        assert_eq!(warnings[0].1, b"warning: undefined variable 'INNER'");
    }

    #[test]
    fn live_goal_deletion_checks_origin_without_blocking_normal_resets() {
        let mut db = Database::default();
        db.variables
            .define_literal(b".DEFAULT_GOAL", b"all", Origin::File)
            .unwrap();
        db.activate_runtime_metadata();
        let error = db.read(b"undefine .DEFAULT_GOAL\n", b"m").unwrap_err();
        assert_eq!(
            error.message.as_bytes(),
            crate::safety::SafetyError::DefaultGoalDeletion
                .message()
                .as_bytes()
        );
        assert_eq!(
            db.variables.get(b".DEFAULT_GOAL").unwrap().raw_value(),
            b"all"
        );
        db.read(b".DEFAULT_GOAL :=\n", b"m").unwrap();
        assert_eq!(db.variables.get(b".DEFAULT_GOAL").unwrap().raw_value(), b"");
        db.variables
            .define_literal(b".DEFAULT_GOAL", b"all", Origin::CommandLine)
            .unwrap();
        db.read(b"undefine .DEFAULT_GOAL\n", b"m").unwrap();
        assert_eq!(
            db.variables.get(b".DEFAULT_GOAL").unwrap().raw_value(),
            b"all"
        );
        assert!(db.read(b"override undefine .DEFAULT_GOAL\n", b"m").is_err());
    }

    #[test]
    fn active_export_marker_unwinds_on_error_and_tracks_global_binding_selection() {
        let mut db = Database::default();
        db.read(b"export A = $(eval undefine A)BAD\n", b"m")
            .unwrap();
        let mut variables = db.variables.clone();
        let result = db.expand_environment_in(&mut variables, b"A", ExpansionContext::pure());
        assert!(
            result
                .unwrap_err()
                .diagnostic()
                .as_bytes()
                .windows(4)
                .any(|part| part == b"S003")
        );
        assert!(db.exporting.is_empty());
        assert!(db.variables.get(b"A").is_some());
        db.read(b"A := VALUE\n", b"m").unwrap();
        assert_eq!(
            db.expand_environment_in(&mut variables, b"A", ExpansionContext::pure())
                .unwrap(),
            b"VALUE"
        );
        assert!(db.exporting.is_empty());

        db.read(b"all: export A = $(eval undefine A)LOCAL\n", b"m")
            .unwrap();
        let mut variables = target_variables(&mut db, b"all");
        assert_eq!(
            db.expand_environment_in(&mut variables, b"A", ExpansionContext::pure())
                .unwrap(),
            b"LOCAL"
        );
        assert!(db.variables.get(b"A").is_none());
        assert!(db.exporting.is_empty());
    }

    fn target_variables(database: &mut Database, target: &[u8]) -> VariableStore {
        let globals = database.variables.clone();
        database
            .variables_for_target_with(target, &globals, ExpansionContext::pure())
            .unwrap()
    }

    fn normals(rule: &Rule) -> Vec<Vec<u8>> {
        rule.normal_prerequisites().map(Vec::from).collect()
    }

    fn orders(rule: &Rule) -> Vec<Vec<u8>> {
        rule.dependencies
            .iter()
            .filter(|d| d.order_only)
            .map(|d| d.name.clone())
            .collect()
    }

    #[test]
    fn expanded_empty_export_lists_preserve_global_policy_and_emit_effects() {
        for initial in [false, true] {
            for directive in [b"export".as_slice(), b"unexport"] {
                for expression in [
                    b"$(UNDEFINED)".as_slice(),
                    b"$(SPACES)",
                    b"$(warning visited)",
                ] {
                    let mut db = Database {
                        export_all: initial,
                        ..Database::default()
                    };
                    db.variables
                        .define_literal(b"SPACES", b" \t\n\x0b\x0c\r", Origin::File)
                        .unwrap();
                    let mut warnings = Vec::new();
                    let input = [directive, b" ", expression, b"\n"].concat();
                    db.read_with(
                        &input,
                        b"m",
                        ExpansionContext::new(None, &mut |output| {
                            if let Output::Warning { message, .. } = output {
                                warnings.push(message.to_vec());
                            }
                        }),
                    )
                    .unwrap();
                    assert_eq!(db.export_all, initial);
                    assert_eq!(
                        warnings.len(),
                        usize::from(expression == b"$(warning visited)")
                    );
                    assert!(db.exports.is_empty() && db.unexports.is_empty());
                }
            }
        }
        let mut db = Database::default();
        let mut messages = Vec::new();
        assert!(
            db.read_with(
                b"export $(warning retained)$(error stop)\n",
                b"m",
                ExpansionContext::new(None, &mut |output| {
                    if let Output::Warning { message, .. } = output {
                        messages.push(message.to_vec());
                    }
                })
            )
            .is_err()
        );
        assert_eq!(messages, vec![b"retained".to_vec()]);
        assert!(!db.export_all);
    }

    #[test]
    fn raw_empty_export_directives_toggle_policy_including_vertical_tab_blanks() {
        for (directive, expected) in [(b"export".as_slice(), true), (b"unexport", false)] {
            let mut db = Database {
                export_all: !expected,
                ..Database::default()
            };
            let input = [directive, b" \t\x0b\x0c\r \n"].concat();
            db.read(&input, b"m").unwrap();
            assert_eq!(db.export_all, expected);
            assert!(db.exports.is_empty() && db.unexports.is_empty());
        }
    }

    #[test]
    fn expanded_export_names_split_c_whitespace_but_preserve_other_byte_names() {
        for directive in [b"export".as_slice(), b"unexport"] {
            let mut db = Database::default();
            db.variables
                .define_literal(
                    b"NAMES",
                    b"X\tY\x0bZ\x0cQ\rR\nS X A\x85B C\xa0D",
                    Origin::File,
                )
                .unwrap();
            db.read(&[directive, b" $(NAMES)\n"].concat(), b"m")
                .unwrap();
            for name in [
                b"X".as_slice(),
                b"Y",
                b"Z",
                b"Q",
                b"R",
                b"S",
                b"A\x85B",
                b"C\xa0D",
            ] {
                assert_eq!(
                    db.variables.get(name).unwrap().flavor(),
                    crate::variables::Flavor::Simple
                );
                assert_eq!(db.exports.contains(name), directive == b"export");
                assert_eq!(db.unexports.contains(name), directive == b"unexport");
            }
            assert!(db.variables.get(b"Y\x0bZ").is_none());
            assert!(!db.export_all);
        }
    }

    #[test]
    fn export_and_unexport_lists_materialize_empty_simple_file_bindings() {
        for directive in [b"export".as_slice(), b"unexport"] {
            let mut db = Database::default();
            let input = [directive, b" X X Y\nX ?= $(error must-not-expand)\n"].concat();
            db.read(&input, b"m").unwrap();
            for name in [b"X".as_slice(), b"Y"] {
                let value = db.variables.get(name).unwrap();
                assert_eq!(value.raw_value(), b"");
                assert_eq!(value.flavor(), crate::variables::Flavor::Simple);
                assert_eq!(value.origin(), Origin::File);
                assert_eq!(db.exports.contains(name), directive == b"export");
                assert_eq!(db.unexports.contains(name), directive == b"unexport");
            }
            let mut messages = Vec::new();
            db.read_with(
                b"X += $(warning immediate)added\n",
                b"m",
                ExpansionContext::new(None, &mut |output| {
                    if let Output::Warning { message, .. } = output {
                        messages.push(message.to_vec());
                    }
                }),
            )
            .unwrap();
            assert_eq!(messages, vec![b"immediate".to_vec()]);
            assert_eq!(db.variables.get(b"X").unwrap().raw_value(), b"added");
        }
    }

    #[test]
    fn export_name_lists_leave_existing_binding_and_definition_unchanged() {
        for origin in [
            Origin::Default,
            Origin::Environment,
            Origin::File,
            Origin::EnvironmentOverride,
            Origin::CommandLine,
            Origin::Override,
            Origin::Automatic,
        ] {
            for assignment in [Assignment::Simple, Assignment::Recursive] {
                let mut db = Database::default();
                db.variables
                    .define_at(
                        b"X",
                        b"original",
                        assignment,
                        origin,
                        &Location {
                            file: b"old".as_slice().into(),
                            line: 7,
                        },
                    )
                    .unwrap();
                let original = db.variables.get(b"X").unwrap().clone();
                db.read(b"export X\nunexport X\nexport X\n", b"new")
                    .unwrap();
                let exported = db.variables.get(b"X").unwrap();
                assert_eq!(exported.raw_value(), original.raw_value());
                assert_eq!(exported.flavor(), original.flavor());
                assert_eq!(exported.origin(), original.origin());
                assert_eq!(exported.definition(), original.definition());
                assert_eq!(exported.is_private(), original.is_private());
                assert_eq!(exported.export_policy(), Some(true));
                assert!(db.exports.contains(b"X".as_slice()));
            }
        }
    }

    #[test]
    fn export_list_names_are_expanded_once_and_equals_names_remain_metadata() {
        let mut db = Database::default();
        db.read(
            b"X = $(error must-not-rescan)\nN := $$(X) Y=value\nexport $(N)\n",
            b"m",
        )
        .unwrap();
        for name in [b"$(X)".as_slice(), b"Y=value"] {
            assert_eq!(db.variables.get(name).unwrap().raw_value(), b"");
            assert!(db.exports.contains(name));
        }
        assert!(db.variables.get(b"Y").is_none());
    }

    #[test]
    fn assignment_operators_only_start_at_unconsumed_name_tokens() {
        for (input, name, kind) in [
            (
                b"X$+=value".as_slice(),
                b"X$+".as_slice(),
                Assignment::Recursive,
            ),
            (b"X$?=value", b"X$?", Assignment::Recursive),
            (b"X$!=value", b"X$!", Assignment::Recursive),
            (b"X$:=value", b"X$:", Assignment::Recursive),
            (b"X$::=value", b"X$:", Assignment::Simple),
            (b"X$:::=value", b"X$:", Assignment::Simple),
            (b"X$::::=value", b"X$:", Assignment::ImmediateRecursive),
            (b"X$ +=value", b"X$ ", Assignment::Append),
            (b"X$ :=value", b"X$ ", Assignment::Simple),
        ] {
            let spec = assignment(input).unwrap();
            assert_eq!(
                (spec.name, spec.kind, spec.value),
                (name, kind, b"value".as_slice())
            );
        }
        assert_eq!(assignment(b"X!=value").unwrap().kind, Assignment::Shell);
        assert_eq!(assignment(b"X$#Y=value").unwrap().name, b"X$#Y");
        assert_eq!(
            assignment(b"X$(subst z,Z,#)Y=value").unwrap().name,
            b"X$(subst z,Z,#)Y"
        );
        assert_eq!(assignment(b"X;Y=value").unwrap().name, b"X;Y");
    }

    #[test]
    fn assignment_delimiters_ignore_backslash_and_quote_shell_conventions() {
        for (input, name, value, kind) in [
            (
                b"X\\=Y=value".as_slice(),
                b"X\\".as_slice(),
                b"Y=value".as_slice(),
                Assignment::Recursive,
            ),
            (b"X\\:=value", b"X\\", b"value", Assignment::Simple),
            (b"'X=Y'=value", b"'X", b"Y'=value", Assignment::Recursive),
            (
                b"\\$(subst =, ,A=B)=value",
                b"\\$(subst =, ,A=B)",
                b"value",
                Assignment::Recursive,
            ),
            (
                b"${N:=Y}=value",
                b"${N:=Y}",
                b"value",
                Assignment::Recursive,
            ),
        ] {
            let spec = assignment(input).unwrap();
            assert_eq!((spec.name, spec.value, spec.kind), (name, value, kind));
        }
        for input in [
            b"X: =value".as_slice(),
            b"target: X=value",
            b"X::::=value",
            b"$(missing=value",
            b"${missing=value",
            b"X#Y=value",
            b"X\\#Y=value",
        ] {
            assert!(assignment(input).is_none(), "{input:?}");
        }
    }

    #[test]
    fn consumed_shell_operator_is_deferred_data_and_true_shell_assignment_stays_unsupported() {
        let mut db = Database::default();
        db.read(
            b"X$! = $(error deferred)\nY$:=recursive\nZ$::=simple\n",
            b"m",
        )
        .unwrap();
        assert_eq!(
            db.variables.get(b"X").unwrap().raw_value(),
            b"$(error deferred)"
        );
        assert_eq!(
            db.variables.get(b"Y").unwrap().flavor(),
            crate::variables::Flavor::Recursive
        );
        assert_eq!(
            db.variables.get(b"Z").unwrap().flavor(),
            crate::variables::Flavor::Simple
        );
        let error = Database::default()
            .read(b"X!=must-not-run\n", b"m")
            .unwrap_err();
        assert_eq!(
            error.message.as_bytes(),
            b"unsupported expansion feature '!='"
        );
    }

    #[test]
    fn assignment_recognition_rejects_literal_blanks_without_shell_quoting() {
        for name in [
            b"X Y".as_slice(),
            b"X\tY",
            b"X\\ Y",
            b"'X Y'",
            b"\"X Y\"",
            b"$$(N X)",
            b"X$$ Y",
            b"X$\\ Y",
        ] {
            for operator in [b"=".as_slice(), b":=", b"::=", b":::=", b"+=", b"?="] {
                let input = [name, operator, b"value"].concat();
                assert!(assignment(&input).is_none(), "{input:?}");
            }
        }
        assert_eq!(assignment(b"X\\ =value").unwrap().name, b"X\\");
        assert_eq!(assignment(b"X\x0bY=value").unwrap().name, b"X\x0bY");
        assert_eq!(assignment(b"X$ =value").unwrap().name, b"X$ ");
        assert_eq!(assignment(b"X$\t \t=value").unwrap().name, b"X$\t");
        assert!(assignment(b"X$=value").is_none());
    }

    #[test]
    fn assignment_recognition_allows_short_and_balanced_name_references() {
        for name in [
            b"X$ Y".as_slice(),
            b"X$\tY",
            b"$(subst _, ,X_Y)",
            b"pre$(N X)post",
            b"${N ${INNER X}}",
            b"\\$(subst _, ,X_Y)",
            b"$(call f,(a b))",
        ] {
            let input = [name, b"=value"].concat();
            assert_eq!(assignment(&input).unwrap().name, name);
        }
        let mut db = Database::default();
        db.read(b"X$ Y = short\n$(subst _, ,X_Y) = computed\n", b"m")
            .unwrap();
        assert_eq!(db.variables.get(b"XY").unwrap().raw_value(), b"short");
        assert_eq!(db.variables.get(b"X Y").unwrap().raw_value(), b"computed");
        db.read(b"X$ =terminal\n", b"m").unwrap();
        assert_eq!(db.variables.get(b"X").unwrap().raw_value(), b"terminal");
        let error = Database::default().read(b"$ =empty\n", b"m").unwrap_err();
        assert_eq!(error.message.as_bytes(), b"empty variable name");
        let error = Database::default().read(b"X Y=value\n", b"m").unwrap_err();
        assert_eq!(error.message.as_bytes(), b"missing separator");
    }

    #[test]
    fn assignment_starts_skip_c_whitespace_but_trailing_name_only_skips_blanks() {
        for operator in [b"=".as_slice(), b":=", b"::=", b":::=", b"+=", b"?="] {
            for byte in b" \t\n\x0b\x0c\r" {
                let input = [
                    &[*byte],
                    b"X",
                    &[*byte],
                    operator,
                    &[*byte],
                    b"value",
                    &[*byte],
                ]
                .concat();
                let spec = assignment(&input).unwrap();
                let name = if matches!(byte, b' ' | b'\t') {
                    b"X".to_vec()
                } else {
                    vec![b'X', *byte]
                };
                assert_eq!(spec.name, name, "operator {operator:?}, byte {byte}");
                assert_eq!(spec.value, [b"value".as_slice(), &[*byte]].concat());
            }
        }
        assert_eq!(assignment(b"X+ =value").unwrap().name, b"X+");
        assert_eq!(assignment(b"X+\x0b=value").unwrap().name, b"X+\x0b");
        assert_eq!(
            assignment(b"X+ =value").unwrap().kind,
            Assignment::Recursive
        );
        assert_eq!(assignment(b"X='\x0bvalue'").unwrap().value, b"'\x0bvalue'");
        assert!(assignment(b" \t\x0b\x0c\r=value").unwrap().name.is_empty());
        let error = Database::default()
            .read(b"\x0b \t=value\n", b"m")
            .unwrap_err();
        assert_eq!(error.message.as_bytes(), b"empty variable name");
    }

    #[test]
    fn assignment_name_whitespace_from_expansion_remains_literal() {
        for name in [
            b" X".as_slice(),
            b"X ",
            b"\tX",
            b"X\t",
            b"\x0bX",
            b"X\x0b",
            b"\x0cX",
            b"X\r",
        ] {
            let mut db = Database::default();
            db.variables
                .define_literal(b"NAME", name, Origin::File)
                .unwrap();
            db.read(b"$(NAME) := exact\n", b"m").unwrap();
            assert_eq!(db.variables.get(name).unwrap().raw_value(), b"exact");
            assert!(db.variables.get(b"X").is_none());
        }
    }

    #[test]
    fn assignment_whitespace_does_not_change_reader_continuation_blanks() {
        let mut db = Database::default();
        db.read(
            b"\x0bX = \x0bvalue\x0b\nY = a\x0b\\
\tb\nZ\x0c := keep\n",
            b"m",
        )
        .unwrap();
        assert_eq!(db.variables.get(b"X").unwrap().raw_value(), b"value\x0b");
        assert_eq!(db.variables.get(b"Y").unwrap().raw_value(), b"a\x0b b");
        assert_eq!(db.variables.get(b"Z\x0c").unwrap().raw_value(), b"keep");
        assert!(db.variables.get(b"Z").is_none());
    }

    #[test]
    fn conditional_extra_text_warning_occurs_after_left_expansion_before_right() {
        for quoted in [false, true] {
            let args = if quoted {
                b"\"$(warning left)\" \"$(warning right)\"".as_slice()
            } else {
                b"($(warning left),$(warning right))"
            };
            let mut db = Database::default();
            let mut messages = Vec::new();
            let input = [b"ifeq ".as_slice(), args, b" trailing\nendif\n"].concat();
            db.read_with(
                &input,
                b"m",
                ExpansionContext::new(None, &mut |output| {
                    if let Output::Warning { message, .. } = output {
                        messages.push(message.to_vec());
                    }
                }),
            )
            .unwrap();
            assert_eq!(
                messages,
                vec![
                    b"left".to_vec(),
                    b"extraneous text after 'ifeq' directive".to_vec(),
                    b"right".to_vec()
                ]
            );
        }
        let mut db = Database::default();
        let mut emitted = false;
        assert!(
            db.read_with(
                b"ifeq ($(error left),) trailing\nendif\n",
                b"m",
                ExpansionContext::new(None, &mut |_| emitted = true)
            )
            .is_err()
        );
        assert!(!emitted);
    }

    #[test]
    fn conditional_left_effects_precede_malformed_right_operand_errors() {
        for input in [
            b"ifeq ($(warning left),missing\n".as_slice(),
            b"ifeq \"$(warning left)\" \"missing\n",
            b"ifeq \"$(warning left)\" missing\n",
        ] {
            let mut db = Database::default();
            let mut messages = Vec::new();
            let error = db
                .read_with(
                    input,
                    b"m",
                    ExpansionContext::new(None, &mut |output| {
                        if let Output::Warning { message, .. } = output {
                            messages.push(message.to_vec());
                        }
                    }),
                )
                .unwrap_err();
            assert_eq!(error.message.as_bytes(), b"invalid syntax in conditional");
            assert_eq!(messages, vec![b"left".to_vec()]);
        }
    }

    #[test]
    fn selected_default_goal_preserves_already_expanded_literal_dollars() {
        let mut db = Database::default();
        db.read(b"X = $(error must-not-expand)\n$$(X): ;\n", b"m")
            .unwrap();
        let goal = db.variables.get(b".DEFAULT_GOAL").unwrap();
        assert_eq!(goal.flavor(), crate::variables::Flavor::Simple);
        assert_eq!(goal.raw_value(), b"$(X)");
        assert!(db.rules.contains_key(b"$(X)".as_slice()));
    }

    #[test]
    fn parser_emits_raw_warnings_in_order_even_when_a_later_line_fails() {
        let mut db = Database::default();
        let mut events = Vec::new();
        let error = db
            .read_with(
                b"t\xff: ; old\n$(warning between)\nt\xff: ; new\n$(info retained)$(error stop)\n",
                b"file\xfe",
                ExpansionContext::new(None, &mut |output| match output {
                    Output::Info(message) => events.push((0, message.to_vec())),
                    Output::Warning { location, message } => {
                        events.push((location.unwrap().line, message.to_vec()))
                    }
                    Output::SystemWarning { location, message } => {
                        events.push((location.unwrap().line, message.english()))
                    }
                }),
            )
            .unwrap_err();
        assert_eq!(error.location.line, 4);
        assert_eq!(
            events,
            vec![
                (2, b"between".to_vec()),
                (3, b"warning: overriding recipe for target 't\xff'".to_vec()),
                (
                    1,
                    b"warning: ignoring old recipe for target 't\xff'".to_vec()
                ),
                (0, b"retained".to_vec())
            ]
        );
        assert_eq!(db.warnings.len(), 2);
    }

    #[test]
    fn immediate_define_output_uses_endef_and_ignored_simple_assignment_still_emits() {
        let mut db = Database::default();
        db.variables
            .define_literal(b"X", b"cli", Origin::CommandLine)
            .unwrap();
        let mut events = Vec::new();
        db.read_with(b"define V :=\n$(warning body)\nendef\nX := $(warning discarded)\nX ?= $(warning absent)\nall: ;\n", b"m", ExpansionContext::new(None, &mut |output| {
            if let Output::Warning { location, message } = output { events.push((location.unwrap().line, message.to_vec())); }
        })).unwrap();
        assert_eq!(
            events,
            vec![(3, b"body".to_vec()), (4, b"discarded".to_vec())]
        );
        assert_eq!(db.variables.get(b"X").unwrap().raw_value(), b"cli");
    }

    #[test]
    fn initial_suffix_metadata_has_no_goal_or_synthetic_suffix_rules() {
        for enabled in [false, true] {
            let mut db = Database::default();
            db.initialize_suffixes(enabled).unwrap();
            let suffixes = db.variables.get(b"SUFFIXES").unwrap();
            assert_eq!(suffixes.origin(), Origin::Default);
            assert_eq!(suffixes.flavor(), crate::variables::Flavor::Simple);
            assert_eq!(
                suffixes.raw_value(),
                if enabled { DEFAULT_SUFFIXES } else { b"" }
            );
            assert!(db.variables.get(b".DEFAULT_GOAL").is_none());
            assert_eq!(db.rules.len(), usize::from(enabled));
            assert_eq!(
                db.explicit_stem(b"item.o"),
                if enabled { b"item".as_slice() } else { b"" }
            );
            if enabled {
                let rule = &db.rules[b".SUFFIXES".as_slice()];
                assert_eq!(
                    normals(rule),
                    words(DEFAULT_SUFFIXES).map(Vec::from).collect::<Vec<_>>()
                );
                assert!(rule.recipes.is_none());
                assert!(rule.recipe_location.is_none());
            }
        }
    }

    #[test]
    fn suffix_variable_priority_and_value_are_independent_of_metadata() {
        let mut db = Database::default();
        db.variables
            .define(
                b"SUFFIXES",
                b".env",
                Assignment::Recursive,
                Origin::Environment,
            )
            .unwrap();
        db.initialize_suffixes(true).unwrap();
        assert_eq!(db.explicit_stem(b"item.o"), b"item");
        assert_eq!(db.explicit_stem(b"item.env"), b"");
        db.read(b".SUFFIXES:\n.SUFFIXES: .custom\n", b"m").unwrap();
        assert_eq!(db.explicit_stem(b"item.custom"), b"item");
        let variable = db.variables.get(b"SUFFIXES").unwrap();
        assert_eq!(variable.raw_value(), b".env");
        assert_eq!(variable.origin(), Origin::Environment);
        assert_eq!(variable.flavor(), crate::variables::Flavor::Recursive);
    }

    #[test]
    fn empty_suffix_declarations_clear_only_dependencies() {
        for declaration in [
            b".SUFFIXES:".as_slice(),
            b".SUFFIXES: $(EMPTY)",
            b".SUFFIXES: |",
        ] {
            let mut db = Database::default();
            db.read(b".SUFFIXES: .x | .y ; retained\nordinary: prior\n", b"m")
                .unwrap();
            db.read(declaration, b"later").unwrap();
            db.read(b"ordinary:\n", b"later").unwrap();
            let rule = &db.rules[b".SUFFIXES".as_slice()];
            assert!(rule.dependencies.is_empty());
            assert_eq!(rule.recipes.as_ref().unwrap()[0].bytes, b" retained");
            assert_eq!(
                rule.recipe_location,
                Some(Location {
                    file: b"m".as_slice().into(),
                    line: 1
                })
            );
            assert_eq!(
                normals(&db.rules[b"ordinary".as_slice()]),
                [b"prior".to_vec()]
            );
            assert!(db.warnings.is_empty());
        }
    }

    #[test]
    fn repeated_clearing_suffix_target_keeps_recipe_offsets_valid() {
        let mut db = Database::default();
        db.read(b".SUFFIXES: .x ; old\n.SUFFIXES .SUFFIXES: ; new\n", b"m")
            .unwrap();
        let rule = &db.rules[b".SUFFIXES".as_slice()];
        assert!(rule.dependencies.is_empty());
        assert_eq!(rule.recipes.as_ref().unwrap()[0].bytes, b" new");
        assert_eq!(rule.recipe_location.as_ref().unwrap().line, 2);
        assert_eq!(db.warnings.len(), 3);
    }

    #[test]
    fn suffix_order_only_entries_append_and_recipe_blocks_prepend() {
        let mut db = Database::default();
        db.read(b".SUFFIXES: .x\n.SUFFIXES: | .a.x .y\n", b"m")
            .unwrap();
        assert_eq!(db.explicit_stem(b"item.a.x"), b"item.a");
        assert_eq!(db.explicit_stem(b"item.y"), b"item");
        db.read(b".SUFFIXES: .a.x | .z ;\n", b"m").unwrap();
        assert_eq!(db.explicit_stem(b"item.a.x"), b"item");
        assert_eq!(db.explicit_stem(b"item.z"), b"item");
        assert_eq!(
            db.rules[b".SUFFIXES".as_slice()]
                .dependencies
                .iter()
                .map(|d| d.name.as_slice())
                .collect::<Vec<_>>(),
            [b".a.x".as_slice(), b".z", b".x", b".a.x", b".y"]
        );
    }

    #[test]
    fn explicit_stems_use_final_metadata_and_preserve_filename_bytes() {
        let mut db = Database::default();
        db.read(b"dir/item.o: ;\n.PHONY: dir/item.o\nX = .o\n.SUFFIXES: $(X) .\xfe /special x\nX = .later\n", b"m").unwrap();
        for (target, stem) in [
            (b"dir/item.o".as_slice(), b"dir/item".as_slice()),
            (b".o", b""),
            (b"dir/.o", b"dir/"),
            (b"dir/item\xff.\xfe", b"dir/item\xff"),
            (b"dir/special", b"dir"),
            (b"item.x", b"item."),
            (b"item.later", b""),
        ] {
            assert_eq!(db.explicit_stem(target), stem);
        }
    }

    #[test]
    fn default_goal_skips_current_single_and_double_suffix_names() {
        let mut db = Database::default();
        db.read(
            b".SUFFIXES: x | y dir/z\nx: ;\nxy: ;\nxx: ;\ndir/z: ;\nordinary: ;\n",
            b"m",
        )
        .unwrap();
        assert_eq!(
            db.variables.expand(b"$(.DEFAULT_GOAL)").unwrap(),
            b"ordinary"
        );
        db.read(b".DEFAULT_GOAL :=\n.SUFFIXES:\nx: ;\n", b"later")
            .unwrap();
        assert_eq!(db.variables.expand(b"$(.DEFAULT_GOAL)").unwrap(), b"x");
    }

    #[test]
    fn default_goal_uses_suffix_metadata_before_the_current_declaration() {
        for program in [
            b".SUFFIXES x: x ;\n".as_slice(),
            b"x: ;\n.SUFFIXES: x\n",
            b".SUFFIXES: x y\nzxy: ;\n",
        ] {
            let mut db = Database::default();
            db.read(program, b"m").unwrap();
            let expected = if program.starts_with(b".SUFFIXES: x y") {
                b"zxy".as_slice()
            } else {
                b"x"
            };
            assert_eq!(db.variables.expand(b"$(.DEFAULT_GOAL)").unwrap(), expected);
        }
    }

    #[test]
    fn immediate_prerequisites_and_deferred_recipes() {
        let mut db = Database::default();
        db.read(b"X=early\nall: $(X)\n\t@echo $(X)\nX=late\n", b"Makefile")
            .unwrap();
        assert_eq!(normals(&db.rules[b"all".as_slice()]), [b"early".to_vec()]);
        let recipe = &db.rules[b"all".as_slice()].recipes.as_ref().unwrap()[0];
        assert_eq!(recipe.location.line, 3);
        assert_eq!(db.variables.expand(&recipe.bytes).unwrap(), b"@echo late");
    }

    #[test]
    fn conditionals_do_not_expand_inactive_branches() {
        let mut db = Database::default();
        db.read(b"ifeq (a,b)\n$(error hidden)\nifeq ($(error hidden),x)\nendif\nelse ifeq (c,c)\nX=yes\nelse\nX=no\nendif\nall: ;\n", b"Makefile").unwrap();
        assert_eq!(db.variables.expand(b"$(X)").unwrap(), b"yes");
    }

    #[test]
    fn definition_retains_blank_lines_and_defers_expansion() {
        let mut db = Database::default();
        db.read(
            b"define NL\n\n\nendef\ndefine X\n$(Y)\nlast\nendef\nY=first\n",
            b"m",
        )
        .unwrap();
        assert_eq!(db.variables.expand(b"$(NL)").unwrap(), b"\n");
        assert_eq!(db.variables.expand(b"$(X)").unwrap(), b"first\nlast");
    }

    #[test]
    fn inline_recipe_keeps_shell_comment() {
        let mut db = Database::default();
        db.read(b"all: ; echo '# literal' # shell comment\n", b"m")
            .unwrap();
        assert_eq!(
            db.rules[b"all".as_slice()].recipes.as_ref().unwrap()[0].bytes,
            b" echo '# literal' # shell comment"
        );
    }

    #[test]
    fn rule_assignment_is_not_a_global_assignment() {
        assert!(assignment(b"target: NAME = value").is_none());
        assert!(assignment(b"NAME ::= value").is_some());
        let mut db = Database::default();
        db.read(b"target: NAME = value\n", b"m").unwrap();
        assert!(db.rules.is_empty());
        assert!(db.variables.get(b"NAME").is_none());
        assert!(db.variables.get(b".DEFAULT_GOAL").is_none());
        assert_eq!(
            target_variables(&mut db, b"target")
                .expand(b"$(NAME)")
                .unwrap(),
            b"value"
        );
    }

    #[test]
    fn pattern_declarations_do_not_create_rules_or_default_goals() {
        let mut db = Database::default();
        db.read(b"%.o:X=pattern\n", b"m").unwrap();
        assert_eq!(db.pattern_assignments.len(), 1);
        assert!(db.target_scopes.is_empty());
        assert!(db.rules.is_empty());
        assert!(db.variables.get(b".DEFAULT_GOAL").is_none());
        assert!(db.variables.get(b"X").is_none());
        assert_eq!(
            target_variables(&mut db, b"item.o")
                .expand(b"$(X)")
                .unwrap(),
            b"pattern"
        );
    }

    #[test]
    fn pattern_preparation_uses_global_names_and_expands_simple_values_per_pattern() {
        let mut db = Database::default();
        let mut messages = Vec::new();
        db.read_with(
            b"N=global\n%.o:N=local\n%.o %.x:$(N):=$(info read-$(N))value-$(N)-$$N\nN=later\n",
            b"m",
            ExpansionContext::new(None, &mut |output| {
                if let Output::Info(message) = output {
                    messages.push(message.to_vec());
                }
            }),
        )
        .unwrap();
        assert_eq!(messages, [b"read-global".to_vec(), b"read-global".to_vec()]);
        let variables = target_variables(&mut db, b"one.o");
        assert_eq!(
            variables.expand(b"$(N)|$(global)|$(local)").unwrap(),
            b"local|value-global-$N|"
        );
    }

    #[test]
    fn pattern_scope_matching_preserves_full_paths_and_requires_a_nonempty_stem() {
        let mut db = Database::default();
        db.read(b"%.o:X=generic\nf%.o:X=basename\ndir/f%.o:X=path\na%b%c:Y=second-percent-literal\na\\%b%c:Z=escaped\n", b"m").unwrap();
        for (target, expected) in [
            (b"dir/foo.o".as_slice(), b"path".as_slice()),
            (b"other/foo.o".as_slice(), b"generic".as_slice()),
            (b"foo.o".as_slice(), b"basename".as_slice()),
            (b".o".as_slice(), b"".as_slice()),
        ] {
            assert_eq!(
                target_variables(&mut db, target).expand(b"$(X)").unwrap(),
                expected
            );
        }
        assert_eq!(
            target_variables(&mut db, b"axxb%c")
                .expand(b"$(Y)")
                .unwrap(),
            b"second-percent-literal"
        );
        assert_eq!(
            target_variables(&mut db, b"axxbxc")
                .expand(b"$(Y)")
                .unwrap(),
            b""
        );
        assert_eq!(
            target_variables(&mut db, b"a%bxc").expand(b"$(Z)").unwrap(),
            b"escaped"
        );
        assert_eq!(
            target_variables(&mut db, b"a%bc").expand(b"$(Z)").unwrap(),
            b""
        );
    }

    #[test]
    fn repeated_pattern_assignments_keep_interleaved_declaration_order() {
        let mut db = Database::default();
        db.read(b"X=base\n%.o:X+=one\na.%:X+=two\n%.o:X+=three\n", b"m")
            .unwrap();
        assert_eq!(
            target_variables(&mut db, b"a.o")
                .expand(b"$(X)|$(value X)|$(flavor X)")
                .unwrap(),
            b"base one two three|one two three|recursive"
        );
    }

    #[test]
    fn escaped_percent_can_name_a_literal_scope_and_explicit_rule() {
        let mut db = Database::default();
        db.read(b"a\\%b.o:X=literal\na\\%b.o:;\n", b"m").unwrap();
        assert!(db.pattern_assignments.is_empty());
        assert!(db.rules.contains_key(b"a%b.o".as_slice()));
        // GNU rejects a percent-containing name during automatic default-goal
        // selection even when quoting makes its file record explicit.
        assert_eq!(db.variables.expand(b"$(.DEFAULT_GOAL)").unwrap(), b"");
        assert_eq!(
            target_variables(&mut db, b"a%b.o").expand(b"$(X)").unwrap(),
            b"literal"
        );
        assert_eq!(
            target_variables(&mut db, b"axb.o").expand(b"$(X)").unwrap(),
            b""
        );
        let mut patterns = Database::default();
        patterns.read(b"%.o:;\n", b"m").unwrap();
        assert_eq!(patterns.implicit_candidates(b"file.o").len(), 1);
        let mut quoted = Database::default();
        quoted.read(b"a\\ b:;\n", b"m").unwrap();
        assert!(quoted.rules.contains_key(b"a b".as_slice()));
    }

    #[test]
    fn compiled_implicit_replacements_refresh_after_rule_edits() {
        use std::fmt::Write;

        let mut padding = String::new();
        for index in 0..128 {
            writeln!(&mut padding, "ignored{index}%.unused: ;").unwrap();
        }
        let sources: [&[u8]; 2] = [
            b"%.out: seed ; @old\n%.out: | seed ; @new\n%.out %.peer: seed ; @group\n",
            b"%.out: seed\n%.out: other ; @fallback\n",
        ];
        let mut small = Database::default();
        let mut indexed = Database::default();
        indexed.read(padding.as_bytes(), b"padding").unwrap();

        let recipes = |db: &Database| {
            db.implicit_candidates(b"item.out")
                .into_iter()
                .map(|candidate| candidate.rule.recipes.unwrap()[0].bytes.to_vec())
                .collect::<Vec<_>>()
        };
        for source in sources {
            small.read(source, b"rules").unwrap();
            indexed.read(source, b"rules").unwrap();
            assert_eq!(recipes(&indexed), recipes(&small));
            assert!(matches!(indexed.implicit_rule_shadow.get(), Some(Some(_))));
        }
        assert_eq!(
            recipes(&indexed),
            [b" @group".to_vec(), b" @fallback".to_vec()]
        );
    }

    #[test]
    fn medium_implicit_rule_indexes_warm_up_and_reset_after_edits() {
        use std::fmt::Write;

        let mut source = String::from("%.warm: seed ; @before\n");
        for index in 0..64 {
            writeln!(source, "ignored{index}%.unused: ;").unwrap();
        }
        let mut database = Database::default();
        database.read(source.as_bytes(), b"rules").unwrap();
        let recipes = |database: &Database| {
            database
                .implicit_candidates(b"unit.warm")
                .into_iter()
                .map(|candidate| candidate.rule.recipes.unwrap()[0].bytes.clone())
                .collect::<Vec<_>>()
        };

        for _ in 0..16 {
            assert_eq!(recipes(&database), [b" @before".to_vec()]);
        }
        assert!(database.implicit_rule_suffixes.get().is_none());
        assert!(database.implicit_rule_shadow.get().is_none());

        assert_eq!(recipes(&database), [b" @before".to_vec()]);
        assert!(matches!(
            database.implicit_rule_suffixes.get(),
            Some(Some(_))
        ));
        assert!(matches!(database.implicit_rule_shadow.get(), Some(Some(_))));

        database.read(b"%.warm: seed ; @after\n", b"edit").unwrap();
        assert_eq!(recipes(&database), [b" @after".to_vec()]);
        assert!(database.implicit_rule_suffixes.get().is_none());
        assert!(database.implicit_rule_shadow.get().is_none());
    }

    #[test]
    fn implicit_suffix_filter_preserves_order_raw_names_and_edits() {
        use std::fmt::Write;

        let mut small = Database::default();
        let source = b"ab%: ; @prefix\n%xy: ; @suffix\n%:: ; @terminal\n\
            %.out: ; @basename\nobj/%.out: ; @directory\n(%.o): ; @archive\n%.\xff: ; @raw\n";
        small.read(source, b"rules").unwrap();
        let mut indexed = small.clone();
        let mut padding = String::new();
        for index in 0..128 {
            writeln!(padding, "ignored{index}%.unused: ;").unwrap();
        }
        indexed.read(padding.as_bytes(), b"padding").unwrap();

        let recipes = |candidates: Vec<ImplicitCandidate>| {
            candidates
                .into_iter()
                .map(|candidate| candidate.rule.recipes.unwrap()[0].bytes.to_vec())
                .collect::<Vec<_>>()
        };
        for target in [b"abxy".as_slice(), b"obj/file.out", b"item.\xff", b"x"] {
            assert_eq!(
                recipes(indexed.implicit_candidates(target)),
                recipes(small.implicit_candidates(target)),
            );
        }
        assert_eq!(
            recipes(indexed.implicit_candidates(b"abxy")),
            [
                b" @prefix".to_vec(),
                b" @suffix".to_vec(),
                b" @terminal".to_vec()
            ],
        );
        assert_eq!(
            recipes(indexed.implicit_candidates_excluding(b"abxy", &[0])),
            [b" @suffix".to_vec(), b" @terminal".to_vec()],
        );
        let member = b"lib.a(dir/unit.o)";
        assert_eq!(
            recipes(indexed.implicit_archive_candidates_excluding(member, &[])),
            recipes(small.implicit_archive_candidates_excluding(member, &[])),
        );
        assert!(indexed.implicit_rule_suffixes.get().is_some());
        indexed.read(b"%.new: ; @added\n", b"edit").unwrap();
        assert!(indexed.implicit_rule_suffixes.get().is_none());
        assert_eq!(
            recipes(indexed.implicit_candidates(b"item.new")),
            [b" @added".to_vec(), b" @terminal".to_vec()],
        );
    }

    #[test]
    fn more_specific_patterns_precede_exact_target_scope_and_ignore_source_order() {
        let mut db = Database::default();
        db.read(
            b"f%.o:X=narrow\n%.o:X=broad\n%:X=any\nfoo.o:X+=exact\n",
            b"m",
        )
        .unwrap();
        assert_eq!(
            target_variables(&mut db, b"foo.o").expand(b"$(X)").unwrap(),
            b"narrow exact"
        );
        assert_eq!(
            target_variables(&mut db, b"bar.o").expand(b"$(X)").unwrap(),
            b"broad"
        );
        assert_eq!(
            target_variables(&mut db, b"other").expand(b"$(X)").unwrap(),
            b"any"
        );
    }

    #[test]
    fn pattern_immediate_recursive_assignments_wait_until_activation() {
        let mut db = Database::default();
        let mut messages = Vec::new();
        db.read_with(
            b"K=early\n%.o:X:::=$(info deferred-$(K))value-$(K)-$$K\nK=late\n",
            b"m",
            ExpansionContext::new(None, &mut |output| {
                if let Output::Info(message) = output {
                    messages.push(message.to_vec());
                }
            }),
        )
        .unwrap();
        assert!(messages.is_empty());
        let globals = db.variables.clone();
        let variables = db
            .variables_for_target_with(
                b"one.o",
                &globals,
                ExpansionContext::new(None, &mut |output| {
                    if let Output::Info(message) = output {
                        messages.push(message.to_vec());
                    }
                }),
            )
            .unwrap();
        assert_eq!(messages, [b"deferred-late".to_vec()]);
        assert_eq!(
            variables.expand(b"$(X)|$(flavor X)").unwrap(),
            b"value-late-$K|recursive"
        );
    }

    #[test]
    fn pattern_conditionals_use_activation_globals_and_refresh_parent_snapshots() {
        let mut db = Database::default();
        db.read(b"%.o:X?=pattern\nX=later\n", b"m").unwrap();
        assert_eq!(
            target_variables(&mut db, b"one.o").expand(b"$(X)").unwrap(),
            b"later"
        );
        db.read(b"undefine X\n", b"m").unwrap();
        assert_eq!(
            target_variables(&mut db, b"two.o").expand(b"$(X)").unwrap(),
            b"pattern"
        );
        db.read(b"X=global\n%.o:private X?=ignored\n", b"m")
            .unwrap();
        let stale_parent = db.variables.clone();
        let variables = db
            .variables_for_target_with(b"three.o", &stale_parent, ExpansionContext::pure())
            .unwrap();
        assert_eq!(
            variables.expand(b"$(X)|$(origin X)").unwrap(),
            b"|undefined"
        );
        assert!(db.variables.get(b"X").unwrap().is_private());
    }

    #[test]
    fn target_assignment_does_not_attach_a_recipe_or_select_default_goal() {
        let mut db = Database::default();
        db.read(b"scope: X=value\nreal:;\n", b"m").unwrap();
        assert_eq!(db.variables.expand(b"$(.DEFAULT_GOAL)").unwrap(), b"real");
        assert!(!db.rules.contains_key(b"scope".as_slice()));
        let error = Database::default()
            .read(b"real:\nscope:X=value\n\t@echo detached\n", b"m")
            .unwrap_err();
        assert_eq!(error.location.line, 3);
        assert_eq!(
            error.message.as_bytes(),
            b"recipe commences before first target"
        );
    }

    #[test]
    fn target_assignment_keeps_semicolons_and_defers_recursive_values() {
        let mut db = Database::default();
        db.read(b"all: X = before;$(error deferred) # comment\n", b"m")
            .unwrap();
        let variables = target_variables(&mut db, b"all");
        assert_eq!(
            variables.get(b"X").unwrap().raw_value(),
            b"before;$(error deferred) "
        );
        assert!(db.rules.is_empty());
    }

    #[test]
    fn target_names_values_and_computed_variable_names_use_distinct_contexts() {
        let mut db = Database::default();
        let mut messages = Vec::new();
        db.read_with(
            b"TARGETS=a b\nN=global\na:N=LEFT\nb:N=RIGHT\n$(TARGETS): $(N) := $(info $(N))value-$(N)\n",
            b"m",
            ExpansionContext::new(None, &mut |output| {
                if let Output::Info(message) = output {
                    messages.push(message.to_vec());
                }
            }),
        )
        .unwrap();
        assert_eq!(messages, [b"LEFT".to_vec(), b"RIGHT".to_vec()]);
        for (target, name, value) in [
            (
                b"a".as_slice(),
                b"LEFT".as_slice(),
                b"value-LEFT".as_slice(),
            ),
            (
                b"b".as_slice(),
                b"RIGHT".as_slice(),
                b"value-RIGHT".as_slice(),
            ),
        ] {
            let variables = target_variables(&mut db, target);
            assert_eq!(variables.get(name).unwrap().raw_value(), value);
            assert!(variables.get(b"global").is_none());
        }
    }

    #[test]
    fn target_qualifiers_are_binding_metadata_and_can_also_name_variables() {
        let mut db = Database::default();
        db.read(
            b"all: private override export X=one\nall: unexport Y=two\nall: private = three\nall: export = four\n",
            b"m",
        )
        .unwrap();
        let variables = target_variables(&mut db, b"all");
        let x = variables.get(b"X").unwrap();
        assert_eq!(x.origin(), Origin::Override);
        assert!(x.is_private());
        assert_eq!(x.export_policy(), Some(true));
        assert_eq!(variables.get(b"Y").unwrap().export_policy(), Some(false));
        assert_eq!(
            variables.expand(b"$(private)|$(export)").unwrap(),
            b"three|four"
        );
        assert!(db.exports.is_empty());
        assert!(db.unexports.is_empty());
    }

    #[test]
    fn global_export_directives_replace_policy_from_target_conditional_aliases() {
        for (target_modifier, global_modifier, expected) in [
            (b"export".as_slice(), b"unexport".as_slice(), false),
            (b"unexport".as_slice(), b"export".as_slice(), true),
        ] {
            for assignment in [b"X\n".as_slice(), b"X=replaced\n"] {
                let mut db = Database::default();
                db.read(
                    &[
                        b"X=global\nall: ",
                        target_modifier,
                        b" X?=ignored\n",
                        global_modifier,
                        b" ",
                        assignment,
                    ]
                    .concat(),
                    b"m",
                )
                .unwrap();
                assert_eq!(
                    db.variables.get(b"X").unwrap().export_policy(),
                    Some(expected)
                );
                assert_eq!(
                    target_variables(&mut db, b"all").export_policy(b"X"),
                    Some(expected)
                );
            }
        }
    }

    #[test]
    fn global_private_assignments_remain_visible_while_reading_and_hidden_on_descent() {
        let mut db = Database::default();
        db.read(
            b"private X=first\nBEFORE:=$(X)\nX=second\nAFTER:=$(X)\nprivate=ordinary\nexport=also-ordinary\n",
            b"m",
        )
        .unwrap();
        assert_eq!(
            db.variables.expand(b"$(BEFORE)|$(AFTER)").unwrap(),
            b"first|second"
        );
        assert!(db.variables.get(b"X").unwrap().is_private());
        let target = target_variables(&mut db, b"all");
        assert_eq!(target.expand(b"$(X)|$(origin X)").unwrap(), b"|undefined");
        assert_eq!(
            target.expand(b"$(private)|$(export)").unwrap(),
            b"ordinary|also-ordinary"
        );
        db.read(b"Y=existing\nprivate Y?=ignored\n", b"m").unwrap();
        assert!(db.variables.get(b"Y").unwrap().is_private());
        assert_eq!(db.variables.expand(b"$(Y)").unwrap(), b"existing");
    }

    #[test]
    fn undefine_preserves_and_promotes_environment_override_unless_qualified() {
        let mut db = Database::default();
        db.variables.set_environment_override(true);
        db.variables
            .define(
                b"X",
                b"imported",
                Assignment::Recursive,
                Origin::Environment,
            )
            .unwrap();
        db.read(b"all:X=local\nundefine X\n", b"m").unwrap();
        assert_eq!(
            db.variables.expand(b"$(X)|$(origin X)").unwrap(),
            b"imported|environment override"
        );
        assert_eq!(
            target_variables(&mut db, b"all")
                .expand(b"$(X)|$(origin X)")
                .unwrap(),
            b"local|file"
        );
        db.read(b"override undefine X\n", b"m").unwrap();
        assert!(db.variables.get(b"X").is_none());
    }

    #[test]
    fn target_prerequisites_still_expand_from_global_read_context() {
        let mut db = Database::default();
        db.read(b"X=global\nall:X=local\nall:$(X)\n", b"m").unwrap();
        assert_eq!(normals(&db.rules[b"all".as_slice()]), [b"global".to_vec()]);
        assert_eq!(
            target_variables(&mut db, b"all").expand(b"$(X)").unwrap(),
            b"local"
        );
    }

    #[test]
    fn generated_colon_starts_target_assignment_but_generated_prerequisite_does_not() {
        let mut db = Database::default();
        db.read(
            b"DECL=all: X=$$(error deferred)\n$(DECL)\nPREREQ=Y=value\nall:$(PREREQ)\n",
            b"m",
        )
        .unwrap();
        assert_eq!(normals(&db.rules[b"all".as_slice()]), [b"Y=value".to_vec()]);
        assert_eq!(
            target_variables(&mut db, b"all")
                .get(b"X")
                .unwrap()
                .raw_value(),
            b"$(error deferred)"
        );
        assert!(db.variables.get(b"Y").is_none());
    }

    #[test]
    fn generated_colon_preserves_unexpanded_rhs_and_both_delimiters() {
        let mut db = Database::default();
        let mut messages = Vec::new();
        db.read_with(
            b"P=$(info prefix)all:\n$(P) X=$(error deferred);a:b\nall:;\n",
            b"m",
            ExpansionContext::new(None, &mut |output| {
                if let Output::Info(message) = output {
                    messages.push(message.to_vec());
                }
            }),
        )
        .unwrap();
        assert_eq!(messages, [b"prefix".to_vec()]);
        assert_eq!(
            target_variables(&mut db, b"all")
                .get(b"X")
                .unwrap()
                .raw_value(),
            b"$(error deferred);a:b"
        );
        assert!(db.rules[b"all".as_slice()].recipes.is_some());
        let mut db = Database::default();
        db.read(b"P=%.o:\n$(P) X=$(error deferred)\nall:;\n", b"m")
            .unwrap();
        assert_eq!(db.pattern_assignments.len(), 1);
    }

    #[test]
    fn generated_colon_expands_simple_rhs_once_per_explicit_target() {
        let mut db = Database::default();
        let mut messages = Vec::new();
        db.read_with(
            b"P=$(info prefix)a b:\na:X=first\nb:X=second\n$(P) Y:=$(info rhs-$(X))$(X);value:tail\n",
            b"m",
            ExpansionContext::new(None, &mut |output| {
                if let Output::Info(message) = output { messages.push(message.to_vec()); }
            }),
        ).unwrap();
        assert_eq!(
            messages,
            [
                b"prefix".to_vec(),
                b"rhs-first".to_vec(),
                b"rhs-second".to_vec()
            ]
        );
        assert_eq!(
            target_variables(&mut db, b"a").expand(b"$(Y)").unwrap(),
            b"first;value:tail"
        );
        assert_eq!(
            target_variables(&mut db, b"b").expand(b"$(Y)").unwrap(),
            b"second;value:tail"
        );
    }

    #[test]
    fn incremental_rule_expansion_emits_each_prefix_and_prerequisite_effect_once() {
        let mut db = Database::default();
        let mut messages = Vec::new();
        db.read_with(
            b"P=$(info prefix)all:\n$(P) $(info prerequisite)dep;@echo untouched-$$(X) # shell\n",
            b"m",
            ExpansionContext::new(None, &mut |output| {
                if let Output::Info(message) = output {
                    messages.push(message.to_vec());
                }
            }),
        )
        .unwrap();
        assert_eq!(messages, [b"prefix".to_vec(), b"prerequisite".to_vec()]);
        assert_eq!(normals(&db.rules[b"all".as_slice()]), [b"dep".to_vec()]);
        assert_eq!(
            db.rules[b"all".as_slice()].recipes.as_ref().unwrap()[0].bytes,
            b"@echo untouched-$$(X) # shell"
        );
    }

    #[test]
    fn double_colon_variable_declaration_does_not_create_double_colon_rule() {
        let mut db = Database::default();
        db.read(b"all:: X=value\nall:;\n", b"m").unwrap();
        assert_eq!(
            target_variables(&mut db, b"all").expand(b"$(X)").unwrap(),
            b"value"
        );
        assert!(db.rules[b"all".as_slice()].recipes.is_some());
    }

    #[test]
    fn target_assignment_reports_immediate_errors_in_its_parse_context() {
        let error = Database::default()
            .read(b"all:X=$(error failure)\nall:Y:=$(X)\n", b"m")
            .unwrap_err();
        assert_eq!(error.location.line, 2);
        assert_eq!(error.message.as_bytes(), b"failure");
        let error = Database::default()
            .read(b"all: =empty\n", b"m")
            .unwrap_err();
        assert_eq!(error.message.as_bytes(), b"empty variable name");
    }

    #[test]
    fn error_function_uses_immediate_parse_context_including_define_endef() {
        for (input, line) in [
            (
                b"FAIL = $(error raw\xff)\n# gap\nRESULT := $(FAIL)\n".as_slice(),
                3,
            ),
            (
                b"FAIL = $(error raw\xff)\ndefine RESULT :=\n$(FAIL)\nendef\n",
                4,
            ),
        ] {
            let error = Database::default()
                .read(input, b"input\xfe.mk")
                .unwrap_err();
            assert_eq!(
                error.location,
                Location {
                    file: b"input\xfe.mk".as_slice().into(),
                    line
                }
            );
            assert_eq!(error.message.as_bytes(), b"raw\xff");
        }
    }

    #[test]
    fn expansion_diagnostics_preserve_invalid_operand_bytes_and_location() {
        for input in [
            b"X := $(intcmp \xff,0)\n".as_slice(),
            b"$(intcmp \xff,0):\n",
            b"$(intcmp \xff,0) = value\n",
        ] {
            let error = Database::default().read(input, b"bytes.mk").unwrap_err();
            assert_eq!(error.location.file.as_ref(), b"bytes.mk");
            assert_eq!(error.location.line, 1);
            assert_eq!(
                error.message.as_bytes(),
                b"non-numeric first argument to 'intcmp' function: '\xff'"
            );
        }
    }

    #[test]
    fn define_header_binding_and_immediate_rhs_have_distinct_locations() {
        for (input, expected) in [
            (
                b"# gap\ndefine V :=\n$(intcmp bad,0)\nendef\n".as_slice(),
                4,
            ),
            (b"# gap\ndefine $(intcmp bad,0)\nvalue\nendef\n", 2),
            (
                b"NAME = $(intcmp bad,0)\ndefine $(NAME) :=\nvalue\nendef\n",
                1,
            ),
            (
                b"INNER = $(intcmp bad,0)\ndefine V :=\n$(INNER)\nendef\n",
                1,
            ),
        ] {
            let error = Database::default().read(input, b"define.mk").unwrap_err();
            assert_eq!(
                error.location,
                Location {
                    file: b"define.mk".as_slice().into(),
                    line: expected
                }
            );
        }
        let mut database = Database::default();
        database
            .read(
                b"# gap\ndefine V\nfirst\n$(intcmp bad,0)\nendef\n",
                b"define.mk",
            )
            .unwrap();
        assert_eq!(
            database
                .variables
                .expand_variable(b"V")
                .unwrap_err()
                .location(),
            Some(&Location {
                file: b"define.mk".as_slice().into(),
                line: 2
            })
        );
    }

    #[test]
    fn appended_bindings_use_the_new_file_without_rewriting_prior_snapshots() {
        let mut database = Database::default();
        database
            .read(b"# gap\nV = $(intcmp bad,0)\n", b"first\xff.mk")
            .unwrap();
        let snapshot = database.variables.clone();
        database.read(b"V += suffix\n", b"second\xfe.mk").unwrap();
        let error = database.variables.expand_variable(b"V").unwrap_err();
        assert_eq!(
            error.location(),
            Some(&Location {
                file: b"second\xfe.mk".as_slice().into(),
                line: 1
            })
        );
        assert_eq!(
            snapshot.expand_variable(b"V").unwrap_err().location(),
            Some(&Location {
                file: b"first\xff.mk".as_slice().into(),
                line: 2
            })
        );
    }

    #[test]
    fn bytes_and_dynamic_recipe_prefix_are_preserved() {
        let mut db = Database::default();
        db.read(b".RECIPEPREFIX=>\n\xff: \xfe\n>@echo \xfd\n", b"m")
            .unwrap();
        assert_eq!(normals(&db.rules[b"\xff".as_slice()]), [b"\xfe".to_vec()]);
        assert_eq!(
            db.rules[b"\xff".as_slice()].recipes.as_ref().unwrap()[0].bytes,
            b"@echo \xfd"
        );
        assert_eq!(
            db.rules[b"\xff".as_slice()].recipes.as_ref().unwrap()[0].prefix,
            b'>'
        );
    }

    #[test]
    fn recipe_prefix_metadata_survives_later_changes_and_inline_recipes() {
        let mut db = Database::default();
        db.read(
            b".RECIPEPREFIX := >\nfirst: ; @inline\n>>@second\n.RECIPEPREFIX := !\nsecond:\n!@third\n",
            b"Makefile",
        ).unwrap();
        let first = db.rules[b"first".as_slice()].recipes.as_ref().unwrap();
        assert_eq!(first.len(), 2);
        assert!(first.iter().all(|recipe| recipe.prefix == b'>'));
        assert_eq!(first[0].bytes, b" @inline");
        assert_eq!(first[1].bytes, b">@second");
        assert_eq!(
            db.rules[b"second".as_slice()].recipes.as_ref().unwrap()[0].prefix,
            b'!'
        );
    }

    #[test]
    fn recipe_declaration_prerequisites_take_precedence_without_losing_duplicates() {
        let mut db = Database::default();
        db.read(
            b"all: a b | old\nall: c a | new\n\t@echo $+\n\t@echo second\nall: d | later\n",
            b"m",
        )
        .unwrap();
        let rule = &db.rules[b"all".as_slice()];
        assert_eq!(normals(rule), [b"c", b"a", b"a", b"b", b"d"].map(Vec::from));
        assert_eq!(
            orders(rule),
            [b"new".to_vec(), b"old".to_vec(), b"later".to_vec()]
        );
        assert_eq!(rule.recipes.as_ref().unwrap().len(), 2);
        assert!(db.warnings.is_empty());
    }

    #[test]
    fn later_recipe_replaces_previous_recipe_but_retains_all_dependencies() {
        let mut db = Database::default();
        db.read(b"all: old\n\t@echo old\nall: new\n\t@echo new\n", b"m")
            .unwrap();
        let rule = &db.rules[b"all".as_slice()];
        assert_eq!(normals(rule), [b"new".to_vec(), b"old".to_vec()]);
        assert_eq!(rule.recipes.as_ref().unwrap()[0].bytes, b"@echo new");
        assert_eq!(db.warnings.len(), 2);
        assert_eq!(db.warnings[0].0.line, 4);
        assert_eq!(db.warnings[1].0.line, 2);
    }

    #[test]
    fn repeated_targets_retain_duplicate_dependencies_but_not_duplicate_recipes() {
        let mut db = Database::default();
        db.read(b"all all: child\n\t@echo once\n", b"m").unwrap();
        let rule = &db.rules[b"all".as_slice()];
        assert_eq!(normals(rule), [b"child".to_vec(), b"child".to_vec()]);
        assert_eq!(rule.recipes.as_ref().unwrap().len(), 1);
        assert_eq!(db.warnings.len(), 1);
        assert_eq!(
            db.warnings[0].1.as_bytes(),
            b"target 'all' given more than once in the same rule"
        );
    }

    #[test]
    fn no_recipe_and_explicit_empty_recipe_remain_distinct() {
        let mut db = Database::default();
        db.read(b"bare:\nempty: ;\n", b"m").unwrap();
        assert!(db.rules[b"bare".as_slice()].recipes.is_none());
        let empty = db.rules[b"empty".as_slice()].recipes.as_ref().unwrap();
        assert_eq!(empty.len(), 1);
        assert!(empty[0].bytes.is_empty());
    }

    #[test]
    fn empty_ifdef_name_is_false_and_ifndef_is_true() {
        let mut db = Database::default();
        db.read(b"ifdef\nWRONG=yes\nelse\nSELECTED=yes\nendif\nifndef $(UNDEFINED)\nSECOND=yes\nendif\n", b"m").unwrap();
        assert!(db.variables.get(b"WRONG").is_none());
        assert_eq!(
            db.variables.expand(b"$(SELECTED) $(SECOND)").unwrap(),
            b"yes yes"
        );
        assert_eq!(
            db.read(b"ifdef two names\nendif\n", b"m")
                .unwrap_err()
                .message
                .as_bytes(),
            b"invalid syntax in conditional"
        );
    }

    #[test]
    fn ifdef_tests_raw_value_and_conditionals_preserve_significant_spaces() {
        let mut db = Database::default();
        db.read(b"A=$(UNDEFINED)\nifdef A\nRAW=yes\nendif\nifeq ( x, x)\nWRONG=yes\nelse\nSPACES=yes\nendif\nifeq (x , x)\nTRIMMED=yes\nendif\n", b"m").unwrap();
        assert_eq!(
            db.variables.expand(b"$(RAW) $(SPACES) $(TRIMMED)").unwrap(),
            b"yes yes yes"
        );
        assert!(db.variables.get(b"WRONG").is_none());
    }

    #[test]
    fn conditional_nested_parentheses_and_trailing_text_follow_gnu() {
        let mut db = Database::default();
        db.read(
            b"ifeq (a(b,c),a(b,c)) trailing\nX=yes\nelse ignored\nX=no\nendif ignored\n",
            b"m",
        )
        .unwrap();
        assert_eq!(db.variables.expand(b"$(X)").unwrap(), b"yes");
        assert_eq!(
            db.warnings
                .iter()
                .map(|(_, text)| text.as_bytes())
                .collect::<Vec<_>>(),
            [
                b"extraneous text after 'ifeq' directive".as_slice(),
                b"extraneous text after 'else' directive",
                b"extraneous text after 'endif' directive",
            ]
        );
    }

    #[test]
    fn recipe_prefixed_define_text_does_not_open_a_nested_definition() {
        let mut db = Database::default();
        db.read(b"define X\n\tdefine inside\nendef\nall: ;\n", b"m")
            .unwrap();
        assert_eq!(db.variables.expand(b"$(X)").unwrap(), b"\tdefine inside");
    }

    #[test]
    fn inline_recipe_generated_by_expansion_has_immediate_expansion_timing() {
        let mut db = Database::default();
        db.read(b"SEP=;\nall: $(SEP) @echo [$@]\n", b"m").unwrap();
        let rule = &db.rules[b"all".as_slice()];
        assert!(rule.dependencies.is_empty());
        assert_eq!(rule.recipes.as_ref().unwrap()[0].bytes, b" @echo []");
    }

    #[test]
    fn assignment_semicolons_and_comments_are_not_inline_recipes() {
        let mut db = Database::default();
        db.read(
            b"V=literal ; kept # comment\nall: ; echo '# kept' # shell comment\n",
            b"m",
        )
        .unwrap();
        assert_eq!(db.variables.expand(b"$(V)").unwrap(), b"literal ; kept ");
        assert_eq!(
            db.rules[b"all".as_slice()].recipes.as_ref().unwrap()[0].bytes,
            b" echo '# kept' # shell comment"
        );
    }

    #[test]
    fn missing_final_newline_keeps_last_recipe_and_source_location() {
        let mut db = Database::default();
        db.read(b"all:\n\t@echo last", b"m").unwrap();
        let recipe = &db.rules[b"all".as_slice()].recipes.as_ref().unwrap()[0];
        assert_eq!(recipe.bytes, b"@echo last");
        assert_eq!(recipe.location.line, 2);
    }

    #[test]
    fn core_syntax_errors_have_precise_locations_and_messages() {
        for (input, line, message) in [
            (b"not a rule".as_slice(), 1, "missing separator"),
            (
                b"all:\n        echo wrong\n",
                2,
                "missing separator (did you mean TAB instead of 8 spaces?)",
            ),
            (b"\t@echo bad\n", 1, "recipe commences before first target"),
            (b"=value\n", 1, "empty variable name"),
            (b"endif\n", 1, "extraneous 'endif'"),
            (b"ifeq (a,a)\nall: ;\n", 3, "missing 'endif'"),
        ] {
            let error = Database::default().read(input, b"fixture.mk").unwrap_err();
            assert_eq!(error.message.as_bytes(), message.as_bytes());
            assert_eq!(
                error.location,
                Location {
                    file: b"fixture.mk".as_slice().into(),
                    line
                }
            );
        }
    }

    fn dependency_pairs(dependencies: &[Dependency]) -> Vec<(&[u8], bool)> {
        dependencies
            .iter()
            .map(|dependency| (dependency.name.as_slice(), dependency.order_only))
            .collect()
    }

    #[test]
    fn recipe_moves_its_complete_dependency_block_before_earlier_declarations() {
        let mut db = Database::default();
        db.read(
            b"all: a | x\nall: b | y\n\t@echo first\n\t@echo second\nall: c | z\n",
            b"m",
        )
        .unwrap();
        let rule = &db.rules[b"all".as_slice()];
        // Oracle traversal is b,y,a,x,c,z. Splitting normal/order-only lists
        // changes this order, and rotating again for the second recipe does too.
        assert_eq!(
            dependency_pairs(&rule.dependencies),
            vec![
                (b"b".as_slice(), false),
                (b"y", true),
                (b"a", false),
                (b"x", true),
                (b"c", false),
                (b"z", true),
            ]
        );
        assert_eq!(rule.recipes.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn empty_recipe_with_only_order_only_dependencies_still_prepends_its_block() {
        let mut db = Database::default();
        db.read(b"all: a | x\nall: | y ;\nall: b | z\n", b"m")
            .unwrap();
        let rule = &db.rules[b"all".as_slice()];
        assert_eq!(
            dependency_pairs(&rule.dependencies),
            vec![
                (b"y".as_slice(), true),
                (b"a", false),
                (b"x", true),
                (b"b", false),
                (b"z", true),
            ]
        );
        assert_eq!(normals(rule), [b"a", b"b"].map(Vec::from));
        assert!(rule.recipes.as_ref().unwrap()[0].bytes.is_empty());
    }

    #[test]
    fn canonical_promotion_does_not_rewrite_raw_first_or_repeated_prerequisites() {
        let mut db = Database::default();
        db.read(b"all: | x x z ; @echo all\nall: b x x | y\n", b"m")
            .unwrap();
        let rule = &db.rules[b"all".as_slice()];
        let raw = rule.dependencies.clone();
        let canonical = rule.canonical_dependencies();
        assert_eq!(
            dependency_pairs(&canonical),
            vec![
                (b"x".as_slice(), false),
                (b"z", true),
                (b"b", false),
                (b"y", true),
            ]
        );
        // GNU gives $< = b and $+ = b x x, but $^/$? = x b and $| = z y.
        assert_eq!(rule.normal_prerequisites().next(), Some(b"b".as_slice()));
        assert_eq!(normals(rule), [b"b", b"x", b"x"].map(Vec::from));
        assert_eq!(orders(rule), [b"x", b"x", b"z", b"y"].map(Vec::from));
        assert_eq!(rule.dependencies, raw);
        assert_eq!(rule.canonical_dependencies(), canonical);
    }

    #[test]
    fn multiple_targets_rotate_against_their_own_prior_dependency_blocks() {
        let mut db = Database::default();
        db.read(
            b"left: a | x\nright: b | y\nleft right: c | z\n\t@echo shared\n",
            b"m",
        )
        .unwrap();
        assert_eq!(
            dependency_pairs(&db.rules[b"left".as_slice()].dependencies),
            vec![
                (b"c".as_slice(), false),
                (b"z", true),
                (b"a", false),
                (b"x", true)
            ]
        );
        assert_eq!(
            dependency_pairs(&db.rules[b"right".as_slice()].dependencies),
            vec![
                (b"c".as_slice(), false),
                (b"z", true),
                (b"b", false),
                (b"y", true)
            ]
        );
    }

    #[test]
    fn repeated_targets_preserve_repeated_whole_blocks_for_raw_observations() {
        let mut db = Database::default();
        db.read(b"all all: a | x\n\t@echo once\nall: b | y\n", b"m")
            .unwrap();
        let rule = &db.rules[b"all".as_slice()];
        assert_eq!(
            dependency_pairs(&rule.dependencies),
            vec![
                (b"a".as_slice(), false),
                (b"x", true),
                (b"a", false),
                (b"x", true),
                (b"b", false),
                (b"y", true),
            ]
        );
        assert_eq!(normals(rule), [b"a", b"a", b"b"].map(Vec::from));
        assert_eq!(rule.canonical_dependencies().len(), 4);
        assert_eq!(rule.recipes.as_ref().unwrap().len(), 1);
        assert_eq!(db.warnings.len(), 1);
    }

    #[test]
    fn merged_dependency_views_preserve_non_utf8_names() {
        let mut db = Database::default();
        db.read(b"all: | \xff ;\nall: \xfe \xff\n", b"m").unwrap();
        let rule = &db.rules[b"all".as_slice()];
        assert_eq!(normals(rule), [b"\xfe".to_vec(), b"\xff".to_vec()]);
        assert_eq!(
            dependency_pairs(&rule.canonical_dependencies()),
            vec![(b"\xff".as_slice(), false), (b"\xfe", false)]
        );
    }
}

#[cfg(test)]
mod rule_tilde_tests {
    use super::*;

    #[test]
    fn literal_names_and_named_accounts_do_not_evaluate_home() {
        let mut db = Database::default();
        db.read(b"HOME=$(error unused)\n", b"m").unwrap();
        assert!(matches!(
            db.rule_tilde(b"word~name", &mut ExpansionContext::pure())
                .unwrap(),
            Cow::Borrowed(_)
        ));
        let mut calls = Vec::new();
        let mut filesystem = |effect: FilesystemEffect<'_>| {
            let FilesystemEffect::Tilde(name) = effect else {
                panic!("unexpected filesystem effect")
            };
            calls.push(name.to_vec());
            Ok(Vec::new())
        };
        assert_eq!(
            db.rule_tilde(
                b"~unknown/a",
                &mut ExpansionContext::pure().with_filesystem(&mut filesystem)
            )
            .unwrap()
            .as_ref(),
            b"~unknown/a"
        );
        assert_eq!(calls, [b"~unknown/a".to_vec()]);
    }

    #[test]
    fn home_is_expanded_once_and_filename_fallbacks_remain_bounded() {
        let mut db = Database::default();
        db.read(b"HOME=$(eval SEEN:=yes)~literal\n", b"m").unwrap();
        assert_eq!(
            db.rule_tilde(b"~/a", &mut ExpansionContext::pure())
                .unwrap()
                .as_ref(),
            b"~literal/a"
        );
        assert_eq!(db.variables.expand(b"$(SEEN)").unwrap(), b"yes");
        db.read(b"HOME=\n", b"m").unwrap();
        let mut filesystem = |_effect: FilesystemEffect<'_>| Ok(vec![b'x'; MAX_OUTPUT + 1]);
        assert!(matches!(
            db.rule_tilde(
                b"~",
                &mut ExpansionContext::pure().with_filesystem(&mut filesystem)
            ),
            Err(ExpandError::ExpansionLimit)
        ));
    }
}

#[cfg(test)]
mod recursive_home_tests {
    use super::*;

    #[test]
    fn nested_filename_query_replaces_the_suffix_with_owned_bytes() {
        let mut db = Database::default();
        db.read(b"HOME=$(if $(wildcard marker),late,early)\n", b"Makefile")
            .unwrap();
        let mut filesystem = |_effect: FilesystemEffect<'_>| Ok(Vec::new());
        let mut context = ExpansionContext::pure().with_filesystem(&mut filesystem);
        assert_eq!(
            db.rule_tilde(b"~/target", &mut context).unwrap(),
            b"earlyarker".as_slice()
        );
        assert!(db.filename_scratch.is_none());
    }

    #[test]
    fn a_nested_failure_releases_the_filename_session() {
        let mut db = Database::default();
        db.read(b"HOME=$(wildcard marker)$(error stop)\n", b"Makefile")
            .unwrap();
        let mut filesystem = |_effect: FilesystemEffect<'_>| Ok(Vec::new());
        let mut output = |_output: Output<'_>| {};
        let mut context = ExpansionContext::new(None, &mut output).with_filesystem(&mut filesystem);
        assert!(db.rule_filenames(b"~/target", &mut context).is_err());
        assert!(db.filename_scratch.is_none());
        db.read(b"HOME=ordinary\n", b"later.mk").unwrap();
        assert_eq!(
            db.rule_tilde(b"~/next", &mut context).unwrap(),
            b"ordinary/next".as_slice()
        );
        assert!(db.filename_scratch.is_none());
    }

    #[test]
    fn a_group_continuation_observes_the_prior_nul_and_retained_prefix() {
        let mut db = Database::default();
        db.read(b"HOME=$(wildcard x)early\n", b"Makefile").unwrap();
        let mut filesystem = |_effect: FilesystemEffect<'_>| Ok(Vec::new());
        let mut context = ExpansionContext::pure().with_filesystem(&mut filesystem);
        let names: Vec<Vec<u8>> = db
            .rule_filenames(b"~/lib(a b)", &mut context)
            .unwrap()
            .map(Cow::into_owned)
            .collect();
        assert_eq!(names, [b"early".to_vec(), b"x".to_vec()]);
        assert!(db.filename_scratch.is_none());
    }
}

#[cfg(test)]
mod posix_mode_tests {
    use super::*;

    #[test]
    fn activation_controls_later_reads_and_does_not_repeat_defaults() {
        let mut db = Database::default();
        db.variables
            .define_literal(b"CC", b"mine", Origin::File)
            .unwrap();
        db.read(
            b"A:=before  \\\n after\n.POSIX:\nB:=after  \\\n next\n",
            b"one.mk",
        )
        .unwrap();
        assert!(db.posix);
        assert_eq!(db.variables.get(b"A").unwrap().raw_value(), b"before after");
        assert_eq!(db.variables.get(b"B").unwrap().raw_value(), b"after   next");
        assert_eq!(db.variables.get(b"CC").unwrap().raw_value(), b"mine");
        assert_eq!(db.variables.get(b"CFLAGS").unwrap().raw_value(), b"-O1");
        assert_eq!(
            db.variables.get(b"CFLAGS").unwrap().origin(),
            Origin::Default
        );
        db.read(
            b"undefine CFLAGS\n.POSIX:\nC:=later  \\\n read\n",
            b"two.mk",
        )
        .unwrap();
        assert!(db.variables.get(b"CFLAGS").is_none());
        assert_eq!(db.variables.get(b"C").unwrap().raw_value(), b"later   read");
    }

    #[test]
    fn references_and_target_assignments_do_not_activate_the_mode() {
        for input in [
            b".PHONY: .POSIX\n".as_slice(),
            b"all: .POSIX\n",
            b".POSIX: X=local\n",
            b"N=.POSIX\n",
        ] {
            let mut db = Database::default();
            db.read(input, b"input.mk").unwrap();
            assert!(!db.posix);
            assert!(db.variables.get(b"CFLAGS").is_none());
        }
        let mut db = Database::default();
        db.read(b"$(eval .POSIX:)\n", b"input.mk").unwrap();
        assert!(db.posix);
    }

    #[test]
    fn two_suffix_rule_with_prerequisites_stays_an_ordinary_record() {
        for posix in [false, true] {
            let mut db = Database {
                posix,
                ..Database::default()
            };
            db.read(b".SUFFIXES: .x .y\n.x.y: extra\n\t@true\n", b"input.mk")
                .unwrap();
            db.finalize_implicit_rules(false, &mut ExpansionContext::pure())
                .unwrap();
            let converted = db
                .implicit_rules
                .iter()
                .any(|(pattern, rule)| pattern.literal_bytes() == b"%.y" && rule.recipes.is_some());
            assert_eq!(converted, !posix);
            assert_eq!(
                db.rules.get(b".x.y".as_slice()).unwrap().dependencies[0].name,
                b"extra"
            );
        }
    }
}

#[cfg(test)]
mod reader_event_tests {
    use super::*;
    use crate::diagnostic::{IncludedFile, OutputReceiver, ReaderEvent};
    struct Events(Vec<Vec<u8>>);
    impl OutputReceiver for Events {
        fn emit(&mut self, event: Output<'_>) {
            if let Output::Info(text) = event {
                self.0.push(text.to_vec());
            }
        }
        fn reader_event(&mut self, event: ReaderEvent<'_>) {
            self.0.push(match event {
                ReaderEvent::Reading { name, optional, .. } => [
                    if optional {
                        b"optional:".as_slice()
                    } else {
                        b"include:"
                    },
                    name,
                ]
                .concat(),
                ReaderEvent::Bom(name) => [b"bom:".as_slice(), name].concat(),
            });
        }
    }
    #[test]
    fn nested_reader_events_precede_effects_without_replaying_include_or_bom() {
        let mut database = Database::default();
        let mut events = Events(Vec::new());
        let mut reads = 0;
        database
            .read_with(
                b"-include absent.mk\ninclude child.mk\n$(info parent)\n",
                b"parent.mk",
                ExpansionContext::with_receiver(None, &mut events).with_includes(&mut |name| {
                    reads += 1;
                    if name == b"absent.mk" {
                        return Err(b"No such file or directory".to_vec().into());
                    }
                    Ok(IncludedFile {
                        name: name.to_vec(),
                        input: Input::Bytes(b"\xef\xbb\xbf$(info child)\n".to_vec()),
                    })
                }),
            )
            .unwrap();
        assert_eq!(reads, 2);
        assert_eq!(
            events.0,
            [
                b"optional:absent.mk".to_vec(),
                b"include:child.mk".to_vec(),
                b"bom:child.mk".to_vec(),
                b"child".to_vec(),
                b"parent".to_vec()
            ]
        );
    }
}
