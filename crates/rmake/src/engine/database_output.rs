// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Database diagnostics consume retained state, never repeat interpreter effects.
use super::*;
use rmake_core::database::{
    self, AutomaticVariables, DatabaseView, DirectoryState, DirectoryView, FileStatus, FileTime,
    FileView, HashStats,
};
use rmake_core::variables::Variable;

struct FileSnapshot<'a> {
    view: FileView<'a>,
    timestamp: Option<Vec<u8>>,
    automatic: Option<Vec<(&'a [u8], &'a Variable)>>,
    automatic_stats: HashStats,
}

impl Engine {
    /// Acquiring the post-read special-file policy is one semantic lookup
    /// phase, independent of later cached flag tests during graph traversal.
    pub(super) fn record_database_file_policy(&self) {
        if let Some(history) = self.database.database_history() {
            for name in [
                b".PRECIOUS".as_slice(),
                b".LOW_RESOLUTION_TIME",
                b".PHONY",
                b".NOTINTERMEDIATE",
                b".INTERMEDIATE",
                b".SECONDARY",
                b".EXPORT_ALL_VARIABLES",
                b".IGNORE",
                b".SILENT",
                b".NOTPARALLEL",
            ] {
                history.record_file_lookup(name);
            }
        }
    }

    pub(super) fn print_database(&self) {
        if !self.output.database_requested() || self.database_dumped.replace(true) {
            return;
        }
        self.output.print_compatibility_header(true);
        if crate::native::has_modules() {
            let _ = rmake_linux::native::flush_stream(false);
        }
        let result = self.write_database(&mut io::stdout().lock());
        if let Err(error) = result {
            self.stderr(&[b"write error: ".as_slice(), os_error(&error).as_bytes()].concat());
        }
    }

    fn write_database(&self, output: &mut impl Write) -> io::Result<()> {
        let history = self
            .database
            .database_history()
            .expect("startup database history");
        let mut snapshots = Vec::new();
        for name in self.database.file_records.names() {
            if self.options.no_rules && history.is_builtin_file(name) {
                continue;
            }
            let head = self.database.rules.get(name);
            if let Some(branches) = head.and_then(|rule| rule.double_colon.as_ref()) {
                for (ordinal, rule) in branches.iter().enumerate() {
                    snapshots.push(self.database_file(name, Some(rule), Some(ordinal))?);
                }
            } else {
                snapshots.push(self.database_file(name, head, None)?);
            }
        }
        let files: Vec<_> = snapshots
            .iter()
            .map(|snapshot| {
                let mut view = snapshot.view;
                if let Some(timestamp) = snapshot.timestamp.as_deref() {
                    view.time = FileTime::Formatted(timestamp);
                }
                if let Some(definitions) = snapshot.automatic.as_deref() {
                    view.local_variables = Some(definitions);
                    view.automatic_variables = Some(AutomaticVariables {
                        definitions,
                        stats: snapshot.automatic_stats,
                    });
                }
                view
            })
            .collect();
        let filesystem = self.filesystem.borrow();
        let observations: HashMap<_, _> = filesystem
            .database_directories()
            .map(|directory| (directory.name, directory))
            .collect();
        let directory_names = history.directory_names();
        let directories: Vec<_> = directory_names
            .iter()
            .filter_map(|name| {
                let observation = observations.get(name.as_slice())?;
                Some(DirectoryView {
                    name,
                    state: match (observation.identity, observation.contents) {
                        (None, _) => DirectoryState::Missing,
                        (Some((device, inode)), Some(contents)) if contents.opened => {
                            DirectoryState::Enumerated {
                                device: device as i64,
                                inode: inode as i64,
                                files: contents.files,
                                impossible: self
                                    .impossible_prerequisites
                                    .iter()
                                    .filter(|name| {
                                        Path::new(os(name))
                                            .parent()
                                            .filter(|path| !path.as_os_str().is_empty())
                                            .map_or(b".".as_slice(), |path| {
                                                path.as_os_str().as_bytes()
                                            })
                                            == observation.name
                                    })
                                    .count(),
                                complete: !contents.stream_open,
                            }
                        }
                        (Some((device, inode)), _) => DirectoryState::Unopened {
                            device: device as i64,
                            inode: inode as i64,
                        },
                    },
                })
            })
            .collect();
        let general_vpath: Vec<_> = self.vpath_dirs.iter().map(Vec::as_slice).collect();
        database::write_database_open(output, &rmake_linux::clock::current_ctime()?)?;
        history.with_string_cache(|string_cache| {
            database::write_database_contents(
                output,
                DatabaseView {
                    database: &self.database,
                    variable_stats: history.variable_stats(),
                    directories: &directories,
                    files: &files,
                    file_stats: history.file_stats(),
                    general_vpath: &general_vpath,
                    string_cache,
                },
            )
        })?;
        database::write_database_close(output, &rmake_linux::clock::current_ctime()?)?;
        output.flush()
    }

    fn database_file<'a>(
        &'a self,
        name: &'a [u8],
        rule: Option<&'a Rule>,
        branch: Option<usize>,
    ) -> io::Result<FileSnapshot<'a>> {
        let variables = if let Some(ordinal) = branch {
            self.database_branch_contexts.get(&(name.to_vec(), ordinal))
        } else {
            self.database_contexts.get(name)
        }
        .or_else(|| self.target_contexts.get(name));
        let mut view = FileView::unchecked(name, rule);
        view.double_colon |= branch.is_some();
        view.phony = self.special(b".PHONY", name, false);
        view.precious = self.special_for_rule(b".PRECIOUS", name);
        view.command_line = self.options.goals.iter().any(|goal| goal == name);
        view.dont_care = self
            .database
            .makefiles
            .iter()
            .any(|file| file.name == name && file.optional)
            && !self.database_remake_started.contains(name);
        view.builtin = self
            .database
            .database_history()
            .is_some_and(|history| history.is_builtin_file(name));
        view.implicit_tried = self.database_implicit.contains(name);
        view.stem = rule.and_then(|rule| rule.stem.as_deref()).or_else(|| {
            variables
                .and_then(|variables| variables.get(b"*"))
                .map(|variable| variable.raw_value())
        });
        view.intermediate = self.intermediates.contains(name);
        view.not_intermediate = self.special(b".NOTINTERMEDIATE", name, true);
        view.secondary =
            self.special(b".SECONDARY", name, true) || self.database_secondary.contains(name);
        if let Some(group) = rule.and_then(|rule| rule.group.as_deref()) {
            view.also_make = &group.targets;
        }
        view.target_variables = self.database.target_scopes.get(name);
        let outcome = if let Some(ordinal) = branch {
            self.database_branch_outcomes.get(&(name.to_vec(), ordinal))
        } else {
            self.done
                .get(name)
                .or_else(|| self.database_makefiles.get(name))
        };
        let observed = self.path_observations.get(name);
        let times = self.database_times.borrow();
        // Sequential branches keep separate update results, then share the
        // final file timestamp once the last declared branch has completed.
        let shared_time = branch
            .and_then(|_| self.database.rules.get(name))
            .and_then(|head| head.double_colon.as_ref())
            .filter(|branches| !branches.is_empty())
            .filter(|branches| {
                self.database_branch_outcomes
                    .contains_key(&(name.to_vec(), branches.len() - 1))
            })
            .and_then(|_| self.done.get(name));
        let cached = shared_time
            .or(outcome)
            .and_then(|outcome| outcome.time)
            .or_else(|| {
                observed
                    .and_then(|observation| observation.as_ref())
                    .and_then(|(_, time)| *time)
            })
            .or_else(|| times.get(name).copied().flatten());
        // Invocation-only ages live outside the filesystem timestamp range.
        // Print their logical GNU representation without passing those wide
        // sentinels through libc calendar conversion.
        let unchecked = self.database_unchecked_times.borrow().contains(name);
        let timestamp = if unchecked {
            None
        } else if self
            .assumptions
            .as_ref()
            .is_some_and(|files| files.is_old_time(cached))
        {
            view.time = FileTime::Old;
            None
        } else if self
            .assumptions
            .as_ref()
            .is_some_and(|files| files.is_new_time(cached))
        {
            Some(rmake_linux::clock::file_time(
                SystemTime::UNIX_EPOCH + Duration::new((1u64 << 34) - 1, 107_374_182),
            )?)
        } else {
            cached.map(rmake_linux::clock::file_time).transpose()?
        };
        if !unchecked
            && !matches!(view.time, FileTime::Old)
            && (outcome.is_some() || observed.is_some() || times.contains_key(name) || view.phony)
        {
            view.time = FileTime::Missing;
        }
        if let Some(outcome) = outcome {
            view.updated = true;
            view.status = if outcome.failed {
                FileStatus::Failed
            } else if outcome.question_outdated {
                FileStatus::Question
            } else {
                FileStatus::Success
            };
        } else if self.parallel_jobs.contains_key(name) {
            view.status = FileStatus::Running;
        } else if self.pending_rules.contains_key(name)
            || self.pending_double_colon.contains_key(name)
        {
            view.status = FileStatus::DependenciesRunning;
        }
        let automatic = variables
            .map(|variables| variables.database_file_definitions())
            .or_else(|| {
                view.target_variables
                    .map(|scope| scope.database_definitions().collect())
            });
        let automatic_stats = variables
            .and_then(VariableStore::database_stats)
            .or_else(|| view.target_variables.and_then(TargetScope::database_stats))
            .unwrap_or_default();
        Ok(FileSnapshot {
            view,
            timestamp,
            automatic,
            automatic_stats,
        })
    }
}

pub(super) fn print_startup(host: &startup_exit::Host) {
    if !host.output.database_requested() || host.database_dumped.replace(true) {
        return;
    }
    host.output.print_compatibility_header(true);
    let Some(history) = host.database.database_history() else {
        return;
    };
    let result = (|| {
        let mut output = io::stdout().lock();
        database::write_database_open(&mut output, &rmake_linux::clock::current_ctime()?)?;
        let files: Vec<_> = host
            .database
            .file_records
            .names()
            .map(|name| FileView::unchecked(name, host.database.rules.get(name)))
            .collect();
        history.with_string_cache(|string_cache| {
            database::write_database_contents(
                &mut output,
                DatabaseView {
                    database: &host.database,
                    variable_stats: history.variable_stats(),
                    directories: &[],
                    files: &files,
                    file_stats: history.file_stats(),
                    general_vpath: &[],
                    string_cache,
                },
            )
        })?;
        database::write_database_close(&mut output, &rmake_linux::clock::current_ctime()?)?;
        output.flush()
    })();
    if let Err(error) = result {
        host.output.write_parts(
            true,
            &[
                &host.program,
                b": write error: ",
                os_error(&error).as_bytes(),
                b"\n",
            ],
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine() -> Engine {
        let launch = Engine::invocation(
            [OsString::from("-rR")],
            b"rmake",
            OsStr::new("rmake"),
            None,
            None,
        )
        .unwrap();
        Engine::from_invocation(launch, b"rmake".to_vec(), OsStr::new("rmake")).unwrap()
    }

    #[test]
    fn file_diagnostics_use_retained_times_and_do_not_change_logical_history() {
        let engine = engine();
        let name = b"/rmake-database-unit/never-created";
        let time = SystemTime::UNIX_EPOCH + Duration::new(1_600_000_000, 123_456_789);
        engine
            .database_times
            .borrow_mut()
            .insert(name.to_vec(), Some(time));
        let history = engine.database.database_history().unwrap();
        let before = (
            history.variable_stats(),
            history.file_stats(),
            engine.filesystem.borrow().generation(),
        );
        let snapshot = engine.database_file(name, None, None).unwrap();
        assert!(snapshot.timestamp.unwrap().ends_with(b".123456789"));
        assert!(!snapshot.view.updated);
        let after = (
            history.variable_stats(),
            history.file_stats(),
            engine.filesystem.borrow().generation(),
        );
        assert_eq!(before, after);
    }

    #[test]
    fn logical_assumed_ages_never_enter_wide_libc_calendar_conversion() {
        let mut engine = engine();
        engine.options.old_files = vec!["old".into()];
        engine.options.new_files = vec!["new".into()];
        engine.assumptions = assumptions::Files::from_options(&engine.options).unwrap();
        for name in [b"old".as_slice(), b"new"] {
            engine
                .database_times
                .borrow_mut()
                .insert(name.to_vec(), engine.assumed_time(name));
        }
        let old = engine.database_file(b"old", None, None).unwrap();
        assert!(matches!(old.view.time, FileTime::Old));
        assert!(old.timestamp.is_none());
        let new = engine.database_file(b"new", None, None).unwrap();
        assert!(new.timestamp.unwrap().ends_with(b".107374182"));
    }

    #[test]
    fn double_colon_views_borrow_their_own_completed_local_values() {
        let mut engine = engine();
        let mut head = engine.database.variables.with_target_scopes(&[]);
        head.define_literal(b"@", b"first", Origin::Automatic)
            .unwrap();
        head.define_literal(b"*", b"first-stem", Origin::Automatic)
            .unwrap();
        let mut branch = head.fresh_double_colon_scope();
        branch
            .define_literal(b"@", b"second", Origin::Automatic)
            .unwrap();
        branch
            .define_literal(b"*", b"second-stem", Origin::Automatic)
            .unwrap();
        engine.retain_database_context(b"target", Some(0), &head);
        engine.retain_database_context(b"target", Some(1), &branch);
        for (ordinal, expected, stem) in [
            (0, b"first".as_slice(), b"first-stem".as_slice()),
            (1, b"second", b"second-stem"),
        ] {
            let snapshot = engine
                .database_file(b"target", None, Some(ordinal))
                .unwrap();
            assert!(snapshot.view.double_colon);
            assert_eq!(snapshot.view.stem, Some(stem));
            assert_eq!(
                snapshot
                    .automatic
                    .unwrap()
                    .iter()
                    .find(|(name, _)| *name == b"@")
                    .unwrap()
                    .1
                    .raw_value(),
                expected
            );
        }
    }

    #[test]
    fn failed_branch_views_keep_separate_results_and_unqueried_command_time() {
        let mut engine = engine();
        engine.database_branch_outcomes.insert(
            (b"target".to_vec(), 0),
            Outcome {
                ran: true,
                failed: true,
                ..Outcome::default()
            },
        );
        engine.database_branch_outcomes.insert(
            (b"target".to_vec(), 1),
            Outcome {
                ran: true,
                ..Outcome::default()
            },
        );
        engine
            .database_times
            .borrow_mut()
            .insert(b"target".to_vec(), None);
        engine
            .database_unchecked_times
            .borrow_mut()
            .insert(b"target".to_vec());
        for ordinal in 0..2 {
            let snapshot = engine
                .database_file(b"target", None, Some(ordinal))
                .unwrap();
            assert!(snapshot.view.updated);
            assert!(matches!(snapshot.view.time, FileTime::Unchecked));
            assert!(matches!(
                (ordinal, snapshot.view.status),
                (0, FileStatus::Failed) | (1, FileStatus::Success)
            ));
        }
        assert!(
            engine
                .database_unchecked_times
                .borrow()
                .contains(b"target".as_slice())
        );
    }

    #[test]
    fn declaration_only_target_scope_retains_its_actual_table_statistics() {
        let mut engine = engine();
        engine
            .database
            .read(b"all: private DB_SCOPE = retained\nall:;\n", b"scope.mk")
            .unwrap();
        let snapshot = engine
            .database_file(b"all", engine.database.rules.get(b"all".as_slice()), None)
            .unwrap();
        assert!(
            snapshot
                .automatic
                .unwrap()
                .iter()
                .any(|(name, _)| *name == b"DB_SCOPE")
        );
        assert_eq!(snapshot.automatic_stats.filled, 1);
        assert!(!snapshot.view.updated);
    }

    #[test]
    fn publishing_flags_keeps_recursive_payloads_raw_and_preserves_remake_debug_mask() {
        let mut engine = engine();
        engine
            .database
            .variables
            .define(
                b"MAKEOVERRIDES",
                b"$(error publication-expanded-user-code)",
                Assignment::Recursive,
                Origin::File,
            )
            .unwrap();
        engine.options.debug_mask = crate::debug::Mask::BASIC;
        engine.output.set_debug_mask(crate::debug::Mask::NONE);
        engine.publish_phase_makeflags(false).unwrap();
        assert_eq!(engine.output.debug_mask(), crate::debug::Mask::NONE);
        assert!(
            engine
                .database
                .variables
                .get(b"MAKEFLAGS")
                .unwrap()
                .raw_value()
                .ends_with(b" -- $(MAKEOVERRIDES)")
        );
    }

    #[test]
    fn optional_reader_status_retires_only_after_the_actual_remake_phase() {
        let mut engine = engine();
        engine.database.makefiles.push(Makefile {
            name: b"missing.mk".to_vec(),
            optional: true,
            missing: true,
            read_error: None,
        });
        assert!(
            engine
                .database_file(b"missing.mk", None, None)
                .unwrap()
                .view
                .dont_care
        );
        engine
            .database_remake_started
            .insert(b"missing.mk".to_vec());
        engine.database_makefiles.insert(
            b"missing.mk".to_vec(),
            Outcome {
                failed: true,
                ..Outcome::default()
            },
        );
        let snapshot = engine.database_file(b"missing.mk", None, None).unwrap();
        assert!(!snapshot.view.dont_care);
        assert!(snapshot.view.updated);
        assert!(matches!(snapshot.view.status, FileStatus::Failed));
    }
}
