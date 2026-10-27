// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Pure ordinary-recipe lifecycle decisions. Original implementation informed
//! by oracle experiments and the reference study recorded in
//! docs/forced-touch.md; no GNU implementation code is included here.

use crate::process::Invocation;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Status {
    #[default]
    Success,
    Question,
    Failure,
    /// Expansion or another invocation-wide error; keep-going cannot recover.
    Fatal,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Cleanup {
    #[default]
    None,
    /// A launched child failed: .DELETE_ON_ERROR and changed mtime are required.
    ChangedOnError,
    /// A signal removes changed targets even without .DELETE_ON_ERROR.
    ChangedOnSignal,
    /// A continuation stopped after target completion marked it synthetic-new.
    /// GNU consequently removes even unchanged regular files on this path.
    ExistingAfterContinuation,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Execution {
    pub ran: bool,
    pub status: Status,
    pub cleanup: Cleanup,
    /// At completion, every logical recipe line is recursive after prefix scanning.
    pub all_recursive: bool,
}

impl Execution {
    pub fn with_recursion(mut self, all_recursive: bool) -> Self {
        self.all_recursive = all_recursive;
        self
    }

    pub fn success(ran: bool) -> Self {
        Self {
            ran,
            ..Self::default()
        }
    }

    pub fn failure(ran: bool, cleanup: Cleanup) -> Self {
        Self {
            ran,
            status: Status::Failure,
            cleanup,
            all_recursive: false,
        }
    }

    pub fn fatal(ran: bool) -> Self {
        Self {
            ran,
            status: Status::Fatal,
            cleanup: Cleanup::None,
            all_recursive: false,
        }
    }

    pub fn question(ran: bool, cleanup: Cleanup) -> Self {
        Self {
            ran,
            status: Status::Question,
            cleanup,
            all_recursive: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Attempt {
    #[default]
    None,
    Launched,
    LaunchFailed,
}

/// Terminal interpreter shutdown is separate from ordinary keep-going failures.
/// A fatal report from a reaped command can retain GNU's recipe owner while
/// other workers drain; another fatal report interrupts that final wait.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Shutdown {
    #[default]
    Running,
    Draining,
    RetainedOwner,
    Reentered,
}

impl Shutdown {
    pub fn report(&mut self, owner: bool) -> bool {
        let first = *self == Self::Running;
        *self = if !first {
            Self::Reentered
        } else if owner {
            Self::RetainedOwner
        } else {
            Self::Draining
        };
        first && owner
    }

    pub fn drain(&mut self) {
        if *self == Self::Running {
            *self = Self::Draining;
        }
    }
}

pub struct Flow {
    recursive: Vec<bool>,
    attempt: Attempt,
    scanned_since_attempt: bool,
    registered: Option<bool>,
}

impl Flow {
    pub fn new(recursive: Vec<bool>) -> Self {
        Self {
            recursive,
            attempt: Attempt::None,
            scanned_since_attempt: false,
            registered: None,
        }
    }

    pub fn has_ordinary(&self) -> bool {
        self.recursive.iter().any(|recursive| !recursive)
    }

    /// Expanded '+' remains attached to its original logical recipe, including
    /// commands after an expansion-generated newline.
    pub fn recursive(&self, index: usize) -> bool {
        self.recursive[index]
    }

    /// Call only for nonzero expanded bodies, before command/configuration
    /// preparation. Modifier-only expansions are distinct from zero bytes.
    pub fn visit(&mut self, index: usize, forced: bool) {
        self.recursive[index] |= forced;
        self.scanned_since_attempt = true;
    }

    /// An ignored failed launch resets cleanup eligibility even if an earlier
    /// command really ran. Optimized no-child commands never call this method.
    pub fn attempted(&mut self, launched: bool) {
        self.attempted_in_pool(launched, false);
    }

    pub fn attempted_in_pool(&mut self, launched: bool, active_peer: bool) {
        // A failed dispatch can acquire a logical completion owner when another
        // child is already registered. Keep this separate from actual launch
        // success: cleanup still follows the last attempt's real outcome.
        self.registered.get_or_insert(launched || active_peer);
        self.attempt = if launched {
            Attempt::Launched
        } else {
            Attempt::LaunchFailed
        };
        self.scanned_since_attempt = false;
    }

    pub fn retained_owner(&self) -> bool {
        self.registered == Some(true)
    }

    pub fn failed_launch(&self) -> bool {
        self.attempt == Attempt::LaunchFailed
    }

    pub fn continuation_cleanup(&self) -> Cleanup {
        if self.attempt == Attempt::Launched {
            Cleanup::ExistingAfterContinuation
        } else {
            Cleanup::None
        }
    }

    /// Finishing while scanning invokes both dispatch and owner completion.
    /// Finishing directly after a child (only zero-byte bodies remain) invokes
    /// owner completion alone. These passes produce observable duplicate touches.
    pub fn completion_passes(&self) -> u8 {
        if self.attempt != Attempt::None && !self.scanned_since_attempt {
            1
        } else {
            2
        }
    }
}

/// GNU avoids launching exactly this prepared shell invocation. Classifying
/// source text alone would incorrectly optimize `-ce`, extra args, or wrappers.
pub fn optimized_noop(invocation: &Invocation) -> bool {
    let (Invocation::Shell { program, args } | Invocation::Direct { program, args }) = invocation;
    let basename = program
        .rsplit(|byte| *byte == b'/')
        .next()
        .unwrap_or(program);
    matches!(
        basename,
        b"sh" | b"bash" | b"dash" | b"ash" | b"ksh" | b"rksh" | b"zsh"
    ) && args.len() == 2
        && matches!(args[0].as_slice(), b"-c" | b"-ec")
        && args[1] == b":"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_dispatch_can_inherit_a_peer_owner_without_becoming_a_launch() {
        let mut flow = Flow::new(vec![false]);
        flow.visit(0, false);
        flow.attempted_in_pool(false, true);
        assert!(flow.retained_owner());
        assert!(flow.failed_launch());
        assert_eq!(flow.continuation_cleanup(), Cleanup::None);
    }

    #[test]
    fn fatal_shutdown_retains_only_the_first_owner() {
        let mut state = Shutdown::Running;
        assert!(state.report(true));
        assert_eq!(state, Shutdown::RetainedOwner);
        assert!(!state.report(true));
        assert_eq!(state, Shutdown::Reentered);
    }

    #[test]
    fn ordinary_failure_drain_does_not_gain_a_late_owner_wait() {
        let mut state = Shutdown::Running;
        state.drain();
        assert!(!state.report(true));
        assert_eq!(state, Shutdown::Reentered);
    }

    #[test]
    fn an_initial_failed_launch_never_registers_the_later_owner() {
        let mut flow = Flow::new(vec![false, false]);
        flow.attempted(false);
        flow.attempted(true);
        assert!(!flow.retained_owner());
        assert_eq!(
            flow.continuation_cleanup(),
            Cleanup::ExistingAfterContinuation
        );
    }

    #[test]
    fn a_later_failed_launch_preserves_an_existing_owner() {
        let mut flow = Flow::new(vec![false, false]);
        flow.attempted(true);
        flow.attempted(false);
        assert!(flow.retained_owner());
        assert!(flow.failed_launch());
        assert_eq!(flow.continuation_cleanup(), Cleanup::None);
    }

    #[test]
    fn initial_and_post_child_exhaustion_have_different_completion_counts() {
        let mut flow = Flow::new(vec![true, false]);
        assert_eq!(flow.completion_passes(), 2);
        flow.visit(0, true);
        flow.attempted(true);
        assert_eq!(flow.completion_passes(), 1);
        flow.visit(1, false);
        assert_eq!(flow.completion_passes(), 2);
    }

    #[test]
    fn failed_launch_resets_cleanup_and_no_child_scan_preserves_it() {
        let mut flow = Flow::new(vec![true, true, false]);
        assert_eq!(flow.continuation_cleanup(), Cleanup::None);
        flow.attempted(true);
        flow.visit(1, true);
        assert_eq!(
            flow.continuation_cleanup(),
            Cleanup::ExistingAfterContinuation
        );
        flow.attempted(false);
        flow.visit(2, false);
        assert_eq!(flow.continuation_cleanup(), Cleanup::None);
    }

    #[test]
    fn expanded_force_changes_only_reached_line() {
        let mut flow = Flow::new(vec![true, false, false]);
        flow.visit(1, true);
        assert!(flow.has_ordinary());
        flow.visit(2, true);
        assert!(!flow.has_ordinary());
    }

    #[test]
    fn exact_shell_invocation_controls_noop_optimization() {
        for shell in [b"/bin/sh".as_slice(), b"/bin/bash", b"some/path/dash"] {
            for flags in [b"-c".as_slice(), b"-ec"] {
                assert!(optimized_noop(&Invocation::Shell {
                    program: shell.to_vec(),
                    args: vec![flags.to_vec(), b":".to_vec()],
                }));
            }
        }
        for (shell, flags, command) in [
            (b"/bin/sh".as_slice(), b"-ce".as_slice(), b":".as_slice()),
            (b"/bin/sh", b"-c", b": "),
            (b"./wrapper", b"-c", b":"),
        ] {
            assert!(!optimized_noop(&Invocation::Shell {
                program: shell.to_vec(),
                args: vec![flags.to_vec(), command.to_vec()],
            }));
        }
        assert!(!optimized_noop(&Invocation::Direct {
            program: b":".to_vec(),
            args: vec![]
        }));
        assert!(optimized_noop(&Invocation::Direct {
            program: b"./sh".to_vec(),
            args: vec![b"-c".to_vec(), b":".to_vec()],
        }));
        for extra in [b"".as_slice(), b"argument"] {
            assert!(!optimized_noop(&Invocation::Direct {
                program: b"./sh".to_vec(),
                args: vec![b"-c".to_vec(), b":".to_vec(), extra.to_vec()],
            }));
        }
    }
}
