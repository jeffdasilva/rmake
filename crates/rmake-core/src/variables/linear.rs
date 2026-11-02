// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! One-reference continuations whose remaining text cannot observe leaf effects.
use super::*;
use std::collections::HashSet;
use std::ops::Range;

type Parts = (Range<usize>, Range<usize>, Range<usize>);
fn parts(bytes: &[u8]) -> Option<Parts> {
    let mut cursor = 0;
    let mut reference = None;
    while cursor < bytes.len() {
        if bytes[cursor] != b'$' || cursor + 1 == bytes.len() {
            cursor += 1;
        } else if bytes[cursor + 1] == b'$' {
            cursor += 2;
        } else {
            if reference.is_some() {
                return None;
            }
            let start = cursor;
            let (name, next) = if matches!(bytes[cursor + 1], b'(' | b'{') {
                let (end, next, _) = expansion_reference_span(bytes, cursor + 1).ok()?;
                (cursor + 2..end, next)
            } else {
                (cursor + 1..cursor + 2, cursor + 2)
            };
            literal_alias(&bytes[start..next])?;
            reference = Some((start, name, next));
            cursor = next;
        }
    }
    reference.map(|(start, name, next)| (0..start, name, next..bytes.len()))
}
fn append_literal(output: &mut Vec<u8>, bytes: &[u8]) {
    let mut cursor = 0;
    while cursor < bytes.len() {
        let start = cursor;
        cursor += bytes[cursor..]
            .iter()
            .position(|b| *b == b'$')
            .unwrap_or(bytes.len() - cursor);
        output.extend_from_slice(&bytes[start..cursor]);
        if cursor < bytes.len() {
            output.push(b'$');
            cursor += if bytes.get(cursor + 1) == Some(&b'$') {
                2
            } else {
                1
            };
        }
    }
}
pub(super) struct Frame {
    pub(super) prefix: Range<usize>,
    pub(super) suffix: Range<usize>,
    pub(super) nearest: Option<Arc<Location>>,
}
pub(super) enum Tail {
    Value(Vec<u8>),
    Missing(Vec<u8>),
    Error(ExpandError),
    Binding {
        name: Vec<u8>,
        variable: Variable,
        layer: Option<usize>,
    },
}
pub(super) struct Plan {
    pub(super) bytes: Vec<u8>,
    pub(super) frames: Vec<Frame>,
    pub(super) active: Vec<(Vec<u8>, Option<usize>)>,
    pub(super) tail: Tail,
}
impl Plan {
    pub(super) fn finish(self, value: Vec<u8>) -> Result<Vec<u8>, ExpandError> {
        // Check each raw return before its NUL boundary. Only the innermost
        // suffix is discarded by a raw NUL supplied by the leaf.
        let nul = value.iter().position(|byte| *byte == 0);
        let visible = nul.unwrap_or(value.len());
        let mut len = value.len();
        for (index, frame) in self.frames.iter().enumerate().rev() {
            let raw = frame
                .prefix
                .len()
                .saturating_add(len)
                .saturating_add(frame.suffix.len());
            if raw > MAX_OUTPUT {
                return Err(ExpandError::ExpansionLimit.at(frame.nearest.as_ref()));
            }
            len = if nul.is_some() && index + 1 == self.frames.len() {
                frame.prefix.len() + visible
            } else {
                raw
            };
        }
        let mut output = Vec::with_capacity(len);
        for frame in &self.frames {
            output.extend_from_slice(&self.bytes[frame.prefix.clone()]);
        }
        output.extend_from_slice(&value[..visible]);
        for (index, frame) in self.frames.iter().enumerate().rev() {
            if nul.is_none() || index + 1 != self.frames.len() {
                output.extend_from_slice(&self.bytes[frame.suffix.clone()]);
            }
        }
        Ok(output)
    }
}

impl Expansion<'_, '_> {
    pub(super) fn linear_plan<'c>(
        &'c self,
        mut name: &'c [u8],
        mut variable: &'c Variable,
        mut layer: Option<usize>,
    ) -> Option<Plan> {
        if variable.scope.append_outer || variable.scope.variable_names {
            return None;
        }
        // Until this plan owns its bytes, no evaluator callback may run.
        // Every suspended suffix is literal and cannot observe a later mutation.
        let mut root = Some(parts(c_string_prefix(&variable.value))?);
        let mut seen: HashSet<_> = self
            .active_keys()
            .map(|(name, layer)| (name.as_slice(), *layer))
            .collect();
        let mut used = self.active_count().checked_mul(128)?;
        let mut nearest: Option<Arc<Location>> = None;
        let mut pending = Vec::new();
        let mut text_len = 0usize;
        let tail = loop {
            if self.context.is_cancelled() {
                break Tail::Error(ExpandError::Cancelled);
            }
            used = used.checked_add(512 + name.len() + variable.value.len())?;
            if used > MAX_OUTPUT {
                return None;
            }
            if !variable.scope.variable_names && variable.flavor == Flavor::Simple {
                break Tail::Value(variable.value.clone());
            }
            let bytes = c_string_prefix(&variable.value);
            if !variable.scope.variable_names
                && !variable.scope.append_outer
                && !bytes.contains(&b'$')
            {
                // A newly literal recursive value still shares its active key;
                // only simple rebinding bypasses that recursive identity.
                if seen.contains(&(name, layer)) {
                    break Tail::Error(
                        ExpandError::RecursiveVariable(name.to_vec())
                            .at(variable.definition.as_ref().or(nearest.as_ref())),
                    );
                }
                break Tail::Value(bytes.to_vec());
            }
            let shape = if pending.is_empty() {
                root.take()
            } else {
                parts(bytes)
            };
            let Some((prefix, next, suffix)) =
                shape.filter(|_| !variable.scope.append_outer && !variable.scope.variable_names)
            else {
                break Tail::Binding {
                    name: name.to_vec(),
                    variable: variable.clone(),
                    layer,
                };
            };
            if !seen.insert((name, layer)) {
                break Tail::Error(
                    ExpandError::RecursiveVariable(name.to_vec())
                        .at(variable.definition.as_ref().or(nearest.as_ref())),
                );
            }
            if let Some(definition) = &variable.definition {
                nearest = Some(definition.clone());
            }
            text_len = text_len.checked_add(prefix.len() + suffix.len())?;
            pending.push((name, layer, &bytes[prefix], &bytes[suffix], nearest.clone()));
            let next = &bytes[next];
            let binding = self
                .bindings
                .iter()
                .rev()
                .find_map(|scope| scope.get(next))
                .map(|v| (v, None))
                .or_else(|| {
                    self.store
                        .lookup_from(next, 0)
                        .map(|(layer, v)| (v, Some(layer)))
                });
            let Some((binding, next_layer)) = binding else {
                break Tail::Missing(next.to_vec());
            };
            name = next;
            variable = binding;
            layer = next_layer;
        };
        let mut bytes = Vec::with_capacity(text_len);
        let mut frames = Vec::with_capacity(pending.len());
        let mut active = Vec::new();
        for (name, layer, prefix, suffix, nearest) in pending {
            // Full evaluation must see all suspended parents for cycle checks.
            if matches!(tail, Tail::Binding { .. }) {
                active.push((name.to_vec(), layer));
            }
            let start = bytes.len();
            append_literal(&mut bytes, prefix);
            let middle = bytes.len();
            append_literal(&mut bytes, suffix);
            frames.push(Frame {
                prefix: start..middle,
                suffix: middle..bytes.len(),
                nearest,
            });
        }
        Some(Plan {
            bytes,
            frames,
            active,
            tail,
        })
    }

    pub(super) fn expand_linear(
        &mut self,
        name: &[u8],
        variable: &Variable,
        layer: Option<usize>,
    ) -> Option<Result<Vec<u8>, ExpandError>> {
        let mut plan = self.linear_plan(name, variable, layer)?;
        let entered = self.binding_context.is_none();
        if entered {
            self.binding_context = Some(variable.definition.clone());
        }
        let active_start = self.active.len();
        self.active.extend(std::mem::take(&mut plan.active));
        let nearest = plan.frames.last().and_then(|frame| frame.nearest.clone());
        let result = (|| {
            if self.context.is_cancelled() {
                return Err(ExpandError::Cancelled);
            }
            let value = match std::mem::replace(&mut plan.tail, Tail::Value(Vec::new())) {
                Tail::Value(value) => Ok(value),
                Tail::Error(error) => Err(error),
                Tail::Missing(name) => {
                    if self.context.warns_undefined() {
                        self.warn_missing(&name).map(|()| Vec::new())
                    } else {
                        Ok(Vec::new())
                    }
                }
                Tail::Binding {
                    name,
                    variable,
                    layer,
                } => self.selected_variable(&name, &variable, layer),
            }
            .map_err(|error| error.at(nearest.as_ref()))?;
            if self.context.is_cancelled() {
                return Err(ExpandError::Cancelled);
            }
            plan.finish(value)
        })();
        self.active.truncate(active_start);
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
    use std::sync::atomic::{AtomicBool, Ordering};
    fn define(store: &mut VariableStore, name: &[u8], value: &[u8]) {
        store
            .define(name, value, Assignment::Recursive, Origin::File)
            .unwrap();
    }
    #[test]
    fn deep_linear_frames_resume_after_a_full_function_leaf() {
        let mut store = VariableStore::new();
        for i in 0..5000 {
            define(
                &mut store,
                format!("R{i}").as_bytes(),
                format!("[$(R{})]", i + 1).as_bytes(),
            );
        }
        define(&mut store, b"R5000", b"$(strip   end  )");
        let mut expected = vec![b'['; 5000];
        expected.extend_from_slice(b"end");
        expected.extend(std::iter::repeat_n(b']', 5000));
        expected.extend_from_slice(b":after");
        assert_eq!(store.expand(b"$(R0):after").unwrap(), expected);
    }
    #[test]
    fn leaf_rebinding_keeps_selected_parent_text_and_flavor_cycle_policy() {
        let mut store = VariableStore::new();
        define(&mut store, b"A", b"before$(B)after");
        let selected = store.get(b"A").unwrap().clone();
        define(&mut store, b"B", b"$(if yes,$(A))");
        store.define_literal(b"A", b"new", Origin::File).unwrap();
        assert_eq!(
            store
                .expansion(ExpansionContext::pure())
                .selected_variable(b"A", &selected, Some(0))
                .unwrap(),
            b"beforenewafter"
        );
        define(&mut store, b"A", b"new");
        assert_eq!(
            store
                .expansion(ExpansionContext::pure())
                .selected_variable(b"A", &selected, Some(0)),
            Err(ExpandError::RecursiveVariable(b"A".to_vec()))
        );
    }
    #[test]
    fn cancellation_in_leaf_prevents_file_effect_and_restores_active_state() {
        let mut store = VariableStore::new();
        define(&mut store, b"A", b"[$(B)]");
        define(&mut store, b"B", b"$(warning stop)$(file >blocked,X)");
        let flag = AtomicBool::new(false);
        let mut count = 0;
        let mut emit = |_: Output<'_>| {
            count += 1;
            flag.store(true, Ordering::Relaxed);
        };
        let mut filesystem = |_: FilesystemEffect<'_>| -> Result<Vec<u8>, FilesystemError> {
            panic!("effect after cancelled warning")
        };
        let mut expansion = store.expansion(
            ExpansionContext::new(None, &mut emit)
                .with_cancellation(&flag)
                .with_filesystem(&mut filesystem),
        );
        assert_eq!(expansion.expand(b"$(A)"), Err(ExpandError::Cancelled));
        assert!(expansion.active.is_empty());
        assert!(expansion.binding_context.is_none());
        flag.store(false, Ordering::Relaxed);
        assert_eq!(expansion.expand(b"safe").unwrap(), b"safe");
        assert_eq!(count, 1);
    }
    #[test]
    fn leaf_effect_precedes_parent_output_limit_and_is_not_repeated() {
        let mut store = VariableStore::new();
        define(&mut store, b"A", b"$(B)x");
        define(&mut store, b"B", b"$(file <payload)");
        let mut reads = 0;
        let mut filesystem = |effect: FilesystemEffect<'_>| -> Result<Vec<u8>, FilesystemError> {
            assert!(matches!(effect, FilesystemEffect::ReadFile(b"payload")));
            reads += 1;
            Ok(vec![b'z'; MAX_OUTPUT])
        };
        assert_eq!(
            store.expand_with(
                b"$(A)",
                ExpansionContext::pure().with_filesystem(&mut filesystem)
            ),
            Err(ExpandError::ExpansionLimit)
        );
        assert_eq!(reads, 1);
    }
    #[test]
    fn simple_nul_discards_only_the_innermost_suffix() {
        let mut store = VariableStore::new();
        define(&mut store, b"A", b"[$(B)]$$");
        define(&mut store, b"B", b"<$(S)>");
        store
            .define_literal(b"S", b"raw$\0discard", Origin::File)
            .unwrap();
        assert_eq!(store.expand(b"$(A):after").unwrap(), b"[<raw$]$:after");
    }
    #[test]
    fn unlocated_root_locks_warning_context_but_leaf_error_keeps_definition() {
        let mut store = VariableStore::new();
        store
            .define(b"A", b"[$(B)]", Assignment::Recursive, Origin::CommandLine)
            .unwrap();
        let location = Location {
            file: b"leaf".as_slice().into(),
            line: 7,
        };
        store
            .define_at(
                b"B",
                b"$(warning before)$(intcmp bad,0)",
                Assignment::Recursive,
                Origin::File,
                &location,
            )
            .unwrap();
        let mut events = Vec::new();
        let mut emit = |event: Output<'_>| {
            if let Output::Warning { location, message } = event {
                events.push((location.cloned(), message.to_vec()));
            }
        };
        let error = store
            .expand_with(b"$(A)", ExpansionContext::new(None, &mut emit))
            .unwrap_err();
        assert_eq!(error.location(), Some(&location));
        assert_eq!(events, [(None, b"before".to_vec())]);
    }
    #[test]
    fn oversized_plan_rejects_before_running_the_leaf() {
        let mut store = VariableStore::new();
        define(&mut store, b"A", b"[$(BIG)]");
        store
            .define_literal(b"BIG", &vec![b'z'; MAX_OUTPUT], Origin::File)
            .unwrap();
        let expansion = store.expansion(ExpansionContext::pure());
        assert!(
            expansion
                .linear_plan(b"A", store.get(b"A").unwrap(), Some(0))
                .is_none()
        );
        assert_eq!(store.expand(b"$(A)"), Err(ExpandError::ExpansionLimit));
    }
}
