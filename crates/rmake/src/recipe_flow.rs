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
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Cleanup {
    #[default]
    None,
    /// A launched child failed: .DELETE_ON_ERROR and changed mtime are required.
    ChangedOnError,
    /// A continuation stopped after target completion marked it synthetic-new.
    /// GNU consequently removes even unchanged regular files on this path.
    ExistingAfterContinuation,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Execution {
    pub ran: bool,
    pub status: Status,
    pub cleanup: Cleanup,
}

impl Execution {
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
        }
    }

    pub fn question(ran: bool, cleanup: Cleanup) -> Self {
        Self {
            ran,
            status: Status::Question,
            cleanup,
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

pub struct Flow {
    recursive: Vec<bool>,
    attempt: Attempt,
    scanned_since_attempt: bool,
}

impl Flow {
    pub fn new(recursive: Vec<bool>) -> Self {
        Self {
            recursive,
            attempt: Attempt::None,
            scanned_since_attempt: false,
        }
    }

    pub fn has_ordinary(&self) -> bool {
        self.recursive.iter().any(|recursive| !recursive)
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
        self.attempt = if launched {
            Attempt::Launched
        } else {
            Attempt::LaunchFailed
        };
        self.scanned_since_attempt = false;
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
