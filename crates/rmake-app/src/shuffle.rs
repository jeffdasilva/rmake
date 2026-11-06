// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Owned traversal permutations. Declaration order remains available to autos.
use crate::cli::unsigned_option;
use rmake_core::parser::{Database, Rule};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Mode {
    #[default]
    None,
    Identity,
    Reverse,
    Seed(u32),
}

impl Mode {
    pub fn parse(raw: &[u8]) -> Result<Self, Vec<u8>> {
        let raw = raw.split(|b| *b == 0).next().unwrap_or_default();
        for (word, mode) in [
            (b"none".as_slice(), Self::None),
            (b"identity", Self::Identity),
            (b"reverse", Self::Reverse),
        ] {
            if raw.eq_ignore_ascii_case(word) {
                return Ok(mode);
            }
        }
        if raw.eq_ignore_ascii_case(b"random") {
            let mut random = Random(0);
            return Ok(Self::Seed(random.next()));
        }
        unsigned_option(raw).map(Self::Seed).ok_or_else(|| {
            [
                b"invalid shuffle mode: ".as_slice(),
                if raw.is_empty() {
                    b"Missing value"
                } else {
                    b"Invalid value"
                },
                b": '",
                raw,
                b"'",
            ]
            .concat()
        })
    }

    pub fn spelling(self) -> Option<Vec<u8>> {
        match self {
            Self::None => None,
            Self::Identity => Some(b"identity".to_vec()),
            Self::Reverse => Some(b"reverse".to_vec()),
            Self::Seed(value) => Some(value.to_string().into_bytes()),
        }
    }
}

struct Random(u32);
impl Random {
    fn next(&mut self) -> u32 {
        if self.0 == 0 {
            let seconds = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            self.0 = ((seconds ^ u64::from(std::process::id())) as u32).wrapping_add(1);
        }
        // The specified 32-bit xor/shift linear transform, expressed as a
        // sequence of signed bit distances rather than native library rand().
        for distance in [13_i32, -17, 5] {
            self.0 ^= if distance > 0 {
                self.0.wrapping_shl(distance as u32)
            } else {
                self.0.wrapping_shr(distance.unsigned_abs())
            };
        }
        self.0
    }
}

#[derive(Default)]
pub(crate) struct Planner(Option<Box<Enabled>>);
struct Enabled {
    mode: Mode,
    random: Random,
    suffix: Vec<u8>,
    // Presence records visited files, including files with no prerequisites.
    orders: HashMap<Vec<u8>, Vec<usize>>,
}

impl Planner {
    pub fn new(mode: Mode) -> Self {
        Self(mode.spelling().map(|text| {
            Box::new(Enabled {
                mode,
                random: Random(0),
                suffix: [b" shuffle=".as_slice(), &text].concat(),
                orders: HashMap::new(),
            })
        }))
    }
    pub fn enabled(&self) -> bool {
        self.0.is_some()
    }
    pub fn suffix(&self) -> &[u8] {
        self.0.as_ref().map_or(b"", |s| &s.suffix)
    }
    pub fn start(&mut self, goals: &mut Vec<Vec<u8>>, database: &Database, disabled: bool) {
        let Some(state) = self.0.as_mut().filter(|_| !disabled) else {
            return;
        };
        state.reseed();
        let order = state.indices(goals.len());
        state.walk(goals.iter().map(Vec::as_slice), database);
        let original = std::mem::take(goals);
        let mut slots: Vec<_> = original.into_iter().map(Some).collect();
        goals.extend(
            order
                .into_iter()
                .map(|i| slots[i].take().expect("permutation is unique")),
        );
    }
    pub fn order(&self, name: &[u8], len: usize) -> Vec<usize> {
        self.0
            .as_ref()
            .and_then(|s| s.orders.get(name))
            .filter(|order| order.len() == len)
            .cloned()
            .unwrap_or_default()
    }
    pub fn regenerate(&mut self, name: &[u8], rule: &Rule, database: &Database, disabled: bool) {
        let Some(state) = self.0.as_mut().filter(|_| !disabled) else {
            return;
        };
        state.reseed();
        let order = state.rule_indices(name, rule, database);
        state.orders.insert(name.to_vec(), order);
        state.walk(children(rule), database);
    }
    pub fn discard(&mut self, name: &[u8], index: usize) {
        if let Some(order) = self.0.as_mut().and_then(|s| s.orders.get_mut(name)) {
            order.retain(|i| *i != index);
            for i in order {
                if *i > index {
                    *i -= 1;
                }
            }
        }
    }
}

fn children(rule: &Rule) -> impl DoubleEndedIterator<Item = &[u8]> {
    rule.dependencies
        .iter()
        .enumerate()
        .filter(|(index, dep)| {
            dep.name != b".WAIT"
                && rule
                    .secondary
                    .as_ref()
                    .is_none_or(|clauses| !clauses.iter().any(|clause| clause.index == *index))
        })
        .map(|(_, dep)| dep.name.as_slice())
}

impl Enabled {
    fn reseed(&mut self) {
        if let Mode::Seed(seed) = self.mode {
            self.random = Random(seed);
        }
    }
    fn indices(&mut self, len: usize) -> Vec<usize> {
        let mut order: Vec<_> = (0..len).collect();
        match self.mode {
            Mode::Reverse => order.reverse(),
            Mode::Seed(_) if len != 0 => {
                for slot in 0..len {
                    let partner = self.random.next() as usize % len;
                    order.swap(slot, partner);
                }
            }
            _ => {}
        }
        order
    }
    fn rule_indices(&mut self, name: &[u8], rule: &Rule, db: &Database) -> Vec<usize> {
        let count = rule
            .dependencies
            .iter()
            .filter(|d| d.name != b".WAIT")
            .count();
        let targeted = count > 1
            && db
                .rules
                .get(b".NOTPARALLEL".as_slice())
                .is_some_and(|r| r.dependencies.iter().any(|d| d.name == name));
        let wait = rule
            .dependencies
            .iter()
            .rev()
            .skip_while(|d| d.name == b".WAIT")
            .any(|d| d.name == b".WAIT");
        if targeted || wait {
            return Vec::new();
        }
        let mut order = self.indices(count);
        // A trailing .WAIT has no following dependency to constrain. Keep the
        // interpreter's marker after the permuted actual occurrences.
        order.extend(count..rule.dependencies.len());
        if order.iter().copied().eq(0..order.len()) {
            Vec::new()
        } else {
            order
        }
    }
    fn walk<'a>(&mut self, roots: impl IntoIterator<Item = &'a [u8]>, db: &'a Database) {
        let mut stack: Vec<_> = roots.into_iter().collect();
        stack.reverse();
        while let Some(name) = stack.pop() {
            if self.orders.contains_key(name) {
                continue;
            }
            let rule = db.rules.get(name).map(|r| {
                r.double_colon
                    .as_ref()
                    .and_then(|branches| branches.first())
                    .unwrap_or(r)
            });
            let order = rule.map_or_else(Vec::new, |r| self.rule_indices(name, r, db));
            self.orders.insert(name.to_vec(), order);
            if let Some(rule) = rule {
                stack.extend(children(rule).rev());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn seed_conversion_retains_linux_unsigned_boundaries_and_raw_errors() {
        for (raw, expected) in [
            (b"+0001".as_slice(), 1),
            (b"-1", u32::MAX),
            (b"4294967297", 1),
            (b"18446744073709551616", u32::MAX),
            (b"-18446744073709551616", u32::MAX),
            (b" \x0b2", 2),
        ] {
            assert_eq!(Mode::parse(raw), Ok(Mode::Seed(expected)));
        }
        assert_eq!(Mode::parse(b"ReVeRsE"), Ok(Mode::Reverse));
        assert_eq!(Mode::parse(b"NONE").unwrap().spelling(), None);
        assert_eq!(
            Mode::parse(b""),
            Err(b"invalid shuffle mode: Missing value: ''".to_vec())
        );
        assert_eq!(
            Mode::parse(b"bad\xff"),
            Err(b"invalid shuffle mode: Invalid value: 'bad\xff'".to_vec())
        );
    }
}
