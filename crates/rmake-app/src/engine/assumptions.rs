// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Invocation-only logical file ages. They never become filesystem timestamps.
use super::*;
const OLD: u8 = 1;
const DOUBLE_COLON: u8 = 4;
pub(super) enum CachedDoubleColonInput {
    Unvisited(SystemTime),
    Failed,
}

#[derive(Default)]
struct Entry {
    bits: u8,
    new_count: usize,
}

pub(super) struct Files {
    entries: HashMap<Vec<u8>, Entry>,
    // These input-only records were published after their declared branches
    // finished, or failed after an ordinary visit. Repeated visits in one
    // goal pass prune to the declared head.
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
        let mut entries: HashMap<_, Entry> = HashMap::new();
        for (paths, new) in [(&options.old_files, false), (&options.new_files, true)] {
            for path in paths {
                let name =
                    rmake_core::filenames::without_dot_prefix(path.as_os_str().as_bytes()).to_vec();
                let entry = entries.entry(name).or_default();
                if new {
                    entry.new_count += 1;
                } else {
                    entry.bits |= OLD;
                }
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
        for (name, entry) in &mut self.entries {
            if !new_active && entry.bits & OLD == 0 {
                continue;
            }
            Self::bind_record(database, name, &mut entry.bits);
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
        self.entries.get(name).and_then(|entry| {
            if entry.bits & DOUBLE_COLON != 0 {
                None
            } else if self.new_active && entry.new_count != 0 {
                Some(self.new)
            } else {
                (entry.bits & OLD != 0).then_some(self.old)
            }
        })
    }
    fn ignores_rules(&self, name: &[u8]) -> bool {
        self.entries
            .get(name)
            .is_some_and(|entry| entry.bits & OLD != 0 && entry.bits & DOUBLE_COLON == 0)
    }
    fn new_double_colon_entry_count(&self, name: &[u8]) -> usize {
        self.entries.get(name).map_or(0, |entry| {
            if self.new_active && entry.bits & DOUBLE_COLON != 0 {
                entry.new_count
            } else {
                0
            }
        })
    }
    fn release_new(&mut self, name: &[u8]) {
        if self.new_active
            && let Some(entry) = self.entries.get_mut(name)
        {
            if entry.bits & DOUBLE_COLON != 0 {
                return;
            }
            entry.new_count = 0;
            if entry.bits == 0 {
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
        for (name, entry) in &mut files.entries {
            if entry.new_count == 0 {
                continue;
            }
            Files::bind_record(&mut self.database, name, &mut entry.bits);
            if entry.bits & DOUBLE_COLON != 0 {
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
    pub(super) fn new_double_colon_entry_count(&self, name: &[u8]) -> usize {
        let count = self
            .assumptions
            .as_ref()
            .map_or(0, |files| files.new_double_colon_entry_count(name));
        if count > 1
            && self.options.question
            && !self.options.keep_going
            && (self.remaking || self.active_makefile_remake.is_some())
        {
            // Ordinary goals can search later entries until their first
            // query verdict. Keep the cap for makefile remake traversal.
            1
        } else {
            count
        }
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
    pub(super) fn forget_cached_double_colon_input(&mut self, name: &[u8]) {
        if let Some(files) = &mut self.assumptions {
            files.cached_double_colon_pass.remove(name);
        }
    }

    pub(super) fn remember_failed_double_colon_input(&mut self, name: &[u8]) {
        if let Some(files) = &mut self.assumptions {
            files
                .cached_double_colon_pass
                .insert(name.to_vec(), Some(self.debug_pass));
        }
    }

    pub(super) fn visited_cached_double_colon_input(&self, name: &[u8]) -> bool {
        if self
            .pending_double_colon
            .get(name)
            .is_some_and(|state| state.synthetic_tail.is_some())
        {
            return false;
        }
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
    fn double_colon_records_keep_real_times_rules_and_the_new_input_tails() {
        let mut files = files(&["-odep", "-Wdep", "-W./dep", "-oold"]);
        let mut database = Database::default();
        database.read(b"dep::;\nold::;\n", b"Makefile").unwrap();
        files.bind(&mut database, true);
        assert_eq!(files.time(b"dep"), None);
        assert!(!files.ignores_rules(b"dep"));
        assert!(!files.ignores_rules(b"old"));
        assert_eq!(files.new_double_colon_entry_count(b"dep"), 2);
        assert_eq!(files.new_double_colon_entry_count(b"old"), 0);
        files.release_new(b"dep");
        assert_eq!(files.new_double_colon_entry_count(b"dep"), 2);
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
