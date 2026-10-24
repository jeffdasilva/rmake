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

/// Split already-expanded filename text, preserving glob escapes. Quoting a
/// blank consumes one quoting backslash after collapsing pairs at that blank.
/// Archive groups produce individual references before tilde/glob processing.
pub fn sequence(input: &[u8]) -> Vec<Vec<u8>> {
    let input = &input[..input.iter().position(|&b| b == 0).unwrap_or(input.len())];
    let mut tokens = Vec::new();
    let mut pos = 0;
    while pos < input.len() {
        while pos < input.len() && matches!(input[pos], b' ' | b'\t'..=b'\r') {
            pos += 1;
        }
        if pos == input.len() {
            break;
        }
        let mut token = Vec::new();
        while pos < input.len() && !matches!(input[pos], b' ' | b'\t') {
            if input[pos] == b'\\' {
                let start = pos;
                while pos < input.len() && input[pos] == b'\\' {
                    pos += 1;
                }
                let count = pos - start;
                if pos < input.len() && matches!(input[pos], b' ' | b'\t') {
                    token.extend(std::iter::repeat_n(b'\\', count / 2));
                    if count % 2 == 0 {
                        break;
                    }
                    token.push(input[pos]);
                    pos += 1;
                } else {
                    token.extend_from_slice(&input[start..pos]);
                }
            } else {
                token.push(input[pos]);
                pos += 1;
            }
        }
        tokens.push(token);
    }
    if tokens.len() < 2 || !tokens.iter().any(|token| token.contains(&b'(')) {
        return tokens;
    }
    let mut result = Vec::new();
    let mut group: Option<Vec<u8>> = None;
    // Every possible opener asks the same suffix question. One reverse scan
    // avoids quadratic work on lists containing many unclosed archive names.
    let last_close = tokens.iter().rposition(|word| word.ends_with(b")"));
    for (index, token) in tokens.iter().enumerate() {
        let mut member = token.as_slice();
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
            if !member.is_empty() && member != b")" {
                result.push(name);
            }
            if closes {
                group = None;
            }
        } else {
            result.push(token.clone());
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::sequence;

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
