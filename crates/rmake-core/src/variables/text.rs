// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Borrowed pure-text syntax, bounded temporary pieces and iterative evaluation.
use super::*;
use std::collections::{HashMap, HashSet};
use std::ops::Range;

#[derive(Clone)]
enum Op {
    Bytes(Range<usize>),
    Dollar,
    Name(Range<usize>),
    Reference {
        node: usize,
        definition: Option<Arc<Location>>,
    },
    Missing(Range<usize>),
}

enum Body<'a> {
    Simple { bytes: &'a [u8], nul: Option<usize> },
    Text { bytes: &'a [u8], ops: Range<usize> },
}
impl Body<'_> {
    fn bytes(&self) -> &[u8] {
        match self {
            Self::Simple { bytes, .. } | Self::Text { bytes, .. } => bytes,
        }
    }
}
struct Node<'a> {
    name: &'a [u8],
    body: Body<'a>,
    externally_active: bool,
}
struct Program<'a> {
    nodes: Vec<Node<'a>>,
    ops: Vec<Op>,
}
#[derive(Default)]
struct Budget {
    used: usize,
}
impl Budget {
    fn charge(&mut self, bytes: usize) -> Option<()> {
        self.used = self.used.checked_add(bytes)?;
        (self.used <= MAX_OUTPUT).then_some(())
    }
}
fn bounded_push<T>(items: &mut Vec<T>, item: T, budget: &mut Budget) -> Option<()> {
    if items.len() == items.capacity() {
        let next = items.capacity().max(2).checked_mul(2)?;
        let extra = next - items.capacity();
        budget.charge(extra.checked_mul(std::mem::size_of::<T>())?)?;
        items.try_reserve_exact(extra).ok()?;
    }
    items.push(item);
    Some(())
}
fn operations(bytes: &[u8], ops: &mut Vec<Op>, budget: &mut Budget) -> Option<Range<usize>> {
    let start = ops.len();
    let mut cursor = 0;
    while cursor < bytes.len() {
        let op = if bytes[cursor] != b'$' {
            let start = cursor;
            cursor += bytes[cursor..]
                .iter()
                .position(|byte| *byte == b'$')
                .unwrap_or(bytes.len() - cursor);
            Op::Bytes(start..cursor)
        } else if cursor + 1 == bytes.len() {
            cursor += 1;
            Op::Dollar
        } else if bytes[cursor + 1] == b'$' {
            cursor += 2;
            Op::Dollar
        } else {
            let (range, next) = if matches!(bytes[cursor + 1], b'(' | b'{') {
                let (end, next, _) = expansion_reference_span(bytes, cursor + 1).ok()?;
                (cursor + 2..end, next)
            } else {
                (cursor + 1..cursor + 2, cursor + 2)
            };
            literal_alias(&bytes[cursor..next])?;
            cursor = next;
            Op::Name(range)
        };
        bounded_push(ops, op, budget)?;
    }
    Some(start..ops.len())
}

#[derive(Clone, Copy)]
struct Span {
    piece: usize,
    len: usize,
}
type Value = Option<Span>;
enum PieceKind {
    Bytes { node: usize, range: Range<usize> },
    Dollar,
    Join { left: Span, right: Span },
}
struct Piece {
    kind: PieceKind,
    nul: Option<usize>,
}
enum TrialError {
    Budget,
    Evaluation(ExpandError),
}
impl TrialError {
    fn at(self, location: Option<&Arc<Location>>) -> Self {
        match self {
            Self::Evaluation(error) => Self::Evaluation(error.at(location)),
            other => other,
        }
    }
}
#[derive(Default)]
struct Arena {
    pieces: Vec<Piece>,
    budget: Budget,
}
impl Arena {
    fn add(
        &mut self,
        kind: PieceKind,
        len: usize,
        nul: Option<usize>,
    ) -> Result<Value, TrialError> {
        if len == 0 {
            return Ok(None);
        }
        let piece = self.pieces.len();
        bounded_push(&mut self.pieces, Piece { kind, nul }, &mut self.budget)
            .ok_or(TrialError::Budget)?;
        Ok(Some(Span { piece, len }))
    }
    fn nul(&self, value: Span) -> Option<usize> {
        self.pieces[value.piece]
            .nul
            .filter(|offset| *offset < value.len)
    }
    fn join(&mut self, left: Value, right: Value) -> Result<Value, TrialError> {
        let (Some(left), Some(right)) = (left, right) else {
            return Ok(left.or(right));
        };
        let len = left.len.saturating_add(right.len);
        if len > MAX_OUTPUT {
            return Err(TrialError::Evaluation(ExpandError::ExpansionLimit));
        }
        let nul = self
            .nul(left)
            .or_else(|| self.nul(right).map(|offset| left.len + offset));
        self.add(PieceKind::Join { left, right }, len, nul)
    }
    fn truncate(&self, value: Value) -> Value {
        value.and_then(|mut span| {
            if let Some(nul) = self.nul(span) {
                span.len = nul;
            }
            (span.len != 0).then_some(span)
        })
    }
}
struct Frame {
    node: usize,
    cursor: usize,
    value: Value,
    nearest: Option<Arc<Location>>,
}
struct Warning {
    node: usize,
    range: Range<usize>,
    nearest: Option<Arc<Location>>,
}
struct Prepared {
    arena: Arena,
    warnings: Vec<Warning>,
    result: Result<Value, ExpandError>,
}

impl Expansion<'_, '_> {
    fn text_program<'c, 'active>(
        store: &'c StoreAccess<'_>,
        bindings: &'c [LocalFrame],
        active: (
            usize,
            impl Iterator<Item = &'active (Vec<u8>, Option<usize>)>,
        ),
        context: &ExpansionContext<'_>,
        name: &'c [u8],
        variable: &'c Variable,
        layer: Option<usize>,
    ) -> Option<Program<'c>> {
        let mut budget = Budget::default();
        // Conservative allowance covers indexed source/node/frame bookkeeping;
        // instruction capacity and source-work bytes are charged separately.
        budget.charge(1024 + name.len() + variable.value.len())?;
        budget.charge(active.0.checked_mul(128)?)?;
        let mut instructions = Vec::new();
        let root_ops = operations(
            c_string_prefix(&variable.value),
            &mut instructions,
            &mut budget,
        )?;
        if !instructions[root_ops.clone()]
            .iter()
            .any(|op| matches!(op, Op::Name(_)))
        {
            return None;
        }
        let external: HashSet<_> = active
            .1
            .map(|(name, layer)| (name.as_slice(), *layer))
            .collect();
        let mut sources = vec![(name, variable, layer)];
        let mut indices = HashMap::from([((name, layer, false), 0)]);
        let mut nodes = vec![None];
        let mut index = 0;
        while index < sources.len() {
            if context.is_cancelled() {
                return None;
            }
            let (name, variable, layer) = sources[index];
            if variable.scope.variable_names
                || variable.scope.append_outer
                || variable.value.len() > MAX_OUTPUT
            {
                return None;
            }
            let simple = variable.flavor == Flavor::Simple;
            let body = if simple {
                Body::Simple {
                    bytes: variable.value.as_slice(),
                    nul: variable.value.iter().position(|byte| *byte == 0),
                }
            } else {
                let bytes = c_string_prefix(&variable.value);
                let ops = if index == 0 {
                    root_ops.clone()
                } else {
                    operations(bytes, &mut instructions, &mut budget)?
                };
                for op in &mut instructions[ops.clone()] {
                    let Op::Name(range) = op else { continue };
                    let range = range.clone();
                    let next = &bytes[range.clone()];
                    let binding = bindings
                        .iter()
                        .rev()
                        .find_map(|scope| scope.get(next))
                        .map(|variable| (variable, None))
                        .or_else(|| {
                            store
                                .lookup_from(next, 0)
                                .map(|(layer, variable)| (variable, Some(layer)))
                        });
                    if let Some((binding, layer)) = binding {
                        // Simple rebinding bypasses an older recursive binding's
                        // active key, just as the full evaluator does.
                        let key = (next, layer, binding.flavor == Flavor::Simple);
                        let target = if let Some(target) = indices.get(&key) {
                            *target
                        } else {
                            budget.charge(1024 + next.len() + binding.value.len())?;
                            let target = sources.len();
                            indices.insert(key, target);
                            sources.push((next, binding, layer));
                            nodes.push(None);
                            target
                        };
                        *op = Op::Reference {
                            node: target,
                            definition: binding.definition.clone(),
                        };
                    } else {
                        *op = Op::Missing(range);
                    }
                }
                Body::Text { bytes, ops }
            };
            nodes[index] = Some(Node {
                name: if simple { b"" } else { name },
                body,
                externally_active: !simple && external.contains(&(name, layer)),
            });
            index += 1;
        }
        Some(Program {
            nodes: nodes.into_iter().collect::<Option<Vec<_>>>()?,
            ops: instructions,
        })
    }

    fn prepare_text(
        &self,
        program: &Program<'_>,
        definition: Option<Arc<Location>>,
    ) -> Option<Prepared> {
        let mut arena = Arena::default();
        let mut warnings = Vec::new();
        let result = (|| {
            if program.nodes[0].externally_active {
                return Err(TrialError::Evaluation(
                    ExpandError::RecursiveVariable(program.nodes[0].name.to_vec())
                        .at(definition.as_ref()),
                ));
            }
            let mut active = vec![false; program.nodes.len()];
            active[0] = true;
            let mut frames = vec![Frame {
                node: 0,
                cursor: 0,
                value: None,
                nearest: definition,
            }];
            loop {
                if self.context.is_cancelled() {
                    return Err(TrialError::Evaluation(ExpandError::Cancelled));
                }
                let frame = frames.last_mut().expect("root text frame");
                let Body::Text { ops, .. } = &program.nodes[frame.node].body else {
                    unreachable!("simple values have no frame")
                };
                let Some(op) = program.ops[ops.clone()].get(frame.cursor).cloned() else {
                    let finished = frames.pop().unwrap();
                    let value = arena.truncate(finished.value);
                    active[finished.node] = false;
                    if let Some(parent) = frames.last_mut() {
                        parent.value = arena
                            .join(parent.value, value)
                            .map_err(|error| error.at(parent.nearest.as_ref()))?;
                        continue;
                    }
                    return Ok(value);
                };
                frame.cursor += 1;
                match op {
                    Op::Bytes(range) => {
                        let value = arena.add(
                            PieceKind::Bytes {
                                node: frame.node,
                                range: range.clone(),
                            },
                            range.len(),
                            None,
                        )?;
                        frame.value = arena
                            .join(frame.value, value)
                            .map_err(|error| error.at(frame.nearest.as_ref()))?;
                    }
                    Op::Dollar => {
                        let value = arena.add(PieceKind::Dollar, 1, None)?;
                        frame.value = arena
                            .join(frame.value, value)
                            .map_err(|error| error.at(frame.nearest.as_ref()))?;
                    }
                    Op::Missing(range) => {
                        bounded_push(
                            &mut warnings,
                            Warning {
                                node: frame.node,
                                range,
                                nearest: frame.nearest.clone(),
                            },
                            &mut arena.budget,
                        )
                        .ok_or(TrialError::Budget)?;
                    }
                    Op::Reference { node, definition } => match &program.nodes[node].body {
                        Body::Simple { bytes, nul } => {
                            let value = arena.add(
                                PieceKind::Bytes {
                                    node,
                                    range: 0..bytes.len(),
                                },
                                bytes.len(),
                                *nul,
                            )?;
                            frame.value = arena
                                .join(frame.value, value)
                                .map_err(|error| error.at(frame.nearest.as_ref()))?;
                        }
                        Body::Text { .. } => {
                            let nearest = definition.or_else(|| frame.nearest.clone());
                            if active[node] || program.nodes[node].externally_active {
                                return Err(TrialError::Evaluation(
                                    ExpandError::RecursiveVariable(
                                        program.nodes[node].name.to_vec(),
                                    )
                                    .at(nearest.as_ref()),
                                ));
                            }
                            active[node] = true;
                            frames.push(Frame {
                                node,
                                cursor: 0,
                                value: None,
                                nearest,
                            });
                        }
                    },
                    Op::Name(_) => unreachable!("references are bound before evaluation"),
                }
            }
        })();
        let result = match result {
            Err(TrialError::Budget) => return None,
            Err(TrialError::Evaluation(error)) => Err(error),
            Ok(value) => Ok(value),
        };
        Some(Prepared {
            arena,
            warnings,
            result,
        })
    }

    pub(super) fn expand_text_program(
        &mut self,
        name: &[u8],
        variable: &Variable,
        layer: Option<usize>,
    ) -> Option<Result<Vec<u8>, ExpandError>> {
        let program = Self::text_program(
            &self.store,
            &self.bindings,
            (self.active_count(), self.active_keys()),
            &self.context,
            name,
            variable,
            layer,
        )?;
        let prepared = self.prepare_text(&program, variable.definition.clone())?;
        // Borrow only immutable bindings; receivers can change diagnostic policy,
        // but no interpreter effect or mutable store access occurs in this path.
        let entered = self.binding_context.is_none();
        if entered {
            self.binding_context = Some(variable.definition.clone());
        }
        let result = (|| {
            for warning in prepared.warnings {
                if self.context.is_cancelled() {
                    return Err(ExpandError::Cancelled);
                }
                if self.context.warns_undefined() {
                    warn_missing(
                        &mut self.context,
                        &self.binding_context,
                        &program.nodes[warning.node].body.bytes()[warning.range],
                    )
                    .map_err(|error| error.at(warning.nearest.as_ref()))?;
                }
            }
            let value = prepared.result?;
            let mut output = Vec::with_capacity(value.map_or(0, |span| span.len));
            let mut pending: Vec<_> = value.into_iter().collect();
            while let Some(span) = pending.pop() {
                if self.context.is_cancelled() {
                    return Err(ExpandError::Cancelled);
                }
                match &prepared.arena.pieces[span.piece].kind {
                    PieceKind::Bytes { node, range } => output.extend_from_slice(
                        &program.nodes[*node].body.bytes()[range.start..range.start + span.len],
                    ),
                    PieceKind::Dollar => output.extend_from_slice(&b"$"[..span.len]),
                    PieceKind::Join { left, right } => {
                        let right_len = span.len.saturating_sub(left.len).min(right.len);
                        if right_len != 0 {
                            pending.push(Span {
                                piece: right.piece,
                                len: right_len,
                            });
                        }
                        let left_len = span.len.min(left.len);
                        if left_len != 0 {
                            pending.push(Span {
                                piece: left.piece,
                                len: left_len,
                            });
                        }
                    }
                }
            }
            Ok(output)
        })();
        if entered {
            self.binding_context = None;
        }
        Some(if self.context.is_cancelled() {
            Err(ExpandError::Cancelled.at(variable.definition.as_ref()))
        } else {
            result
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostic::{Output, WarningOutput};
    use std::cell::Cell;
    use std::sync::atomic::{AtomicBool, Ordering};

    fn define(store: &mut VariableStore, name: &[u8], value: &[u8]) {
        store
            .define(name, value, Assignment::Recursive, Origin::File)
            .unwrap();
    }

    #[test]
    fn deep_text_frames_preserve_prefix_suffix_and_cycle_identity() {
        let mut store = VariableStore::new();
        for index in 0..5000 {
            define(
                &mut store,
                format!("R{index}").as_bytes(),
                format!("[$(R{})]", index + 1).as_bytes(),
            );
        }
        store
            .define_literal(b"R5000", b"end$", Origin::File)
            .unwrap();
        let mut expected = vec![b'['; 5000];
        expected.extend_from_slice(b"end$");
        expected.extend(std::iter::repeat_n(b']', 5000));
        expected.extend_from_slice(b":after");
        assert_eq!(store.expand(b"$(R0):after").unwrap(), expected);
        define(&mut store, b"R5000", b"[$(R0)]");
        assert_eq!(
            store.expand(b"$(R0)"),
            Err(ExpandError::RecursiveVariable(b"R0".to_vec()))
        );
    }

    #[test]
    fn nul_return_views_preserve_caller_suffix_and_later_warning_effects() {
        let mut store = VariableStore::new();
        store
            .define_literal(b"S", b"raw$\0discarded", Origin::File)
            .unwrap();
        define(&mut store, b"B", b"<$(S)>$(ABSENT)");
        define(&mut store, b"A", b"[$(B)]$(B)$$");
        let enabled = Cell::new(true);
        let mut events = Vec::new();
        let mut emit = |event: Output<'_>| {
            if let Output::Warning { message, .. } = event {
                events.push(message.to_vec());
            }
        };
        let mut output = WarningOutput::new(&mut emit, &enabled);
        assert_eq!(
            store
                .expand_with(
                    b"$(A):after",
                    ExpansionContext::with_receiver(None, &mut output)
                )
                .unwrap(),
            b"[<raw$]<raw$$:after"
        );
        assert_eq!(
            events,
            vec![b"warning: undefined variable 'ABSENT'".to_vec(); 2]
        );
        define(&mut store, b"B", b"<$(S)\0$(MUST_NOT_WARN)");
        assert_eq!(store.expand(b"$(A):after").unwrap(), b"[<raw$]<raw$$:after");
    }

    #[test]
    fn warning_policy_is_live_and_shared_children_are_revisited() {
        let mut store = VariableStore::new();
        define(&mut store, b"B", b"<$(ABSENT)>");
        define(&mut store, b"A", b"[$(B)$(B)]");
        let enabled = Cell::new(true);
        let mut count = 0;
        let mut emit = |_: Output<'_>| {
            count += 1;
            enabled.set(false);
        };
        let mut output = WarningOutput::new(&mut emit, &enabled);
        assert_eq!(
            store
                .expand_with(b"$(A)", ExpansionContext::with_receiver(None, &mut output))
                .unwrap(),
            b"[<><>]"
        );
        assert_eq!(count, 1);
    }

    #[test]
    fn cancelled_warning_preempts_remaining_warnings_and_prepared_cycle() {
        let mut store = VariableStore::new();
        define(&mut store, b"A", b"[$(ABSENT)$(OTHER)$(A)]");
        let flag = AtomicBool::new(false);
        let enabled = Cell::new(true);
        let mut count = 0;
        let mut emit = |_: Output<'_>| {
            count += 1;
            flag.store(true, Ordering::Relaxed);
        };
        let mut output = WarningOutput::new(&mut emit, &enabled);
        assert_eq!(
            store.expand_with(
                b"$(A)",
                ExpansionContext::with_receiver(None, &mut output).with_cancellation(&flag)
            ),
            Err(ExpandError::Cancelled)
        );
        assert_eq!(count, 1);
    }

    #[test]
    fn compiler_budget_falls_back_before_warning_emission() {
        let mut store = VariableStore::new();
        let bytes = vec![b'x'; MAX_OUTPUT - 32];
        store.define_literal(b"BIG", &bytes, Origin::File).unwrap();
        define(&mut store, b"A", b"$(ABSENT)$(BIG)");
        let enabled = Cell::new(true);
        let mut count = 0;
        let mut emit = |_: Output<'_>| count += 1;
        let mut output = WarningOutput::new(&mut emit, &enabled);
        assert_eq!(
            store
                .expand_with(b"$(A)", ExpansionContext::with_receiver(None, &mut output))
                .unwrap(),
            bytes
        );
        assert_eq!(count, 1);
    }

    #[test]
    fn output_limit_preserves_warning_order_before_failure() {
        let mut store = VariableStore::new();
        store
            .define_literal(b"BIG", &vec![b'x'; MAX_OUTPUT / 4], Origin::File)
            .unwrap();
        define(
            &mut store,
            b"A",
            b"$(BEFORE)$(BIG)$(BIG)$(BIG)$(BIG)x$(AFTER)",
        );
        let enabled = Cell::new(true);
        let mut events = Vec::new();
        let mut emit = |event: Output<'_>| {
            if let Output::Warning { message, .. } = event {
                events.push(message.to_vec());
            }
        };
        let mut output = WarningOutput::new(&mut emit, &enabled);
        assert_eq!(
            store.expand_with(b"$(A)", ExpansionContext::with_receiver(None, &mut output)),
            Err(ExpandError::ExpansionLimit)
        );
        assert_eq!(events, [b"warning: undefined variable 'BEFORE'".to_vec()]);
    }

    #[test]
    fn trial_budget_fallback_never_replays_prepared_warnings_twice() {
        let mut store = VariableStore::new();
        for index in 0..20 {
            define(
                &mut store,
                format!("R{index}").as_bytes(),
                format!("$(R{0})$(R{0})", index + 1).as_bytes(),
            );
        }
        define(&mut store, b"R20", b"$(ABSENT)");
        let expansion = store.expansion(ExpansionContext::pure());
        let program = Expansion::text_program(
            &expansion.store,
            &expansion.bindings,
            (expansion.active_count(), expansion.active_keys()),
            &expansion.context,
            b"R0",
            store.get(b"R0").unwrap(),
            Some(0),
        )
        .unwrap();
        assert!(
            expansion.prepare_text(&program, None).is_none(),
            "trial must hit its temporary budget before any warning receiver runs"
        );
        let enabled = Cell::new(true);
        let mut count = 0;
        let mut emit = |_: Output<'_>| count += 1;
        let mut output = WarningOutput::new(&mut emit, &enabled);
        assert_eq!(
            store
                .expand_with(
                    b"$(R0):after",
                    ExpansionContext::with_receiver(None, &mut output)
                )
                .unwrap(),
            b":after"
        );
        assert_eq!(count, 1 << 20);
    }

    #[test]
    fn simple_rebinding_bypasses_older_recursive_active_key() {
        let mut store = VariableStore::new();
        store.define_literal(b"A", b"new$", Origin::File).unwrap();
        let mut previous = store.get(b"A").unwrap().clone();
        previous.flavor = Flavor::Recursive;
        previous.value = b"[$(A)]".to_vec();
        let mut expansion = store.expansion(ExpansionContext::pure());
        assert_eq!(
            expansion
                .selected_variable(b"A", &previous, Some(0))
                .unwrap(),
            b"[new$]"
        );
    }
}
