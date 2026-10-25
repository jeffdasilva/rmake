// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Intermediate-file policy fixed after each complete Makefile read.

use rmake_core::parser::{Database, Rule};
use std::collections::HashSet;

#[derive(Default)]
pub struct Policy {
    pub all_secondary: bool,
    pub no_intermediates: bool,
    intermediate: HashSet<Vec<u8>>,
    secondary: HashSet<Vec<u8>>,
    not_intermediate: HashSet<Vec<u8>>,
}

fn branches<'a>(database: &'a Database, name: &[u8]) -> &'a [Rule] {
    database.rules.get(name).map_or(&[], |rule| {
        rule.double_colon
            .as_deref()
            .unwrap_or_else(|| std::slice::from_ref(rule))
    })
}

fn global_setting(database: &Database, name: &[u8]) -> bool {
    let rules = branches(database, name);
    if rules.is_empty() {
        // A prerequisite mention or target-specific binding also enters a
        // special filename record, even without a corresponding build rule.
        database.explicit_names.contains(name) || database.target_scopes.contains_key(name)
    } else {
        rules.iter().any(|rule| rule.dependencies.is_empty())
    }
}

impl Policy {
    pub fn read(database: &Database) -> Result<Self, Vec<u8>> {
        let not_intermediate: HashSet<_> = branches(database, b".NOTINTERMEDIATE")
            .iter()
            .flat_map(|rule| &rule.dependencies)
            .map(|dependency| dependency.name.clone())
            .collect();
        let mut policy = Self {
            no_intermediates: global_setting(database, b".NOTINTERMEDIATE"),
            all_secondary: global_setting(database, b".SECONDARY"),
            not_intermediate,
            ..Self::default()
        };
        for name in [b".INTERMEDIATE".as_slice(), b".SECONDARY"] {
            for dependency in branches(database, name)
                .iter()
                .flat_map(|rule| &rule.dependencies)
            {
                if policy.not_intermediate.contains(&dependency.name) {
                    return Err([
                        dependency.name.as_slice(),
                        b" cannot be both .NOTINTERMEDIATE and ",
                        name,
                    ]
                    .concat());
                }
                if name == b".INTERMEDIATE" {
                    policy.intermediate.insert(dependency.name.clone());
                } else {
                    policy.secondary.insert(dependency.name.clone());
                }
            }
        }
        if policy.all_secondary && policy.no_intermediates {
            return Err(b".NOTINTERMEDIATE and .SECONDARY are mutually exclusive".to_vec());
        }
        Ok(policy)
    }

    pub fn named(&self, special: &[u8]) -> Option<&HashSet<Vec<u8>>> {
        match special {
            b".INTERMEDIATE" => Some(&self.intermediate),
            b".SECONDARY" => Some(&self.secondary),
            b".NOTINTERMEDIATE" => Some(&self.not_intermediate),
            _ => None,
        }
    }
}
