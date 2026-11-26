// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Database diagnostics consume retained state, never repeat interpreter effects.
use super::*;
use rmake_core::database::{
    self, AutomaticVariables, DatabaseView, DirectoryState, DirectoryView, FileStatus, FileTime,
    FileView, HashStats,
};
use rmake_core::file_records::FileRecordEntry;
use rmake_core::variables::Variable;

const PRESENTATION: database::Presentation<'static> = database::Presentation::localized(
    &rmake_linux::database_numeric::Formatting,
    rmake_linux::messages::lookup_static,
);

struct FileSnapshot<'a> {
    view: FileView<'a>,
    timestamp: Option<Vec<u8>>,
    also_make: Vec<&'a [u8]>,
    automatic: Option<Vec<(&'a [u8], &'a Variable)>>,
    automatic_stats: HashStats,
}

struct ExpandedPrerequisiteRecord {
    dependencies: Option<Vec<Dependency>>,
    explicit_stem_prepared: bool,
}

/// A recipe can request `-p` after prerequisite expansion. Preserve the
/// visited record so printing never expands the rule or repeats effects.
#[derive(Default)]
pub(super) struct ExpandedPrerequisites {
    targets: HashMap<Vec<u8>, ExpandedPrerequisiteRecord>,
    branches: HashMap<Vec<u8>, HashMap<usize, ExpandedPrerequisiteRecord>>,
}

impl ExpandedPrerequisites {
    fn get(&self, target: &[u8], branch: Option<usize>) -> Option<&ExpandedPrerequisiteRecord> {
        if let Some(ordinal) = branch {
            self.branches.get(target)?.get(&ordinal)
        } else {
            self.targets.get(target)
        }
    }
}

impl database::FileNames for Engine {
    fn resolve<'a>(&'a self, name: &'a [u8]) -> &'a [u8] {
        self.resolved_name_ref(name)
    }
}

impl Engine {
    pub(super) fn retain_expanded_prerequisites(
        &mut self,
        target: &[u8],
        branch: Option<usize>,
        dependencies: Option<&[Dependency]>,
        explicit_stem_prepared: bool,
    ) {
        if dependencies.is_none() && !explicit_stem_prepared {
            return;
        }
        let captured = self
            .database_expanded_prerequisites
            .get_or_insert_with(Box::default);
        let record = ExpandedPrerequisiteRecord {
            dependencies: dependencies.map(<[Dependency]>::to_vec),
            explicit_stem_prepared,
        };
        if let Some(ordinal) = branch {
            captured
                .branches
                .entry(target.to_vec())
                .or_default()
                .insert(ordinal, record);
        } else {
            captured.targets.insert(target.to_vec(), record);
        }
    }

    pub(super) fn expanded_prerequisites_mut(
        &mut self,
        target: &[u8],
        branch: Option<usize>,
    ) -> Option<&mut Vec<Dependency>> {
        let captured = self.database_expanded_prerequisites.as_mut()?;
        let record = if let Some(ordinal) = branch {
            captured.branches.get_mut(target)?.get_mut(&ordinal)
        } else {
            captured.targets.get_mut(target)
        }?;
        record.dependencies.as_mut()
    }

    pub(super) fn database_stem_prepared(&self, target: &[u8], branch: Option<usize>) -> bool {
        self.database_expanded_prerequisites
            .as_ref()
            .and_then(|captured| captured.get(target, branch))
            .is_some_and(|record| record.explicit_stem_prepared)
    }

    pub(super) fn record_explicit_stem_lookup(&mut self) {
        if let Some(history) = self.database.database_history() {
            history.intern(b".SUFFIXES");
        }
        self.database.file_records.insert(b".SUFFIXES");
    }

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
        let mut output = rmake_linux::output::Stdout::default();
        let result = self.write_database(&mut output);
        if let Err(error) = result
            && !output.failed()
        {
            self.stderr(&[b"write error: ".as_slice(), os_error(&error).as_bytes()].concat());
        }
    }

    fn recreated_visited_goal_source(&self, entry: FileRecordEntry<'_>, canonical: &[u8]) -> bool {
        if self.merged_goal_reports.is_empty() || entry.lookup_name == canonical {
            return false;
        }
        // The queue still names the retired source. The file table can now
        // hold a different owner at the same spelling.
        self.database.file_records.moved_lookup_id_at_snapshot(entry.lookup_name)
            .is_some_and(|queued| {
                queued != entry.id
                    && matches!(self.merged_goal_reports.get(&queued),
                                Some(MergedGoalReport::CompletedVisitedSource(name)) if name == canonical)
            })
    }

    fn write_database(&self, output: &mut impl Write) -> io::Result<()> {
        let history = self
            .database
            .database_history()
            .expect("startup database history");
        let moved_original_names: HashSet<_> = self
            .moved_archives
            .values()
            .map(|owner| owner.original.as_slice())
            .collect();
        let archive_reads = self.archive_vpath_reads.borrow();
        let permanent_archive_original_names: HashSet<_> = archive_reads
            .iter()
            .flat_map(|reads| reads.permanent_containers.values().map(Vec::as_slice))
            .collect();
        let mut snapshots = Vec::new();
        for entry in self.database.file_records.entries() {
            let moved = self.moved_archives.get(&entry.id);
            let name = if moved.is_some() {
                entry.lookup_name
            } else {
                entry.logical_name
            };
            if self.options.no_rules && history.is_builtin_file(name) {
                continue;
            }
            let head = self.database.rules.get(name);
            if let Some(branches) = head.and_then(|rule| rule.double_colon.as_ref()) {
                for (ordinal, rule) in branches.iter().enumerate() {
                    snapshots.push(self.database_file(name, Some(rule), Some(ordinal))?);
                }
            } else {
                let mut snapshot = self.database_file(name, head, None)?;
                if snapshot.view.updated
                    && matches!(
                        self.merged_goal_reports.get(&entry.id),
                        Some(MergedGoalReport::UnattemptedDestination)
                    )
                {
                    // Rehash copied the completed source's updated flag and
                    // locals, leaving the destination's result and stem intact.
                    snapshot.view.stem = None;
                    snapshot.view.status = FileStatus::Unattempted;
                }
                if let Some(owner) = moved {
                    snapshot.view.name = entry.logical_name;
                    snapshot.view.command_line = owner.command_goal;
                    if let Some(variables) = snapshot.view.local_scope {
                        snapshot.automatic =
                            Some(variables.database_file_definitions_with_scope(
                                snapshot.view.target_variables,
                            ));
                    }
                } else {
                    let canonical = self.aliases.resolve(name);
                    if permanent_archive_original_names.contains(name)
                        || self.recreated_visited_goal_source(entry, canonical)
                        || self.database.is_recreated_file_head_name(name, canonical)
                    {
                        snapshot.view.command_line = false;
                        snapshot.view.preserve_name = true;
                    } else if moved_original_names.contains(name) {
                        snapshot.view.command_line = false;
                    }
                }
                snapshots.push(snapshot);
            }
        }
        let files: Vec<_> = snapshots
            .iter()
            .map(|snapshot| {
                let mut view = snapshot.view;
                view.also_make = &snapshot.also_make;
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
        PRESENTATION.open(output, &rmake_linux::clock::current_ctime()?)?;
        history.with_string_cache(|string_cache| {
            PRESENTATION.contents(
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
                Some(self),
            )
        })?;
        PRESENTATION.close(output, &rmake_linux::clock::current_ctime()?)?;
        output.flush()
    }

    fn database_file<'a>(
        &'a self,
        name: &'a [u8],
        rule: Option<&'a Rule>,
        branch: Option<usize>,
    ) -> io::Result<FileSnapshot<'a>> {
        let is_head = branch.is_none_or(|ordinal| ordinal == 0);
        let branch_key = branch.map(|ordinal| (name.to_vec(), ordinal));
        let detached_head = is_head && self.database.has_detached_file_head(name);
        let variables = if detached_head {
            // The canonical hash entry owns a different table from the head
            // selected by eager declarations.
            None
        } else if let Some(key) = &branch_key {
            self.database_branch_contexts.get(key)
        } else {
            self.database_contexts.get(name)
        }
        .or_else(|| {
            if is_head && !detached_head {
                self.target_contexts.get(name)
            } else {
                None
            }
        });
        let mut view = FileView::unchecked(name, rule);
        if rule.is_some_and(|rule| rule.secondary.is_some())
            && let Some(captured) = &self.database_expanded_prerequisites
        {
            view.expanded_prerequisites = captured
                .get(name, branch)
                .and_then(|record| record.dependencies.as_deref());
        }
        view.double_colon |= branch.is_some();
        view.phony = self.special(b".PHONY", name, false);
        view.precious = self.special_for_rule(b".PRECIOUS", name);
        view.command_line = is_head
            && self
                .options
                .goals
                .iter()
                .any(|goal| self.resolved_name_ref(goal) == self.resolved_name_ref(name));
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
        view.implicit_tried = self.database_implicit.contains(name)
            || branch_key
                .as_ref()
                .is_some_and(|key| self.database_branch_implicit.contains(key));
        view.stem = rule.and_then(|rule| rule.stem.as_deref()).or_else(|| {
            variables
                .and_then(|variables| variables.get(b"*"))
                .map(|variable| variable.raw_value())
        });
        view.intermediate = self.intermediates.contains(name);
        view.not_intermediate = self.special(b".NOTINTERMEDIATE", name, true);
        let named_secondary = self.special(b".SECONDARY", name, false);
        view.secondary = named_secondary
            || match &branch_key {
                Some(key) => self.database_branch_secondary.contains(key),
                None => self.database_secondary.contains(name),
            };
        let mut also_make = Vec::new();
        if let Some(rule) = rule
            && let Some(group) = rule.group.as_deref()
        {
            also_make.extend(
                group
                    .targets
                    .iter()
                    .rev()
                    .filter(|peer| {
                        rule.implicit_pattern.is_none()
                            || self.resolved_name_ref(peer) != self.resolved_name_ref(name)
                    })
                    .map(Vec::as_slice),
            );
        }
        if is_head {
            view.target_variables = self.database.target_scopes.get(name);
        }
        let outcome = if let Some(key) = &branch_key {
            self.database_branch_outcomes.get(key)
        } else {
            self.file_traversals.outcome(name)
        };
        let observed = self.path_observations.get(name);
        let times = self.database_times.borrow();
        // Sequential branches keep separate update results, then share the
        // final file timestamp once the last declared branch has completed.
        let shared_time = branch
            .and_then(|_| self.database.rules.get(name))
            .and_then(|head| head.double_colon.as_ref())
            .filter(|branches| branches.len() > 1)
            .filter(|branches| {
                self.database_branch_outcomes
                    .contains_key(&(name.to_vec(), branches.len() - 1))
            })
            .and_then(|_| self.file_traversals.outcome(name));
        let timestamp_outcome = if detached_head {
            self.file_traversals.outcome(name).or(outcome)
        } else {
            shared_time.or(outcome)
        };
        let peer_time =
            branch.and_then(|ordinal| self.database_group_peer_times.get(name)?.get(ordinal));
        let (database_result, cached) = if let Some(GroupPeerTime::Observed(time)) = peer_time {
            (DatabaseResult::ObservedGroup, *time)
        } else {
            let result = timestamp_outcome
                .map_or(DatabaseResult::Outcome, |outcome| outcome.database_result);
            let retained = || times.get(name).copied().flatten();
            let outcome_time = || timestamp_outcome.and_then(|outcome| outcome.time);
            let cached = match result {
                _ if detached_head => outcome_time().or_else(retained),
                DatabaseResult::ObservedGroup if branch.is_some() => outcome_time(),
                DatabaseResult::ObservedGroup | DatabaseResult::ObservedDoubleColon => retained(),
                _ => outcome_time()
                    .or_else(|| {
                        observed
                            .and_then(|observation| observation.as_ref())
                            .and_then(|(_, time)| *time)
                    })
                    .or_else(retained),
            };
            (result, cached)
        };
        // Invocation-only ages live outside the filesystem timestamp range.
        // Print their logical GNU representation without passing those wide
        // sentinels through libc calendar conversion.
        let unchecked = matches!(peer_time, Some(GroupPeerTime::Unchecked))
            || (!matches!(
                database_result,
                DatabaseResult::ObservedGroup
                    | DatabaseResult::ObservedDoubleColon
                    | DatabaseResult::SimulatedRecipeCompletion
            ) && self.database_unchecked_times.borrow().contains(name));
        let timestamp = if unchecked {
            None
        } else if self
            .assumptions
            .as_ref()
            .is_some_and(|files| files.is_old_time(cached))
        {
            view.time = FileTime::Old;
            None
        } else if database_result.has_simulated_time()
            || self
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
        // Secondary declarations mark the read-time file records as
        // intermediate, with empty .SECONDARY reaching only each head.
        // The default record is registered at startup without a graph rule.
        // Built-in suffix transformations enter the file table later.
        let globally_intermediate = is_head
            && !view.not_intermediate
            && self.intermediate_policy.all_secondary
            && if view.builtin {
                name == b".SUFFIXES"
                    || rmake_core::parser::words(rmake_core::parser::DEFAULT_SUFFIXES)
                        .any(|suffix| suffix == name)
            } else {
                name == b".DEFAULT" || self.intermediate_policy.global_intermediate(name)
            };
        // An unvisited record or a query that observed no file retains
        // its intermediate marker.
        // In grouped rules the reported time can stay unchecked after
        // a recipe changes the file, so use the update outcome as well.
        if (named_secondary || globally_intermediate)
            && ((outcome.is_none() && matches!(view.time, FileTime::Unchecked))
                || (self.options.question
                    && timestamp.is_none()
                    && matches!(view.time, FileTime::Missing)
                    && outcome.is_none_or(|outcome| !outcome.ran)))
        {
            view.intermediate = true;
        }
        if let Some(outcome) = outcome {
            if outcome.database_result == DatabaseResult::MergedVisitedFile {
                // The destination's own stem is separate from imported $*.
                view.stem = None;
            }
            view.updated = !matches!(
                outcome.database_result,
                DatabaseResult::UnfinishedMissingRule | DatabaseResult::UnfinishedRecipeExpansion
            );
            view.status = if !view.updated {
                FileStatus::Unattempted
            } else if detached_head
                || outcome.database_result == DatabaseResult::MergedArchiveContainer
            {
                // Rehash copies the updated flag onto the unvisited physical
                // destination and leaves its update result untouched.
                if outcome.database_result == DatabaseResult::MergedArchiveContainer {
                    // Its file stem is separate from the source's recipe locals.
                    view.stem = None;
                }
                FileStatus::Unattempted
            } else if outcome.failed {
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
            .map(|variables| {
                if branch.is_none() {
                    variables.database_file_definitions_with_scope(view.target_variables)
                } else {
                    variables.database_file_definitions()
                }
            })
            .or_else(|| {
                view.target_variables
                    .map(|scope| scope.database_definitions().collect())
            });
        view.local_scope = variables;
        let automatic_stats = variables
            .and_then(VariableStore::database_stats)
            .or_else(|| view.target_variables.and_then(TargetScope::database_stats))
            .unwrap_or_default();
        Ok(FileSnapshot {
            view,
            timestamp,
            also_make,
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
    let mut output = rmake_linux::output::Stdout::default();
    let result = (|| {
        PRESENTATION.open(&mut output, &rmake_linux::clock::current_ctime()?)?;
        let files: Vec<_> = host
            .database
            .file_records
            .names()
            .map(|name| FileView::unchecked(name, host.database.rules.get(name)))
            .collect();
        history.with_string_cache(|string_cache| {
            PRESENTATION.contents(
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
                None,
            )
        })?;
        PRESENTATION.close(&mut output, &rmake_linux::clock::current_ctime()?)?;
        output.flush()
    })();
    if let Err(error) = result
        && !output.failed()
    {
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
    fn accepted_intermediate_search_is_retained_before_its_recipe_runs() {
        for (rules, children) in [
            ("%.out: marker.in ;\n", &[][..]),
            ("%.out: %.mid ;\n%.mid: marker.in ;\n", &["marker.mid"][..]),
            (
                "%.out: %.tmp ;\n%.tmp: %.mid ;\n%.mid: marker.in ;\n",
                &["marker.mid", "marker.tmp"][..],
            ),
        ] {
            let mut engine = engine();
            let source = format!("marker.in: ;\n{rules}");
            engine
                .database
                .read(source.as_bytes(), b"Makefile")
                .unwrap();
            let plan = engine
                .plan_implicit_update(b"marker.out", 0, true, None)
                .unwrap()
                .unwrap();
            for (index, name) in children.iter().enumerate() {
                assert_eq!(
                    engine.database_implicit.contains(name.as_bytes()),
                    index + 1 < children.len(),
                    "{rules}: {name}",
                );
            }
            engine.install_implicit_plan(plan, ImplicitPlacement::Root);
            for name in std::iter::once("marker.out").chain(children.iter().copied()) {
                let snapshot = engine
                    .database_file(
                        name.as_bytes(),
                        engine.database.rules.get(name.as_bytes()),
                        None,
                    )
                    .unwrap();
                assert!(snapshot.view.implicit_tried, "{rules}: {name}");
                assert!(!snapshot.view.updated);
                assert!(engine.file_traversals.outcome(name.as_bytes()).is_none());
            }
            assert!(!engine.database_implicit.contains(b"marker.in".as_slice()));
        }
    }

    #[test]
    fn rejected_parent_does_not_retain_a_successful_speculative_child_search() {
        let mut engine = engine();
        engine
            .database
            .read(
                b"marker.in: ;\n\
                  %.out: %.mid %.absent ;\n%.mid: marker.in ;\n\
                  %.out: %.tmp ;\n%.tmp: marker.in ;\n",
                b"Makefile",
            )
            .unwrap();
        assert!(engine.resolve_implicit_at(b"marker.out", 0, false));
        assert!(engine.database_implicit.contains(b"marker.tmp".as_slice()));
        assert!(!engine.database_implicit.contains(b"marker.mid".as_slice()));
        assert!(!engine.database.rules.contains_key(b"marker.mid".as_slice()));
    }

    #[test]
    fn rejected_ancestor_keeps_the_children_of_an_accepted_intermediate() {
        let mut engine = engine();
        engine
            .database
            .read(
                b"marker.in: ;\n\
                  %.out: %.tmp %.absent ;\n%.tmp: %.mid ;\n%.mid: marker.in ;\n",
                b"Makefile",
            )
            .unwrap();
        assert!(
            engine
                .plan_implicit_update(b"marker.out", 0, true, None)
                .unwrap()
                .is_none()
        );
        let descendant = b"marker.mid".as_slice();
        assert!(engine.database_implicit.contains(descendant));
        assert!(engine.record_is_target(descendant));
        assert!(engine.file_traversals.outcome(descendant).is_none());
        assert!(!engine.record_is_target(b"marker.tmp"));
        assert!(!engine.record_is_target(b"marker.out"));
    }

    #[test]
    fn detached_implicit_root_keeps_its_branch_marker_separate_from_children() {
        let mut engine = engine();
        engine
            .database
            .read(
                b"marker.in: ;\nmarker.out::\n\
                  %.out: %.mid ;\n%.mid: marker.in ;\n",
                b"Makefile",
            )
            .unwrap();
        let plan = engine
            .plan_implicit_update(b"marker.out", 0, false, Some(0))
            .unwrap()
            .unwrap();
        assert!(
            engine
                .install_implicit_plan(plan, ImplicitPlacement::DoubleColonRoot)
                .is_some()
        );
        assert!(engine.database_implicit.contains(b"marker.mid".as_slice()));
        assert!(!engine.database_implicit.contains(b"marker.out".as_slice()));
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
        let mut branch = head.fresh_double_colon_scope(1);
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
    fn canonical_flags_print_the_named_assignment_without_changing_runtime_state() {
        for (declaration, printed) in [
            ("a.out: MAKEFLAGS += -k", "a.out: MAKEFLAGS += "),
            ("a.out: MAKEFLAGS = -k", "a.out: MAKEFLAGS = "),
            ("a.out: MAKEFLAGS := -k", "a.out: MAKEFLAGS = "),
            ("%.out: MAKEFLAGS += -k", "a.out: MAKEFLAGS = "),
        ] {
            let mut engine = engine();
            let source = format!(
                ".SECONDEXPANSION:\n{declaration}\na.in: ;\n\
                 %.out: $$(eval MAKEFLAGS += -p) %.in ;\n"
            );
            engine
                .database
                .read(source.as_bytes(), b"Makefile")
                .unwrap();
            assert!(engine.resolve_implicit_at(b"a.out", 0, false));
            let variables = engine.target_variables(b"a.out").unwrap();
            let before = variables.get(b"MAKEFLAGS").unwrap().clone();
            let statistics = variables.database_stats();
            assert_eq!(
                before.is_outer_append(),
                declaration == "a.out: MAKEFLAGS += -k"
            );
            let snapshot = engine
                .database_file(
                    b"a.out",
                    engine.database.rules.get(b"a.out".as_slice()),
                    None,
                )
                .unwrap();
            let mut view = snapshot.view;
            view.local_variables = snapshot.automatic.as_deref();
            let mut output = Vec::new();
            database::FilePrinter::new(&mut output).write(view).unwrap();
            let expected = [printed.as_bytes(), before.raw_value(), b"\n"].concat();
            assert!(output.windows(expected.len()).any(|part| part == expected));
            assert_eq!(variables.get(b"MAKEFLAGS"), Some(&before));
            assert_eq!(variables.database_stats(), statistics);
        }
    }

    #[test]
    fn implicit_secondary_flags_remain_in_the_owning_file_scope() {
        for (rules, owner) in [
            (
                b"%.out: $$(eval MAKEFLAGS += -p) %.in ;\n".as_slice(),
                b"a.out".as_slice(),
            ),
            (
                b"%.out: $$(eval MAKEFLAGS += -p) %.absent ;\n%.out: %.in ;\n",
                b"a.out",
            ),
            (
                b"%.out: %.mid ;\n%.mid: $$(eval MAKEFLAGS += -p) %.in ;\n",
                b"a.mid",
            ),
        ] {
            let mut engine = engine();
            // Keep the expected flag spelling independent of the parent's overrides.
            let mut source = b"MAKEOVERRIDES :=\n.SECONDEXPANSION:\na.in: ;\nother: ;\n".to_vec();
            source.extend_from_slice(rules);
            engine.database.read(&source, b"Makefile").unwrap();
            assert!(engine.resolve_implicit_at(b"a.out", 0, false));
            let variables = engine.target_variables(owner).unwrap();
            let definitions = variables.database_file_definitions();
            assert!(engine.options.print_database);
            for (name, origin, expected) in [
                (
                    b"MFLAGS".as_slice(),
                    Origin::Environment,
                    b"-prR".as_slice(),
                ),
                (b"MAKEFLAGS".as_slice(), Origin::File, b"prR".as_slice()),
            ] {
                let (_, value) = definitions
                    .iter()
                    .find(|(key, _)| *key == name)
                    .expect("the deferred expansion owns the normalized flag binding");
                assert_eq!(value.origin(), origin);
                assert_eq!(value.raw_value(), expected);
            }
            assert!(variables.database_lookup(b"@").is_none());
            let unrelated = engine.target_variables(b"other").unwrap();
            assert!(unrelated.database_file_definitions().is_empty());
            if owner != b"a.out" {
                let parent = engine.target_variables(b"a.out").unwrap();
                assert!(parent.database_file_definitions().is_empty());
            }
        }
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
            location: None,
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
        engine.file_traversals.set_outcome(
            b"missing.mk",
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
