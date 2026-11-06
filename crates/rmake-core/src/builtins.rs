// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Pinned Linux declarative defaults and suffix-to-pattern conversion.
//! The embedded recipe/variable data was observed from the oracle's public
//! database output. There is no reference executable at runtime.

use crate::diagnostic::{ExpansionContext, Location, Output, SourceName};
use crate::parser::{Database, Dependency, ParseError, Rule, assignment};
use crate::variables::{ExpandError, Origin, Pattern};

const VARIABLES: &[u8] = include_bytes!("../data/default-variables.mk");
const SUFFIX_RULES: &[u8] = include_bytes!("../data/default-suffix-rules.mk");
const PATTERN_RULES: &[u8] = include_bytes!("../data/default-pattern-rules.mk");

fn builtin_location(rule: &mut Rule) {
    rule.prerequisite_location = None;
    rule.recipe_location = None;
    if let Some(recipes) = &mut rule.recipes {
        for recipe in recipes {
            recipe.location = Location {
                file: SourceName::null(),
                line: 0,
            };
            recipe.builtin = true;
        }
    }
}

// Declarative Linux suffix-name order from GNU Make 4.4.1 default_suffix_rules.
// These names also appear in the attributed public-database compatibility data.
// Order is observable when user records collide with late default installation.
const SUFFIX_RECORD_ORDER: &[u8] = b".o .s .S .c .cc .C .cpp .f .m .p .F .r .mod .def.sym .sh .s.o .S.o .c.o .cc.o .C.o .cpp.o .f.o .m.o .p.o .F.o .r.o .mod.o .c.ln .y.ln .l.ln .y.c .l.c .ym.m .lm.m .F.f .r.f .l.r .S.s .texinfo.info .texi.info .txinfo.info .tex.dvi .texinfo.dvi .texi.dvi .txinfo.dvi .w.c .web.p .w.tex .web.tex";

impl Database {
    pub fn record_builtin_suffixes(&mut self) {
        for name in SUFFIX_RECORD_ORDER.split(|byte| *byte == b' ') {
            if let Some(history) = self.database_history() {
                history.intern(name);
            }
            self.file_records.insert_builtin(name);
        }
    }

    pub fn initialize_builtin_variables(&mut self) -> Result<(), ExpandError> {
        for line in VARIABLES.split(|byte| *byte == b'\n') {
            if let Some(spec) = assignment(line) {
                self.variables
                    .define(spec.name, spec.value, spec.kind, Origin::Default)?;
            }
        }
        Ok(())
    }

    /// A newly selected -R removes only the pinned default-variable names at
    /// default priority. Runtime metadata and stronger user definitions survive.
    pub fn disable_builtin_variables(&mut self) {
        for line in VARIABLES.split(|byte| *byte == b'\n') {
            if let Some(spec) = assignment(line)
                && self.variables.undefine(spec.name, Origin::Default)
            {
                self.exports.remove(spec.name);
                self.unexports.remove(spec.name);
            }
        }
    }

    /// Remove still-built-in suffix recipes before conversion after late -r.
    /// Explicit user recipes/dependencies remain ordinary user declarations.
    pub fn disable_builtin_rules(&mut self) -> Result<(), ExpandError> {
        if let Some(rule) = self.rules.get_mut(b".SUFFIXES".as_slice())
            && rule.record_only
        {
            rule.dependencies.clear();
        }
        self.remove_builtin_recipes();
        self.variables
            .define_literal(b"SUFFIXES", b"", Origin::Default)
    }

    /// Eval can select -r after default variables/suffix metadata exist, but
    /// before built-in recipes are installed by GNU. Preserve those metadata.
    pub fn remove_builtin_recipes(&mut self) {
        // The source table is bounded independently of the user's graph. Its
        // set equality with the pinned declarative recipes is tested below.
        for name in SUFFIX_RECORD_ORDER.split(|byte| *byte == b' ') {
            let Some(rule) = self.rules.get_mut(name) else {
                continue;
            };
            let builtin = rule.recipes.as_ref().is_some_and(|recipes| {
                !recipes.is_empty() && recipes.iter().all(|recipe| recipe.builtin)
            });
            if builtin {
                rule.recipes = None;
                if rule.record_only {
                    self.rules.remove(name);
                }
            }
        }
    }

    pub fn initialize_builtin_rules(&mut self) -> Result<(), ParseError> {
        let mut defaults = Self::default();
        defaults.read(SUFFIX_RULES, b"<builtin>")?;
        for (name, mut rule) in defaults.rules {
            builtin_location(&mut rule);
            rule.record_only = true;
            self.rules.entry(name).or_insert(rule);
        }
        Ok(())
    }

    fn append_pattern(&mut self, pattern: Pattern, rule: Rule) {
        self.append_pattern_with_cache(pattern, rule, true);
    }

    fn append_pattern_with_cache(&mut self, pattern: Pattern, rule: Rule, intern_target: bool) {
        if let Some(history) = self.database_history() {
            if intern_target {
                history.intern(pattern.literal_bytes());
            }
            for dependency in &rule.dependencies {
                history.intern(&dependency.name);
            }
        }
        // User definitions (including cancellations) precede generated and
        // built-in alternatives, even when their recipes are empty.
        if !self
            .implicit_rules
            .iter()
            .any(|(old, existing)| *old == pattern && existing.dependencies == rule.dependencies)
        {
            self.invalidate_implicit_rule_shadow();
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
                hidden: false,
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
                if let Some(history) = self.database_history() {
                    history.record_file_lookup(&name);
                }
                if let Some(rule) = self
                    .rules
                    .get(&name)
                    .filter(|rule| rule.recipes.is_some())
                    .cloned()
                {
                    if !rule.dependencies.is_empty() {
                        if self.posix {
                            continue;
                        }
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
                            hidden: false,
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
                // Compiled default-pattern targets have permanent storage in
                // GNU; only their dependency spellings enter its string cache.
                self.append_pattern_with_cache(pattern, rule, false);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod record_order_tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn disabled_default_variables_preserve_metadata_user_values_and_remove_export_policy() {
        let mut db = Database::default();
        db.initialize_builtin_variables().unwrap();
        db.variables
            .define_literal(b".FEATURES", b"feature", Origin::Default)
            .unwrap();
        db.variables
            .define_literal(b".SHELLFLAGS", b"-c", Origin::Default)
            .unwrap();
        db.variables
            .define_literal(b"CXX", b"user", Origin::File)
            .unwrap();
        db.read(b"export CC\n", b"m").unwrap();
        db.disable_builtin_variables();
        assert!(db.variables.get(b"CC").is_none());
        assert!(!db.exports.contains(b"CC".as_slice()));
        assert_eq!(db.variables.get(b"CXX").unwrap().raw_value(), b"user");
        assert_eq!(
            db.variables.get(b".FEATURES").unwrap().raw_value(),
            b"feature"
        );
        assert_eq!(db.variables.get(b".SHELLFLAGS").unwrap().raw_value(), b"-c");
    }

    #[test]
    fn disabled_default_rules_preserve_user_suffix_declarations_and_recipes() {
        let mut db = Database::default();
        db.initialize_suffixes(true).unwrap();
        db.initialize_builtin_rules().unwrap();
        db.read(
            b".SUFFIXES: .custom\n.c.o:;@custom\n.f.o: user-dependency\n",
            b"m",
        )
        .unwrap();
        db.disable_builtin_rules().unwrap();
        assert!(!db.rules[b".SUFFIXES".as_slice()].dependencies.is_empty());
        assert_eq!(db.variables.get(b"SUFFIXES").unwrap().raw_value(), b"");
        assert!(db.rules[b".c.o".as_slice()].recipes.is_some());
        assert!(db.rules[b".f.o".as_slice()].recipes.is_none());
        assert!(!db.rules.contains_key(b".p.o".as_slice()));
    }

    #[test]
    fn builtin_recipe_origin_is_independent_of_nullable_or_builtin_named_sources() {
        for file in [
            SourceName::null(),
            b"".as_slice().into(),
            b"<builtin>".as_slice().into(),
        ] {
            let mut database = Database::default();
            database.initialize_builtin_rules().unwrap();
            assert!(database.rules[b".c.o".as_slice()].recipes.as_ref().unwrap()[0].builtin);
            assert!(
                database.rules[b".c.o".as_slice()].recipes.as_ref().unwrap()[0]
                    .location
                    .file
                    .is_null()
            );
            let location = Location { file, line: 0 };
            database
                .read_evaluated(b".c.o:;@user\n", Some(&location), ExpansionContext::pure())
                .unwrap();
            database.remove_builtin_recipes();
            let recipes = database.rules[b".c.o".as_slice()].recipes.as_ref().unwrap();
            assert_eq!(recipes[0].bytes, b"@user");
            assert_eq!(recipes[0].location, location);
            assert!(!recipes[0].builtin);
            assert!(!database.rules.contains_key(b".p.o".as_slice()));
        }
    }

    #[test]
    fn late_default_record_names_match_the_pinned_declarative_rules() {
        let mut database = Database::default();
        database.initialize_builtin_rules().unwrap();
        let actual: HashSet<_> = SUFFIX_RECORD_ORDER
            .split(|byte| *byte == b' ')
            .map(Vec::from)
            .collect();
        assert_eq!(
            actual.len(),
            SUFFIX_RECORD_ORDER.split(|byte| *byte == b' ').count()
        );
        assert_eq!(actual, database.rules.into_keys().collect());
    }
}
