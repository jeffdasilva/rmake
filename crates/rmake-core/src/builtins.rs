// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Pinned Linux declarative defaults and suffix-to-pattern conversion.
//! The embedded recipe/variable data was observed from the oracle's public
//! database output. There is no reference executable at runtime.

use crate::diagnostic::{ExpansionContext, Location, Output};
use crate::parser::{Database, Dependency, ParseError, Recipe, Rule, assignment};
use crate::variables::{ExpandError, Origin, Pattern};

const VARIABLES: &[u8] = include_bytes!("../data/default-variables.mk");
const SUFFIX_RULES: &[u8] = include_bytes!("../data/default-suffix-rules.mk");
const PATTERN_RULES: &[u8] = include_bytes!("../data/default-pattern-rules.mk");

fn builtin_location(rule: &mut Rule) {
    rule.prerequisite_location = None;
    rule.recipe_location = None;
    if let Some(recipes) = &mut rule.recipes {
        for Recipe { location, .. } in recipes {
            *location = Location {
                file: b"<builtin>".to_vec(),
                line: 0,
            };
        }
    }
}

impl Database {
    pub fn initialize_builtin_variables(&mut self) -> Result<(), ExpandError> {
        for line in VARIABLES.split(|byte| *byte == b'\n') {
            if let Some(spec) = assignment(line) {
                self.variables
                    .define(spec.name, spec.value, spec.kind, Origin::Default)?;
            }
        }
        Ok(())
    }

    pub fn initialize_builtin_rules(&mut self) -> Result<(), ParseError> {
        let mut defaults = Self::default();
        defaults.read(SUFFIX_RULES, b"<builtin>")?;
        for (name, mut rule) in defaults.rules {
            builtin_location(&mut rule);
            self.rules.entry(name).or_insert(rule);
        }
        Ok(())
    }

    fn append_pattern(&mut self, pattern: Pattern, rule: Rule) {
        // User definitions (including cancellations) precede generated and
        // built-in alternatives, even when their recipes are empty.
        if !self
            .implicit_rules
            .iter()
            .any(|(old, existing)| *old == pattern && existing.dependencies == rule.dependencies)
        {
            self.implicit_rules.push((pattern, rule));
        }
    }

    fn append_suffix(&mut self, source: &[u8], destination: &[u8], rule: &Rule) {
        let mut converted = rule.clone();
        converted.dependencies = if source.is_empty() {
            Vec::new()
        } else {
            vec![Dependency {
                name: [b"%".as_slice(), source].concat(),
                order_only: false,
            }]
        };
        converted.secondary = None;
        converted.double_colon = None;
        self.append_pattern(Pattern::new(&[b"%", destination].concat()), converted);
    }

    /// Called after each complete read, before makefile remaking. Suffix-list
    /// order controls conversion priority; user patterns precede conversions.
    pub fn finalize_implicit_rules(
        &mut self,
        builtins: bool,
        context: &mut ExpansionContext<'_>,
    ) -> Result<(), ParseError> {
        let suffixes = self
            .rules
            .get(b".SUFFIXES".as_slice())
            .map_or_else(Vec::new, |rule| {
                rule.canonical_dependencies()
                    .into_iter()
                    .map(|dep| dep.name)
                    .collect::<Vec<_>>()
            });
        for source in &suffixes {
            self.append_suffix(b"", source, &Rule::default());
            if let Some(rule) = self
                .rules
                .get(source)
                .filter(|rule| rule.recipes.is_some())
                .cloned()
            {
                self.append_suffix(source, b"", &rule);
            }
            for destination in &suffixes {
                if source == destination {
                    continue;
                }
                let name = [source.as_slice(), destination].concat();
                if let Some(rule) = self
                    .rules
                    .get(&name)
                    .filter(|rule| rule.recipes.is_some())
                    .cloned()
                {
                    if !rule.dependencies.is_empty() {
                        let location = rule
                            .recipe_location
                            .as_ref()
                            .or(rule.prerequisite_location.as_ref());
                        context.emit(Output::Warning {
                            location,
                            message: b"warning: ignoring prerequisites on suffix rule definition",
                        });
                    }
                    if destination == b".a" {
                        let mut archive = rule.clone();
                        archive.dependencies = vec![Dependency {
                            name: [b"%".as_slice(), source].concat(),
                            order_only: false,
                        }];
                        archive.secondary = None;
                        archive.double_colon = None;
                        self.append_pattern(Pattern::new(b"(%.o)"), archive);
                    }
                    self.append_suffix(source, destination, &rule);
                }
            }
        }
        if builtins {
            let mut defaults = Self::default();
            defaults.read(PATTERN_RULES, b"<builtin>")?;
            for (pattern, mut rule) in defaults.implicit_rules {
                builtin_location(&mut rule);
                self.append_pattern(pattern, rule);
            }
        }
        Ok(())
    }
}
