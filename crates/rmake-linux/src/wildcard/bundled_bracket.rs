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
        if let Some(caret) = class.strip_prefix(b"^")
            && let Some(close) = caret.iter().position(|&byte| byte == b']')
            && caret[..close].iter().all(u8::is_ascii_alphanumeric)
            && std::env::var_os("POSIXLY_CORRECT").is_some()
        {
            // GNU treats caret as a literal member when this variable is
            // present. Use a non-leading caret with the native matcher.
            let prefix = &pattern[..open];
            let suffix = &caret[close + 1..];
            let adjusted = if close == 0 {
                [prefix, b"^", suffix].concat()
            } else {
                [prefix, b"[", &caret[..close], b"^]", suffix].concat()
            };
            return posix::Matcher::new(&adjusted).map(Self::Host);
        }
        let negated = class.starts_with(b"!")
            || (class.starts_with(b"^") && std::env::var_os("POSIXLY_CORRECT").is_none());
        let members_start = open + 1 + usize::from(negated);
        let class = &pattern[members_start..];
        if let [first, b'-', last, b']', suffix @ ..] = class
            && first.is_ascii_alphanumeric()
            && last.is_ascii_alphanumeric()
            && first > last
        {
            // The bundled matcher tests the first endpoint as a literal
            // member. A descending interval adds no other members.
            let adjusted = [&pattern[..members_start], &[*first, b']'], suffix].concat();
            return posix::Matcher::new(&adjusted).map(Self::Host);
        }
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

#[cfg(test)]
mod tests {
    use super::Matcher;

    #[test]
    fn descending_range_retains_only_the_opening_endpoint() {
        let letters = Matcher::new(b"[x-a]").unwrap();
        assert!(letters.matches(c"x"));
        assert!(!letters.matches(c"a"));
        assert!(!letters.matches(c"b"));

        let digits = Matcher::new(b"[!9-0]").unwrap();
        assert!(!digits.matches(c"9"));
        assert!(digits.matches(c"0"));
        assert!(digits.matches(c"7"));
    }
}
