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
            if let Some(history) = &self.database_history {
                history.record_file_remove(from);
            }
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

/// Slot traversal for a freshly populated filename table. The borrowed input
/// is insertion order; the returned indices never copy or modify its names.
/// Directory globbing uses this order before a stable locale collation, where
/// distinct byte names can compare equal. No table or string storage escapes.
pub fn traversal_order(names: &[&[u8]], initial: usize) -> Option<Vec<usize>> {
    if initial < 16 || !initial.is_power_of_two() {
        return None;
    }
    fn insert(slots: &mut [usize], names: &[&[u8]], item: usize) -> bool {
        let mask = slots.len() - 1;
        let mut position = fingerprint(names[item]) as usize & mask;
        loop {
            if slots[position] == usize::MAX {
                slots[position] = item;
                return true;
            }
            if names[slots[position]] == names[item] {
                slots[position] = item;
                return false;
            }
            position = (position + 1) & mask;
        }
    }
    let mut slots = vec![usize::MAX; initial];
    let mut live = 0;
    for item in 0..names.len() {
        live += usize::from(insert(&mut slots, names, item));
        if live > slots.len() * 15 / 16 {
            let size = slots.len().checked_mul(2)?;
            let old = std::mem::replace(&mut slots, vec![usize::MAX; size]);
            for item in old.into_iter().filter(|item| *item != usize::MAX) {
                insert(&mut slots, names, item);
            }
        }
    }
    slots.retain(|item| *item != usize::MAX);
    Some(slots)
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
    fn filename_moves_and_merges_retire_the_actual_logical_key() {
        let history = crate::database::History::default();
        let mut records = FileRecords::default();
        records.enable_database_history(history.clone());
        records.insert(b"alpha");
        records.rehash(b"alpha", b"beta");
        assert_eq!(history.file_stats().filled, 1);
        assert_eq!(history.file_stats().lookups, 3); // enter, remove, enter
        assert_eq!(records.names().collect::<Vec<_>>(), [b"alpha"]);
        records.insert(b"gamma");
        records.rename(b"alpha", b"gamma");
        assert_eq!(records.names().collect::<Vec<_>>(), [b"gamma"]);
        assert_eq!(history.file_stats().filled, records.names().count());
        assert_eq!(history.file_stats().lookups, 6);
    }

    #[test]
    fn directory_traversal_preserves_raw_equal_collation_keys_across_growth() {
        let mut names: Vec<Vec<u8>> = [
            b".".as_slice(),
            b"..",
            b"Makefile",
            b"fZ",
            b"fa",
            b"fE",
            b"f\xc3\x89",
            b"f\x80",
            b"f\xff",
            b"f\xc3",
            b"f\xc3(",
        ]
        .into_iter()
        .map(<[u8]>::to_vec)
        .collect();
        // These distinct raw names have the same fr_FR.UTF-8 collation key.
        // Original GNU2 fixtures pin their observable order before/after each
        // directory-table growth, independently of the Rust map layout.
        for (padding, expected) in [
            (0, [b"f\xff".as_slice(), b"f\x80", b"f\xc3"]),
            (120, [b"f\x80".as_slice(), b"f\xff", b"f\xc3"]),
            (240, [b"f\x80".as_slice(), b"f\xc3", b"f\xff"]),
            (500, [b"f\x80".as_slice(), b"f\xc3", b"f\xff"]),
        ] {
            while names.len() < 11 + padding {
                names.push(format!("pad{:04}", names.len() - 11).into_bytes());
            }
            let borrowed: Vec<_> = names.iter().map(Vec::as_slice).collect();
            let order = traversal_order(&borrowed, 128).unwrap();
            let ties: Vec<_> = order
                .into_iter()
                .map(|index| borrowed[index])
                .filter(|name| expected.contains(name))
                .collect();
            assert_eq!(ties, expected, "padding={padding}");
        }
    }

    #[test]
    fn borrowed_filename_traversal_replaces_duplicate_identity_and_rejects_bad_capacity() {
        let names = [b"same".as_slice(), b"other", b"same"];
        let order = traversal_order(&names, 16).unwrap();
        assert_eq!(order.len(), 2);
        assert!(order.contains(&2));
        assert!(!order.contains(&0));
        for initial in [0, 8, 17] {
            assert!(traversal_order(&names, initial).is_none());
        }
    }

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
