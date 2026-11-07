// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Invocation-only logical file ages. They never become filesystem timestamps.
use super::*;
const OLD: u8 = 1;
const NEW: u8 = 2;
const DOUBLE_COLON: u8 = 4;
pub(super) enum CachedDoubleColonInput {
    Unvisited(SystemTime),
    Failed,
}

pub(super) struct Files {
    entries: HashMap<Vec<u8>, u8>,
    // These input-only records were published after their declared branches
    // finished. Repeated visits in one goal pass prune to the declared head.
    cached_double_colon_pass: HashMap<Vec<u8>, Option<u64>>,
    old: SystemTime,
    new: SystemTime,
    new_active: bool,
}
impl Files {
    pub(super) fn from_options(options: &Options) -> Result<Option<Box<Self>>, &'static [u8]> {
        if options.old_files.is_empty() && options.new_files.is_empty() {
            return Ok(None);
        }
        // Reserve values far outside GNU's representable filesystem timestamp
        // range while keeping existing comparisons in SystemTime's wider domain.
        let span = Duration::from_secs(1u64 << 60);
        let old = SystemTime::UNIX_EPOCH
            .checked_sub(span)
            .ok_or(b"rmake: assumed-file timestamp range unavailable".as_slice())?;
        let new = SystemTime::UNIX_EPOCH
            .checked_add(span)
            .ok_or(b"rmake: assumed-file timestamp range unavailable".as_slice())?;
        let mut entries = HashMap::new();
        for (paths, bit) in [(&options.old_files, OLD), (&options.new_files, NEW)] {
            for path in paths {
                let name =
                    rmake_core::filenames::without_dot_prefix(path.as_os_str().as_bytes()).to_vec();
                *entries.entry(name).or_insert(0) |= bit;
            }
        }
        Ok(Some(Box::new(Self {
            entries,
            cached_double_colon_pass: HashMap::new(),
            old,
            new,
            new_active: true,
        })))
    }
    pub(super) fn bind(&mut self, database: &mut Database, new_active: bool) {
        self.new_active = new_active;
        for (name, bits) in &mut self.entries {
            if !new_active && *bits & OLD == 0 {
                continue;
            }
            Self::bind_record(database, name, bits);
        }
    }
    fn bind_record(database: &mut Database, name: &[u8], bits: &mut u8) {
        database.file_records.insert(name);
        if database
            .rules
            .get(name)
            .is_some_and(|rule| rule.double_colon.is_some())
        {
            *bits |= DOUBLE_COLON;
        }
    }
    pub(super) fn is_old_time(&self, time: Option<SystemTime>) -> bool {
        time == Some(self.old)
    }
    pub(super) fn is_new_time(&self, time: Option<SystemTime>) -> bool {
        time == Some(self.new)
    }
    fn time(&self, name: &[u8]) -> Option<SystemTime> {
        self.entries.get(name).and_then(|bits| {
            if bits & DOUBLE_COLON != 0 {
                None
            } else if self.new_active && bits & NEW != 0 {
                Some(self.new)
            } else {
                (bits & OLD != 0).then_some(self.old)
            }
        })
    }
    fn ignores_rules(&self, name: &[u8]) -> bool {
        self.entries
            .get(name)
            .is_some_and(|bits| bits & OLD != 0 && bits & DOUBLE_COLON == 0)
    }
    fn has_new_double_colon_entry(&self, name: &[u8]) -> bool {
        self.new_active
            && self
                .entries
                .get(name)
                .is_some_and(|bits| bits & (NEW | DOUBLE_COLON) == (NEW | DOUBLE_COLON))
    }
    fn release_new(&mut self, name: &[u8]) {
        if self.new_active
            && let Some(bits) = self.entries.get_mut(name)
        {
            if *bits & DOUBLE_COLON != 0 {
                return;
            }
            *bits &= !NEW;
            if *bits == 0 {
                self.entries.remove(name);
            }
        }
    }
}
impl Engine {
    pub(super) fn publish_restarted_assumptions(&mut self) {
        let Some(files) = &mut self.assumptions else {
            return;
        };
        files.new_active = true;
        for (name, bits) in &mut files.entries {
            if *bits & NEW == 0 {
                continue;
            }
            Files::bind_record(&mut self.database, name, bits);
            if *bits & DOUBLE_COLON != 0 {
                if self.done.contains_key(name) {
                    files.cached_double_colon_pass.insert(name.clone(), None);
                }
                continue;
            }
            self.path_observations.remove(name);
            // The remake graph can already have updated this file. Retain
            // its completed status and publish the new logical age.
            if let Some(outcome) = self.done.get_mut(name) {
                outcome.time = Some(files.new);
            }
        }
    }

    pub(super) fn assumed_time(&self, name: &[u8]) -> Option<SystemTime> {
        self.assumptions.as_ref().and_then(|files| files.time(name))
    }
    pub(super) fn ignores_file_rules(&self, name: &[u8]) -> bool {
        self.assumptions
            .as_ref()
            .is_some_and(|files| files.ignores_rules(name))
    }
    pub(super) fn new_double_colon_entry(&self, name: &[u8]) -> bool {
        self.assumptions
            .as_ref()
            .is_some_and(|files| files.has_new_double_colon_entry(name))
    }
    pub(super) fn cached_double_colon_input_due(&self, name: &[u8]) -> bool {
        self.assumptions
            .as_ref()
            .and_then(|files| files.cached_double_colon_pass.get(name))
            .is_some_and(|pass| *pass != Some(self.debug_pass))
    }
    pub(super) fn visit_cached_double_colon_input(
        &mut self,
        name: &[u8],
    ) -> Option<CachedDoubleColonInput> {
        let files = self.assumptions.as_mut()?;
        let considered = files.cached_double_colon_pass.get_mut(name)?;
        if *considered == Some(self.debug_pass) {
            return None;
        }
        Some(if considered.replace(self.debug_pass).is_some() {
            CachedDoubleColonInput::Failed
        } else {
            CachedDoubleColonInput::Unvisited(files.new)
        })
    }
    pub(super) fn visited_cached_double_colon_input(&self, name: &[u8]) -> bool {
        self.assumptions
            .as_ref()
            .and_then(|files| files.cached_double_colon_pass.get(name))
            .is_some_and(Option::is_some)
    }
    pub(super) fn release_assumed_new(&mut self, name: &[u8]) {
        if let Some(files) = &mut self.assumptions {
            files.release_new(name);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files(args: &[&str]) -> Box<Files> {
        Files::from_options(&Options::parse(args.iter().map(OsString::from)).unwrap())
            .unwrap()
            .unwrap()
    }

    #[test]
    fn absent_operands_have_no_virtual_file_table() {
        assert!(Files::from_options(&Options::default()).unwrap().is_none());
    }

    #[test]
    fn old_and_new_are_independent_properties_of_the_same_byte_name() {
        let files = files(&["-o./dep", "-Wdep", "-odep"]);
        assert_eq!(files.entries.len(), 1);
        assert!(files.ignores_rules(b"dep"));
        assert_eq!(files.time(b"dep"), Some(files.new));
        assert!(!files.ignores_rules(b"other"));
        assert_eq!(files.time(b"other"), None);
        // The qualified GNU filesystem timestamp domain is much narrower than
        // these logical sentinels. Neither is submitted to the filesystem.
        assert!(files.old < SystemTime::UNIX_EPOCH - Duration::from_secs(1u64 << 34));
        assert!(files.new > SystemTime::UNIX_EPOCH + Duration::from_secs(1u64 << 34));
        assert!(files.is_new_time(Some(files.new)));
        assert!(!files.is_new_time(None));
    }

    #[test]
    fn a_real_update_releases_only_the_new_assumption() {
        let mut files = files(&["-Wnew", "-o./both", "-Wboth", "-oold"]);
        files.release_new(b"new");
        assert_eq!(files.time(b"new"), None);
        files.release_new(b"both");
        assert_eq!(files.time(b"both"), Some(files.old));
        assert!(files.ignores_rules(b"both"));
        files.release_new(b"old");
        assert_eq!(files.time(b"old"), Some(files.old));
    }

    #[test]
    fn double_colon_records_keep_real_times_rules_and_the_new_input_tail() {
        let mut files = files(&["-odep", "-Wdep", "-oold"]);
        let mut database = Database::default();
        database.read(b"dep::;\nold::;\n", b"Makefile").unwrap();
        files.bind(&mut database, true);
        assert_eq!(files.time(b"dep"), None);
        assert!(!files.ignores_rules(b"dep"));
        assert!(!files.ignores_rules(b"old"));
        assert!(files.has_new_double_colon_entry(b"dep"));
        assert!(!files.has_new_double_colon_entry(b"old"));
        files.release_new(b"dep");
        assert!(files.has_new_double_colon_entry(b"dep"));
        assert_eq!(
            database.rules[b"dep".as_slice()]
                .double_colon
                .as_ref()
                .unwrap()
                .len(),
            1
        );
    }
}
