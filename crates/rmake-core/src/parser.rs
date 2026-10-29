// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Incremental reading and immediate evaluation of the initial language subset.
//!
//! This module has no filesystem or process access. Recipes remain unexpanded
//! until the executor selects a target. Unsupported rule families fail explicitly.

pub use crate::diagnostic::Location;
use crate::diagnostic::{Diagnostic, ExpansionContext, FilesystemEffect, FilesystemError, Output};
use crate::reader::{Reader, strip_comment};
use crate::variables::{
    Assignment, ExpandError, MAX_OUTPUT, Origin, Pattern, PatternVariable, TargetScope,
    VariableStore, reference_end,
};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

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

#[derive(Clone, Debug)]
pub struct SecondaryPrerequisite {
    pub index: usize,
    /// A static declaration's stem can differ from the target's final stem.
    pub stem: Option<Vec<u8>>,
}

impl Rule {
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
    pub read_error: Option<(Location, Vec<u8>)>,
}

#[derive(Clone, Default)]
pub struct Database {
    pub file_records: crate::file_records::FileRecords,
    pub variables: VariableStore,
    /// Target-only variable declarations never create rules or select a goal.
    pub target_scopes: HashMap<Vec<u8>, TargetScope>,
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
    pub(crate) implicit_rules: Vec<(Pattern, Rule)>,
    vpaths: Vec<Vpath>,
    include_depth: usize,
    next_group_id: usize,
    evaluation_depth: usize,
    pub(crate) evaluation_scope: Option<VariableStore>,
    rules_frozen: bool,
    suppress_default_goal: bool,
    second_expansion: bool,
    // One warning per category across includes/eval: prerequisites=1, commands=2.
    wait_warnings: u8,
    pub(crate) evaluation_bindings: Vec<crate::variables::LocalFrame>,
    // Active only while parsing a potentially recursive HOME filename. Owned
    // bytes preserve shared-name overwrites without aliased or dangling pointers.
    filename_scratch: Option<Vec<u8>>,
    filename_session_depth: usize,
    runtime_metadata_live: bool,
    /// Global bindings being expanded for an environment vector. Target-local
    /// bindings do not refer to the global object removed by undefine.
    pub(crate) exporting: Vec<Vec<u8>>,
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
        let mut matching = self
            .pattern_assignments
            .iter()
            .filter(|assignment| {
                assignment
                    .pattern
                    .matches(target)
                    .is_some_and(|stem| !stem.is_empty())
            })
            .collect::<Vec<_>>();
        // More fixed bytes mean a shorter stem, hence higher precedence.
        // Stable sorting preserves declaration order when stem lengths tie.
        matching.sort_by_key(|assignment| assignment.pattern.fixed_len());
        // Most targets have no matching pattern bindings. Do not allocate an
        // empty scope for them; matching declarations still run in full order.
        let mut pattern_scope = None;
        for assignment in matching {
            if pattern_scope
                .get_or_insert_with(TargetScope::default)
                .apply_pattern_with(
                    &mut self.variables,
                    &assignment.variable,
                    context.reborrow(),
                )?
            {
                // A suppressed conditional can return the global binding. Its
                // modifiers then replace global export policy too.
                let name = assignment.variable.name();
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
        parent.refresh_globals(&self.variables);
        match (pattern, self.target_scopes.get(target)) {
            (Some(pattern), Some(scope)) => parent.with_target_scopes(&[pattern, scope]),
            (Some(pattern), None) => parent.with_target_scopes(&[pattern]),
            (None, Some(scope)) => parent.with_target_scopes(&[scope]),
            (None, None) => parent.with_target_scopes(&[]),
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

    pub fn vpath_group_count(&self) -> usize {
        self.vpaths.len()
    }

    /// Produce candidates in stem-length and declaration order, without I/O.
    /// The runtime checks prerequisites and commits only a viable candidate.
    pub fn implicit_candidates(&self, target: &[u8]) -> Vec<ImplicitCandidate> {
        self.implicit_candidates_excluding(target, &[])
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
            target
                .iter()
                .rposition(|byte| *byte == b'/')
                .map_or(b"".as_slice(), |slash| &target[..=slash])
        };
        for (index, (pattern, rule)) in self.implicit_rules.iter().enumerate() {
            let identity = rule
                .group
                .as_ref()
                .and_then(|group| group.implicit_identity)
                .unwrap_or(index);
            if active.contains(&identity) {
                continue;
            }
            let name = pattern.literal_bytes();
            let stripped = !name.contains(&b'/');
            let word = if stripped {
                &target[directory.len()..]
            } else {
                target
            };
            let Some(stem) = pattern.matches(word).filter(|stem| !stem.is_empty()) else {
                continue;
            };
            // Nonmatching patterns cannot affect replacement/cancellation or
            // match-anything suppression. Avoid scanning later definitions for
            // every irrelevant pattern on every target.
            if self.implicit_rules[index + 1..]
                .iter()
                .any(|(later_pattern, later_rule)| {
                    // GNU 4.4.1 replacement treats the old declaration as
                    // matching only when all its output patterns are the same
                    // as this one. Distinct-output declarations consequently
                    // keep their original priority, even after a repeated group.
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
            {
                continue;
            }
            specific |= name != b"%" && (rule.recipes.is_some() || rule.dependencies.is_empty());
            if rule.recipes.is_none() {
                continue;
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
            for (index, dependency) in candidate.dependencies.iter_mut().enumerate() {
                if candidate
                    .secondary
                    .as_ref()
                    .is_some_and(|clauses| clauses.iter().any(|clause| clause.index == index))
                {
                    continue;
                }
                let pattern = Pattern::new(&dependency.name);
                if pattern.has_wildcard() {
                    stem_dependencies.push(index);
                }
                let Ok(name) = pattern.replace(stem) else {
                    valid = false;
                    break;
                };
                let name = crate::filenames::without_dot_prefix(&name);
                dependency.name = if stripped && pattern.has_wildcard() {
                    [directory, name].concat()
                } else {
                    name.to_vec()
                };
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
        self.file_records.insert(b".SUFFIXES");
        let suffixes = if builtin_rules { DEFAULT_SUFFIXES } else { b"" };
        for name in words(suffixes) {
            self.file_records.insert(name);
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
                next_argument: 1,
            })
        };
        let mut read = |name: &[u8]| {
            assert_eq!(name, b"probe.mk");
            assert!(!published.get());
            Ok(crate::diagnostic::IncludedFile {
                name: name.to_vec(),
                bytes: b"SEEN := old-search\n".to_vec(),
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
                next_argument: 1,
            })
        };
        let mut read = |name: &[u8]| {
            assert_eq!(&*search.borrow(), b"inner");
            Ok(crate::diagnostic::IncludedFile {
                name: name.to_vec(),
                bytes: b"SEEN := inner\n".to_vec(),
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
                publish_includes: None,
                assignments: Vec::new(),
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
                publish_includes: None,
                assignments: Vec::new(),
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
                bytes: b"V=child\n-include absent.mk\n".to_vec(),
            }),
            _ => Err(b"No such file or directory".to_vec()),
        };
        db.read_with(
            b"include child.mk\n$(info $(V):$(MAKEFILE_LIST))\nall:;\n",
            b"Makefile",
            ExpansionContext::new(None, &mut emit).with_includes(&mut read),
        )
        .unwrap();
        assert_eq!(output, [b"child:child.mk".to_vec()]);
        assert_eq!(db.makefiles.len(), 2);
        assert!(db.makefiles[1].optional && db.makefiles[1].missing);
        assert!(db.rules.contains_key(b"all".as_slice()));
    }

    #[test]
    fn absent_required_include_is_retained_for_remake_and_has_no_early_warning() {
        let mut db = Database::default();
        let mut events = 0;
        let mut emit = |_: Output<'_>| events += 1;
        let mut read = |_: &[u8]| Err(b"No such file or directory".to_vec());
        db.read_with(
            b"include generated.mk\ngenerated.mk:;\n",
            b"Makefile",
            ExpansionContext::new(None, &mut emit).with_includes(&mut read),
        )
        .unwrap();
        assert_eq!(events, 0);
        let file = &db.makefiles[0];
        assert!(file.missing && !file.optional);
        assert_eq!(file.read_error.as_ref().unwrap().0.line, 1);
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
                let message = error.diagnostic().as_bytes().to_vec();
                match error.diagnostic_location(None) {
                    Some(location) => ExpandError::Located {
                        location: std::sync::Arc::new(location.clone()),
                        error: Box::new(ExpandError::FileError(message)),
                    },
                    None => ExpandError::UnlocatedError(message),
                }
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
        let scan = bytes
            .iter()
            .any(|byte| matches!(byte, b'(' | b'*' | b'?' | b'[' | b'~'));
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
                for (mut name, emit, offset) in crate::filenames::sequence_events(bytes) {
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
                        let effect = if crate::filenames::archive_parts(&name).is_some() {
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

/// References that survive immediate expansion are literal prerequisite bytes
/// at the static-pattern separator scan. Backslash parity still quotes colons.
fn expanded_colon(bytes: &[u8]) -> Option<usize> {
    let mut escaped = false;
    for (index, byte) in bytes.iter().copied().enumerate() {
        if escaped {
            escaped = false;
        } else if byte == b'\\' {
            escaped = true;
        } else if byte == b':' {
            return Some(index);
        }
    }
    None
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
    colon: Option<usize>,
}

/// Modifiers are keywords only when the complete remaining text is not itself
/// an assignment: `target: private = value` names the variable `private`.
fn target_assignment(mut bytes: &[u8]) -> Option<TargetAssignment<'_>> {
    bytes = assignment_start(bytes);
    let mut origin = Origin::File;
    let mut private = false;
    let mut export = None;
    loop {
        if let Some(spec) = assignment(bytes) {
            if separator(spec.name, b';').is_some() {
                return None;
            }
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
    context.emit(Output::Warning {
        location: Some(&location),
        message: message.as_bytes(),
    });
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

    /// Expand only the target portion. A colon generated by a reference fixes
    /// the assignment/prerequisite boundary before the untouched tail runs.
    /// Keep the consumed raw offset so ordinary rules can resume expansion
    /// without emitting any prefix effects a second time.
    fn expand_rule_prefix(
        &mut self,
        bytes: &[u8],
        location: &Location,
        context: &mut ExpansionContext<'_>,
    ) -> Result<RulePrefix, ParseError> {
        let limit = separator(bytes, b';').unwrap_or(bytes.len());
        let mut expanded = Vec::new();
        let mut consumed = 0;
        while consumed < limit {
            let start = consumed;
            let part = if bytes[start] == b'$' {
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
                return Err(ParseError {
                    unlocated: false,
                    location: location.clone(),
                    message: ExpandError::ExpansionLimit.diagnostic(),
                });
            }
            let has_colon = part.contains(&b':');
            let has_semicolon = part.contains(&b';');
            expanded.extend_from_slice(&part);
            if has_colon || has_semicolon {
                let colon = separator(&expanded, b':');
                let semicolon = separator(&expanded, b';');
                if semicolon.is_some_and(|semicolon| colon.is_none_or(|colon| semicolon < colon)) {
                    return Ok(RulePrefix {
                        expanded,
                        consumed,
                        colon: None,
                    });
                }
                if colon.is_some() {
                    return Ok(RulePrefix {
                        expanded,
                        consumed,
                        colon,
                    });
                }
            }
        }
        Ok(RulePrefix {
            expanded,
            consumed,
            colon: None,
        })
    }

    fn define(
        &mut self,
        spec: AssignmentSpec<'_>,
        origin: Origin,
        location: &Location,
        evaluation_location: &Location,
        context: &mut ExpansionContext<'_>,
    ) -> Result<Vec<u8>, ParseError> {
        let name = self.expand(spec.name, location, context)?;
        if name.is_empty() {
            return Err(ParseError {
                unlocated: false,
                location: location.clone(),
                message: "empty variable name".into(),
            });
        }
        self.define_expanded(
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
        Ok(name)
    }

    /// Store a definition after its name expansion, without evaluating it again.
    fn define_expanded(
        &mut self,
        spec: AssignmentSpec<'_>,
        origin: Origin,
        location: &Location,
        evaluation_location: &Location,
        context: &mut ExpansionContext<'_>,
    ) -> Result<(), ParseError> {
        self.define_variable_with(
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
        mut context: ExpansionContext<'_>,
    ) -> Result<(), ExpandError> {
        if spec.name == b"MAKEFLAGS"
            && spec.kind == Assignment::Append
            && context.has_makeflags()
            && !spec.value.is_empty()
        {
            self.variables.strip_makeflags_assignments(origin);
        }
        // Literal RHS bytes cannot invoke eval or read scoped bindings. Keep
        // their assignment in the store, but still run special setters below.
        // Shell definitions retain the full database for environment effects.
        let result = if spec.kind != Assignment::Shell && !spec.value.contains(&b'$') {
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
                |variables, bytes, shell, context| {
                    std::mem::swap(variables, &mut self.variables);
                    let result = if shell {
                        crate::variables::shell_database(self, bytes, context)
                    } else {
                        self.expand_with(bytes, context)
                    };
                    std::mem::swap(variables, &mut self.variables);
                    result
                },
            );
            self.variables = variables;
            result
        };
        result?;
        if spec.name == b"MAKEFLAGS" && context.has_makeflags() {
            self.reset_assigned_flags(origin, context.reborrow())?;
        }
        Ok(())
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

    pub(crate) fn update_mflags(
        &mut self,
        flags: &[u8],
        environment_override: bool,
    ) -> Result<(), ExpandError> {
        if let Some(scope) = &mut self.evaluation_scope {
            scope.update_mflags(flags, environment_override)
        } else {
            self.variables.update_mflags(flags, environment_override)
        }
    }

    fn reset_assigned_flags(
        &mut self,
        origin: Origin,
        mut context: ExpansionContext<'_>,
    ) -> Result<(), ExpandError> {
        let encoded_overrides = self
            .variables
            .get(b"MAKEFLAGS")
            .is_some_and(|variable| Self::has_command_overrides_reference(variable.raw_value()));
        let value = self.expand_with(b"$(MAKEFLAGS)", context.reborrow())?;
        context.update_makeflags(self, origin, value, encoded_overrides)
    }

    fn define_target_assignments(
        &mut self,
        targets: &[u8],
        assignment: TargetAssignment<'_>,
        location: &Location,
        context: &mut ExpansionContext<'_>,
    ) -> Result<(), ParseError> {
        let targets = crate::filenames::target_colons(targets);
        let targets = self
            .parsed_rule_filenames(&targets, location, context)?
            .collect::<Vec<_>>();
        if targets.last().is_some_and(|target| target.ends_with(b"&")) {
            return Err(ParseError {
                unlocated: false,
                location: location.clone(),
                message: "rmake: unsupported rule form".into(),
            });
        }
        for target in targets {
            let target = target.as_ref();
            let pattern = Pattern::new(target);
            if pattern.has_wildcard() {
                let name = self.expand(assignment.spec.name, location, context)?;
                let variable = PatternVariable::prepare_with(
                    &self.variables,
                    &name,
                    assignment.spec.value,
                    assignment.spec.kind,
                    assignment.origin,
                    assignment.private,
                    assignment.export,
                    Some(location),
                    context.at(Some(location)),
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
            self.file_records.insert(target);
            let scope = self.target_scopes.entry(target.to_vec()).or_default();
            let parse_variables = scope.context(&self.variables);
            let name = parse_variables
                .expand_with(assignment.spec.name, context.at(Some(location)))
                .map_err(|error| ParseError {
                    unlocated: error.is_unlocated(),
                    location: error
                        .diagnostic_location(Some(location))
                        .unwrap_or(location)
                        .clone(),
                    message: error.diagnostic(),
                })?;
            if name.is_empty() {
                return Err(ParseError {
                    unlocated: false,
                    location: location.clone(),
                    message: "empty variable name".into(),
                });
            }
            scope
                .define_with(
                    &mut self.variables,
                    &name,
                    assignment.spec.value,
                    assignment.spec.kind,
                    assignment.origin,
                    assignment.private,
                    assignment.export,
                    Some(location),
                    context.at(Some(location)),
                )
                .map_err(|error| ParseError {
                    unlocated: error.is_unlocated(),
                    location: error
                        .diagnostic_location(Some(location))
                        .unwrap_or(location)
                        .clone(),
                    message: error.diagnostic(),
                })?;
        }
        Ok(())
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
                    return Err(ParseError {
                        unlocated: false,
                        location: location.clone(),
                        message: "invalid syntax in conditional".into(),
                    });
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
                let invalid = || ParseError {
                    unlocated: false,
                    location: location.clone(),
                    message: "invalid syntax in conditional".into(),
                };
                let (left, right, extra) = if args.starts_with(b"(") {
                    // The outer parentheses delimit the directive; literal
                    // parentheses within either argument also nest. Do not
                    // expand first: commas created by expansion are data.
                    let mut depth = 1usize;
                    let mut comma = None;
                    for (index, byte) in args.iter().copied().enumerate().skip(1) {
                        match byte {
                            b'(' => depth += 1,
                            b')' => {
                                depth -= 1;
                                if depth == 0 {
                                    break;
                                }
                            }
                            b',' if depth == 1 => {
                                comma = Some(index);
                                break;
                            }
                            _ => {}
                        }
                    }
                    let comma = comma.ok_or_else(invalid)?;
                    // GNU evaluates the complete left operand before it checks
                    // that the right operand has a closing delimiter.
                    let left = &args[1..comma];
                    let left_end = left
                        .iter()
                        .rposition(|byte| !matches!(byte, b' ' | b'\t'))
                        .map_or(0, |i| i + 1);
                    let left = self.expand(&left[..left_end], location, context)?;
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
                    let closing = closing.ok_or_else(invalid)?;
                    (
                        left,
                        assignment_start(&args[comma + 1..closing]),
                        &args[closing + 1..],
                    )
                } else {
                    let quote = *args.first().ok_or_else(invalid)?;
                    if quote != b'\'' && quote != b'"' {
                        return Err(invalid());
                    }
                    let end = args[1..]
                        .iter()
                        .position(|b| *b == quote)
                        .ok_or_else(invalid)?
                        + 1;
                    let left = self.expand(&args[1..end], location, context)?;
                    let rest = assignment_start(&args[end + 1..]);
                    let quote2 = *rest.first().ok_or_else(invalid)?;
                    if quote2 != b'\'' && quote2 != b'"' {
                        return Err(invalid());
                    }
                    let end2 = rest[1..]
                        .iter()
                        .position(|b| *b == quote2)
                        .ok_or_else(invalid)?
                        + 1;
                    (left, &rest[1..end2], &rest[end2 + 1..])
                };
                if !assignment_start(extra).is_empty() {
                    record_warning(
                        &mut self.warnings,
                        context,
                        location.clone(),
                        format!(
                            "extraneous text after '{}' directive",
                            String::from_utf8_lossy(keyword)
                        )
                        .into(),
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
        self.read_at(bytes, file, None, context)
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
                self.file_records.insert(&dependency.name);
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
            .get(b".EXTRA_PREREQS")
            .map(|variable| variable.raw_value().to_vec());
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
            .get(b".EXTRA_PREREQS")
            .map(|variable| variable.raw_value().to_vec());
        let global = self.extra_prerequisites(raw.as_deref(), context)?;
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
        }
        self.rules_frozen = true;
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
            scope.refresh_globals(&self.variables);
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
        variables.refresh_globals(&self.variables);
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
        variables.refresh_globals(&self.variables);
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
        bindings: Vec<crate::variables::LocalFrame>,
        context: ExpansionContext<'_>,
    ) -> Result<(), ParseError> {
        let previous_scope =
            std::mem::replace(&mut self.evaluation_scope, scope.as_deref().cloned());
        let previous_bindings = std::mem::replace(&mut self.evaluation_bindings, bindings);
        let result = self.read_evaluated(bytes, location, context);
        if let Some(scope) = scope {
            *scope = self.evaluation_scope.take().expect("evaluation scope");
        }
        self.evaluation_scope = previous_scope;
        self.evaluation_bindings = previous_bindings;
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
        let (file, offset) = location.map_or((b"".as_slice(), 0), |location| {
            (location.file.as_ref(), location.line.saturating_sub(1))
        });
        if self.evaluation_depth >= 128 {
            return Err(ParseError {
                location: Location {
                    file: file.into(),
                    line: offset + 1,
                },
                unlocated: location.is_none(),
                message: "rmake: evaluation nesting limit exceeded".into(),
            });
        }
        self.evaluation_depth += 1;
        // GNU eval buffers select defaults even inside a MAKEFILES preload.
        let suppress_default = std::mem::replace(&mut self.suppress_default_goal, false);
        let result = self
            .read_at(
                &bytes[..bytes
                    .iter()
                    .position(|byte| *byte == 0)
                    .unwrap_or(bytes.len())],
                file,
                Some(offset + 1),
                context,
            )
            .map_err(|mut error| {
                if location.is_none() && error.location.file.is_empty() {
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
        bytes: &[u8],
        file: &[u8],
        evaluated_at: Option<usize>,
        mut context: ExpansionContext<'_>,
    ) -> Result<(), ParseError> {
        let context = &mut context;
        let source_name: std::sync::Arc<[u8]> = file.into();
        let mut reader = Reader::new(bytes);
        // Keep declaration boundaries until its first recipe is seen: GNU
        // places that declaration's prerequisites before earlier declarations.
        let mut declaration = RuleDeclaration::default();
        let mut conditions = Vec::<Conditional>::new();
        let mut last_line = 0;
        loop {
            if context.is_cancelled() {
                return Err(ParseError {
                    location: Location {
                        file: source_name.clone(),
                        line: last_line.max(1),
                    },
                    message: Diagnostic::from("interrupted"),
                    unlocated: true,
                });
            }
            let prefix = self
                .variables
                .get(b".RECIPEPREFIX")
                .and_then(|v| v.raw_value().first().copied())
                .unwrap_or(b'\t');
            reader.set_posix(self.posix);
            if evaluated_at == Some(1) {
                reader.skip_eval_bom();
            }
            let Some(mut line) = reader.next_line(prefix, !declaration.targets.is_empty()) else {
                break;
            };
            // GNU strips one UTF-8 BOM from the first observed logical line,
            // after file-stream reads have already counted its bytes.
            if evaluated_at.is_none() && line.line == 1 && line.bytes.starts_with(b"\xef\xbb\xbf") {
                line.bytes.drain(..3);
            }
            last_line = line.end_line;
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
                if active {
                    self.add_recipe(
                        &mut declaration,
                        Recipe {
                            bytes: line.bytes[1..].to_vec(),
                            location,
                            prefix,
                        },
                    );
                }
                continue;
            }
            // Assignment values may contain semicolons. Rule inline recipes
            // retain shell comments, so comment stripping happens separately.
            let normal = strip_comment(&line.bytes);
            let text = assignment_start(&normal);
            if text.is_empty() {
                continue;
            }
            // Assignment syntax wins over directive keywords. In particular,
            // `ifdef = value` must not open a conditional, even in an ignored
            // branch. Recognition itself has no expansion effects.
            if let Some(spec) = assignment(text) {
                if active {
                    self.finish_declaration(&mut declaration, &location, context)?;
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
                        "extraneous text after 'else' directive".into(),
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
                        "extraneous text after 'endif' directive".into(),
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
                    self.finish_declaration(&mut declaration, &location, context)?;
                    let spec = assignment(header);
                    if spec.as_ref().is_some_and(|spec| !spec.value.is_empty()) {
                        record_warning(
                            &mut self.warnings,
                            context,
                            location.clone(),
                            "extraneous text after 'define' directive".into(),
                        );
                    }
                    let (raw_name, kind) = spec.map_or((header, Assignment::Recursive), |spec| {
                        (spec.name, spec.kind)
                    });
                    let mut name = self.expand(raw_name, &location, context)?;
                    let start = name
                        .iter()
                        .position(|byte| !is_c_space(*byte))
                        .unwrap_or(name.len());
                    name.drain(..start);
                    while name.last().is_some_and(|byte| matches!(byte, b' ' | b'\t')) {
                        name.pop();
                    }
                    if name.is_empty() {
                        return Err(ParseError {
                            unlocated: false,
                            location,
                            message: "empty variable name".into(),
                        });
                    }
                    Some((name, kind))
                } else {
                    None
                };
                let mut body = Vec::new();
                let mut depth = 1;
                let mut warning_line = location.line;
                loop {
                    let Some(part) = reader.next_line(prefix, false) else {
                        if !active {
                            // Ignored definitions are skipped by the outer
                            // reader, whose EOF error is the open conditional.
                            break;
                        }
                        return Err(ParseError {
                            unlocated: false,
                            location,
                            message: "missing 'endef', unterminated 'define'".into(),
                        });
                    };
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
                    last_line = part.end_line;
                    if !active {
                        // Ignored definitions have no nesting counter: the
                        // first standalone endef resumes conditional reading.
                        let normal = strip_comment(&part.bytes);
                        if (part.bytes.first() != Some(&prefix) || declaration.targets.is_empty())
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
                                    "extraneous text after 'endef' directive".into(),
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
                    self.define_expanded(
                        AssignmentSpec {
                            name: &name,
                            value: &body,
                            kind,
                        },
                        origin,
                        &location,
                        &Location {
                            file: source_name.clone(),
                            line: last_line,
                        },
                        context,
                    )?;
                    if private {
                        self.variables.set_private(&name, true);
                    }
                    if let Some(export) = export {
                        self.set_export(name, export);
                    }
                }
                continue;
            }
            if !active {
                continue;
            }
            if let Some(spec) = variable_assignment {
                self.finish_declaration(&mut declaration, &location, context)?;
                let name = self.define(spec, origin, &location, &location, context)?;
                if private {
                    self.variables.set_private(&name, true);
                }
                if let Some(export) = export {
                    self.set_export(name, export);
                }
                continue;
            }
            if let Some(rest) = directive(variable_text, b"undefine") {
                self.finish_declaration(&mut declaration, &location, context)?;
                let name = self.expand(rest, &location, context)?;
                let name = trim(&name);
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
                self.finish_declaration(&mut declaration, &location, context)?;
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
                    // A name-list declaration creates missing empty simple
                    // bindings. Existing bindings retain all their metadata.
                    // These names were already expanded with the whole list.
                    if self.variables.get(name).is_none() {
                        self.variables
                            .define_at(name, b"", Assignment::Simple, Origin::File, &location)
                            .map_err(|error| ParseError {
                                unlocated: error.is_unlocated(),
                                location: location.clone(),
                                message: error.diagnostic(),
                            })?;
                    }
                    self.set_export(name.to_vec(), export);
                }
                continue;
            }
            if [b"include".as_slice(), b"-include", b"sinclude"].contains(&keyword) {
                self.finish_declaration(&mut declaration, &location, context)?;
                let names = self.expand(directive(text, keyword).unwrap(), &location, context)?;
                let optional = keyword != b"include";
                for mut name in crate::filenames::sequence(&names) {
                    name = crate::filenames::without_dot_prefix(&name).to_vec();
                    if self.filename_scratch.is_some() {
                        self.write_filename(&name, 0);
                    }
                    if name.starts_with(b"~") {
                        name = self
                            .rule_tilde(&name, &mut context.at(Some(&location)))
                            .map_err(|error| ParseError {
                                unlocated: error.is_unlocated(),
                                location: error
                                    .diagnostic_location(Some(&location))
                                    .unwrap_or(&location)
                                    .clone(),
                                message: error.diagnostic(),
                            })?
                            .into_owned();
                    }
                    let matches = if name.iter().any(|byte| matches!(byte, b'*' | b'?' | b'[')) {
                        context
                            .perform_filesystem(FilesystemEffect::Wildcard(&name))
                            .and_then(Result::ok)
                            .unwrap_or_default()
                    } else {
                        Vec::new()
                    };
                    let paths = if matches.is_empty() {
                        vec![name.to_vec()]
                    } else {
                        words(&matches).map(Vec::from).collect()
                    };
                    for path in paths {
                        self.file_records.insert(&path);
                        let result = context.read_include(&path).ok_or_else(|| ParseError {
                            unlocated: false,
                            location: location.clone(),
                            message: "rmake: include requires a makefile reader".into(),
                        })?;
                        match result {
                            Ok(included) => {
                                if self.include_depth >= 128 {
                                    return Err(ParseError {
                                        unlocated: false,
                                        location: location.clone(),
                                        message: "rmake: makefile inclusion nesting limit exceeded"
                                            .into(),
                                    });
                                }
                                self.makefiles.push(Makefile {
                                    name: included.name.clone(),
                                    optional,
                                    missing: false,
                                    read_error: None,
                                });
                                self.variables
                                    .append_literal_at(
                                        b"MAKEFILE_LIST",
                                        &included.name,
                                        Origin::File,
                                        &location,
                                    )
                                    .map_err(|error| ParseError {
                                        unlocated: false,
                                        location: location.clone(),
                                        message: error.diagnostic(),
                                    })?;
                                self.include_depth += 1;
                                let result =
                                    self.read_with(&included.bytes, &path, context.reborrow());
                                self.include_depth -= 1;
                                result?;
                            }
                            Err(error) => {
                                if error == b"Is a directory" {
                                    return Err(ParseError {
                                        unlocated: true,
                                        location: location.clone(),
                                        message: [path.as_slice(), b": ", &error].concat().into(),
                                    });
                                }
                                self.makefiles.push(Makefile {
                                    name: path,
                                    optional,
                                    missing: true,
                                    read_error: Some((location.clone(), error)),
                                });
                            }
                        }
                    }
                }
                continue;
            }
            if keyword == b"vpath" {
                self.finish_declaration(&mut declaration, &location, context)?;
                let value = self.expand(directive(text, b"vpath").unwrap(), &location, context)?;
                let mut names = words(&value);
                if let Some(pattern) = names.next() {
                    let directories: Vec<_> = names
                        .flat_map(|name| name.split(|byte| *byte == b':'))
                        .filter(|path| !path.is_empty())
                        .map(Vec::from)
                        .collect();
                    if directories.is_empty() {
                        self.vpaths.retain(|path| path.key != pattern);
                    } else {
                        self.vpaths.push(Vpath {
                            key: pattern.to_vec(),
                            pattern: Pattern::new(pattern),
                            directories,
                        });
                    }
                } else {
                    self.vpaths.clear();
                }
                continue;
            }
            if keyword == b"load" {
                self.finish_declaration(&mut declaration, &location, context)?;
                return Err(ParseError {
                    unlocated: false,
                    location,
                    message: format!(
                        "rmake: unsupported directive '{}'",
                        String::from_utf8_lossy(keyword)
                    )
                    .into(),
                });
            }
            self.finish_declaration(&mut declaration, &location, context)?;
            if line.bytes.first() == Some(&prefix) && prefix == b'\t' {
                return Err(ParseError {
                    unlocated: false,
                    location,
                    message: "recipe commences before first target".into(),
                });
            }
            // Preserve raw assignment text after either a literal colon or one
            // generated by expanding the target prefix. Semicolons and colons
            // in that RHS remain value bytes, and recursive effects stay lazy.
            let prefix_expansion = self.expand_rule_prefix(text, &location, context)?;
            if let Some(colon) = prefix_expansion.colon {
                let mut tail = prefix_expansion.expanded[colon + 1..].to_vec();
                tail.extend_from_slice(&text[prefix_expansion.consumed..]);
                let tail = tail.strip_prefix(b":").unwrap_or(&tail);
                if let Some(assignment) = target_assignment(tail) {
                    self.define_target_assignments(
                        prefix_expansion.expanded[..colon]
                            .strip_suffix(b"&")
                            .unwrap_or(&prefix_expansion.expanded[..colon]),
                        assignment,
                        &location,
                        context,
                    )?;
                    continue;
                }
            }
            // GNU observes target globs before expanding the prerequisite
            // tail. A shell/file effect in that tail must not change which
            // targets this declaration names. Ordinary words remain borrowed.
            let observed_targets = if let Some(colon) = prefix_expansion.colon
                && prefix_expansion.expanded[..colon]
                    .iter()
                    .any(|byte| matches!(byte, b'(' | b'*' | b'?' | b'[' | b'~'))
            {
                let targets = prefix_expansion.expanded[..colon]
                    .strip_suffix(b"&")
                    .unwrap_or(&prefix_expansion.expanded[..colon]);
                let targets = crate::filenames::target_colons(targets);
                Some(
                    self.parsed_rule_filenames(&targets, &location, context)?
                        .map(Cow::into_owned)
                        .collect::<Vec<_>>(),
                )
            } else {
                None
            };
            let inline = separator(&line.bytes, b';')
                .filter(|i| separator(&line.bytes[..*i], b'#').is_none());
            let rule_end = separator(text, b';').unwrap_or(text.len());
            let tail = self.expand(
                &text[prefix_expansion.consumed..rule_end],
                &location,
                context,
            )?;
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
            let expanded_inline = if inline.is_none() {
                separator(&expanded_line, b';')
            } else {
                None
            };
            let expanded =
                expanded_inline.map_or(expanded_line.as_slice(), |i| &expanded_line[..i]);
            if trim(expanded).is_empty() {
                continue;
            }
            let Some(colon) = separator(expanded, b':') else {
                let message = if line.bytes.starts_with(b"        ") {
                    "missing separator (did you mean TAB instead of 8 spaces?)"
                } else {
                    "missing separator"
                };
                return Err(ParseError {
                    unlocated: false,
                    location,
                    message: message.into(),
                });
            };
            let double_colon = expanded.get(colon + 1) == Some(&b':');
            let grouped = expanded[..colon].ends_with(b"&");
            declaration.location = Some(location.clone());
            declaration.grouped = grouped;
            let target_end = colon - usize::from(grouped);
            let early_targets = observed_targets.is_some();
            let target_text = crate::filenames::target_colons(&expanded[..target_end]);
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
                continue;
            }
            let mut prereqs = &expanded[colon + 1 + usize::from(double_colon)..];
            let static_pattern = if let Some(second) = expanded_colon(prereqs) {
                let pattern_text = crate::filenames::target_colons(&prereqs[..second]);
                let patterns: Vec<_> = crate::filenames::words(&pattern_text).collect();
                if patterns.len() != 1 || !Pattern::new(&patterns[0]).has_wildcard() {
                    return Err(ParseError {
                        unlocated: false,
                        location,
                        message: if patterns.is_empty() {
                            "missing target pattern"
                        } else if patterns.len() > 1 {
                            "multiple target patterns"
                        } else {
                            "target pattern contains no '%'"
                        }
                        .into(),
                    });
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
                double_colon,
            });
            let recipe = inline
                .map(|i| line.bytes[i + 1..].to_vec())
                .or_else(|| expanded_inline.map(|i| expanded_line[i + 1..].to_vec()));
            if let Some(bytes) = recipe {
                self.add_recipe(
                    &mut declaration,
                    Recipe {
                        bytes,
                        location,
                        prefix,
                    },
                );
            }
        }
        if !conditions.is_empty() {
            return Err(ParseError {
                unlocated: false,
                location: Location {
                    file: source_name.clone(),
                    line: evaluated_at.unwrap_or(last_line + 1),
                },
                message: "missing 'endif'".into(),
            });
        }
        self.finish_declaration(
            &mut declaration,
            &Location {
                file: source_name.clone(),
                line: evaluated_at.unwrap_or(last_line + 1),
            },
            context,
        )?;
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
                        rule.dependencies.push(Dependency {
                            name,
                            order_only,
                            hidden: false,
                        });
                    }
                }
            }
            if !implicit {
                if !secondary && !unmatched {
                    // Clearing .SUFFIXES can shorten the rule below its old length.
                    let start = record_start.min(rule.dependencies.len());
                    for dependency in rule.dependencies[start..]
                        .iter()
                        .filter(|dep| dep.name != b".WAIT")
                    {
                        self.file_records.insert(&dependency.name);
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

    fn set_export(&mut self, name: Vec<u8>, export: bool) {
        if self
            .evaluation_scope
            .as_ref()
            .is_some_and(|scope| scope.set_scoped_export_policy(&name, export))
        {
            return;
        }
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
                    [
                        b"warning: overriding recipe for target '".as_slice(),
                        target,
                        b"'",
                    ]
                    .concat()
                    .into(),
                ));
                warnings.push((
                    previous.clone(),
                    [
                        b"warning: ignoring old recipe for target '".as_slice(),
                        target,
                        b"'",
                    ]
                    .concat()
                    .into(),
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
    fn anonymous_eval_preserves_real_included_file_errors() {
        let mut db = Database::default();
        let mut include = |_: &[u8]| {
            Ok(crate::diagnostic::IncludedFile {
                name: b"child.mk".to_vec(),
                bytes: b"$(error included)\n".to_vec(),
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
