// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Permanent filename identities, separate from tentative VPATH spellings.

use std::collections::HashMap;

#[derive(Default)]
pub struct Aliases {
    names: HashMap<Vec<u8>, (Vec<u8>, bool)>,
}

impl Aliases {
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    pub fn resolve<'a>(&'a self, mut name: &'a [u8]) -> &'a [u8] {
        while let Some((next, _)) = self.names.get(name) {
            name = next;
        }
        name
    }

    pub fn contains(&self, name: &[u8]) -> bool {
        self.names.contains_key(name)
    }

    pub fn merged(&self, name: &[u8]) -> bool {
        self.names.get(name).is_some_and(|(_, merged)| *merged)
    }

    pub fn insert(&mut self, from: &[u8], to: &[u8], merged: bool) {
        let to = self.resolve(to);
        if from != to {
            let to = to.to_vec();
            self.names.insert(from.to_vec(), (to, merged));
        }
    }
}
