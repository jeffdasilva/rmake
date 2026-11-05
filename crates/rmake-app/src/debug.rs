// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Debug requests preserve input history; events describe interpreter decisions.
//! Rendering never evaluates make expressions or performs filesystem searches.
use crate::debug_messages as msg;
use rmake_core::messages::{Argument, checked_format};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mask(u8);
impl Mask {
    pub const BASIC: Self = Self(1);
    pub const VERBOSE: Self = Self(2);
    pub const IMPLICIT: Self = Self(4);
    pub const JOBS: Self = Self(8);
    pub const MAKEFILES: Self = Self(16);
    pub const PRINT: Self = Self(32);
    pub const WHY: Self = Self(64);
    pub const ALL: Self = Self(127);
    pub const NONE: Self = Self(0);
    pub(crate) fn bits(self) -> u8 {
        self.0
    }
    pub(crate) fn from_bits(bits: u8) -> Self {
        Self(bits & Self::ALL.0)
    }
    pub fn has(self, category: Self) -> bool {
        self.0 & category.0 != 0
    }
    fn add(&mut self, category: Self) {
        self.0 |= category.0;
    }
}

#[derive(Clone, Debug, Default)]
pub struct Requests {
    pub all: bool,
    pub values: Vec<Vec<u8>>,
}
impl Requests {
    pub fn push(&mut self, value: Vec<u8>) {
        if !self.values.contains(&value) {
            self.values.push(value);
        }
    }
    #[cfg(test)]
    pub fn decode(&mut self, trace: bool) -> Result<Mask, String> {
        self.decode_from(trace, Mask::NONE)
    }
    pub fn decode_from(&mut self, trace: bool, current: Mask) -> Result<Mask, String> {
        // A signal changes the effective mask, not recorded option history.
        // Completed decodes replay requests onto that live selection.
        let mut mask = if self.all { Mask::ALL } else { current };
        if trace {
            mask.add(Mask::PRINT);
            mask.add(Mask::WHY);
        }
        for request in &self.values {
            let mut cursor = 0;
            while cursor < request.len() {
                let remaining = &request[cursor..];
                match remaining[0].to_ascii_lowercase() {
                    b'a' => mask = Mask::ALL,
                    b'b' => mask.add(Mask::BASIC),
                    b'v' => {
                        mask.add(Mask::BASIC);
                        mask.add(Mask::VERBOSE);
                    }
                    b'i' => {
                        mask.add(Mask::BASIC);
                        mask.add(Mask::IMPLICIT);
                    }
                    b'j' => mask.add(Mask::JOBS),
                    b'm' => {
                        mask.add(Mask::BASIC);
                        mask.add(Mask::MAKEFILES);
                    }
                    b'p' => mask.add(Mask::PRINT),
                    b'w' => mask.add(Mask::WHY),
                    b'n' => mask = Mask::NONE,
                    _ => {
                        return Err(format!(
                            "unknown debug level specification '{}'",
                            String::from_utf8_lossy(remaining)
                        ));
                    }
                }
                cursor += remaining
                    .iter()
                    .position(|byte| matches!(byte, b',' | b' '))
                    .map_or(remaining.len(), |separator| separator + 1);
            }
        }
        if mask == Mask::NONE {
            self.all = false;
        }
        Ok(mask)
    }
}

/// Protocol header for debug mode. Explicit --version retains rmake branding.
pub fn print_compatibility_header(commented: bool) {
    let prefix: &[u8] = if commented { b"# " } else { b"" };
    raw(&[
        prefix,
        b"GNU Make 4.4.1\n",
        &localized(
            msg::BUILT_FOR,
            &[bytes(prefix), bytes(b"x86_64-pc-linux-gnu")],
        ),
        prefix,
        b"Copyright (C) 1988-2023 Free Software Foundation, Inc.\n",
        &localized(msg::LICENSE, &[bytes(prefix), bytes(prefix), bytes(prefix)]),
    ]);
}

fn bytes(text: &[u8]) -> Argument<'_> {
    Argument::Bytes(text)
}

fn localized(key: msg::Key, arguments: &[Argument<'_>]) -> Vec<u8> {
    // A native module can install a different public gettext domain. Keep a
    // checked English fallback if that foreign format has an invalid schema.
    rmake_linux::messages::render_format(key.msgid, arguments).unwrap_or_else(|_| {
        checked_format(key.msgid.to_bytes(), arguments)
            .expect("static debug key and arguments have matching types")
    })
}

pub fn raw(parts: &[&[u8]]) {
    if crate::native::has_modules() {
        let _ = rmake_linux::native::flush_stream(false);
    }
    crate::output::stdout_parts(parts);
}

pub fn restart_prefix(count: u32) -> Vec<u8> {
    localized(msg::REEXECUTING, &[Argument::Unsigned(count.into())])
}

#[derive(Clone, Copy)]
pub enum Prerequisite {
    Older,
    Newer,
    Missing,
    OrderOnly,
}

#[derive(Clone, Copy)]
pub enum Event<'a> {
    Reading,
    UpdatingMakefiles,
    UpdatingGoals,
    Considering(&'a [u8], usize),
    Missing(&'a [u8], usize),
    Finished(&'a [u8], usize),
    Prerequisite(Prerequisite, &'a [u8], &'a [u8], usize),
    MustRemake(&'a [u8], usize),
    Remade(&'a [u8], usize),
    Failed(&'a [u8], usize),
    NoNeed(&'a [u8], Option<&'a [u8]>, usize),
    Already(&'a [u8], usize),
    Pruning(&'a [u8], usize),
    GiveUp(&'a [u8], usize),
    DoubleWithoutPrerequisites(&'a [u8], usize),
    NoRecipeNoChanges(&'a [u8], usize),
    AlwaysMake(&'a [u8], usize),
    Looking(&'a [u8], usize),
    LookingArchive(&'a [u8], usize),
    NoArchiveRule(&'a [u8], usize),
    TryingRule(&'a [u8], &'a [u8], usize),
    TryingPrerequisite(&'a [u8], bool, usize),
    TryingHarder(usize),
    AvoidingRecursion(&'a [u8], usize),
    FoundPrerequisite(&'a [u8], usize),
    FoundVpath(&'a [u8], &'a [u8], usize),
    MissingPrerequisite(&'a [u8], usize),
    SearchingChildRule(&'a [u8], bool, usize),
    RejectedPrerequisite(&'a [u8], &'a [u8], Option<bool>, usize),
    NotOughtToExist(&'a [u8], &'a [u8], usize),
    UsingCompatibilityRule(&'a [u8], &'a [u8], usize),
    SearchingCompatibility(&'a [u8], usize),
    RemovingIntermediates,
    OughtToExist(&'a [u8], usize),
    FoundRule(&'a [u8], &'a [u8], usize),
    NoRule(&'a [u8], usize),
}

pub fn emit(output: &crate::output::Controller, event: Event<'_>) {
    let (category, depth) = match event {
        Event::Prerequisite(state, _, _, depth) => (
            match state {
                Prerequisite::Older | Prerequisite::OrderOnly => Mask::VERBOSE,
                Prerequisite::Newer | Prerequisite::Missing => Mask::BASIC,
            },
            depth,
        ),
        Event::Considering(_, depth)
        | Event::Finished(_, depth)
        | Event::NoNeed(_, _, depth)
        | Event::Already(_, depth)
        | Event::Pruning(_, depth)
        | Event::GiveUp(_, depth)
        | Event::NoRecipeNoChanges(_, depth)
        | Event::AlwaysMake(_, depth) => (Mask::VERBOSE, depth),
        Event::Looking(_, depth)
        | Event::LookingArchive(_, depth)
        | Event::NoArchiveRule(_, depth)
        | Event::TryingRule(_, _, depth)
        | Event::TryingPrerequisite(_, _, depth)
        | Event::TryingHarder(depth)
        | Event::AvoidingRecursion(_, depth)
        | Event::FoundPrerequisite(_, depth)
        | Event::FoundVpath(_, _, depth)
        | Event::MissingPrerequisite(_, depth)
        | Event::SearchingChildRule(_, _, depth)
        | Event::RejectedPrerequisite(_, _, _, depth)
        | Event::NotOughtToExist(_, _, depth)
        | Event::UsingCompatibilityRule(_, _, depth)
        | Event::SearchingCompatibility(_, depth)
        | Event::OughtToExist(_, depth)
        | Event::FoundRule(_, _, depth)
        | Event::NoRule(_, depth) => (Mask::IMPLICIT, depth),
        Event::Missing(_, depth)
        | Event::MustRemake(_, depth)
        | Event::Remade(_, depth)
        | Event::Failed(_, depth)
        | Event::DoubleWithoutPrerequisites(_, depth) => (Mask::BASIC, depth),
        _ => (Mask::BASIC, 0),
    };
    if !output.debug_mask().has(category) {
        return;
    }
    let indent = vec![b' '; depth];
    let prefix = indent.as_slice();
    let print = |key, arguments: &[Argument<'_>]| raw(&[prefix, &localized(key, arguments)]);
    match event {
        Event::Reading => print(msg::READING, &[]),
        Event::UpdatingMakefiles => print(msg::UPDATING_MAKEFILES, &[]),
        Event::UpdatingGoals => print(msg::UPDATING_GOALS, &[]),
        Event::Considering(name, _) => print(msg::CONSIDERING, &[bytes(name)]),
        Event::Missing(name, _) => print(msg::MISSING, &[bytes(name)]),
        Event::Finished(name, _) => print(msg::FINISHED, &[bytes(name)]),
        Event::Prerequisite(state, dependency, target, _) => print(
            match state {
                Prerequisite::Older => msg::PREREQ_OLDER,
                Prerequisite::Newer => msg::PREREQ_NEWER,
                Prerequisite::Missing => msg::PREREQ_MISSING,
                Prerequisite::OrderOnly => msg::PREREQ_ORDER_ONLY,
            },
            &[bytes(dependency), bytes(target)],
        ),
        Event::MustRemake(name, _) => print(msg::MUST_REMAKE, &[bytes(name)]),
        Event::Remade(name, _) => print(msg::REMADE, &[bytes(name)]),
        Event::Failed(name, _) => print(msg::FAILED, &[bytes(name)]),
        Event::NoNeed(name, path, _) => {
            let line = localized(msg::NO_NEED, &[bytes(name)]);
            let suffix = path.map(|path| localized(msg::USING_VPATH, &[bytes(path)]));
            raw(&[prefix, &line, suffix.as_deref().unwrap_or_default(), b".\n"]);
        }
        Event::Already(name, _) => print(msg::ALREADY, &[bytes(name)]),
        Event::Pruning(name, _) => print(msg::PRUNING, &[bytes(name)]),
        Event::GiveUp(name, _) => print(msg::GIVE_UP, &[bytes(name)]),
        Event::DoubleWithoutPrerequisites(name, _) => {
            print(msg::DOUBLE_WITHOUT_PREREQUISITES, &[bytes(name)])
        }
        Event::NoRecipeNoChanges(name, _) => print(msg::NO_RECIPE_NO_CHANGES, &[bytes(name)]),
        Event::AlwaysMake(name, _) => print(msg::ALWAYS_MAKE, &[bytes(name)]),
        Event::Looking(name, _) => print(msg::LOOKING, &[bytes(name)]),
        Event::LookingArchive(name, _) => print(msg::LOOKING_ARCHIVE, &[bytes(name)]),
        Event::NoArchiveRule(name, _) => print(msg::NO_ARCHIVE_RULE, &[bytes(name)]),
        Event::TryingRule(rule, stem, _) => print(
            msg::TRYING_RULE,
            &[
                bytes(rule),
                Argument::Unsigned(stem.len() as u64),
                bytes(stem),
            ],
        ),
        Event::TryingPrerequisite(name, literal, _) => print(
            if literal {
                msg::TRYING_LITERAL
            } else {
                msg::TRYING_IMPLICIT
            },
            &[bytes(name)],
        ),
        Event::TryingHarder(_) => print(msg::TRYING_HARDER, &[]),
        Event::AvoidingRecursion(rule, _) => print(msg::AVOIDING_RECURSION, &[bytes(rule)]),
        Event::FoundPrerequisite(name, _) => print(msg::FOUND_PREREQUISITE, &[bytes(name)]),
        Event::FoundVpath(name, path, _) => print(msg::FOUND_VPATH, &[bytes(name), bytes(path)]),
        Event::MissingPrerequisite(name, _) => print(msg::MISSING_PREREQUISITE, &[bytes(name)]),
        Event::SearchingChildRule(name, explicit, _) => print(
            if explicit {
                msg::SEARCHING_EXPLICIT
            } else {
                msg::SEARCHING_INTERMEDIATE
            },
            &[bytes(name)],
        ),
        Event::RejectedPrerequisite(rule, name, literal, _) => print(
            match literal {
                Some(true) => msg::REJECTED_LITERAL,
                Some(false) => msg::REJECTED_IMPLICIT,
                None => msg::REJECTED_PREREQUISITE,
            },
            &[bytes(rule), bytes(name)],
        ),
        Event::NotOughtToExist(name, rule, _) => {
            print(msg::NOT_OUGHT_TO_EXIST, &[bytes(name), bytes(rule)])
        }
        Event::UsingCompatibilityRule(rule, name, _) => {
            print(msg::USING_COMPATIBILITY, &[bytes(rule), bytes(name)])
        }
        Event::SearchingCompatibility(name, _) => {
            print(msg::SEARCHING_COMPATIBILITY, &[bytes(name)])
        }
        Event::RemovingIntermediates => print(msg::REMOVING_INTERMEDIATES, &[]),
        Event::OughtToExist(name, _) => print(msg::OUGHT_TO_EXIST, &[bytes(name)]),
        Event::FoundRule(rule, name, _) => print(msg::FOUND_RULE, &[bytes(rule), bytes(name)]),
        Event::NoRule(name, _) => print(msg::NO_RULE, &[bytes(name)]),
    }
}

pub fn reader(output: &crate::output::Controller, event: rmake_core::diagnostic::ReaderEvent<'_>) {
    let category = match event {
        rmake_core::diagnostic::ReaderEvent::Bom(_) => Mask::BASIC,
        _ => Mask::VERBOSE,
    };
    if !output.debug_mask().has(category) {
        return;
    }
    match event {
        rmake_core::diagnostic::ReaderEvent::Reading {
            name,
            search,
            optional,
            no_default_goal,
            no_tilde,
        } => {
            let mut line = localized(msg::READING_FILE, &[bytes(name)]);
            for (flag, key) in [
                (no_default_goal, msg::NO_DEFAULT_GOAL),
                (search, msg::SEARCH_PATH),
                (optional, msg::OPTIONAL),
                (no_tilde, msg::NO_TILDE),
            ] {
                if flag {
                    line.extend(localized(key, &[]));
                }
            }
            raw(&[&line, b"...\n"]);
        }
        rmake_core::diagnostic::ReaderEvent::Bom(name) => {
            raw(&[&localized(msg::SKIPPING_BOM, &[bytes(name)])])
        }
    }
}

pub fn rule_label(database: &rmake_core::parser::Database, identity: usize) -> Vec<u8> {
    let Some((pattern, rule)) = database.implicit_rule_source(identity) else {
        return Vec::new();
    };
    let mut text = if let Some(patterns) = rule.grouped_targets() {
        patterns.join(b" ".as_slice())
    } else {
        pattern.literal_bytes().to_vec()
    };
    text.extend_from_slice(if rule.terminal { b"::" } else { b":" });
    let mut order_only = false;
    for (index, dependency) in rule.dependencies.iter().enumerate() {
        if dependency.order_only && !order_only {
            text.extend_from_slice(b" |");
            order_only = true;
        }
        text.push(b' ');
        let spelling = if rule
            .secondary
            .as_ref()
            .is_some_and(|clauses| clauses.iter().any(|clause| clause.index == index))
        {
            dependency.name.trim_ascii_start()
        } else {
            dependency.name.as_slice()
        };
        text.extend_from_slice(spelling);
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reset_order_depends_on_unique_raw_request_history() {
        let mut separate = Requests::default();
        for value in [b"b".as_slice(), b"n", b"b"] {
            separate.push(value.to_vec());
        }
        assert_eq!(separate.values, [b"b".to_vec(), b"n".to_vec()]);
        assert_eq!(separate.decode(false).unwrap(), Mask::NONE);
        let mut combined = Requests::default();
        combined.push(b"b,n,b".to_vec());
        assert!(combined.decode(false).unwrap().has(Mask::BASIC));
        let mut clear = Requests {
            all: true,
            ..Requests::default()
        };
        clear.push(b"none".to_vec());
        assert_eq!(clear.decode(true).unwrap(), Mask::NONE);
        assert!(!clear.all);
    }
    #[test]
    fn invalid_separators_preserve_the_remaining_debug_request_suffix() {
        for (request, suffix) in [
            (",b", ",b"),
            (" b", " b"),
            ("b,,p", ",p"),
            ("b  p", " p"),
            ("b, p", " p"),
            ("z,p", "z,p"),
            ("b,z,p", "z,p"),
        ] {
            let mut requests = Requests::default();
            requests.push(request.as_bytes().to_vec());
            assert_eq!(
                requests.decode(false).unwrap_err(),
                format!("unknown debug level specification '{suffix}'")
            );
        }
        let mut trailing = Requests::default();
        trailing.push(b"b,".to_vec());
        assert_eq!(trailing.decode(false).unwrap(), Mask::BASIC);
    }

    #[test]
    fn categories_do_not_accidentally_enable_basic() {
        for (token, category, basic) in [
            (b"verbose".as_slice(), Mask::VERBOSE, true),
            (b"implicit", Mask::IMPLICIT, true),
            (b"makefiles", Mask::MAKEFILES, true),
            (b"jobs", Mask::JOBS, false),
            (b"print", Mask::PRINT, false),
            (b"why", Mask::WHY, false),
        ] {
            let mut requests = Requests::default();
            requests.push(token.to_vec());
            let mask = requests.decode(false).unwrap();
            assert!(mask.has(category));
            assert_eq!(mask.has(Mask::BASIC), basic);
        }
        let mut requests = Requests::default();
        requests.push(b"BASIC-suffix".to_vec());
        assert_eq!(requests.decode(false).unwrap(), Mask::BASIC);
    }
}

/// A recipe owns one stable record while its individual commands acquire PIDs.
/// Raw addresses are identities only; no foreign pointer is dereferenced.
pub struct Job {
    target: Vec<u8>,
    pid: std::cell::Cell<u32>,
    linked: std::cell::Cell<bool>,
}
pub type JobHandle = std::rc::Rc<Job>;

#[derive(Default)]
pub struct Jobs {
    /// Explicit chain order survives HashMap movement and command continuations.
    chain: Vec<JobHandle>,
    drained: bool,
}
impl Jobs {
    pub fn record(target: &[u8], mask: Mask) -> Option<JobHandle> {
        mask.has(Mask::JOBS).then(|| {
            std::rc::Rc::new(Job {
                target: target.to_vec(),
                pid: std::cell::Cell::new(0),
                linked: std::cell::Cell::new(false),
            })
        })
    }
    fn identity(job: &JobHandle) -> String {
        format!("{:p}", std::rc::Rc::as_ptr(job))
    }
    pub fn controller(mask: Mask, authentication: &[u8]) {
        if mask.has(Mask::JOBS) {
            raw(&[b"Using jobserver controller ", authentication, b"\n"]);
        }
    }
    pub fn need(&self, mask: Mask) {
        if mask.has(Mask::JOBS) {
            raw(&[if self.chain.is_empty() {
                b"Need a job token; we don't have children\n"
            } else {
                b"Need a job token; we have children\n"
            }]);
        }
    }
    pub fn obtained(mask: Mask, job: Option<&JobHandle>) {
        if mask.has(Mask::JOBS)
            && let Some(job) = job
        {
            raw(&[
                b"Obtained token for child ",
                Self::identity(job).as_bytes(),
                b" (",
                &job.target,
                b").\n",
            ]);
        }
    }
    pub fn launched(&mut self, mask: Mask, job: &JobHandle, pid: u32) {
        job.pid.set(pid);
        if !job.linked.replace(true) {
            self.chain.insert(0, job.clone());
            if mask.has(Mask::JOBS) {
                raw(&[
                    b"Putting child ",
                    Self::identity(job).as_bytes(),
                    b" (",
                    &job.target,
                    b") PID ",
                    pid.to_string().as_bytes(),
                    b" on the chain.\n",
                ]);
            }
        }
    }
    pub fn live(&self, mask: Mask) {
        if !mask.has(Mask::JOBS) {
            return;
        }
        for job in &self.chain {
            raw(&[
                b"Live child ",
                Self::identity(job).as_bytes(),
                b" (",
                &job.target,
                b") PID ",
                job.pid.get().to_string().as_bytes(),
                b" \n",
            ]);
        }
    }
    pub fn reaped(mask: Mask, job: Option<&JobHandle>, success: bool) {
        if mask.has(Mask::JOBS)
            && let Some(job) = job
        {
            raw(&[
                if success {
                    b"Reaping winning child "
                } else {
                    b"Reaping losing child "
                },
                Self::identity(job).as_bytes(),
                b" PID ",
                job.pid.get().to_string().as_bytes(),
                b" \n",
            ]);
        }
    }
    pub fn removed(&mut self, mask: Mask, job: &JobHandle, pooled: bool) {
        if !job.linked.replace(false) {
            return;
        }
        if mask.has(Mask::JOBS) {
            raw(&[
                b"Removing child ",
                Self::identity(job).as_bytes(),
                b" PID ",
                job.pid.get().to_string().as_bytes(),
                b" from chain.\n",
            ]);
        }
        self.chain.retain(|entry| !std::rc::Rc::ptr_eq(entry, job));
        // This describes the logical slot made available by completion, not
        // the particular recipe which happened to own an explicit permit.
        if pooled && !self.chain.is_empty() && mask.has(Mask::JOBS) {
            raw(&[
                b"Released token for child ",
                Self::identity(job).as_bytes(),
                b" (",
                &job.target,
                b").\n",
            ]);
        }
    }
    pub fn drain(&mut self, mask: Mask, slots: Option<usize>) {
        if self.drained {
            return;
        }
        self.drained = true;
        if mask.has(Mask::JOBS)
            && let Some(slots) = slots
        {
            raw(&[
                b"Acquired all ",
                slots.saturating_sub(1).to_string().as_bytes(),
                b" jobserver tokens.\n",
            ]);
        }
    }
}

#[cfg(test)]
mod job_record_tests {
    use super::*;
    #[test]
    fn recipe_record_identity_survives_command_pids_and_chain_reordering() {
        assert!(Jobs::record(b"quiet", Mask::NONE).is_none());
        let first = Jobs::record(b"first", Mask::JOBS).unwrap();
        let first_pointer = std::rc::Rc::as_ptr(&first);
        let weak = std::rc::Rc::downgrade(&first);
        let mut jobs = Jobs::default();
        jobs.launched(Mask::NONE, &first, 11);
        let continuation = first.clone();
        jobs.launched(Mask::NONE, &continuation, 12);
        assert_eq!(jobs.chain.len(), 1);
        assert_eq!(std::rc::Rc::as_ptr(&continuation), first_pointer);
        assert_eq!(jobs.chain[0].pid.get(), 12);
        let second = Jobs::record(b"second", Mask::JOBS).unwrap();
        jobs.launched(Mask::NONE, &second, 13);
        assert!(std::rc::Rc::ptr_eq(&jobs.chain[0], &second));
        assert!(std::rc::Rc::ptr_eq(&jobs.chain[1], &first));
        jobs.removed(Mask::NONE, &first, true);
        assert_eq!(jobs.chain.len(), 1);
        assert!(std::rc::Rc::ptr_eq(&jobs.chain[0], &second));
        drop(first);
        drop(continuation);
        assert!(weak.upgrade().is_none());
        jobs.removed(Mask::NONE, &second, false);
        assert!(jobs.chain.is_empty());
    }
}

#[cfg(test)]
mod live_selection_tests {
    use super::*;
    #[test]
    fn replay_preserves_live_selection_and_only_records_actual_requests() {
        let mut requests = Requests::default();
        assert_eq!(
            requests.decode_from(false, Mask::BASIC).unwrap(),
            Mask::BASIC
        );
        assert!(requests.values.is_empty());
        requests.push(b"print".to_vec());
        let selected = requests.decode_from(false, Mask::BASIC).unwrap();
        assert!(selected.has(Mask::BASIC));
        assert!(selected.has(Mask::PRINT));
        assert_eq!(requests.values, [b"print".to_vec()]);
        assert_eq!(
            requests.decode_from(false, Mask::NONE).unwrap(),
            Mask::PRINT
        );
        requests.push(b"none".to_vec());
        assert_eq!(
            requests.decode_from(false, Mask::BASIC).unwrap(),
            Mask::NONE
        );
    }
}
