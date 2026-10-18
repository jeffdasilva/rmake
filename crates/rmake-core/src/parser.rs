// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Incremental reading and immediate evaluation of the initial language subset.
//!
//! This module has no filesystem or process access. Recipes remain unexpanded
//! until the executor selects a target. Unsupported rule families fail explicitly.

use crate::reader::{Reader, strip_comment};
use crate::variables::{Assignment, Origin, VariableStore};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Location {
    pub file: Vec<u8>,
    pub line: usize,
}

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
    pub message: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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

#[derive(Default)]
pub struct Database {
    pub variables: VariableStore,
    pub rules: HashMap<Vec<u8>, Rule>,
    pub exports: HashSet<Vec<u8>>,
    pub unexports: HashSet<Vec<u8>>,
    pub export_all: bool,
    pub warnings: Vec<(Location, String)>,
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

/// Parse an assignment without expanding either side. The caller chooses the
/// origin and expands the name; the variable store controls RHS timing.
pub fn assignment(bytes: &[u8]) -> Option<AssignmentSpec<'_>> {
    let equal = separator(bytes, b'=')?;
    let (end, kind) = if bytes[..equal].ends_with(b":::") {
        (equal - 3, Assignment::ImmediateRecursive)
    } else if bytes[..equal].ends_with(b"::") {
        (equal - 2, Assignment::Simple)
    } else {
        match equal.checked_sub(1).map(|i| bytes[i]) {
            Some(b':') => (equal - 1, Assignment::Simple),
            Some(b'+') => (equal - 1, Assignment::Append),
            Some(b'?') => (equal - 1, Assignment::Conditional),
            Some(b'!') => (equal - 1, Assignment::Shell),
            _ => (equal, Assignment::Recursive),
        }
    };
    if separator(&bytes[..end], b':').is_some() {
        return None;
    }
    Some(AssignmentSpec {
        name: trim(&bytes[..end]),
        value: trim_start(&bytes[equal + 1..]),
        kind,
    })
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

impl Database {
    fn expand(&self, bytes: &[u8], location: &Location) -> Result<Vec<u8>, ParseError> {
        self.variables.expand(bytes).map_err(|err| ParseError {
            location: location.clone(),
            message: err.to_string(),
        })
    }

    fn define(
        &mut self,
        spec: AssignmentSpec<'_>,
        origin: Origin,
        location: &Location,
    ) -> Result<Vec<u8>, ParseError> {
        let name = self.expand(spec.name, location)?;
        if name.is_empty() {
            return Err(ParseError {
                location: location.clone(),
                message: "empty variable name".into(),
            });
        }
        self.variables
            .define(&name, spec.value, spec.kind, origin)
            .map_err(|err| ParseError {
                location: location.clone(),
                message: err.to_string(),
            })?;
        Ok(name)
    }

    fn condition(&mut self, bytes: &[u8], location: &Location) -> Result<Option<bool>, ParseError> {
        for (keyword, negate) in [(b"ifdef".as_slice(), false), (b"ifndef", true)] {
            if let Some(name) = directive(bytes, keyword) {
                let name = self.expand(name, location)?;
                if words(&name).count() > 1 {
                    return Err(ParseError {
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
                    location: location.clone(),
                    message: "invalid syntax in conditional".into(),
                };
                let (left, right, extra) = if args.starts_with(b"(") {
                    // The outer parentheses delimit the directive; literal
                    // parentheses within either argument also nest. Do not
                    // expand first: commas created by expansion are data.
                    let mut depth = 1usize;
                    let mut comma = None;
                    let mut closing = None;
                    for (index, byte) in args.iter().copied().enumerate().skip(1) {
                        match byte {
                            b'(' => depth += 1,
                            b')' => {
                                depth -= 1;
                                if depth == 0 {
                                    closing = Some(index);
                                    break;
                                }
                            }
                            b',' if depth == 1 && comma.is_none() => comma = Some(index),
                            _ => {}
                        }
                    }
                    let closing = closing.ok_or_else(invalid)?;
                    let comma = comma.filter(|index| *index < closing).ok_or_else(invalid)?;
                    let inner = &args[1..closing];
                    let comma = comma - 1;
                    // Parenthesized conditionals strip trailing blanks on the
                    // left and leading blanks on the right before expansion.
                    let left_end = inner[..comma]
                        .iter()
                        .rposition(|b| !b.is_ascii_whitespace())
                        .map_or(0, |i| i + 1);
                    (
                        &inner[..left_end],
                        trim_start(&inner[comma + 1..]),
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
                    (&args[1..end], &rest[1..end2], &rest[end2 + 1..])
                };
                if !trim(extra).is_empty() {
                    self.warnings.push((
                        location.clone(),
                        format!(
                            "extraneous text after '{}' directive",
                            String::from_utf8_lossy(keyword)
                        ),
                    ));
                }
                return Ok(Some(
                    (self.expand(left, location)? == self.expand(right, location)?) ^ negate,
                ));
            }
        }
        Ok(None)
    }

    pub fn read(&mut self, bytes: &[u8], file: &[u8]) -> Result<(), ParseError> {
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
                let yes = active && self.condition(text, &location)?.unwrap_or(false);
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
                        location,
                        message: "extraneous 'else'".into(),
                    });
                };
                if top.otherwise {
                    return Err(ParseError {
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
                    eligible && self.condition(rest, &location)?.unwrap_or(false)
                } else {
                    self.warnings.push((
                        location.clone(),
                        "extraneous text after 'else' directive".into(),
                    ));
                    top.otherwise = true;
                    eligible
                };
                top.taken |= top.active;
                continue;
            }
            if let Some(rest) = directive(text, b"endif") {
                if conditions.pop().is_none() {
                    return Err(ParseError {
                        location,
                        message: "extraneous 'endif'".into(),
                    });
                }
                if !rest.is_empty() {
                    self.warnings.push((
                        location.clone(),
                        "extraneous text after 'endif' directive".into(),
                    ));
                }
                continue;
            }
            if let Some(header) = directive(text, b"define") {
                let mut body = Vec::new();
                let mut depth = 1;
                loop {
                    let Some(part) = reader.next_line(prefix, false) else {
                        return Err(ParseError {
                            location,
                            message: "missing 'endef', unterminated 'define'".into(),
                        });
                    };
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
                    )?;
                    current.clear();
                }
                continue;
            }
            if !active {
                continue;
            }
            let mut text = text;
            let mut origin = Origin::File;
            let mut export = None;
            loop {
                if let Some(rest) = directive(text, b"override") {
                    origin = Origin::Override;
                    text = rest;
                } else if let Some(rest) = directive(text, b"export") {
                    export = Some(true);
                    text = rest;
                } else if let Some(rest) = directive(text, b"unexport") {
                    export = Some(false);
                    text = rest;
                } else {
                    break;
                }
            }
            if let Some(rest) = directive(text, b"undefine") {
                let name = self.expand(rest, &location)?;
                if self
                    .variables
                    .get(trim(&name))
                    .is_none_or(|v| v.origin() <= origin)
                {
                    self.variables.remove(trim(&name));
                }
                current.clear();
                continue;
            }
            if let Some(spec) = assignment(text) {
                let name = self.define(spec, origin, &location)?;
                if let Some(export) = export {
                    self.set_export(name, export);
                }
                current.clear();
                continue;
            }
            if let Some(export) = export {
                let names = self.expand(text, &location)?;
                if trim(&names).is_empty() {
                    self.export_all = export;
                }
                for name in words(&names) {
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
                    location,
                    message: format!(
                        "rmake: unsupported directive '{}'",
                        String::from_utf8_lossy(keyword)
                    ),
                });
            }
            if line.bytes.first() == Some(&prefix) && prefix == b'\t' {
                return Err(ParseError {
                    location,
                    message: "recipe commences before first target".into(),
                });
            }
            let inline = separator(&line.bytes, b';')
                .filter(|i| separator(&line.bytes[..*i], b'#').is_none());
            let rule_text = inline.map_or(text.to_vec(), |i| strip_comment(&line.bytes[..i]));
            let expanded_line = self.expand(&rule_text, &location)?;
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
                    location,
                    message: message.into(),
                });
            };
            if expanded.get(colon + 1) == Some(&b':')
                || separator(&expanded[colon + 1..], b':').is_some()
                || expanded[..colon].contains(&b'%')
                || expanded[..colon].ends_with(b"&")
                || assignment(&expanded[colon + 1..]).is_some()
            {
                return Err(ParseError {
                    location,
                    message: "rmake: unsupported rule form".into(),
                });
            }
            current = words(&expanded[..colon]).map(Vec::from).collect();
            if current.is_empty() {
                continue;
            }
            if current.iter().any(|target| target.contains(&b'\\')) {
                return Err(ParseError {
                    location,
                    message: "rmake: escaped target names are not implemented".into(),
                });
            }
            let prereqs = &expanded[colon + 1..];
            let (normal, order) = separator(prereqs, b'|')
                .map_or((prereqs, &b""[..]), |i| (&prereqs[..i], &prereqs[i + 1..]));
            current_offsets.clear();
            for target in &current {
                let rule = self.rules.entry(target.clone()).or_default();
                if current_offsets.contains_key(target) {
                    self.warnings.push((
                        location.clone(),
                        format!(
                            "target '{}' given more than once in the same rule",
                            String::from_utf8_lossy(target)
                        ),
                    ));
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
                if (!target.starts_with(b".") || target.contains(&b'/'))
                    && self
                        .variables
                        .get(b".DEFAULT_GOAL")
                        .is_none_or(|v| v.raw_value().is_empty())
                {
                    self.variables
                        .define(b".DEFAULT_GOAL", target, Assignment::Simple, Origin::File)
                        .map_err(|err| ParseError {
                            location: location.clone(),
                            message: err.to_string(),
                        })?;
                }
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
                );
            }
        }
        if !conditions.is_empty() {
            return Err(ParseError {
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
                    self.warnings.push((
                        recipe.location.clone(),
                        format!(
                            "warning: overriding recipe for target '{}'",
                            String::from_utf8_lossy(target)
                        ),
                    ));
                    self.warnings.push((
                        previous.clone(),
                        format!(
                            "warning: ignoring old recipe for target '{}'",
                            String::from_utf8_lossy(target)
                        ),
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
        *started = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(
            db.read(b"target: NAME = value\n", b"m")
                .unwrap_err()
                .message
                .contains("unsupported")
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
            db.warnings[0].1,
            "target 'all' given more than once in the same rule"
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
                .message,
            "invalid syntax in conditional"
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
                .map(|(_, text)| text.as_str())
                .collect::<Vec<_>>(),
            [
                "extraneous text after 'ifeq' directive",
                "extraneous text after 'else' directive",
                "extraneous text after 'endif' directive",
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
            assert_eq!(error.message, message);
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
