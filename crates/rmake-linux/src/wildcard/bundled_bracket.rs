// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Selected filename patterns from GNU Make's bundled matcher on x86-64 musl.
use crate::posix;
use std::ffi::CStr;

pub(super) enum Matcher {
    Host(posix::Matcher),
    Never,
    LeadingClose {
        closing: posix::Matcher,
        members: Option<posix::Matcher>,
    },
}

impl Matcher {
    pub(super) fn new(pattern: &[u8]) -> Option<Self> {
        let host = || posix::Matcher::new(pattern).map(Self::Host);
        let Some(open) = first_unquoted_bracket(pattern) else {
            return host();
        };
        let class = &pattern[open + 1..];
        // Preserve the native matcher for named classes, multiple classes and
        // bracket contents or suffixes containing escapes.
        if class.iter().any(|&byte| matches!(byte, b'[' | b'\\')) {
            return host();
        }
        let negated = class.starts_with(b"!")
            || (class.starts_with(b"^") && std::env::var_os("POSIXLY_CORRECT").is_none());
        let class = &class[usize::from(negated)..];
        let never = || (!pattern.contains(&0)).then_some(Self::Never);
        let Some(tail) = class.strip_prefix(b"]") else {
            return if class.contains(&b']') {
                host()
            } else {
                never()
            };
        };
        let close = tail.iter().position(|&byte| byte == b']');
        if negated {
            return if close.is_some() { host() } else { never() };
        }
        if close.is_some_and(|end| !tail[..end].iter().all(u8::is_ascii_alphanumeric)) {
            return host();
        }

        // The initial ']' matches itself and ends the class immediately. A
        // different input byte can match a later simple member, in which case
        // the later ']' ends the class and selects a different suffix.
        let closing = posix::Matcher::new(&[&pattern[..open], b"\\]", tail].concat())?;
        let members = if close.is_some_and(|end| end != 0) {
            Some(posix::Matcher::new(&[&pattern[..=open], tail].concat())?)
        } else {
            None
        };
        Some(Self::LeadingClose { closing, members })
    }

    pub(super) fn matches(&self, name: &CStr) -> bool {
        match self {
            Self::Host(matcher) => matcher.matches(name),
            Self::Never => false,
            Self::LeadingClose { closing, members } => {
                closing.matches(name)
                    || members
                        .as_ref()
                        .is_some_and(|matcher| matcher.matches(name))
            }
        }
    }
}

fn first_unquoted_bracket(pattern: &[u8]) -> Option<usize> {
    let mut index = 0;
    while let Some(&byte) = pattern.get(index) {
        if byte == b'[' {
            return Some(index);
        }
        index += if byte == b'\\' { 2 } else { 1 };
    }
    None
}
