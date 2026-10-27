// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! An owned index for observable variable traversal. Values live elsewhere.
//! Definition history is cheap to retain; slot realization waits for a consumer.
use std::sync::{Arc, OnceLock};

#[derive(Debug)]
pub(crate) struct NameOrder {
    initial: usize,
    record: bool,
    changes: Vec<(Arc<[u8]>, bool)>,
    ready: OnceLock<Layout>,
}

impl Clone for NameOrder {
    fn clone(&self) -> Self {
        Self {
            initial: self.initial,
            record: self.record,
            changes: if self.record || self.ready.get().is_none() {
                self.changes.clone()
            } else {
                Vec::new()
            },
            ready: self.ready.clone(),
        }
    }
}

impl NameOrder {
    pub(crate) fn new(size: usize, record: bool) -> Self {
        let result = Self::deferred(size, record);
        result
            .ready
            .set(Layout::new(size))
            .expect("fresh order index");
        result
    }
    pub(crate) fn deferred(size: usize, record: bool) -> Self {
        Self {
            initial: size,
            record,
            changes: Vec::new(),
            ready: OnceLock::new(),
        }
    }
    fn layout(&self) -> &Layout {
        self.ready.get_or_init(|| {
            let mut index = Layout::new(self.initial);
            for (name, insert) in &self.changes {
                if *insert {
                    index.insert_owned(name.clone());
                } else {
                    index.remove(name);
                }
            }
            index
        })
    }
    pub(crate) fn insert(&mut self, name: &[u8]) {
        if !name.is_empty() {
            self.insert_owned(name.into());
        }
    }
    pub(crate) fn insert_owned(&mut self, name: Arc<[u8]>) {
        if name.is_empty() {
            return;
        }
        if let Some(index) = self.ready.get_mut() {
            index.insert_owned(name.clone());
            if !self.record {
                self.changes.clear();
                return;
            }
        }
        self.changes.push((name, true));
    }
    pub(crate) fn remove(&mut self, name: &[u8]) {
        if name.is_empty() {
            return;
        }
        if let Some(index) = self.ready.get_mut() {
            index.remove(name);
            if !self.record {
                self.changes.clear();
                return;
            }
        }
        self.changes.push((name.into(), false));
    }
    pub(crate) fn names(&self) -> impl Iterator<Item = &[u8]> {
        self.layout().names()
    }
    pub(crate) fn replay_into(&self, destination: &mut Self) {
        if self.record {
            for (name, insert) in &self.changes {
                if *insert {
                    destination.insert_owned(name.clone());
                } else {
                    destination.remove(name);
                }
            }
        } else {
            for name in self
                .layout()
                .slots
                .iter()
                .flatten()
                .filter(|name| !name.is_empty())
            {
                destination.insert_owned(name.clone());
            }
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Layout {
    initial: usize,
    slots: Vec<Option<Arc<[u8]>>>,
    empty: usize,
    live: usize,
}

impl Layout {
    pub(crate) fn new(size: usize) -> Self {
        Self {
            initial: size,
            slots: Vec::new(),
            empty: 0,
            live: 0,
        }
    }

    fn position(&self, name: &[u8]) -> (usize, bool) {
        if self.slots.is_empty() {
            return (0, false);
        }
        let mask = self.slots.len() - 1;
        let mut at = fingerprint(name) as usize & mask;
        let mut deleted = None;
        loop {
            match self.slots[at].as_deref() {
                None => return (deleted.unwrap_or(at), false),
                Some(old) if old == name => return (at, true),
                Some([]) => {
                    deleted.get_or_insert(at);
                }
                _ => {}
            }
            at = (at + 1) & mask;
        }
    }

    pub(crate) fn insert_owned(&mut self, name: Arc<[u8]>) {
        if name.is_empty() {
            return;
        }
        if self.slots.is_empty() {
            self.slots.resize(self.initial, None);
            self.empty = self.initial;
        }
        let (at, found) = self.position(&name);
        if found {
            return;
        }
        self.empty -= usize::from(self.slots[at].is_none());
        self.live += 1;
        self.slots[at] = Some(name);
        if self.empty < self.slots.len() / 16 {
            let size = self.slots.len()
                * if self.live >= self.slots.len() * 15 / 16 {
                    2
                } else {
                    1
                };
            let old = std::mem::replace(&mut self.slots, vec![None; size]);
            self.empty = size - self.live;
            for name in old.into_iter().flatten().filter(|name| !name.is_empty()) {
                let (at, _) = self.position(&name);
                self.slots[at] = Some(name);
            }
        }
    }

    pub(crate) fn remove(&mut self, name: &[u8]) {
        if name.is_empty() {
            return;
        }
        let (at, found) = self.position(name);
        if found {
            self.slots[at] = Some(Arc::default());
            self.live -= 1;
        }
    }

    pub(crate) fn names(&self) -> impl Iterator<Item = &[u8]> {
        self.slots
            .iter()
            .filter_map(|name| name.as_deref().filter(|name| !name.is_empty()))
    }
}

// Length-aware three-lane fingerprint. The tail is aligned to the final lane;
// full twelve-byte blocks and the final block have distinct permutations.
fn fingerprint(name: &[u8]) -> u32 {
    let mut lanes = [0xdead_beef_u32.wrapping_add(name.len() as u32); 3];
    let blocks = name.len().saturating_sub(1) / 12;
    for block in name[..blocks * 12].chunks_exact(12) {
        for (lane, word) in block.chunks_exact(4).enumerate() {
            lanes[lane] = lanes[lane].wrapping_add(u32::from_le_bytes(word.try_into().unwrap()));
        }
        for (step, angle) in [4, 6, 8, 16, 19, 4].into_iter().enumerate() {
            let target = step % 3;
            let previous = (target + 2) % 3;
            let next = (target + 1) % 3;
            lanes[target] =
                lanes[target].wrapping_sub(lanes[previous]) ^ lanes[previous].rotate_left(angle);
            lanes[previous] = lanes[previous].wrapping_add(lanes[next]);
        }
    }
    let tail = &name[blocks * 12..];
    if !tail.is_empty() {
        let first = 3 - tail.len().div_ceil(4);
        for (lane, bytes) in tail.chunks(4).enumerate() {
            let mut word = [0; 4];
            word[..bytes.len()].copy_from_slice(bytes);
            lanes[first + lane] = lanes[first + lane].wrapping_add(u32::from_le_bytes(word));
        }
        for (step, angle) in [14, 11, 25, 16, 4, 14, 24].into_iter().enumerate() {
            let target = (step + 2) % 3;
            let previous = (target + 2) % 3;
            lanes[target] =
                (lanes[target] ^ lanes[previous]).wrapping_sub(lanes[previous].rotate_left(angle));
        }
    }
    lanes[2]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observed_export_sequences_are_not_alphabetical() {
        for (input, expected) in [
            ([b"A".as_slice(), b"B", b"C"], [b"B".as_slice(), b"A", b"C"]),
            ([b"Z".as_slice(), b"X", b"Y"], [b"Z".as_slice(), b"Y", b"X"]),
        ] {
            let mut order = NameOrder::deferred(1024, false);
            for name in input {
                order.insert(name);
            }
            assert_eq!(order.names().collect::<Vec<_>>(), expected);
        }
    }

    #[test]
    fn delayed_realization_preserves_growth_deletion_and_snapshot_order() {
        let mut delayed = NameOrder::deferred(16, true);
        let mut eager = NameOrder::new(16, true);
        for n in 0..1100 {
            let name = format!("EXPORT{n:04}");
            delayed.insert(name.as_bytes());
            eager.insert(name.as_bytes());
        }
        for n in (0..1100).step_by(3) {
            let name = format!("EXPORT{n:04}");
            delayed.remove(name.as_bytes());
            eager.remove(name.as_bytes());
        }
        let frozen = delayed.clone();
        assert_eq!(
            delayed.names().collect::<Vec<_>>(),
            eager.names().collect::<Vec<_>>()
        );
        let prior: Vec<_> = frozen.names().map(<[u8]>::to_vec).collect();
        for n in (0..1100).step_by(3).rev() {
            let name = format!("EXPORT{n:04}");
            delayed.insert(name.as_bytes());
            eager.insert(name.as_bytes());
        }
        assert_eq!(
            delayed.names().collect::<Vec<_>>(),
            eager.names().collect::<Vec<_>>()
        );
        assert_eq!(delayed.names().count(), 1100);
        assert_eq!(prior.len(), 733);
        let mut replay = NameOrder::new(16, false);
        delayed.replay_into(&mut replay);
        assert_eq!(
            delayed.names().collect::<Vec<_>>(),
            replay.names().collect::<Vec<_>>()
        );
        assert_eq!(
            frozen.names().map(<[u8]>::to_vec).collect::<Vec<_>>(),
            prior
        );
    }
}
