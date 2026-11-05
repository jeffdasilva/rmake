// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Debug requests preserve input history; events describe interpreter decisions.
//! Rendering never evaluates make expressions or performs filesystem searches.
use std::io::{self, Write};

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
pub fn print_header() {
    raw(&[b"GNU Make 4.4.1\nBuilt for x86_64-pc-linux-gnu\nCopyright (C) 1988-2023 Free Software Foundation, Inc.\nLicense GPLv3+: GNU GPL version 3 or later <https://gnu.org/licenses/gpl.html>\nThis is free software: you are free to change and redistribute it.\nThere is NO WARRANTY, to the extent permitted by law.\n"]);
}

pub fn raw(parts: &[&[u8]]) {
    if crate::native::has_modules() {
        let _ = rmake_linux::native::flush_stream(false);
    }
    let mut stdout = io::stdout().lock();
    let _ = parts.iter().try_for_each(|part| stdout.write_all(part));
    let _ = stdout.flush();
}

#[derive(Clone, Copy)]
pub enum Event<'a> {
    Reading,
    UpdatingMakefiles,
    UpdatingGoals,
    Considering(&'a [u8], usize),
    Missing(&'a [u8], usize),
    Finished(&'a [u8], usize),
    MustRemake(&'a [u8], usize),
    Remade(&'a [u8], usize),
    Failed(&'a [u8], usize),
    NoNeed(&'a [u8], usize),
    Already(&'a [u8], usize),
    Looking(&'a [u8], usize),
    TryingRule(&'a [u8], &'a [u8], usize),
    TryingPrerequisite(&'a [u8], usize),
    OughtToExist(&'a [u8], usize),
    FoundRule(&'a [u8], &'a [u8], usize),
    NoRule(&'a [u8], usize),
}

pub fn emit(output: &crate::output::Controller, event: Event<'_>) {
    let (category, depth) = match event {
        Event::Considering(_, depth)
        | Event::Finished(_, depth)
        | Event::NoNeed(_, depth)
        | Event::Already(_, depth) => (Mask::VERBOSE, depth),
        Event::Looking(_, depth)
        | Event::TryingRule(_, _, depth)
        | Event::TryingPrerequisite(_, depth)
        | Event::OughtToExist(_, depth)
        | Event::FoundRule(_, _, depth)
        | Event::NoRule(_, depth) => (Mask::IMPLICIT, depth),
        Event::Missing(_, depth)
        | Event::MustRemake(_, depth)
        | Event::Remade(_, depth)
        | Event::Failed(_, depth) => (Mask::BASIC, depth),
        _ => (Mask::BASIC, 0),
    };
    if !output.debug_mask().has(category) {
        return;
    }
    let indent = vec![b' '; depth];
    let prefix = indent.as_slice();
    match event {
        Event::Reading => raw(&[b"Reading makefiles...\n"]),
        Event::UpdatingMakefiles => raw(&[b"Updating makefiles....\n"]),
        Event::UpdatingGoals => raw(&[b"Updating goal targets....\n"]),
        Event::Considering(name, _) => raw(&[prefix, b"Considering target file '", name, b"'.\n"]),
        Event::Missing(name, _) => raw(&[prefix, b"File '", name, b"' does not exist.\n"]),
        Event::Finished(name, _) => raw(&[
            prefix,
            b"Finished prerequisites of target file '",
            name,
            b"'.\n",
        ]),
        Event::MustRemake(name, _) => raw(&[prefix, b"Must remake target '", name, b"'.\n"]),
        Event::Remade(name, _) => {
            raw(&[prefix, b"Successfully remade target file '", name, b"'.\n"])
        }
        Event::Failed(name, _) => raw(&[prefix, b"Failed to remake target file '", name, b"'.\n"]),
        Event::NoNeed(name, _) => raw(&[prefix, b"No need to remake target '", name, b"'.\n"]),
        Event::Already(name, _) => raw(&[prefix, b"File '", name, b"' was considered already.\n"]),
        Event::Looking(name, _) => {
            raw(&[prefix, b"Looking for an implicit rule for '", name, b"'.\n"])
        }
        Event::TryingRule(rule, stem, _) => raw(&[
            prefix,
            b"Trying pattern rule '",
            rule,
            b"' with stem '",
            stem,
            b"'.\n",
        ]),
        Event::TryingPrerequisite(name, _) => {
            raw(&[prefix, b"Trying implicit prerequisite '", name, b"'.\n"])
        }
        Event::OughtToExist(name, _) => raw(&[prefix, b"'", name, b"' ought to exist.\n"]),
        Event::FoundRule(rule, name, _) => raw(&[
            prefix,
            b"Found implicit rule '",
            rule,
            b"' for '",
            name,
            b"'.\n",
        ]),
        Event::NoRule(name, _) => raw(&[prefix, b"No implicit rule found for '", name, b"'.\n"]),
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
        } => raw(&[
            b"Reading makefile '",
            name,
            b"'",
            if no_default_goal {
                b" (no default goal)"
            } else {
                b""
            },
            if search { b" (search path)" } else { b"" },
            if optional { b" (don't care)" } else { b"" },
            if no_tilde { b" (no ~ expansion)" } else { b"" },
            b"...\n",
        ]),
        rmake_core::diagnostic::ReaderEvent::Bom(name) => {
            raw(&[b"Skipping UTF-8 BOM in makefile '", name, b"'\n"])
        }
    }
}

pub fn rule_label(database: &rmake_core::parser::Database, identity: usize) -> Vec<u8> {
    let Some((pattern, rule)) = database.implicit_rule_source(identity) else {
        return Vec::new();
    };
    let mut text = pattern.literal_bytes().to_vec();
    text.extend_from_slice(if rule.terminal { b"::" } else { b":" });
    let mut order_only = false;
    for dependency in &rule.dependencies {
        if dependency.order_only && !order_only {
            text.extend_from_slice(b" |");
            order_only = true;
        }
        text.push(b' ');
        text.extend_from_slice(&dependency.name);
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
