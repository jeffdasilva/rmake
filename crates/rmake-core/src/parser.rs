// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Incremental reading and immediate evaluation of the initial language subset.
//!
//! This module has no filesystem or process access. Recipes remain unexpanded
//! until the executor selects a target. Unsupported rule families fail explicitly.

pub use crate::diagnostic::Location;
use crate::diagnostic::{Diagnostic, ExpansionContext, Output};
use crate::reader::{Reader, strip_comment};
use crate::variables::{
    Assignment, ExpandError, MAX_OUTPUT, Origin, Pattern, PatternVariable, TargetScope,
    VariableStore, reference_end,
};
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
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dependency {
    pub name: Vec<u8>,
    pub order_only: bool,
}

impl Rule {
    pub fn normal_prerequisites(&self) -> impl Iterator<Item = &[u8]> {
        self.dependencies
            .iter()
            .filter(|dependency| !dependency.order_only)
            .map(|dependency| dependency.name.as_slice())
    }

    /// Automatic variables ^, ? and | use first-occurrence order, with any
    /// normal occurrence promoting the name from order-only to normal. The raw
    /// occurrences remain available for <, + and repeated circular diagnostics.
    pub fn canonical_dependencies(&self) -> Vec<Dependency> {
        let normal: HashSet<_> = self.normal_prerequisites().collect();
        let mut seen = HashSet::new();
        self.dependencies
            .iter()
            .filter(|dependency| seen.insert(dependency.name.as_slice()))
            .map(|dependency| Dependency {
                name: dependency.name.clone(),
                order_only: !normal.contains(dependency.name.as_slice()),
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

#[derive(Default)]
pub struct Database {
    pub variables: VariableStore,
    /// Target-only variable declarations never create rules or select a goal.
    pub target_scopes: HashMap<Vec<u8>, TargetScope>,
    pub pattern_assignments: Vec<PatternAssignment>,
    pub rules: HashMap<Vec<u8>, Rule>,
    pub exports: HashSet<Vec<u8>>,
    pub unexports: HashSet<Vec<u8>>,
    pub export_all: bool,
    pub warnings: Vec<(Location, Diagnostic)>,
}

impl Database {
    /// Activate a target's local bindings over its inherited execution context.
    /// Descendant visibility and append layers are preserved by the store.
    pub fn variables_for_target_with(
        &mut self,
        target: &[u8],
        parent: &VariableStore,
        mut context: ExpansionContext<'_>,
    ) -> Result<VariableStore, ExpandError> {
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
        let mut pattern_scope = TargetScope::default();
        for assignment in matching {
            if pattern_scope.apply_pattern_with(
                &mut self.variables,
                &assignment.variable,
                context.reborrow(),
            )? {
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
        let mut parent = parent.clone();
        parent.refresh_globals(&self.variables);
        Ok(if let Some(scope) = self.target_scopes.get(target) {
            parent.with_target_scopes(&[&pattern_scope, scope])
        } else {
            parent.with_target_scopes(&[&pattern_scope])
        })
    }

    /// Initialize suffix metadata before reading user Makefiles. SUFFIXES is an
    /// independent default variable, defined even when builtin rules are off.
    /// Existing higher-priority variables and explicit rules are preserved.
    pub fn initialize_suffixes(&mut self, builtin_rules: bool) -> Result<(), ExpandError> {
        let suffixes = if builtin_rules { DEFAULT_SUFFIXES } else { b"" };
        self.variables
            .define_literal(b"SUFFIXES", suffixes, Origin::Default)?;
        if builtin_rules {
            self.rules
                .entry(b".SUFFIXES".to_vec())
                .or_insert_with(|| Rule {
                    dependencies: words(suffixes)
                        .map(|name| Dependency {
                            name: name.to_vec(),
                            order_only: false,
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

fn trim_start(bytes: &[u8]) -> &[u8] {
    &bytes[bytes
        .iter()
        .position(|b| !b.is_ascii_whitespace())
        .unwrap_or(bytes.len())..]
}

pub fn words(bytes: &[u8]) -> impl Iterator<Item = &[u8]> {
    bytes
        .split(u8::is_ascii_whitespace)
        .filter(|word| !word.is_empty())
}

/// Percent quoting is shared with pattern matching. Other escaped filename
/// syntax remains unsupported by this reader rather than being reinterpreted.
fn only_percent_escapes(bytes: &[u8]) -> bool {
    let mut cursor = 0;
    while cursor < bytes.len() {
        if bytes[cursor] == b'\\' {
            while bytes.get(cursor) == Some(&b'\\') {
                cursor += 1;
            }
            if bytes.get(cursor) != Some(&b'%') {
                return false;
            }
        }
        cursor += 1;
    }
    true
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
// This deliberately does not alter directive or physical-line whitespace rules.
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
        .filter(|rest| rest.is_empty() || rest[0].is_ascii_whitespace())
        .map(trim_start)
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
        &self,
        bytes: &[u8],
        location: &Location,
        context: &mut ExpansionContext<'_>,
    ) -> Result<Vec<u8>, ParseError> {
        self.variables
            .expand_with(bytes, context.at(Some(location)))
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
        &self,
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
                        reference_end(bytes, start + 1).map_err(|error| ParseError {
                            unlocated: error.is_unlocated(),
                            location: location.clone(),
                            message: error.diagnostic(),
                        })? + 1
                    }
                    Some(_) => start + 2,
                    None => start + 1,
                };
                self.expand(&bytes[start..consumed], location, context)?
            } else {
                consumed += 1;
                while consumed < limit
                    && !matches!(bytes[consumed - 1], b':')
                    && bytes[consumed] != b'$'
                {
                    consumed += 1;
                }
                bytes[start..consumed].to_vec()
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
        self.variables
            .define_with(
                &name,
                spec.value,
                spec.kind,
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
            })?;
        Ok(name)
    }

    fn define_target_assignments(
        &mut self,
        targets: &[u8],
        assignment: TargetAssignment<'_>,
        location: &Location,
        context: &mut ExpansionContext<'_>,
    ) -> Result<(), ParseError> {
        let targets = words(targets).collect::<Vec<_>>();
        if targets.last().is_some_and(|target| target.ends_with(b"&")) {
            return Err(ParseError {
                unlocated: false,
                location: location.clone(),
                message: "rmake: unsupported rule form".into(),
            });
        }
        for target in targets {
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
            if !only_percent_escapes(target) {
                return Err(ParseError {
                    unlocated: false,
                    location: location.clone(),
                    message: "rmake: escaped target names are not implemented".into(),
                });
            }
            let target = pattern.literal_bytes();
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
                if words(&name).count() > 1 {
                    return Err(ParseError {
                        unlocated: false,
                        location: location.clone(),
                        message: "invalid syntax in conditional".into(),
                    });
                }
                return Ok(Some(
                    self.variables
                        .get(trim(&name))
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
                        .rposition(|b| !b.is_ascii_whitespace())
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
                        trim_start(&args[comma + 1..closing]),
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
                    let rest = trim_start(&args[end + 1..]);
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
                if !trim(extra).is_empty() {
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
        mut context: ExpansionContext<'_>,
    ) -> Result<(), ParseError> {
        let context = &mut context;
        let mut reader = Reader::new(bytes);
        let mut current = Vec::<Vec<u8>>::new();
        // Keep declaration boundaries until its first recipe is seen: GNU
        // places that declaration's prerequisites before earlier declarations.
        let mut current_offsets = HashMap::<Vec<u8>, usize>::new();
        let mut recipe_started = false;
        let mut conditions = Vec::<Conditional>::new();
        let mut last_line = 0;
        loop {
            let prefix = self
                .variables
                .get(b".RECIPEPREFIX")
                .and_then(|v| v.raw_value().first().copied())
                .unwrap_or(b'\t');
            let Some(line) = reader.next_line(prefix, !current.is_empty()) else {
                break;
            };
            last_line = line.end_line;
            let location = Location {
                file: file.to_vec(),
                line: line.line,
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
                        &current,
                        &current_offsets,
                        Recipe {
                            bytes: line.bytes[1..].to_vec(),
                            location,
                            prefix,
                        },
                        &mut recipe_started,
                        context,
                    );
                }
                continue;
            }
            // Assignment values may contain semicolons. Rule inline recipes
            // retain shell comments, so comment stripping happens separately.
            let normal = strip_comment(&line.bytes);
            let text = trim_start(&normal);
            if text.is_empty() {
                continue;
            }
            let keyword = words(text).next().unwrap_or_default();
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
                    .contains(&words(rest).next().unwrap_or_default())
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
            if let Some(header) = directive(text, b"define") {
                let mut body = Vec::new();
                let mut depth = 1;
                let mut warning_line = location.line;
                loop {
                    let Some(part) = reader.next_line(prefix, false) else {
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
                                file: file.to_vec(),
                                line: warning_line,
                            },
                            "warning: NUL character seen; rest of line ignored".into(),
                        );
                    }
                    warning_line += part.end_line - part.line + 1;
                    last_line = part.end_line;
                    let token = trim(&part.bytes);
                    if part.bytes.first() != Some(&prefix) {
                        if directive(token, b"define").is_some() {
                            depth += 1;
                        }
                        if directive(token, b"endef").is_some() {
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
                if active {
                    let spec = assignment(header);
                    let (name, kind) =
                        spec.map_or((trim(header), Assignment::Recursive), |s| (s.name, s.kind));
                    self.define(
                        AssignmentSpec {
                            name,
                            value: &body,
                            kind,
                        },
                        Origin::File,
                        &location,
                        &Location {
                            file: file.to_vec(),
                            line: last_line,
                        },
                        context,
                    )?;
                    current.clear();
                }
                continue;
            }
            if !active {
                continue;
            }
            let mut variable_text = text;
            let mut origin = Origin::File;
            let mut export = None;
            let mut private = false;
            loop {
                // A modifier can itself be a variable name (`private = x`).
                if assignment(variable_text).is_some() {
                    break;
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
                    break;
                }
            }
            if let Some(rest) = directive(variable_text, b"undefine") {
                let name = self.expand(rest, &location, context)?;
                let name = trim(&name);
                if self.variables.undefine(name, origin) {
                    self.exports.remove(name);
                    self.unexports.remove(name);
                }
                current.clear();
                continue;
            }
            if let Some(spec) = assignment(variable_text) {
                let name = self.define(spec, origin, &location, &location, context)?;
                if private {
                    self.variables.set_private(&name, true);
                }
                if let Some(export) = export {
                    self.set_export(name, export);
                }
                current.clear();
                continue;
            }
            // Modifiers only belong to a recognized variable operation. If
            // there was none, parse the original statement as a directive or
            // rule. Names such as `private` remain literal export-list names.
            let export_list = directive(text, b"export")
                .map(|names| (true, names))
                .or_else(|| directive(text, b"unexport").map(|names| (false, names)));
            if let Some((export, text)) = export_list {
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
                current.clear();
                continue;
            }
            if [
                b"include".as_slice(),
                b"-include",
                b"sinclude",
                b"vpath",
                b"load",
            ]
            .contains(&keyword)
            {
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
                        &prefix_expansion.expanded[..colon],
                        assignment,
                        &location,
                        context,
                    )?;
                    current.clear();
                    continue;
                }
            }
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
                current.clear();
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
            if expanded.get(colon + 1) == Some(&b':')
                || separator(&expanded[colon + 1..], b':').is_some()
                || expanded[..colon].ends_with(b"&")
            {
                return Err(ParseError {
                    unlocated: false,
                    location,
                    message: "rmake: unsupported rule form".into(),
                });
            }
            current.clear();
            for target in words(&expanded[..colon]) {
                let pattern = Pattern::new(target);
                if pattern.has_wildcard() {
                    return Err(ParseError {
                        unlocated: false,
                        location,
                        message: "rmake: unsupported rule form".into(),
                    });
                }
                if !only_percent_escapes(target) {
                    return Err(ParseError {
                        unlocated: false,
                        location,
                        message: "rmake: escaped target names are not implemented".into(),
                    });
                }
                current.push(pattern.literal_bytes().to_vec());
            }
            if current.is_empty() {
                continue;
            }
            let prereqs = &expanded[colon + 1..];
            let (normal, order) = separator(prereqs, b'|')
                .map_or((prereqs, &b""[..]), |i| (&prereqs[..i], &prereqs[i + 1..]));
            let empty_dependencies =
                words(normal).next().is_none() && words(order).next().is_none();
            // Check the complete declaration before merging any target: a
            // .SUFFIXES target on this same line has not changed the list yet.
            if self
                .variables
                .get(b".DEFAULT_GOAL")
                .is_none_or(|v| v.raw_value().is_empty())
                && let Some(target) = current.iter().find(|target| {
                    (!target.starts_with(b".") || target.contains(&b'/'))
                        && !self.is_suffix_rule_name(target)
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
            current_offsets.clear();
            for target in &current {
                let rule = self.rules.entry(target.clone()).or_default();
                if target == b".SUFFIXES" && empty_dependencies {
                    // Clearing suffix metadata retains its recipe and location.
                    // Record offsets only after clearing, including repeated
                    // target names in the same declaration.
                    rule.dependencies.clear();
                }
                if current_offsets.contains_key(target) {
                    record_warning(
                        &mut self.warnings,
                        context,
                        location.clone(),
                        [
                            b"target '".as_slice(),
                            target,
                            b"' given more than once in the same rule",
                        ]
                        .concat()
                        .into(),
                    );
                } else {
                    current_offsets.insert(target.clone(), rule.dependencies.len());
                }
                rule.dependencies
                    .extend([(normal, false), (order, true)].into_iter().flat_map(
                        |(names, order_only)| {
                            words(names).map(move |name| Dependency {
                                name: name.to_vec(),
                                order_only,
                            })
                        },
                    ));
            }
            recipe_started = false;
            let recipe = inline
                .map(|i| line.bytes[i + 1..].to_vec())
                .or_else(|| expanded_inline.map(|i| expanded_line[i + 1..].to_vec()));
            if let Some(bytes) = recipe {
                self.add_recipe(
                    &current,
                    &current_offsets,
                    Recipe {
                        bytes,
                        location,
                        prefix,
                    },
                    &mut recipe_started,
                    context,
                );
            }
        }
        if !conditions.is_empty() {
            return Err(ParseError {
                unlocated: false,
                location: Location {
                    file: file.to_vec(),
                    line: last_line + 1,
                },
                message: "missing 'endif'".into(),
            });
        }
        Ok(())
    }

    fn set_export(&mut self, name: Vec<u8>, export: bool) {
        self.variables.set_export_policy(&name, Some(export));
        if export {
            self.unexports.remove(&name);
            self.exports.insert(name);
        } else {
            self.exports.remove(&name);
            self.unexports.insert(name);
        }
    }

    fn add_recipe(
        &mut self,
        targets: &[Vec<u8>],
        offsets: &HashMap<Vec<u8>, usize>,
        recipe: Recipe,
        started: &mut bool,
        context: &mut ExpansionContext<'_>,
    ) {
        let mut seen = HashSet::new();
        for target in targets {
            if !seen.insert(target) {
                continue;
            }
            let rule = self
                .rules
                .get_mut(target)
                .expect("current targets were inserted");
            if !*started {
                let count = rule.dependencies.len() - offsets[target];
                rule.dependencies.rotate_right(count);
                if let Some(previous) = &rule.recipe_location {
                    record_warning(
                        &mut self.warnings,
                        context,
                        recipe.location.clone(),
                        [
                            b"warning: overriding recipe for target '".as_slice(),
                            target,
                            b"'",
                        ]
                        .concat()
                        .into(),
                    );
                    record_warning(
                        &mut self.warnings,
                        context,
                        previous.clone(),
                        [
                            b"warning: ignoring old recipe for target '".as_slice(),
                            target,
                            b"'",
                        ]
                        .concat()
                        .into(),
                    );
                }
                rule.recipes = Some(Vec::new());
                rule.recipe_location = Some(recipe.location.clone());
            }
            rule.recipes
                .as_mut()
                .expect("recipe initialized")
                .push(recipe.clone());
        }
        *started = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
                            file: b"old".to_vec(),
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
                    file: b"m".to_vec(),
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
        assert_eq!(db.variables.expand(b"$(.DEFAULT_GOAL)").unwrap(), b"a%b.o");
        assert_eq!(
            target_variables(&mut db, b"a%b.o").expand(b"$(X)").unwrap(),
            b"literal"
        );
        assert_eq!(
            target_variables(&mut db, b"axb.o").expand(b"$(X)").unwrap(),
            b""
        );
        assert!(Database::default().read(b"%.o:;\n", b"m").is_err());
        assert!(Database::default().read(b"a\\ b:;\n", b"m").is_err());
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
                    file: b"input\xfe.mk".to_vec(),
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
            assert_eq!(error.location.file, b"bytes.mk");
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
                    file: b"define.mk".to_vec(),
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
                file: b"define.mk".to_vec(),
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
                file: b"second\xfe.mk".to_vec(),
                line: 1
            })
        );
        assert_eq!(
            snapshot.expand_variable(b"V").unwrap_err().location(),
            Some(&Location {
                file: b"first\xff.mk".to_vec(),
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
                    file: b"fixture.mk".to_vec(),
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
