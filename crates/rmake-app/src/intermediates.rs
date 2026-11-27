// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Intermediate-file policy fixed after each complete Makefile read.

use rmake_core::messages::Message;
use rmake_core::parser::{Database, Rule};
use std::collections::HashSet;

#[derive(Default)]
pub struct Policy {
    pub all_secondary: bool,
    pub no_intermediates: bool,
    intermediate: HashSet<Vec<u8>>,
    secondary: HashSet<Vec<u8>>,
    not_intermediate: HashSet<Vec<u8>>,
    // Only global .SECONDARY needs this read-time inventory. Keep other
    // policies small rather than embedding another table in every engine.
    #[allow(clippy::box_collection)]
    secondary_files: Option<Box<HashSet<Vec<u8>>>>,
}

#[derive(Debug)]
pub struct PolicyConflict {
    pub diagnostic: Vec<u8>,
    pub last_lookup: &'static [u8],
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
    pub fn read(&mut self, database: &Database) -> Result<(), PolicyConflict> {
        let not_intermediate: HashSet<_> = branches(database, b".NOTINTERMEDIATE")
            .iter()
            .flat_map(|rule| &rule.dependencies)
            .map(|dependency| dependency.name.clone())
            .collect();
        *self = Self {
            not_intermediate,
            ..Self::default()
        };
        for name in [b".INTERMEDIATE".as_slice(), b".SECONDARY"] {
            for dependency in branches(database, name)
                .iter()
                .flat_map(|rule| &rule.dependencies)
            {
                if self.not_intermediate.contains(&dependency.name) {
                    return Err(PolicyConflict {
                        diagnostic: crate::messages::system(Message::IntermediateTargetConflict {
                            target: &dependency.name,
                            secondary: name == b".SECONDARY",
                        }),
                        last_lookup: name,
                    });
                }
                if name == b".INTERMEDIATE" {
                    self.intermediate.insert(dependency.name.clone());
                } else {
                    self.secondary.insert(dependency.name.clone());
                }
            }
        }
        let no_intermediates = global_setting(database, b".NOTINTERMEDIATE");
        let all_secondary = global_setting(database, b".SECONDARY");
        if all_secondary && no_intermediates {
            return Err(PolicyConflict {
                diagnostic: crate::messages::system(Message::IntermediatePolicyConflict),
                last_lookup: b".SECONDARY",
            });
        }
        // Global flags apply to file records only after every named
        // declaration has been processed.
        self.no_intermediates = no_intermediates;
        self.all_secondary = all_secondary;
        self.capture_files(database);
        Ok(())
    }

    /// Global secondary flags apply to records entered before dependencies
    /// are frozen, not names introduced by later deferred expansion.
    fn capture_files(&mut self, database: &Database) {
        if self.all_secondary {
            let mut names = database.explicit_names.clone();
            names.extend(database.rules.keys().cloned());
            names.extend(database.target_scopes.keys().cloned());
            names.extend(database.makefiles.iter().map(|file| file.name.clone()));
            self.secondary_files = Some(Box::new(names));
        }
    }

    pub fn capture_extra_records(&mut self, records: Vec<Vec<u8>>) {
        if let Some(files) = &mut self.secondary_files {
            files.extend(records);
        }
    }

    pub fn global_intermediate(&self, target: &[u8]) -> bool {
        self.secondary_files
            .as_ref()
            .is_some_and(|names| names.contains(target))
    }

    pub fn renamed(&mut self, from: &[u8], to: &[u8], merged: bool) {
        if !merged
            && let Some(names) = &mut self.secondary_files
            && names.contains(from)
        {
            names.insert(to.to_vec());
        }
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
