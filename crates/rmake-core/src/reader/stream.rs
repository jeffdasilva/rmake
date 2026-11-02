// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Live source ownership and contextual logical reads. No I/O lives in core.
use super::{LogicalLine, Reader};

/// A host-owned source stream. Each request consumes at most count-1 bytes,
/// ending at a physical newline, and returns its visible C-string prefix.
/// Some(empty) records an actual read beginning with NUL; None denotes EOF.
/// A sticky read error is selected after the current logical read completes.
pub trait Source {
    fn fragment(&mut self, count: usize) -> Option<Vec<u8>>;
    fn error(&self) -> Option<Vec<u8>>;
    fn finish(&mut self) {}
    fn retained_bytes(&self) -> usize {
        0
    }
}

/// Source ownership survives every nested include/eval callback until parsing
/// completes. In-memory inputs retain their existing independent reader path.
pub enum Input {
    Bytes(Vec<u8>),
    Stream(Box<dyn Source>),
}

struct OwnedBytes {
    bytes: Vec<u8>,
    offset: usize,
}
impl Source for OwnedBytes {
    fn fragment(&mut self, count: usize) -> Option<Vec<u8>> {
        if self.offset == self.bytes.len() {
            return None;
        }
        let rest = &self.bytes[self.offset..];
        let available = (count - 1).min(rest.len());
        let count = rest[..available]
            .iter()
            .position(|b| *b == b'\n')
            .map_or(available, |i| i + 1);
        let chunk = &rest[..count];
        self.offset += count;
        let end = chunk.iter().position(|b| *b == 0).unwrap_or(count);
        Some(chunk[..end].to_vec())
    }
    fn error(&self) -> Option<Vec<u8>> {
        None
    }
    fn retained_bytes(&self) -> usize {
        self.bytes.capacity()
    }
}

pub(crate) enum ParserReader<'a> {
    Memory(Reader<'a>),
    Stream(StreamReader),
}

impl<'a> ParserReader<'a> {
    pub(crate) fn memory(bytes: &'a [u8]) -> Self {
        Self::Memory(Reader::new(bytes))
    }
    pub(crate) fn stream(source: Box<dyn Source>) -> Self {
        Self::Stream(StreamReader::new(source))
    }
    pub(crate) fn owned(input: Input) -> Self {
        match input {
            Input::Bytes(bytes) => Self::stream(Box::new(OwnedBytes { bytes, offset: 0 })),
            Input::Stream(source) => Self::stream(source),
        }
    }
    pub(crate) fn retained_bytes(&self) -> usize {
        match self {
            Self::Memory(_) => 0,
            Self::Stream(r) => r.source.retained_bytes(),
        }
    }
    pub(crate) fn set_posix(&mut self, enabled: bool) {
        match self {
            Self::Memory(r) => r.set_posix(enabled),
            Self::Stream(r) => r.posix = enabled,
        }
    }
    pub(crate) fn skip_eval_bom(&mut self) {
        if let Self::Memory(r) = self {
            r.skip_eval_bom();
        }
    }
    pub(crate) fn next_line(
        &mut self,
        prefix: u8,
        rule: bool,
    ) -> Result<Option<LogicalLine>, Vec<u8>> {
        match self {
            Self::Memory(r) => Ok(r.next_line(prefix, rule)),
            Self::Stream(r) => r.next_line(prefix, rule),
        }
    }
    pub(crate) fn finish(&mut self) {
        if let Self::Stream(r) = self {
            r.source.finish();
        }
    }
    pub(crate) fn take_nul_warnings(&mut self) -> usize {
        match self {
            Self::Memory(r) => r.take_nul_warnings(),
            Self::Stream(r) => std::mem::take(&mut r.nul_warnings),
        }
    }
}

pub(crate) struct StreamReader {
    source: Box<dyn Source>,
    next_line: usize,
    capacity: usize,
    posix: bool,
    nul_warnings: usize,
}
impl StreamReader {
    fn new(source: Box<dyn Source>) -> Self {
        Self {
            source,
            next_line: 1,
            capacity: 200,
            posix: false,
            nul_warnings: 0,
        }
    }
    fn next_line(&mut self, prefix: u8, rule: bool) -> Result<Option<LogicalLine>, Vec<u8>> {
        self.nul_warnings = 0;
        let line = self.next_line;
        let mut raw = Vec::new();
        let mut lines = 0;
        loop {
            // Preserve the pinned signed-count boundary without reserving a
            // potentially huge virtual capacity in the interpreter.
            let count = (self.capacity - raw.len()) as i32;
            if count <= 0 {
                break;
            }
            let Some(chunk) = self.source.fragment(count as usize) else {
                break;
            };
            if chunk.len() >= count as usize || chunk.contains(&0) {
                return Err(b"rmake: invalid source fragment".to_vec());
            }
            if chunk.is_empty() {
                self.nul_warnings += 1;
                raw.push(b'\n');
            } else if raw.is_empty() {
                raw = chunk;
            } else {
                raw.extend_from_slice(&chunk);
            }
            if raw.last() == Some(&b'\n') {
                lines += 1;
                if raw.ends_with(b"\r\n") {
                    raw.remove(raw.len() - 2);
                }
                let continued = raw[..raw.len() - 1]
                    .iter()
                    .rev()
                    .take_while(|&&b| b == b'\\')
                    .count()
                    % 2
                    == 1;
                if !continued {
                    raw.pop();
                    break;
                }
                if self.capacity - raw.len() >= 80 {
                    continue;
                }
            }
            self.capacity = self.capacity.saturating_mul(2);
        }
        if let Some(error) = self.source.error() {
            return Err(error);
        }
        if lines == 0 {
            if raw.is_empty() {
                return Ok(None);
            }
            lines = 1;
        }
        self.next_line += lines;
        if !raw.windows(2).any(|p| p == b"\\\n") {
            let recipe = rule && raw.first() == Some(&prefix);
            return Ok(Some(LogicalLine {
                bytes: raw,
                line,
                end_line: self.next_line - 1,
                recipe,
            }));
        }
        let mut reader = Reader::new(&raw);
        reader.set_posix(self.posix);
        let mut folded = reader.next_line(prefix, rule).unwrap_or(LogicalLine {
            bytes: Vec::new(),
            line,
            end_line: line,
            recipe: false,
        });
        folded.line = line;
        folded.end_line = self.next_line - 1;
        Ok(Some(folded))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostic::{
        ExpansionContext, FilesystemEffect, FilesystemError, IncludedFile, Output,
    };
    use crate::parser::Database;
    use std::cell::Cell;
    use std::rc::Rc;

    #[test]
    fn physical_stream_chunks_keep_nuls_continuations_and_retained_capacity() {
        for source in [
            b"A=a\\\n b\n\t@echo x\\\n\t y\n".as_slice(),
            b"A=a\0hidden\nB=b\n",
            b"\0discard\nX=y\r\nfinal",
            b"A=x\\\r\n y\n",
        ] {
            for posix in [false, true] {
                for rule in [false, true] {
                    let mut memory = Reader::new(source);
                    memory.set_posix(posix);
                    let mut stream = ParserReader::owned(Input::Bytes(source.to_vec()));
                    stream.set_posix(posix);
                    loop {
                        let expected = memory.next_line(b'\t', rule);
                        let actual = stream.next_line(b'\t', rule).unwrap();
                        assert_eq!(actual, expected);
                        assert_eq!(stream.take_nul_warnings(), memory.take_nul_warnings());
                        if actual.is_none() {
                            break;
                        }
                    }
                }
            }
        }
    }

    struct Failing {
        next: usize,
        closed: Rc<Cell<usize>>,
    }
    impl Source for Failing {
        fn fragment(&mut self, _: usize) -> Option<Vec<u8>> {
            self.next += 1;
            (self.next == 1).then(|| b"$(info BEFORE)\n".to_vec())
        }
        fn error(&self) -> Option<Vec<u8>> {
            (self.next > 1).then(|| b"Input/output error".to_vec())
        }
    }
    impl Drop for Failing {
        fn drop(&mut self) {
            self.closed.set(self.closed.get() + 1);
        }
    }
    #[test]
    fn source_error_keeps_prior_effect_and_releases_the_live_reader() {
        let closed = Rc::new(Cell::new(0));
        let mut db = Database::default();
        let mut events = Vec::new();
        let err = db
            .read_input(
                Input::Stream(Box::new(Failing {
                    next: 0,
                    closed: closed.clone(),
                })),
                b"input.mk",
                ExpansionContext::new(None, &mut |event| {
                    if let Output::Info(s) = event {
                        events.push(s.to_vec());
                    }
                }),
            )
            .unwrap_err();
        assert_eq!(events, [b"BEFORE".to_vec()]);
        assert!(err.unlocated);
        assert_eq!(err.message.as_bytes(), b"input.mk: Input/output error");
        assert_eq!(closed.get(), 1);
        db.read(b"AFTER=yes\n", b"after.mk").unwrap();
    }

    #[test]
    fn cancellation_during_a_source_read_prevents_that_lines_effect() {
        use std::sync::atomic::{AtomicBool, Ordering};
        // Runtime streams own their state; a shared atomic can be owned by Arc.
        struct OwnedCancel(std::sync::Arc<AtomicBool>);
        impl Source for OwnedCancel {
            fn fragment(&mut self, _: usize) -> Option<Vec<u8>> {
                self.0.store(true, Ordering::Relaxed);
                Some(b"$(file >forbidden)\n".to_vec())
            }
            fn error(&self) -> Option<Vec<u8>> {
                None
            }
        }
        let cancelled = std::sync::Arc::new(AtomicBool::new(false));
        let mut writes = 0;
        let mut filesystem = |_: FilesystemEffect<'_>| -> Result<Vec<u8>, FilesystemError> {
            writes += 1;
            Ok(Vec::new())
        };
        let mut db = Database::default();
        assert!(
            db.read_input(
                Input::Stream(Box::new(OwnedCancel(cancelled.clone()))),
                b"m",
                ExpansionContext::pure()
                    .with_filesystem(&mut filesystem)
                    .with_cancellation(&cancelled)
            )
            .is_err()
        );
        assert_eq!(writes, 0);
    }

    #[test]
    fn thousands_of_include_frames_resume_parent_output_and_conditions() {
        let depth = 2048;
        let mut db = Database::default();
        let mut output = Vec::new();
        let mut read = |name: &[u8]| {
            let index = std::str::from_utf8(name).unwrap().parse::<usize>().unwrap();
            let next = if index + 1 < depth {
                format!("include {}\n", index + 1)
            } else {
                String::new()
            };
            Ok(IncludedFile {
                name: name.to_vec(),
                input: Input::Bytes(
                    format!("ifeq (yes,yes)\n$(info b{index})\n{next}$(info a{index})\nendif\n")
                        .into_bytes(),
                ),
            })
        };
        db.read_with(
            b"include 0\n$(info DONE)\n",
            b"m",
            ExpansionContext::new(None, &mut |e| {
                if let Output::Info(s) = e {
                    output.push(s.to_vec());
                }
            })
            .with_includes(&mut read),
        )
        .unwrap();
        let mut expected = (0..depth)
            .map(|i| format!("b{i}").into_bytes())
            .collect::<Vec<_>>();
        expected.extend((0..depth).rev().map(|i| format!("a{i}").into_bytes()));
        expected.push(b"DONE".to_vec());
        assert_eq!(output, expected);
    }

    #[test]
    fn eval_cycle_identity_survives_reentry_and_error_cleanup() {
        // The platform-free API deliberately has no stack-growth adapter.
        // Give this native reentry test an explicit host stack allocation.
        std::thread::Builder::new()
            .stack_size(16 * 1024 * 1024)
            .spawn(|| {
                let mut db = Database::default();
                let mut source = String::new();
                for index in 0..100 {
                    source += &format!("R{index}=$(eval $$(R{}))\n", index + 1);
                }
                source += "R100=$(R0)\n$(R0)\n";
                let error = db.read(source.as_bytes(), b"cycle.mk").unwrap_err();
                assert_eq!(
                    error.message.as_bytes(),
                    b"Recursive variable 'R0' references itself (eventually)"
                );
                assert_eq!(error.source_location().unwrap().line, 1);
                assert!(db.evaluation_active.is_none());
                assert!(db.evaluation_bindings.is_empty());
                db.read_evaluated(b"AFTER=yes\n", None, ExpansionContext::pure())
                    .unwrap();
                assert_eq!(db.variables.expand(b"$(AFTER)").unwrap(), b"yes");
            })
            .unwrap()
            .join()
            .unwrap();
    }

    struct Reported {
        closed: Rc<Cell<usize>>,
        reads: Rc<Cell<usize>>,
    }
    impl Source for Reported {
        fn fragment(&mut self, _: usize) -> Option<Vec<u8>> {
            self.reads.set(self.reads.get() + 1);
            Some(b"$(error forbidden)\n".to_vec())
        }
        fn error(&self) -> Option<Vec<u8>> {
            None
        }
        fn retained_bytes(&self) -> usize {
            usize::MAX
        }
    }
    impl Drop for Reported {
        fn drop(&mut self) {
            self.closed.set(self.closed.get() + 1);
        }
    }
    #[test]
    fn additional_reader_storage_rejects_before_read_and_restores_depth() {
        let closed = Rc::new(Cell::new(0));
        let reads = Rc::new(Cell::new(0));
        let mut db = Database::default();
        let mut read = |name: &[u8]| {
            let i = std::str::from_utf8(name).unwrap().parse::<usize>().unwrap();
            Ok(IncludedFile {
                name: name.to_vec(),
                input: if i == 127 {
                    Input::Stream(Box::new(Reported {
                        closed: closed.clone(),
                        reads: reads.clone(),
                    }))
                } else {
                    Input::Bytes(format!("include {}\n", i + 1).into_bytes())
                },
            })
        };
        let error = db
            .read_with(
                b"include 0\n",
                b"m",
                ExpansionContext::pure().with_includes(&mut read),
            )
            .unwrap_err();
        assert_eq!(
            error.message.as_bytes(),
            b"rmake: include continuation storage limit exceeded"
        );
        assert_eq!((closed.get(), reads.get()), (1, 0));
        let mut read = |_: &[u8]| {
            Ok(IncludedFile {
                name: b"ok".to_vec(),
                input: Input::Bytes(b"V=done\n".to_vec()),
            })
        };
        db.read_with(
            b"include ok\n",
            b"m",
            ExpansionContext::pure().with_includes(&mut read),
        )
        .unwrap();
        assert_eq!(db.variables.expand(b"$(V)").unwrap(), b"done");
    }
}
