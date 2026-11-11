// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Byte-oriented physical and logical Makefile lines.
//!
//! The caller supplies rule context and the current recipe prefix for each line.
//! Reading and comment removal remain separate: recipes and `define` bodies do
//! not use the ordinary-line comment rules. No UTF-8 conversion is performed.

mod stream;
pub(crate) use stream::ParserReader;
pub use stream::{Input, Source};

/// A logical line and its inclusive, one-based physical source locations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogicalLine {
    /// The first recipe prefix is retained. Continuation prefixes are removed.
    /// A final line terminator is excluded; recipe continuation newlines remain.
    pub bytes: Vec<u8>,
    pub line: usize,
    pub end_line: usize,
    pub recipe: bool,
}

/// Prefix observations made before ordinary continuation folding.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct LeadingPrefix {
    raw: bool,
    after_bom: bool,
}

impl LeadingPrefix {
    fn new(bytes: &[u8], prefix: u8) -> Self {
        let first = bytes.first().copied();
        Self {
            raw: first == Some(prefix),
            after_bom: first == Some(0xef)
                && bytes
                    .strip_prefix(b"\xef\xbb\xbf")
                    .is_some_and(|rest| rest.first() == Some(&prefix)),
        }
    }

    pub(crate) fn matches(self, bom_removed: bool) -> bool {
        if bom_removed {
            self.after_bom
        } else {
            self.raw
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ParserLine {
    pub logical: LogicalLine,
    pub leading_prefix: LeadingPrefix,
    pub continued_source: Option<Vec<u8>>,
}

/// A reader whose contextual state is supplied by the interpreter.
#[derive(Debug)]
pub struct Reader<'a> {
    source: &'a [u8],
    offset: usize,
    next_physical_line: usize,
    posix: bool,
    strip_crlf: bool,
    nul_input: bool,
    stream_capacity: usize,
    nul_warnings: usize,
}

struct PhysicalLine<'a> {
    bytes: &'a [u8],
    line: usize,
    terminated: bool,
}

impl<'a> Reader<'a> {
    pub fn new(source: &'a [u8]) -> Self {
        Self {
            source,
            offset: 0,
            next_physical_line: 1,
            posix: false,
            strip_crlf: true,
            nul_input: source.contains(&0),
            stream_capacity: 200,
            nul_warnings: 0,
        }
    }

    /// GNU's string reader keeps CR bytes that its file reader removes.
    pub(crate) fn eval(source: &'a [u8]) -> Self {
        Self {
            strip_crlf: false,
            ..Self::new(source)
        }
    }

    /// Select the continuation-whitespace behavior enabled by `.POSIX`.
    ///
    /// The interpreter calls this after reading the special target; it is not
    /// inferred from raw source because target syntax can itself be expanded.
    pub fn set_posix(&mut self, enabled: bool) {
        self.posix = enabled;
    }

    /// Anonymous/caller-line-one eval buffers remove BOMs before classifying
    /// the next logical line. File streams retain their ordinary read path.
    pub(crate) fn skip_eval_bom(&mut self) -> bool {
        if self
            .source
            .get(self.offset..)
            .is_some_and(|rest| rest.starts_with(b"\xef\xbb\xbf"))
        {
            self.offset += 3;
            return true;
        }
        false
    }

    /// Read a logical line using the state that applies at its first byte.
    ///
    /// Outside rule context a prefix byte is ordinary input. Within rule
    /// context, a line starting with `recipe_prefix` preserves backslash-newline
    /// pairs and removes at most one prefix from each continued physical line.
    /// Ordinary continuations collapse surrounding spaces/tabs, except that
    /// POSIX mode preserves whitespace before a continuation and separate joins.
    pub fn next_line(&mut self, recipe_prefix: u8, in_rule_context: bool) -> Option<LogicalLine> {
        self.next_source_line(recipe_prefix, in_rule_context, false)
            .map(|line| line.logical)
    }

    fn next_source_line(
        &mut self,
        recipe_prefix: u8,
        in_rule_context: bool,
        retain_inline: bool,
    ) -> Option<ParserLine> {
        self.nul_warnings = 0;
        if self.nul_input {
            return self.nul_line(recipe_prefix, in_rule_context, retain_inline);
        }
        let start = self.offset;
        let first = self.physical_line()?;
        let leading_prefix = LeadingPrefix::new(first.bytes, recipe_prefix);
        let recipe = in_rule_context && leading_prefix.raw;
        let mut result = LogicalLine {
            bytes: Vec::with_capacity(first.bytes.len()),
            line: first.line,
            end_line: first.line,
            recipe,
        };
        let mut current = first;
        let mut continued = false;

        let trailing_join = loop {
            result.end_line = current.line;
            // Decide physical continuation before removing any recipe prefix:
            // the prefix itself may be a backslash.
            let backslashes = current
                .bytes
                .iter()
                .rev()
                .take_while(|&&b| b == b'\\')
                .count();
            let joins_next = current.terminated && backslashes % 2 == 1;
            let mut bytes = current.bytes;
            if continued {
                if recipe {
                    if bytes.first() == Some(&recipe_prefix) {
                        bytes = &bytes[1..];
                    }
                } else {
                    bytes = trim_start_blanks(bytes);
                }
            }

            if recipe {
                result.bytes.extend_from_slice(bytes);
                if joins_next {
                    result.bytes.push(b'\n');
                }
            } else if joins_next {
                result
                    .bytes
                    .extend_from_slice(&bytes[..bytes.len() - backslashes]);
                result
                    .bytes
                    .extend(std::iter::repeat_n(b'\\', backslashes / 2));
                if !self.posix {
                    while result.bytes.last().is_some_and(|b| is_blank(*b)) {
                        result.bytes.pop();
                    }
                }
                result.bytes.push(b' ');
            } else {
                result.bytes.extend_from_slice(bytes);
            }

            if !joins_next {
                break false;
            }
            let Some(next) = self.physical_line() else {
                break true;
            };
            current = next;
            continued = true;
        };
        let continued_source = if retain_inline && !recipe && (continued || trailing_join) {
            self.inline_source(start, current.terminated && !trailing_join)
        } else {
            None
        };
        Some(ParserLine {
            logical: result,
            leading_prefix,
            continued_source,
        })
    }

    fn inline_source(&self, start: usize, remove_terminator: bool) -> Option<Vec<u8>> {
        let mut source = &self.source[start..self.offset];
        if !source.contains(&b';') {
            return None;
        }
        if remove_terminator {
            source = &source[..source.len() - 1];
            if self.strip_crlf {
                source = source.strip_suffix(b"\r").unwrap_or(source);
            }
        }
        if !self.strip_crlf || !source.contains(&b'\r') {
            return Some(source.to_vec());
        }
        let mut normalized = Vec::with_capacity(source.len());
        for physical in source.split_inclusive(|&byte| byte == b'\n') {
            if let Some(body) = physical.strip_suffix(b"\r\n") {
                normalized.extend_from_slice(body);
                normalized.push(b'\n');
            } else {
                normalized.extend_from_slice(physical);
            }
        }
        Some(normalized)
    }

    /// Warnings belong to the read operation, before interpreting its contents.
    /// Define bodies choose their own diagnostic position for this event.
    pub fn take_nul_warnings(&mut self) -> usize {
        std::mem::take(&mut self.nul_warnings)
    }

    #[cold]
    fn nul_line(
        &mut self,
        recipe_prefix: u8,
        in_rule_context: bool,
        retain_inline: bool,
    ) -> Option<ParserLine> {
        let line = self.next_physical_line;
        let mut raw = Vec::new();
        let mut lines = 0;
        while self.offset < self.source.len() {
            // GNU's file reader retains a growing buffer across logical lines.
            // A read consumes its entire chunk even when C-string observation
            // ends at an embedded NUL. Later reads overwrite the discarded tail.
            // The pinned Linux reader passes the remaining capacity through
            // a 32-bit signed fgets count. Nonpositive counts end the read
            // without consuming input; emulate that boundary without reserving
            // GNU's potentially enormous virtual buffer.
            let read_size = (self.stream_capacity - raw.len()) as i32;
            if read_size <= 0 {
                break;
            }
            let available = read_size as usize - 1;
            let remaining = &self.source[self.offset..];
            let chunk = &remaining[..available.min(remaining.len())];
            let count = chunk
                .iter()
                .position(|&byte| byte == b'\n')
                .map_or(chunk.len(), |position| position + 1);
            let chunk = &chunk[..count];
            self.offset += count;
            let visible = chunk
                .iter()
                .position(|&byte| byte == 0)
                .unwrap_or(chunk.len());
            if visible == 0 {
                self.nul_warnings += 1;
                raw.push(b'\n');
            } else {
                raw.extend_from_slice(&chunk[..visible]);
            }
            if raw.last() == Some(&b'\n') {
                lines += 1;
                if raw.ends_with(b"\r\n") {
                    raw.remove(raw.len() - 2);
                }
                let continuation = raw[..raw.len() - 1]
                    .iter()
                    .rev()
                    .take_while(|&&byte| byte == b'\\')
                    .count()
                    % 2
                    == 1;
                if !continuation {
                    raw.pop();
                    break;
                }
                if self.stream_capacity - raw.len() >= 80 {
                    continue;
                }
            }
            self.stream_capacity = self.stream_capacity.saturating_mul(2);
        }
        if lines == 0 {
            if raw.is_empty() {
                return None;
            }
            lines = 1;
        }
        self.next_physical_line += lines;
        // The retained text is now NUL-free. Reuse ordinary recipe/continuation
        // handling while retaining the file reader's observed line count.
        let mut reader = Reader::new(&raw);
        reader.set_posix(self.posix);
        let mut folded = reader
            .next_source_line(recipe_prefix, in_rule_context, false)
            .unwrap_or(ParserLine {
                logical: LogicalLine {
                    bytes: Vec::new(),
                    line,
                    end_line: line,
                    recipe: false,
                },
                leading_prefix: LeadingPrefix::default(),
                continued_source: None,
            });
        folded.logical.line = line;
        folded.logical.end_line = self.next_physical_line - 1;
        if retain_inline
            && !folded.logical.recipe
            && raw.contains(&b';')
            && raw.windows(2).any(|pair| pair == b"\\\n")
        {
            folded.continued_source = Some(raw);
        }
        Some(folded)
    }

    fn physical_line(&mut self) -> Option<PhysicalLine<'a>> {
        if self.offset == self.source.len() {
            return None;
        }
        let remaining = &self.source[self.offset..];
        let (mut end, terminated) = match remaining.iter().position(|&b| b == b'\n') {
            Some(index) => (self.offset + index, true),
            None => (self.source.len(), false),
        };
        let start = self.offset;
        self.offset = end + usize::from(terminated);
        if terminated && end > start && self.source[end - 1] == b'\r' && self.strip_crlf {
            end -= 1;
        }
        let line = self.next_physical_line;
        self.next_physical_line += 1;
        Some(PhysicalLine {
            bytes: &self.source[start..end],
            line,
            terminated,
        })
    }
}

fn is_blank(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t')
}

fn trim_start_blanks(bytes: &[u8]) -> &[u8] {
    let offset = bytes
        .iter()
        .position(|&b| !is_blank(b))
        .unwrap_or(bytes.len());
    &bytes[offset..]
}

/// Remove an ordinary-line comment, resolving backslash quoting before `#`.
///
/// Variable references protect their contents from comment recognition. `$$`
/// consumes the escaped dollar, so `#` in `$$(text#comment)` is not protected.
/// Matching inside a reference counts its own opening/closing delimiter, rather
/// than interpreting other delimiter types or expansion syntax. This also
/// preserves incomplete references for the expression parser to diagnose.
/// Recipes and `define` bodies must bypass this function.
pub fn strip_comment(line: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity(line.len());
    let mut copied = 0;
    let end = comment_end(line, |index, slashes| {
        result.extend_from_slice(&line[copied..index - slashes]);
        result.extend(std::iter::repeat_n(b'\\', slashes / 2));
        copied = index;
    });
    result.extend_from_slice(&line[copied..end]);
    result
}

pub(crate) fn has_comment(line: &[u8]) -> bool {
    comment_end(line, |_, _| {}) < line.len()
}

fn comment_end(line: &[u8], mut compact: impl FnMut(usize, usize)) -> usize {
    let mut index = 0;
    while let Some(&byte) = line.get(index) {
        match byte {
            b'#' => {
                // The source run can include the byte of a short dollar reference.
                let slashes = line[..index]
                    .iter()
                    .rev()
                    .take_while(|&&b| b == b'\\')
                    .count();
                if slashes != 0 {
                    compact(index, slashes);
                }
                if slashes % 2 == 0 {
                    return index;
                }
                index += 1;
            }
            b'$' => match line.get(index + 1) {
                Some(b'(' | b'{') => {
                    let Ok(close) = crate::variables::reference_end(line, index + 1) else {
                        break;
                    };
                    index = close + 1;
                }
                Some(_) => index += 2,
                None => break,
            },
            _ => index += 1,
        }
    }
    line.len()
}

pub(crate) fn inline_recipe_continuations(source: &[u8], prefix: Option<u8>) -> Vec<u8> {
    if !source.contains(&b'\n') {
        return source.to_vec();
    }
    let mut output = Vec::with_capacity(source.len());
    let mut copied = 0;
    let mut cursor = 0;
    // Fold only the continuations inside a make reference before expansion.
    while let Some(dollar) = source[cursor..].iter().position(|&byte| byte == b'$') {
        let opening = cursor + dollar + 1;
        cursor = opening;
        if !matches!(source.get(opening), Some(b'(' | b'{')) {
            continue;
        }
        let end = crate::variables::reference_end(source, opening).unwrap_or(source.len());
        output.extend_from_slice(&source[copied..opening + 1]);
        fold_recipe_reference(&source[opening + 1..end], &mut output);
        copied = end;
        cursor = end;
    }
    output.extend_from_slice(&source[copied..]);

    // A dollar still has its make expansion meaning at the start of a line.
    if let Some(prefix) = prefix.filter(|&byte| byte != b'$') {
        strip_recipe_continuation_prefix(&mut output, prefix);
    }
    output
}

/// Remove one recipe prefix immediately after each newline in a command.
pub fn strip_recipe_continuation_prefix(bytes: &mut Vec<u8>, prefix: u8) {
    let mut after_newline = false;
    bytes.retain(|&byte| {
        let keep = !(after_newline && byte == prefix);
        after_newline = keep && byte == b'\n';
        keep
    });
}

fn fold_recipe_reference(mut source: &[u8], output: &mut Vec<u8>) {
    let start = output.len();
    while let Some(join) = source.windows(2).position(|pair| pair == b"\\\n") {
        let quoted = source[..join]
            .iter()
            .rev()
            .take_while(|&&byte| byte == b'\\')
            .count()
            % 2
            != 0;
        output.extend_from_slice(&source[..join]);
        source = &source[join + 2..];
        if quoted {
            output.extend_from_slice(b"\\\n");
        } else {
            while output.len() > start && output.last().is_some_and(|&byte| is_blank(byte)) {
                output.pop();
            }
            output.push(b' ');
            let first = source
                .iter()
                .position(|byte| !byte.is_ascii_whitespace() && *byte != 0x0b);
            source = &source[first.unwrap_or(source.len())..];
        }
    }
    output.extend_from_slice(source);
}

#[cfg(test)]
mod tests {

    #[test]
    fn memory_bom_is_removed_before_recipe_continuation_classification() {
        let mut reader = Reader::new(b"\xef\xbb\xbf\tfirst \\\n\tsecond\n");
        assert!(reader.skip_eval_bom());
        assert!(!reader.skip_eval_bom());
        let line = reader.next_line(b'\t', true).unwrap();
        assert!(line.recipe);
        assert_eq!(line.bytes, b"\tfirst \\\nsecond");
    }
    use super::{LogicalLine, Reader, strip_comment};

    fn normal(source: &[u8]) -> LogicalLine {
        Reader::new(source).next_line(b'\t', false).unwrap()
    }

    #[test]
    fn empty_input_has_no_phantom_line() {
        assert!(Reader::new(b"").next_line(b'\t', false).is_none());
    }

    #[test]
    fn nul_reads_append_visible_chunks_and_keep_observed_line_numbers() {
        let mut reader = Reader::new(b"X=one\0discarded\nTWO\n\0discarded\nlast\n");
        let first = reader.next_line(b'\t', false).unwrap();
        assert_eq!(first.bytes, b"X=oneTWO");
        assert_eq!((first.line, first.end_line), (1, 1));
        assert_eq!(reader.take_nul_warnings(), 0);
        let blank = reader.next_line(b'\t', false).unwrap();
        assert!(blank.bytes.is_empty());
        assert_eq!((blank.line, blank.end_line), (2, 2));
        assert_eq!(reader.take_nul_warnings(), 1);
        assert_eq!(reader.take_nul_warnings(), 0);
        let last = reader.next_line(b'\t', false).unwrap();
        assert_eq!(last.bytes, b"last");
        assert_eq!((last.line, last.end_line), (3, 3));
    }

    #[test]
    fn nul_chunk_boundaries_retain_earlier_buffer_growth() {
        let mut source = vec![b'#'];
        source.extend(std::iter::repeat_n(b'x', 300));
        source.extend_from_slice(b"\n#");
        source.extend(std::iter::repeat_n(b'x', 198));
        source.extend_from_slice(b"\0discarded\nnext\nlast\n");
        let mut reader = Reader::new(&source);
        assert_eq!(reader.next_line(b'\t', false).unwrap().bytes.len(), 301);
        let joined = reader.next_line(b'\t', false).unwrap();
        assert!(joined.bytes.ends_with(b"next"));
        assert_eq!(reader.take_nul_warnings(), 0);
        assert_eq!((joined.line, joined.end_line), (2, 2));
        assert_eq!(reader.next_line(b'\t', false).unwrap().line, 3);
    }

    #[test]
    fn physical_locations_and_final_unterminated_line() {
        let mut reader = Reader::new(b"one\n\nthree");
        assert_eq!(
            reader.next_line(b'\t', false).unwrap(),
            LogicalLine {
                bytes: b"one".to_vec(),
                line: 1,
                end_line: 1,
                recipe: false,
            }
        );
        let blank = reader.next_line(b'\t', false).unwrap();
        assert_eq!((blank.line, blank.end_line, blank.bytes), (2, 2, vec![]));
        let final_line = reader.next_line(b'\t', false).unwrap();
        assert_eq!((final_line.line, final_line.end_line), (3, 3));
        assert_eq!(final_line.bytes, b"three");
        assert!(reader.next_line(b'\t', false).is_none());
    }

    #[test]
    fn final_newline_does_not_add_an_empty_line() {
        let mut reader = Reader::new(b"one\n");
        assert_eq!(reader.next_line(b'\t', false).unwrap().bytes, b"one");
        assert!(reader.next_line(b'\t', false).is_none());
    }

    #[test]
    fn crlf_is_a_line_terminator_but_bare_cr_is_data() {
        let mut reader = Reader::new(b"one\r\ntwo\r");
        assert_eq!(reader.next_line(b'\t', false).unwrap().bytes, b"one");
        assert_eq!(reader.next_line(b'\t', false).unwrap().bytes, b"two\r");
    }

    #[test]
    fn normal_continuations_collapse_only_spaces_and_tabs() {
        let line = normal(b"name = first \t\\\n\t  second\n");
        assert_eq!(line.bytes, b"name = first second");
        assert_eq!((line.line, line.end_line), (1, 2));
        assert_eq!(normal(b"a\x0b\\\n\x0cb\n").bytes, b"a\x0b \x0cb");
    }

    #[test]
    fn consecutive_continuations_collapse_to_one_space() {
        let line = normal(b"a\\\n \t\\\n\tb\n");
        assert_eq!(line.bytes, b"a b");
        assert_eq!((line.line, line.end_line), (1, 3));
    }

    #[test]
    fn even_backslash_runs_do_not_continue() {
        for count in [2usize, 4, 6] {
            let mut source = b"a".to_vec();
            source.extend(std::iter::repeat_n(b'\\', count));
            source.extend_from_slice(b"\nb\n");
            let mut reader = Reader::new(&source);
            assert_eq!(
                reader.next_line(b'\t', false).unwrap().bytes,
                source[..count + 1]
            );
            assert_eq!(reader.next_line(b'\t', false).unwrap().bytes, b"b");
        }
    }

    #[test]
    fn odd_backslash_runs_quote_pairs_and_continue() {
        for count in [1usize, 3, 5] {
            let mut source = b"a".to_vec();
            source.extend(std::iter::repeat_n(b'\\', count));
            source.extend_from_slice(b"\nb\n");
            let mut expected = b"a".to_vec();
            expected.extend(std::iter::repeat_n(b'\\', count / 2));
            expected.extend_from_slice(b" b");
            assert_eq!(normal(&source).bytes, expected);
        }
    }

    #[test]
    fn end_of_file_preserves_a_bare_backslash() {
        assert_eq!(normal(b"a\\").bytes, b"a\\");
        let line = normal(b"a\\\n");
        assert_eq!(line.bytes, b"a ");
        assert_eq!((line.line, line.end_line), (1, 1));
    }

    #[test]
    fn inline_recipe_folds_only_references_and_removes_one_physical_prefix() {
        assert_eq!(
            super::inline_recipe_continuations(
                b"$(info left \\\n\t\x0b right) raw\\\n\t\t tail",
                Some(b'\t')
            ),
            b"$(info left right) raw\\\n\t tail"
        );
        assert_eq!(
            super::inline_recipe_continuations(
                b"$(info left \\\n>right) raw\\\n>>tail",
                Some(b'>')
            ),
            b"$(info left >right) raw\\\n>tail"
        );
    }

    #[test]
    fn recipe_continuation_preserves_backslash_and_newline() {
        let line = Reader::new(b"\tprintf first\\\n\t second\n")
            .next_line(b'\t', true)
            .unwrap();
        assert!(line.recipe);
        assert_eq!(line.bytes, b"\tprintf first\\\n second");
        assert_eq!((line.line, line.end_line), (1, 2));
    }

    #[test]
    fn recipe_continuation_removes_only_one_matching_prefix() {
        let line = Reader::new(b">one\\\n>>two\\\n \tthree\n")
            .next_line(b'>', true)
            .unwrap();
        assert_eq!(line.bytes, b">one\\\n>two\\\n \tthree");
        assert_eq!(line.end_line, 3);
    }

    #[test]
    fn recipe_continuation_preserves_backslash_pairs() {
        let source = b"\tone\\\\\\\n\ttwo\n";
        let line = Reader::new(source).next_line(b'\t', true).unwrap();
        assert_eq!(line.bytes, b"\tone\\\\\\\ntwo");
    }

    #[test]
    fn backslash_recipe_prefix_does_not_change_physical_continuation_parity() {
        let source = b"\\first\\\n\\\n\\last\n";
        let line = Reader::new(source).next_line(b'\\', true).unwrap();
        assert_eq!(line.bytes, b"\\first\\\n\nlast");
        assert_eq!(line.end_line, 3);

        let source = b"\\first\\\n\\\\\n\\last\n";
        let mut reader = Reader::new(source);
        let first = reader.next_line(b'\\', true).unwrap();
        assert_eq!(first.bytes, b"\\first\\\n\\");
        assert_eq!(first.end_line, 2);
        let next = reader.next_line(b'\\', true).unwrap();
        assert_eq!(next.bytes, b"\\last");
        assert_eq!(next.line, 3);
    }

    #[test]
    fn recipe_crlf_continuation_normalizes_only_line_endings() {
        let line = Reader::new(b"\tone\\\r\n\ttwo\r\n")
            .next_line(b'\t', true)
            .unwrap();
        assert_eq!(line.bytes, b"\tone\\\ntwo");
    }

    #[test]
    fn recipe_comments_are_retained() {
        let line = Reader::new(b"\t# comment\\\n\tstill a shell comment\n")
            .next_line(b'\t', true)
            .unwrap();
        assert_eq!(line.bytes, b"\t# comment\\\nstill a shell comment");
    }

    #[test]
    fn prefix_changes_apply_to_each_next_logical_line() {
        let mut reader = Reader::new(b">first\n\tsecond\n>third\n");
        assert!(reader.next_line(b'>', true).unwrap().recipe);
        assert!(reader.next_line(b'\t', true).unwrap().recipe);
        assert!(!reader.next_line(b'\t', true).unwrap().recipe);
    }

    #[test]
    fn leading_prefix_outside_rule_context_folds_normally() {
        let line = Reader::new(b"\tNAME = one\\\n\ttwo\n")
            .next_line(b'\t', false)
            .unwrap();
        assert!(!line.recipe);
        assert_eq!(line.bytes, b"\tNAME = one two");
    }

    #[test]
    fn posix_preserves_preceding_whitespace_and_distinct_joins() {
        let mut reader = Reader::new(b"a   \\\n  b\na\\\n \t\\\n b\n");
        reader.set_posix(true);
        assert_eq!(reader.next_line(b'\t', false).unwrap().bytes, b"a    b");
        assert_eq!(reader.next_line(b'\t', false).unwrap().bytes, b"a  b");
    }

    #[test]
    fn non_utf8_bytes_are_preserved_in_both_kinds_of_lines() {
        assert_eq!(normal(b"name=\xff\xfe\n").bytes, b"name=\xff\xfe");
        let line = Reader::new(b"\tprintf \xff\\\n\t\xfe\n")
            .next_line(b'\t', true)
            .unwrap();
        assert_eq!(line.bytes, b"\tprintf \xff\\\n\xfe");
    }

    #[test]
    fn comments_strip_without_trimming_preceding_spaces() {
        assert_eq!(strip_comment(b"name = value  # ignored"), b"name = value  ");
        assert_eq!(strip_comment(b"# whole line"), b"");
        assert_eq!(strip_comment(b"plain\\text"), b"plain\\text");
    }

    #[test]
    fn backslash_parity_controls_hash_quoting() {
        for count in 0usize..=6 {
            let mut input = b"a".to_vec();
            input.extend(std::iter::repeat_n(b'\\', count));
            input.extend_from_slice(b"#tail");
            let mut expected = b"a".to_vec();
            expected.extend(std::iter::repeat_n(b'\\', count / 2));
            if count % 2 == 1 {
                expected.extend_from_slice(b"#tail");
            }
            assert_eq!(strip_comment(&input), expected);
        }
        assert_eq!(strip_comment(b"a\\#b#c"), b"a#b");
    }

    #[test]
    fn references_protect_hashes_but_escaped_dollars_do_not() {
        assert_eq!(
            strip_comment(b"$(subst #,word,#) #drop"),
            b"$(subst #,word,#) "
        );
        assert_eq!(
            strip_comment(b"${subst #,word,#}#drop"),
            b"${subst #,word,#}"
        );
        assert_eq!(strip_comment(b"$$(before#after)"), b"$$(before");
        assert_eq!(strip_comment(b"a$#b"), b"a$#b");
        assert_eq!(strip_comment(b"a$$#b"), b"a$$");
    }

    #[test]
    fn reference_delimiters_nest_by_their_own_kind() {
        assert_eq!(
            strip_comment(b"$(before(inner)#kept) #drop"),
            b"$(before(inner)#kept) "
        );
        assert_eq!(
            strip_comment(b"${before{inner}#kept} #drop"),
            b"${before{inner}#kept} "
        );
        assert_eq!(
            strip_comment(b"$(outer${inner)#after})"),
            b"$(outer${inner)"
        );
        assert_eq!(
            strip_comment(b"${outer$(inner}#after)}"),
            b"${outer$(inner}"
        );
        assert_eq!(strip_comment(b"$(x$)#outside)"), b"$(x$)");
    }

    #[test]
    fn incomplete_references_and_quoted_hashes_inside_them_are_preserved() {
        assert_eq!(strip_comment(b"$(unfinished#kept"), b"$(unfinished#kept");
        assert_eq!(strip_comment(b"$(name\\#part)#drop"), b"$(name\\#part)");
        assert_eq!(strip_comment(b"final$"), b"final$");
    }

    #[test]
    fn comments_are_removed_after_logical_line_folding() {
        let line = normal(b"NAME = value # ignored\\\nALSO = ignored\n");
        assert_eq!(strip_comment(&line.bytes), b"NAME = value ");
        assert_eq!(line.end_line, 2);
    }
}
