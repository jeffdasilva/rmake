// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Lazy implicit-prerequisite words and literal-safe stem substitution.

pub struct Words<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> Words<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }
}

impl<'a> Iterator for Words<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<Self::Item> {
        while self
            .bytes
            .get(self.cursor)
            .is_some_and(|byte| byte.is_ascii_whitespace() || *byte == 0x0b)
        {
            self.cursor += 1;
        }
        let start = self.cursor;
        while let Some(&byte) = self.bytes.get(self.cursor) {
            match byte {
                0 | b' ' | b'\t' => break,
                b'|' => {
                    self.cursor += 1;
                    break;
                }
                b'$' => {
                    self.cursor += 1;
                    if let Some(&opening) = self.bytes.get(self.cursor) {
                        self.cursor = if matches!(opening, b'(' | b'{') {
                            crate::variables::reference_end(self.bytes, self.cursor)
                                .map_or(self.bytes.len(), |end| end + 1)
                        } else {
                            self.cursor + 1
                        };
                    }
                }
                _ => self.cursor += 1,
            }
        }
        if self.cursor == start {
            None
        } else {
            Some(&self.bytes[start..self.cursor])
        }
    }
}

/// Replace the first percent in each whitespace word, including within a
/// surviving reference. Expand the stem as a simple automatic value later so
/// dollar bytes in filenames cannot become another make expression.
pub fn stem_references(bytes: &[u8], basename: bool) -> (Vec<u8>, bool) {
    let mut result = Vec::with_capacity(bytes.len());
    let mut replaced = false;
    let mut found = false;
    for &byte in bytes {
        if byte.is_ascii_whitespace() || byte == 0x0b {
            replaced = false;
        }
        if byte == b'%' && !replaced {
            result.extend_from_slice(if basename { b"$(*F)" } else { b"$*" });
            replaced = true;
            found = true;
        } else {
            result.push(byte);
        }
    }
    (result, found)
}

#[cfg(test)]
mod tests {
    use super::{Words, stem_references};

    #[test]
    fn words_keep_nested_references_together_and_stop_at_top_level_pipes() {
        let bytes = b"first $(strip a $(V)) ${addprefix x, y} tail|last $$()";
        assert_eq!(
            Words::new(bytes).collect::<Vec<_>>(),
            [
                b"first".as_slice(),
                b"$(strip a $(V))",
                b"${addprefix x, y}",
                b"tail|",
                b"last",
                b"$$()",
            ]
        );
    }

    #[test]
    fn malformed_late_references_remain_lazy_and_nul_ends_the_sequence() {
        assert_eq!(
            Words::new(b"missing $(unterminated tail").collect::<Vec<_>>(),
            [b"missing".as_slice(), b"$(unterminated tail"]
        );
        assert_eq!(
            Words::new(b"first\0ignored").collect::<Vec<_>>(),
            [b"first".as_slice()]
        );
    }

    #[test]
    fn stem_references_rewrite_only_the_first_percent_per_whitespace_word() {
        assert_eq!(
            stem_references(b"$(patsubst %.in,%.out,$*) %.x%.y", false),
            (b"$(patsubst $*.in,%.out,$*) $*.x%.y".to_vec(), true)
        );
        assert_eq!(
            stem_references(b"%.in %\xff.src", true),
            (b"$(*F).in $(*F)\xff.src".to_vec(), true)
        );
        assert_eq!(
            stem_references(b"$(INPUT)", true),
            (b"$(INPUT)".to_vec(), false)
        );
    }

    #[test]
    fn leading_control_blanks_and_internal_word_bytes_are_distinct() {
        assert_eq!(
            Words::new(b"\x0bfirst\x0bsecond next").collect::<Vec<_>>(),
            [b"first\x0bsecond".as_slice(), b"next"]
        );
        assert_eq!(
            stem_references(b"%.in\x0b%.out", false),
            (b"$*.in\x0b$*.out".to_vec(), true)
        );
    }
}
