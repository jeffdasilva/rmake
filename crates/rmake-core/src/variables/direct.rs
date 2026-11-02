// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Bounded direct preparation; eligibility never evaluates an expression twice.
use super::linear::Tail;
use super::*;

pub(super) fn references_only(input: &[u8]) -> bool {
    let mut cursor = 0;
    while cursor < input.len() {
        if input[cursor] != b'$' || cursor + 1 == input.len() {
            cursor += 1;
        } else if input[cursor + 1] == b'$' {
            cursor += 2;
        } else if matches!(input[cursor + 1], b'(' | b'{') {
            let start = cursor + 2;
            let mut end = start;
            while input
                .get(end)
                .is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_')
            {
                end += 1;
            }
            let close = if input[cursor + 1] == b'(' {
                b')'
            } else {
                b'}'
            };
            if end == start || input.get(end) != Some(&close) {
                return false;
            }
            cursor = end + 1;
        } else {
            cursor += 2;
        }
    }
    true
}

impl Expansion<'_, '_> {
    pub(super) fn pure_bytes(&self, input: &[u8]) -> Option<Result<Vec<u8>, ExpandError>> {
        use std::borrow::Cow;
        let input = c_string_prefix(input);
        if input.len() > MAX_OUTPUT {
            return Some(Err(ExpandError::ExpansionLimit));
        }
        if !input.contains(&b'$') {
            return Some(Ok(input.to_vec()));
        }
        let mut output = Vec::new();
        let mut cursor = 0;
        let mut literal = 0;
        while cursor < input.len() {
            if input[cursor] != b'$' {
                cursor += input[cursor..]
                    .iter()
                    .position(|b| *b == b'$')
                    .unwrap_or(input.len() - cursor);
                continue;
            }
            if cursor + 1 == input.len() || input[cursor + 1] == b'$' {
                push(&mut output, &input[literal..cursor]).ok()?;
                push(&mut output, b"$").ok()?;
                cursor += if cursor + 1 == input.len() { 1 } else { 2 };
                literal = cursor;
                continue;
            }
            let (name, next) = if matches!(input[cursor + 1], b'(' | b'{') {
                let (end, next, raw) = expansion_reference_span(input, cursor + 1).ok()?;
                // The flag is GNU's malformed nested-name fallback. Only
                // ordinary ASCII literal references use this direct path.
                if raw || literal_alias(&input[cursor..next]).is_none() {
                    return None;
                }
                (&input[cursor + 2..end], next)
            } else {
                (&input[cursor + 1..cursor + 2], cursor + 2)
            };
            if name.contains(&b':') {
                return None;
            }
            let (variable, layer) = self
                .bindings
                .iter()
                .rev()
                .find_map(|scope| scope.get(name))
                .map(|v| (v, None))
                .or_else(|| {
                    self.store
                        .lookup_from(name, 0)
                        .map(|(layer, v)| (v, Some(layer)))
                })?;
            if variable.scope.variable_names {
                return None;
            }
            let value = if variable.flavor == Flavor::Simple {
                Cow::Borrowed(variable.value.as_slice())
            } else if !variable.scope.append_outer && !variable.value.contains(&b'$') {
                if self.active_keys().any(|(n, l)| n == name && *l == layer) {
                    return None;
                }
                Cow::Borrowed(c_string_prefix(&variable.value))
            } else if literal_alias(&variable.value).is_some()
                && let Some(LiteralChain::Value(value)) = self.literal_chain(name, variable, layer)
            {
                Cow::Owned(value)
            } else if let Some(mut plan) = self.linear_plan(name, variable, layer) {
                let Tail::Value(value) = std::mem::replace(&mut plan.tail, Tail::Value(Vec::new()))
                else {
                    return None;
                };
                Cow::Owned(plan.finish(value).ok()?)
            } else {
                return None;
            };
            // Delay literal copying until eligibility is known. Rejected
            // prefixes cannot perform effects or retain an expanded value.
            push(&mut output, &input[literal..cursor]).ok()?;
            push(&mut output, &value).ok()?;
            cursor = next;
            literal = next;
        }
        push(&mut output, &input[literal..]).ok()?;
        if let Some(nul) = output.iter().position(|b| *b == 0) {
            output.truncate(nul);
        }
        Some(Ok(output))
    }

    pub(super) fn direct_expression(
        &mut self,
        input: &[u8],
    ) -> Option<Result<Vec<u8>, ExpandError>> {
        // A recognized eager function cannot be a pure literal reference.
        // Reject that preparation with a short name scan, before walking its
        // balanced argument syntax. This selects dispatch, never a value.
        let eager_prefix = matches!(input.get(..2), Some(b"$(" | b"${")) && {
            let length = input[2..]
                .iter()
                .take_while(|b| b.is_ascii_alphabetic() || **b == b'-')
                .count();
            let name = &input[2..2 + length];
            input.get(2 + length).is_some_and(gnu_whitespace)
                && arity(name).is_some()
                && !super::machine::lazy(name)
        };
        if !eager_prefix && let Some(result) = self.pure_bytes(input) {
            return Some(result);
        }
        if !matches!(input.get(..2), Some(b"$(" | b"${")) {
            return None;
        }
        let (end, next, raw) = expansion_reference_span(input, 1).ok()?;
        if raw || next != input.len() {
            return None;
        }
        let body = &input[2..end];
        let end = body.iter().position(gnu_whitespace)?;
        let name = &body[..end];
        let (minimum, maximum) = arity(name)?;
        if super::machine::lazy(name) {
            return None;
        }
        if name == b"shell"
            && (!self.context.has_shell() || matches!(self.store, StoreAccess::Read(_)))
        {
            return Some(Err(ExpandError::Unsupported(name.to_vec())));
        }
        let argument = trim_function_start(&body[end..]);
        let ranges = split_arg_ranges(argument, input[1], maximum).ok()?;
        if ranges.len() < minimum && !matches!(name, b"word" | b"wordlist") {
            return Some(Err(too_few_arguments(name, ranges.len())));
        }
        // Capture the dispatch decision before any argument effects. Once native
        // preparation starts this invocation must not fall back and replay them.
        let native = self.depth < DIRECT_DEPTH && self.machine_nesting < DIRECT_MACHINES;
        let mut values = Vec::with_capacity(ranges.len());
        for range in ranges {
            let argument = &argument[range];
            let prepared = if native {
                self.expand(argument)
            } else if let Some(result) = self.pure_bytes(argument) {
                result
            } else {
                return None;
            };
            match prepared {
                Ok(value) => values.push(value),
                Err(error) => return Some(Err(error)),
            }
        }
        if self.context.is_cancelled() {
            return Some(Err(ExpandError::Cancelled));
        }
        Some(self.eager_function(name, &values))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostic::OutputReceiver;
    use std::cell::Cell;
    #[test]
    fn rejected_preparation_does_not_query_live_warning_policy() {
        struct Receiver {
            queries: Cell<usize>,
            infos: usize,
            warnings: usize,
        }
        impl OutputReceiver for Receiver {
            fn warns_undefined(&self) -> bool {
                let n = self.queries.get() + 1;
                self.queries.set(n);
                n == 2
            }
            fn emit(&mut self, event: Output<'_>) {
                match event {
                    Output::Info(_) => self.infos += 1,
                    Output::Warning { .. } => self.warnings += 1,
                }
            }
        }
        let store = VariableStore::new();
        let mut receiver = Receiver {
            queries: Cell::new(0),
            infos: 0,
            warnings: 0,
        };
        assert_eq!(
            store
                .expand_with(
                    b"$(info $(MISSING))$(MISSING)",
                    ExpansionContext::with_receiver(None, &mut receiver)
                )
                .unwrap(),
            b""
        );
        assert_eq!(
            (receiver.queries.get(), receiver.infos, receiver.warnings),
            (2, 1, 1)
        );
    }
    #[test]
    fn eager_preparation_crosses_heap_dispatch_without_replaying_an_earlier_argument() {
        let mut store = VariableStore::new();
        for index in 0..1024 {
            store
                .define(
                    format!("R{index}").as_bytes(),
                    format!("$(strip [$(R{})])", index + 1).as_bytes(),
                    Assignment::Recursive,
                    Origin::File,
                )
                .unwrap();
        }
        store
            .define_literal(b"R1024", b"end", Origin::File)
            .unwrap();
        let mut writes = 0;
        let mut filesystem = |_: FilesystemEffect<'_>| -> Result<Vec<u8>, FilesystemError> {
            writes += 1;
            Ok(Vec::new())
        };
        let value = store
            .expand_with(
                b"$(subst $(file >>seen,E)q,q,$(R0))",
                ExpansionContext::pure().with_filesystem(&mut filesystem),
            )
            .unwrap();
        assert_eq!(
            value,
            [vec![b'['; 1024], b"end".to_vec(), vec![b']'; 1024]].concat()
        );
        assert_eq!(writes, 1);
    }

    #[test]
    fn later_syntax_failure_preserves_an_earlier_effect_once() {
        let store = VariableStore::new();
        let mut writes = 0;
        let mut filesystem = |_: FilesystemEffect<'_>| -> Result<Vec<u8>, FilesystemError> {
            writes += 1;
            Ok(Vec::new())
        };
        assert!(matches!(
            store.expand_with(
                b"$(file >seen,E)$(if yes,",
                ExpansionContext::pure().with_filesystem(&mut filesystem)
            ),
            Err(ExpandError::UnterminatedFunction { .. })
        ));
        assert_eq!(writes, 1);
    }
}
