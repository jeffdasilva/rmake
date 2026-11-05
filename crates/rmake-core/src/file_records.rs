// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Observable GNU file-record traversal order, with owned names and no pointers.
//! Rule contents and explicit/intermediate flags live separately from this index.
//! The fingerprint is the Jenkins three-lane string permutation used by the
//! pinned Linux oracle. Numeric vectors are observations, not upstream tests.

use std::collections::HashMap;

#[derive(Clone)]
pub struct FileRecords {
    database_history: Option<crate::database::History>,
    // None is unused; an empty owned name is a deletion marker. Make filenames
    // are nonempty. Keeping tombstones preserves collision chains on renaming.
    slots: Vec<Option<Box<[u8]>>>,
    live: usize,
    empty: usize,
    rehashed_names: HashMap<Vec<u8>, Vec<u8>>,
}

impl Default for FileRecords {
    fn default() -> Self {
        Self {
            database_history: None,
            slots: vec![None; 1024],
            live: 0,
            empty: 1024,
            rehashed_names: HashMap::new(),
        }
    }
}

impl FileRecords {
    pub(crate) fn enable_database_history(&mut self, history: crate::database::History) {
        self.database_history = Some(history);
    }

    fn position(&self, name: &[u8]) -> (usize, bool) {
        let mask = self.slots.len() - 1;
        let mut position = fingerprint(name) as usize & mask;
        let mut deleted = None;
        loop {
            match self.slots[position].as_deref() {
                None => return (deleted.unwrap_or(position), false),
                Some(old) if old == name => return (position, true),
                Some([]) => {
                    deleted.get_or_insert(position);
                }
                Some(_) => {}
            }
            position = (position + 1) & mask;
        }
    }

    pub fn contains(&self, name: &[u8]) -> bool {
        !name.is_empty() && self.position(name).1
    }

    /// GNU prerequisite binding observes the existing file first and enters
    /// a new record only when absent. A found builtin retains its origin.
    pub fn enter_prerequisite(&mut self, name: &[u8]) {
        if name.is_empty() {
            return;
        }
        if let Some(history) = &self.database_history {
            history.record_file_lookup(name);
        }
        if !self.contains(name) {
            self.insert(name);
        }
    }

    /// Install a declarative default without turning an existing explicit
    /// user's record back into a builtin. Ordinary enters clear that origin.
    pub fn insert_builtin(&mut self, name: &[u8]) {
        let explicit = self.contains(name)
            && self
                .database_history
                .as_ref()
                .is_some_and(|history| !history.is_builtin_file(name));
        self.insert(name);
        if !explicit && let Some(history) = &self.database_history {
            history.mark_file_builtin(name);
        }
    }

    pub fn insert(&mut self, name: &[u8]) {
        if name.is_empty() {
            return;
        }
        if let Some(history) = &self.database_history {
            history.record_file_enter(name);
        }
        let (position, found) = self.position(name);
        if found {
            return;
        }
        if self.slots[position].is_none() {
            self.empty -= 1;
        }
        self.slots[position] = Some(name.into());
        self.live += 1;
        if self.empty < self.slots.len() / 16 {
            let size = if self.live >= self.slots.len() * 15 / 16 {
                self.slots.len() * 2
            } else {
                self.slots.len()
            };
            let previous = std::mem::replace(&mut self.slots, vec![None; size]);
            self.empty = size;
            self.live = 0;
            let history = self.database_history.take();
            for name in previous
                .into_iter()
                .flatten()
                .filter(|name| !name.is_empty())
            {
                self.insert(&name);
            }
            self.database_history = history;
        }
    }

    /// Lookup spelling and recipe/cleanup spelling differ for tentative VPATH.
    pub fn logical_name(&self, key: &[u8]) -> Option<&[u8]> {
        self.rehashed_names.get(key).map(Vec::as_slice)
    }

    fn current_key(&self, name: &[u8]) -> Vec<u8> {
        if self.contains(name) {
            return name.to_vec();
        }
        self.rehashed_names
            .iter()
            .find(|(_, original)| original.as_slice() == name)
            .map_or_else(|| name.to_vec(), |(key, _)| key.clone())
    }

    fn move_key(&mut self, from: &[u8], to: &[u8]) {
        let (position, found) = self.position(from);
        if found {
            self.slots[position] = Some(Box::default());
            self.live -= 1;
        }
        self.insert(to);
    }

    pub fn rehash(&mut self, from: &[u8], to: &[u8]) {
        if from == to || from.is_empty() || to.is_empty() {
            return;
        }
        let key = self.current_key(from);
        if key == to || self.contains(to) {
            return;
        }
        let original = self
            .rehashed_names
            .remove(&key)
            .unwrap_or_else(|| from.to_vec());
        self.move_key(&key, to);
        self.rehashed_names.insert(to.to_vec(), original);
    }

    pub fn rename(&mut self, from: &[u8], to: &[u8]) {
        if from == to || from.is_empty() || to.is_empty() {
            return;
        }
        let key = self.current_key(from);
        if key == to {
            self.rehashed_names.remove(to);
            return;
        }
        let destination = self.contains(to);
        self.rehashed_names.remove(&key);
        self.move_key(&key, to);
        if !destination {
            self.rehashed_names.remove(to);
        }
    }

    pub fn slots_len(&self) -> usize {
        self.slots.len()
    }

    pub fn name_in_slot(&self, index: usize) -> Option<&[u8]> {
        self.slots
            .get(index)
            .and_then(|slot| slot.as_deref())
            .filter(|name| !name.is_empty())
    }

    pub fn names(&self) -> impl Iterator<Item = &[u8]> {
        self.slots
            .iter()
            .filter_map(|name| name.as_deref().filter(|name| !name.is_empty()))
            .map(|key| self.logical_name(key).unwrap_or(key))
    }
}

pub(crate) fn fingerprint(name: &[u8]) -> u32 {
    let mut lanes = [0xdead_beef_u32; 3];
    let mut offset = 0;
    loop {
        let lane = offset / 4 % 3;
        let available = (name.len() - offset).min(4);
        let mut word = [0_u8; 4];
        word[..available].copy_from_slice(&name[offset..offset + available]);
        lanes[lane] = lanes[lane].wrapping_add(u32::from_le_bytes(word));
        if available < 4 {
            break;
        }
        if lane == 2 {
            for (step, angle) in [4, 6, 8, 16, 19, 4].into_iter().enumerate() {
                let target = step % 3;
                let previous = (target + 2) % 3;
                let next = (target + 1) % 3;
                lanes[target] = lanes[target].wrapping_sub(lanes[previous])
                    ^ lanes[previous].rotate_left(angle);
                lanes[previous] = lanes[previous].wrapping_add(lanes[next]);
            }
        }
        offset += 4;
    }
    for (step, angle) in [14, 11, 25, 16, 4, 14, 24].into_iter().enumerate() {
        let target = (step + 2) % 3;
        let previous = (target + 2) % 3;
        lanes[target] =
            (lanes[target] ^ lanes[previous]).wrapping_sub(lanes[previous].rotate_left(angle));
    }
    lanes[2].wrapping_add(offset as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observed_numeric_fingerprints_cover_word_and_triple_boundaries() {
        for (name, expected) in [
            (b"".as_slice(), 834184464),
            (b"a", 2103777456),
            (b"ab", 4187565006),
            (b"abc", 3127986206),
            (b"abcd", 798409325),
            (b"abcde", 3847514956),
            (b"abcdefghijkl", 2594690271),
            (b"abcdefghijklm", 3179244857),
            (b"Makefile", 1863004401),
            (b".DEFAULT", 2081839685),
            (b".SUFFIXES", 3058110371),
            (b"a.mid", 1083198314),
            (b"b.mid", 1787702097),
        ] {
            assert_eq!(fingerprint(name), expected, "{name:?}");
        }
    }

    #[test]
    fn tentative_keys_keep_recipe_names_until_a_permanent_rename() {
        let mut records = FileRecords::default();
        records.insert(b"input");
        records.insert(b"other");
        records.rehash(b"input", b"d/input");
        assert!(!records.contains(b"input"));
        assert!(records.contains(b"d/input"));
        assert_eq!(records.logical_name(b"d/input"), Some(b"input".as_slice()));
        assert!(records.names().any(|name| name == b"input"));
        let snapshot = records.clone();
        records.rename(b"input", b"d/input");
        assert_eq!(records.logical_name(b"d/input"), None);
        assert!(records.names().any(|name| name == b"d/input"));
        assert_eq!(snapshot.logical_name(b"d/input"), Some(b"input".as_slice()));
        records.rehash(b"other", b"d/other");
        records.rename(b"other", b"d/input");
        assert!(!records.contains(b"d/other"));
        assert_eq!(records.names().count(), 1);
        assert_eq!(records.logical_name(b"d/input"), None);
    }

    #[test]
    fn collision_tombstones_merges_and_growth_preserve_every_name() {
        let mut records = FileRecords::default();
        let colliding: Vec<_> = (0..10000)
            .map(|n| format!("original-{n}"))
            .filter(|n| fingerprint(n.as_bytes()) & 1023 == 0)
            .take(4)
            .collect();
        assert_eq!(colliding.len(), 4);
        for name in &colliding {
            records.insert(name.as_bytes());
        }
        records.rename(colliding[0].as_bytes(), colliding[2].as_bytes());
        assert!(!records.contains(colliding[0].as_bytes()));
        assert!(records.contains(colliding[1].as_bytes()));
        records.insert(colliding[0].as_bytes());
        assert_eq!(
            records.names().take(4).collect::<Vec<_>>(),
            vec![
                colliding[0].as_bytes(),
                colliding[1].as_bytes(),
                colliding[2].as_bytes(),
                colliding[3].as_bytes()
            ]
        );
        for n in 0..4000 {
            records.insert(format!("raw-{n}-\u{ff}").as_bytes());
        }
        assert_eq!(records.names().count(), 4004);
        for n in 0..4000 {
            assert!(records.contains(format!("raw-{n}-\u{ff}").as_bytes()));
        }
        let clone = records.clone();
        records.rename(colliding[1].as_bytes(), b"replacement");
        assert!(clone.contains(colliding[1].as_bytes()));
        assert!(!records.contains(colliding[1].as_bytes()));
        assert!(records.contains(b"replacement"));
    }
}
