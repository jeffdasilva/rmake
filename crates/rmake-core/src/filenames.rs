// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Filename sequences have blank separators, unlike ordinary function words.

/// Remove only leading ./ components, without resolving symlinks or '..'.
pub fn without_dot_prefix(mut name: &[u8]) -> &[u8] {
    while let Some(rest) = name.strip_prefix(b"./") {
        name = rest;
        while name.starts_with(b"/") {
            name = &name[1..];
        }
    }
    if name.is_empty() { b"./" } else { name }
}

pub fn archive_parts(name: &[u8]) -> Option<(&[u8], &[u8])> {
    let open = name.iter().position(|&b| b == b'(')?;
    (open > 0 && name.ends_with(b")") && open + 2 < name.len())
        .then(|| (&name[..open], &name[open + 1..name.len() - 1]))
}

/// Filename blanks end a token; other whitespace is skipped only between
/// tokens. Own a token only when a blank consumes quoting backslashes.
pub struct Words<'a> {
    input: &'a [u8],
    position: usize,
}

pub fn words(input: &[u8]) -> Words<'_> {
    Words {
        input: &input[..input
            .iter()
            .position(|&byte| byte == 0)
            .unwrap_or(input.len())],
        position: 0,
    }
}

impl<'a> Iterator for Words<'a> {
    type Item = std::borrow::Cow<'a, [u8]>;

    fn next(&mut self) -> Option<Self::Item> {
        use std::borrow::Cow;
        let input = self.input;
        while self.position < input.len() && matches!(input[self.position], b' ' | b'\t'..=b'\r') {
            self.position += 1;
        }
        let start = self.position;
        if start == input.len() {
            return None;
        }
        let mut owned: Option<Vec<u8>> = None;
        loop {
            // Only a blank can consume quoting. Scan ordinary filename bytes
            // together; internal backslashes do not need individual visits.
            let end = input[self.position..]
                .iter()
                .position(|byte| matches!(byte, b' ' | b'\t'))
                .map_or(input.len(), |offset| self.position + offset);
            if end == input.len() {
                if let Some(output) = &mut owned {
                    output.extend_from_slice(&input[self.position..end]);
                }
                self.position = end;
                return Some(owned.map_or_else(|| Cow::Borrowed(&input[start..end]), Cow::Owned));
            }
            let mut quotes = end;
            while quotes > self.position && input[quotes - 1] == b'\\' {
                quotes -= 1;
            }
            let count = end - quotes;
            if count == 0 {
                if let Some(output) = &mut owned {
                    output.extend_from_slice(&input[self.position..end]);
                }
                self.position = end;
                return Some(owned.map_or_else(|| Cow::Borrowed(&input[start..end]), Cow::Owned));
            }
            let output = owned.get_or_insert_with(|| input[start..self.position].to_vec());
            output.extend_from_slice(&input[self.position..quotes]);
            output.extend(std::iter::repeat_n(b'\\', count / 2));
            self.position = end;
            if count.is_multiple_of(2) {
                return Some(Cow::Owned(owned.unwrap()));
            }
            output.push(input[end]);
            self.position += 1;
        }
    }
}

/// Colons have already been identified as syntax. Remove quoting only before
/// literal colons; even backslash runs elsewhere keep their original bytes.
pub fn unquote_colons(input: &[u8]) -> std::borrow::Cow<'_, [u8]> {
    use std::borrow::Cow;
    if !input.contains(&b'\\') {
        return Cow::Borrowed(input);
    }
    let mut output: Option<Vec<u8>> = None;
    let mut position = 0;
    while position < input.len() {
        let start = position;
        if input[position] == b'\\' {
            while input.get(position) == Some(&b'\\') {
                position += 1;
            }
            let count = position - start;
            if input.get(position) == Some(&b':') && count % 2 == 1 {
                output
                    .get_or_insert_with(|| input[..start].to_vec())
                    .extend(std::iter::repeat_n(b'\\', count / 2));
            } else if let Some(output) = &mut output {
                output.extend_from_slice(&input[start..position]);
            }
        } else {
            position += 1;
            if let Some(output) = &mut output {
                output.push(input[start]);
            }
        }
    }
    output.map_or(Cow::Borrowed(input), Cow::Owned)
}

/// The target colon itself is outside this slice. Its even backslash run
/// quoted pairs before the delimiter and must collapse as well.
pub fn target_colons(input: &[u8]) -> std::borrow::Cow<'_, [u8]> {
    let mut name = unquote_colons(input);
    let count = input
        .iter()
        .rev()
        .take_while(|&&byte| byte == b'\\')
        .count();
    if count > 0 && count.is_multiple_of(2) {
        let len = name.len() - count / 2;
        name.to_mut().truncate(len);
    }
    name
}

/// The first unquoted pipe starts order-only prerequisites. Only the normal
/// side quotes pipes: pipes and backslashes on the order-only side are names.
pub fn split_order(input: &[u8]) -> (std::borrow::Cow<'_, [u8]>, Option<&[u8]>) {
    use std::borrow::Cow;
    if !input.contains(&b'\\') {
        return input
            .iter()
            .position(|&byte| byte == b'|')
            .map_or((Cow::Borrowed(input), None), |index| {
                (Cow::Borrowed(&input[..index]), Some(&input[index + 1..]))
            });
    }
    let mut output: Option<Vec<u8>> = None;
    let mut position = 0;
    while position < input.len() {
        let start = position;
        if input[position] == b'\\' {
            while input.get(position) == Some(&b'\\') {
                position += 1;
            }
            let count = position - start;
            if input.get(position) == Some(&b'|') {
                output
                    .get_or_insert_with(|| input[..start].to_vec())
                    .extend(std::iter::repeat_n(b'\\', count / 2));
                if count.is_multiple_of(2) {
                    return (Cow::Owned(output.unwrap()), Some(&input[position + 1..]));
                }
                output.as_mut().unwrap().push(b'|');
                position += 1;
            } else if let Some(output) = &mut output {
                output.extend_from_slice(&input[start..position]);
            }
        } else if input[position] == b'|' {
            return (
                output.map_or(Cow::Borrowed(&input[..position]), Cow::Owned),
                Some(&input[position + 1..]),
            );
        } else {
            position += 1;
            if let Some(output) = &mut output {
                output.push(input[start]);
            }
        }
    }
    (output.map_or(Cow::Borrowed(input), Cow::Owned), None)
}

/// Split already-expanded filename text, preserving glob escapes. Quoting a
/// blank consumes one quoting backslash after collapsing pairs at that blank.
/// Archive groups produce individual references before tilde/glob processing.
pub fn sequence(input: &[u8]) -> Vec<Vec<u8>> {
    let mut result = Vec::new();
    visit_sequence(input, |name, emit, _offset| {
        if emit {
            result.push(name);
        }
    });
    result
}

/// Include the writes for empty archive-group members. They produce no
/// filename but can overwrite a name held by a recursive HOME expansion.
pub(crate) fn sequence_events(input: &[u8]) -> Vec<(Vec<u8>, bool, usize)> {
    let mut result = Vec::new();
    visit_sequence(input, |name, emit, offset| {
        result.push((name, emit, offset))
    });
    result
}

fn visit_sequence(input: &[u8], mut visit: impl FnMut(Vec<u8>, bool, usize)) {
    let tokens: Vec<_> = words(input).map(std::borrow::Cow::into_owned).collect();
    if tokens.len() < 2 || !tokens.iter().any(|token| token.contains(&b'(')) {
        for token in tokens {
            visit(token, true, 0);
        }
        return;
    }
    let mut group: Option<Vec<u8>> = None;
    // Every possible opener asks the same suffix question. One reverse scan
    // avoids quadratic work on lists containing many unclosed archive names.
    let last_close = tokens.iter().rposition(|word| word.ends_with(b")"));
    for (index, token) in tokens.iter().enumerate() {
        let mut member = token.as_slice();
        let offset = group.as_ref().map_or(0, Vec::len);
        if group.is_none()
            && token.first() != Some(&b'(')
            && !token.ends_with(b")")
            && let Some(open) = token.iter().position(|&b| b == b'(')
            && last_close.is_some_and(|close| close > index)
        {
            group = Some(token[..=open].to_vec());
            member = &token[open + 1..];
        }
        if let Some(prefix) = &group {
            let closes = member.ends_with(b")");
            let mut name = prefix.clone();
            name.extend_from_slice(member);
            if !closes {
                name.push(b')');
            }
            if member.is_empty() {
                visit(prefix.clone(), false, offset);
            } else {
                visit(name, member != b")", offset);
            }
            if closes {
                group = None;
            }
        } else {
            visit(token.clone(), true, 0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::sequence;

    #[test]
    fn simple_names_borrow_and_quoted_blanks_own_only_the_changed_token() {
        use std::borrow::Cow;
        let names: Vec<_> = super::words(b"plain a\\b a\\ b a\\\\ b\0ignored").collect();
        assert!(matches!(names[0], Cow::Borrowed(b"plain")));
        assert!(matches!(names[1], Cow::Borrowed(b"a\\b")));
        assert!(matches!(&names[2], Cow::Owned(bytes) if bytes == b"a b"));
        assert!(matches!(&names[3], Cow::Owned(bytes) if bytes == b"a\\"));
        assert!(matches!(names[4], Cow::Borrowed(b"b")));
    }

    #[test]
    fn order_separator_preserves_roles_and_unquotes_only_the_normal_side() {
        let (normal, order) = super::split_order(b"a\\|b c\\\\|d\\|e | tail");
        assert_eq!(normal.as_ref(), b"a|b c\\");
        assert_eq!(order, Some(b"d\\|e | tail".as_slice()));
        assert_eq!(super::split_order(b"a\\\\\\|b").0.as_ref(), b"a\\|b");
        assert_eq!(super::split_order(b"a\\").0.as_ref(), b"a\\");
        assert_eq!(super::split_order(b"abc|def").1, Some(b"def".as_slice()));
    }

    #[test]
    fn literal_colons_consume_odd_quoting_and_preserve_unrelated_bytes() {
        use std::borrow::Cow;
        assert!(matches!(
            super::unquote_colons(b"a\\b c\\\\:d"),
            Cow::Borrowed(_)
        ));
        assert_eq!(
            super::unquote_colons(b"a\\:b c\\\\\\:d").as_ref(),
            b"a:b c\\:d"
        );
        assert_eq!(super::unquote_colons(b"a\\:").as_ref(), b"a:");
        assert_eq!(super::target_colons(b"a\\\\").as_ref(), b"a\\");
        assert_eq!(super::target_colons(b"a\\:b\\\\\\\\").as_ref(), b"a:b\\\\");
    }

    #[test]
    fn blanks_and_quoting_differ_from_word_whitespace() {
        assert_eq!(
            sequence(b"\x0b a\x0bb a\\ b a\\\tb a\rb"),
            [
                b"a\x0bb".to_vec(),
                b"a b".to_vec(),
                b"a\tb".to_vec(),
                b"a\rb".to_vec()
            ]
        );
        assert_eq!(
            sequence(b"a\\\\ b a\\\\\\ b"),
            [b"a\\".to_vec(), b"b".to_vec(), b"a\\ b".to_vec()]
        );
    }

    #[test]
    fn archive_groups_need_a_later_closing_word() {
        assert_eq!(
            sequence(b"lib(a b ) other(x) none(a b"),
            [
                b"lib(a)".to_vec(),
                b"lib(b)".to_vec(),
                b"other(x)".to_vec(),
                b"none(a".to_vec(),
                b"b".to_vec()
            ]
        );
    }
}
