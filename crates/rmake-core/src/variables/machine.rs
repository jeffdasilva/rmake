// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Incremental expression continuations. No language effects are replayed.
use super::linear::{Plan, Tail};
use super::*;
use std::cell::Cell;
use std::ops::Range;
use std::rc::Rc;

pub(super) fn lazy(name: &[u8]) -> bool {
    matches!(
        name,
        b"if" | b"or" | b"and" | b"intcmp" | b"foreach" | b"let" | b"call"
    )
}

#[derive(Default)]
struct Budget {
    used: Cell<usize>,
}
struct Lease {
    budget: Rc<Budget>,
    bytes: usize,
}
impl Budget {
    fn reserve(self: &Rc<Self>, bytes: usize) -> Result<Lease, ExpandError> {
        let total = self
            .used
            .get()
            .checked_add(bytes)
            .ok_or(ExpandError::ExpansionLimit)?;
        if total > MAX_OUTPUT {
            return Err(ExpandError::ExpansionLimit);
        }
        self.used.set(total);
        Ok(Lease {
            budget: self.clone(),
            bytes,
        })
    }
}
impl Lease {
    fn grow(&mut self, bytes: usize) -> Result<(), ExpandError> {
        let total = self
            .budget
            .used
            .get()
            .checked_add(bytes)
            .ok_or(ExpandError::ExpansionLimit)?;
        if total > MAX_OUTPUT {
            return Err(ExpandError::ExpansionLimit);
        }
        let own = self
            .bytes
            .checked_add(bytes)
            .ok_or(ExpandError::ExpansionLimit)?;
        self.budget.used.set(total);
        self.bytes = own;
        Ok(())
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        self.budget.used.set(self.budget.used.get() - self.bytes);
    }
}
struct Owned {
    bytes: Vec<u8>,
    _lease: Option<Lease>,
}

#[derive(Clone)]
enum Text<'a> {
    Borrowed(&'a [u8]),
    Owned(Rc<Owned>),
}
#[derive(Clone)]
struct Source<'a> {
    text: Text<'a>,
    range: Range<usize>,
}
impl<'a> Source<'a> {
    fn borrowed(bytes: &'a [u8]) -> Self {
        Self {
            text: Text::Borrowed(bytes),
            range: 0..bytes.len(),
        }
    }
    fn owned(bytes: Vec<u8>, lease: Option<Lease>) -> Self {
        let len = bytes.len();
        Self {
            text: Text::Owned(Rc::new(Owned {
                bytes,
                _lease: lease,
            })),
            range: 0..len,
        }
    }
    fn bytes(&self) -> &[u8] {
        let bytes = match &self.text {
            Text::Borrowed(b) => *b,
            Text::Owned(b) => b.bytes.as_slice(),
        };
        &bytes[self.range.clone()]
    }
    fn slice(&self, range: Range<usize>) -> Self {
        Self {
            text: self.text.clone(),
            range: self.range.start + range.start..self.range.start + range.end,
        }
    }
    fn trimmed(&self) -> Self {
        let b = self.bytes();
        let start = b.iter().position(|c| !gnu_whitespace(c)).unwrap_or(b.len());
        let end = b
            .iter()
            .rposition(|c| !gnu_whitespace(c))
            .map_or(start, |i| i + 1);
        self.slice(start..end)
    }
}
struct Reader<'a> {
    source: Source<'a>,
    cursor: usize,
    output: Vec<u8>,
}
struct Loop<'a> {
    name: Vec<u8>,
    list: Arc<Vec<u8>>,
    cursor: usize,
    index: usize,
    body: Source<'a>,
    output: Vec<u8>,
}
enum Kind<'a> {
    Read(Reader<'a>),
    Append(Reader<'a>),
    Reference(Source<'a>, u8),
    Resolve,
    Variable(Vec<u8>),
    Selected(Vec<u8>, Variable, Option<usize>),
    VariableEnd(usize, usize),
    Layer(Vec<u8>, Variable, Option<usize>, usize),
    LayerOuter(Variable),
    LayerBody(Variable, Vec<u8>),
    LayerJoin(Vec<u8>),
    BindingEnd(bool),
    ScopeEnd(usize, usize),
    LinearEnd(Plan, usize, bool, usize),
    Substitution(Vec<u8>, Vec<u8>),
    Function(Vec<u8>, Vec<Source<'a>>),
    Args(Vec<u8>, Vec<Source<'a>>, usize, Vec<Vec<u8>>),
    ArgValue(Vec<u8>, Vec<Source<'a>>, usize, Vec<Vec<u8>>),
    If(Vec<Source<'a>>),
    Boolean(bool, Vec<Source<'a>>, usize),
    BooleanValue(bool, Vec<Source<'a>>, usize),
    Intcmp(Vec<Source<'a>>, Option<Vec<u8>>),
    Foreach(Vec<Source<'a>>, Option<Vec<u8>>),
    Let(Vec<Source<'a>>, Option<Vec<u8>>),
    Iterate(Loop<'a>),
    Iterated(Loop<'a>),
}
impl Kind<'_> {
    fn storage(&self) -> usize {
        let vecs = |v: &Vec<Vec<u8>>| {
            v.capacity() * std::mem::size_of::<Vec<u8>>()
                + v.iter().map(Vec::capacity).sum::<usize>()
        };
        match self {
            Self::Read(r) | Self::Append(r) => r.output.capacity(),
            Self::Variable(n) => n.capacity(),
            Self::Selected(n, v, _) | Self::Layer(n, v, _, _) => n.capacity() + v.value.capacity(),
            Self::LayerOuter(v) => v.value.capacity(),
            Self::LayerBody(v, out) => v.value.capacity() + out.capacity(),
            Self::LayerJoin(v) => v.capacity(),
            Self::VariableEnd(_, bytes) | Self::ScopeEnd(_, bytes) => *bytes,
            Self::LinearEnd(p, _, _, active) => {
                p.bytes.capacity()
                    + p.frames.capacity() * std::mem::size_of::<super::linear::Frame>()
                    + active
            }
            Self::Substitution(a, b) => a.capacity() + b.capacity(),
            Self::Args(n, args, _, values) | Self::ArgValue(n, args, _, values) => {
                n.capacity() + args.capacity() * std::mem::size_of::<Source<'_>>() + vecs(values)
            }
            Self::Function(n, args) => {
                n.capacity() + args.capacity() * std::mem::size_of::<Source<'_>>()
            }
            Self::If(args) | Self::Boolean(_, args, _) | Self::BooleanValue(_, args, _) => {
                args.capacity() * std::mem::size_of::<Source<'_>>()
            }
            Self::Intcmp(args, value) | Self::Foreach(args, value) | Self::Let(args, value) => {
                args.capacity() * std::mem::size_of::<Source<'_>>()
                    + value.as_ref().map_or(0, Vec::capacity)
            }
            Self::Iterate(state) | Self::Iterated(state) => {
                state.name.capacity() + state.list.capacity() + state.output.capacity()
            }
            _ => 0,
        }
    }
}
fn scope_storage(scopes: &[LocalFrame]) -> usize {
    scopes
        .iter()
        .flat_map(|s| s.iter())
        .map(|(name, v)| name.capacity() + v.value.capacity() + 128)
        .sum()
}
struct Job<'a> {
    kind: Kind<'a>,
    depth: usize,
    nearest: Option<Arc<Location>>,
    _lease: Option<Lease>,
}
struct Machine<'a> {
    jobs: Vec<Job<'a>>,
    values: Vec<Vec<u8>>,
    extra: Rc<Budget>,
}
impl<'a> Machine<'a> {
    fn job(
        &mut self,
        kind: Kind<'a>,
        depth: usize,
        nearest: Option<Arc<Location>>,
    ) -> Result<(), ExpandError> {
        // Keep legacy-depth acceptance unchanged. Additional heap continuations
        // reserve live metadata and payload storage, released as jobs complete.
        let lease = if depth >= MAX_DEPTH {
            Some(
                self.extra
                    .reserve(512usize.saturating_add(kind.storage()))
                    .map_err(|e| e.at(nearest.as_ref()))?,
            )
        } else {
            None
        };
        self.jobs.push(Job {
            kind,
            depth,
            nearest,
            _lease: lease,
        });
        Ok(())
    }
    fn read(
        &mut self,
        source: Source<'a>,
        depth: usize,
        nearest: Option<Arc<Location>>,
    ) -> Result<(), ExpandError> {
        let end = c_string_prefix(source.bytes()).len();
        if end > MAX_OUTPUT {
            return Err(ExpandError::ExpansionLimit.at(nearest.as_ref()));
        }
        if !source.bytes()[..end].contains(&b'$') {
            self.values.push(source.bytes()[..end].to_vec());
            return Ok(());
        }
        self.job(
            Kind::Read(Reader {
                source: source.slice(0..end),
                cursor: 0,
                output: Vec::new(),
            }),
            depth,
            nearest,
        )
    }
    fn owned(&self, bytes: Vec<u8>, depth: usize) -> Result<Source<'a>, ExpandError> {
        let lease = if depth >= MAX_DEPTH {
            Some(self.extra.reserve(bytes.capacity())?)
        } else {
            None
        };
        Ok(Source::owned(bytes, lease))
    }
    fn value(&mut self) -> Vec<u8> {
        self.values.pop().expect("expression result")
    }
}
impl Expansion<'_, '_> {
    pub(super) fn expand_machine(&mut self, input: &[u8]) -> Result<Vec<u8>, ExpandError> {
        self.machine_nesting += 1;
        let active_start = self.active.len();
        let bindings_start = self.bindings.len();
        let context_start = self.binding_context.clone();
        let logical_start = self.logical_depth;
        let mut m = Machine {
            jobs: Vec::new(),
            values: Vec::new(),
            extra: Rc::new(Budget::default()),
        };
        let result = (|| {
            m.read(Source::borrowed(input), logical_start, None)?;
            while let Some(Job {
                kind,
                depth,
                nearest,
                _lease,
            }) = m.jobs.pop()
            {
                drop(_lease);
                self.logical_depth = depth;
                if self.context.is_cancelled() {
                    return Err(ExpandError::Cancelled);
                }
                self.machine_step(&mut m, kind, depth, nearest.clone())
                    .map_err(|error| error.at(nearest.as_ref()))?;
            }
            debug_assert_eq!(m.values.len(), 1);
            Ok(m.value())
        })();
        let result = result.map_err(|mut error| {
            // foreach checks its separator after evaluating the body, even if
            // the body failed. Preserve that narrow error-precedence rule.
            while let Some(job) = m.jobs.pop() {
                if let Kind::Iterated(mut state) = job.kind
                    && state.index != 0
                    && let Err(replacement) = push(&mut state.output, b" ")
                {
                    error = replacement.at(job.nearest.as_ref());
                }
            }
            error
        });
        self.active.truncate(active_start);
        self.bindings.truncate(bindings_start);
        self.binding_context = context_start;
        self.logical_depth = logical_start;
        self.machine_nesting -= 1;
        result
    }

    fn machine_step<'a>(
        &mut self,
        m: &mut Machine<'a>,
        kind: Kind<'a>,
        depth: usize,
        nearest: Option<Arc<Location>>,
    ) -> Result<(), ExpandError> {
        match kind {
            Kind::Read(mut reader) => {
                let bytes = reader.source.bytes();
                while reader.cursor < bytes.len() {
                    let cursor = reader.cursor;
                    if bytes[cursor] != b'$' {
                        let end = cursor
                            + bytes[cursor..]
                                .iter()
                                .position(|c| *c == b'$')
                                .unwrap_or(bytes.len() - cursor);
                        push(&mut reader.output, &bytes[cursor..end])?;
                        reader.cursor = end;
                        continue;
                    }
                    if cursor + 1 == bytes.len() || bytes[cursor + 1] == b'$' {
                        push(&mut reader.output, b"$")?;
                        reader.cursor += if cursor + 1 == bytes.len() { 1 } else { 2 };
                        continue;
                    }
                    if matches!(bytes[cursor + 1], b'(' | b'{') {
                        let (end, next, raw) = expansion_reference_span(bytes, cursor + 1)?;
                        let source = reader.source.slice(cursor + 2..end);
                        let delimiter = bytes[cursor + 1];
                        reader.cursor = next;
                        m.job(Kind::Append(reader), depth, nearest.clone())?;
                        if raw {
                            m.values.push(source.bytes().to_vec());
                            m.job(Kind::Resolve, depth, nearest)?;
                        } else {
                            m.job(Kind::Reference(source, delimiter), depth, nearest)?;
                        }
                    } else {
                        let name = bytes[cursor + 1..cursor + 2].to_vec();
                        reader.cursor += 2;
                        m.job(Kind::Append(reader), depth, nearest.clone())?;
                        m.job(Kind::Variable(name), depth, nearest)?;
                    }
                    return Ok(());
                }
                if let Some(nul) = reader.output.iter().position(|b| *b == 0) {
                    reader.output.truncate(nul);
                }
                m.values.push(reader.output);
            }
            Kind::Append(mut reader) => {
                let value = m.value();
                push(&mut reader.output, &value)?;
                m.job(Kind::Read(reader), depth, nearest)?;
            }
            Kind::Reference(source, delimiter) => {
                if let Some(end) = source.bytes().iter().position(gnu_whitespace)
                    && let Some((minimum, maximum)) = arity(&source.bytes()[..end])
                {
                    let name = source.bytes()[..end].to_vec();
                    let start = end + source.bytes()[end..].len()
                        - trim_function_start(&source.bytes()[end..]).len();
                    let argument = source.slice(start..source.bytes().len());
                    let ranges = split_arg_ranges(argument.bytes(), delimiter, maximum)?;
                    if ranges.len() < minimum && !matches!(name.as_slice(), b"word" | b"wordlist") {
                        return Err(too_few_arguments(&name, ranges.len()));
                    }
                    let args = ranges.into_iter().map(|r| argument.slice(r)).collect();
                    m.job(Kind::Function(name, args), depth, nearest)?;
                } else {
                    m.job(Kind::Resolve, depth, nearest.clone())?;
                    m.read(source, depth + 1, nearest)?;
                }
            }
            Kind::Resolve => {
                let name = m.value();
                if let Some(colon) = name.iter().position(|b| *b == b':')
                    && let Some(equal) = name[colon + 1..].iter().position(|b| *b == b'=')
                {
                    let equal = colon + 1 + equal;
                    let from = name[colon + 1..equal].to_vec();
                    let to = name[equal + 1..].to_vec();
                    let variable = name[..colon].to_vec();
                    m.job(Kind::Substitution(from, to), depth, nearest.clone())?;
                    m.job(Kind::Variable(variable), depth, nearest)?;
                } else {
                    m.job(Kind::Variable(name), depth, nearest)?;
                }
            }
            Kind::Substitution(mut from, mut to) => {
                let value = m.value();
                if Pattern::new(&from).wildcard.is_none() {
                    from.insert(0, b'%');
                    to.insert(0, b'%');
                }
                m.values.push(pattern_substitute(&from, &to, &value)?);
            }
            Kind::Variable(name) => {
                if self.depth < DIRECT_DEPTH && self.machine_nesting < DIRECT_MACHINES {
                    m.values.push(self.reference_variable(&name)?);
                    return Ok(());
                }
                let binding = self
                    .bindings
                    .iter()
                    .rev()
                    .find_map(|scope| scope.get(&name))
                    .map(|v| (v.clone(), None))
                    .or_else(|| {
                        self.store
                            .lookup_from(&name, 0)
                            .map(|(layer, v)| (v.clone(), Some(layer)))
                    });
                if let Some((variable, layer)) = binding {
                    m.job(Kind::Selected(name, variable, layer), depth, nearest)?;
                } else {
                    if self.context.warns_undefined() {
                        self.warn_missing(&name)?;
                    }
                    m.values.push(Vec::new());
                }
            }
            Kind::Selected(name, mut variable, layer) => {
                if variable.scope.variable_names {
                    variable.value = self.store.variable_names(true)?;
                    variable.scope.variable_names = false;
                }
                if variable.flavor == Flavor::Simple {
                    m.values.push(variable.value);
                    return Ok(());
                }
                if literal_alias(&variable.value).is_some()
                    && let Some(chain) = self.literal_chain(&name, &variable, layer)
                {
                    let value = match chain {
                        LiteralChain::Value(v) => v,
                        LiteralChain::Error(e) => return Err(e),
                        LiteralChain::Missing { name, definition } => {
                            let entered = self.binding_context.is_none();
                            if entered {
                                self.binding_context = Some(variable.definition.clone());
                            }
                            let result = if self.context.warns_undefined() {
                                self.warn_missing(&name)
                            } else {
                                Ok(())
                            };
                            if entered {
                                self.binding_context = None;
                            }
                            result.map_err(|e| e.at(definition.as_ref()))?;
                            Vec::new()
                        }
                    };
                    m.values.push(value);
                    return Ok(());
                }
                if let Some(mut plan) = self.linear_plan(&name, &variable, layer) {
                    let tail = std::mem::replace(&mut plan.tail, Tail::Value(Vec::new()));
                    let active = self.active.len();
                    let active_bytes = plan.active.iter().map(|(n, _)| n.capacity() + 64).sum();
                    let entered = self.binding_context.is_none();
                    if entered {
                        self.binding_context = Some(variable.definition.clone());
                    }
                    self.active.extend(std::mem::take(&mut plan.active));
                    let child_nearest = plan
                        .frames
                        .last()
                        .and_then(|f| f.nearest.clone())
                        .or(nearest.clone());
                    let child_depth = depth.saturating_add(plan.frames.len());
                    m.job(
                        Kind::LinearEnd(plan, active, entered, active_bytes),
                        depth,
                        nearest,
                    )?;
                    match tail {
                        Tail::Value(v) => m.values.push(v),
                        Tail::Error(e) => return Err(e),
                        Tail::Missing(n) => {
                            if self.context.warns_undefined() {
                                self.warn_missing(&n)
                                    .map_err(|e| e.at(child_nearest.as_ref()))?;
                            }
                            m.values.push(Vec::new());
                        }
                        Tail::Binding {
                            name,
                            variable,
                            layer,
                        } => m.job(
                            Kind::Selected(name, variable, layer),
                            child_depth,
                            child_nearest,
                        )?,
                    }
                    return Ok(());
                }
                if variable.value.contains(&b'$')
                    && let Some(result) = self.expand_text_program(&name, &variable, layer)
                {
                    m.values.push(result?);
                    return Ok(());
                }
                if self.active.iter().any(|(n, l)| *n == name && *l == layer) {
                    return Err(
                        ExpandError::RecursiveVariable(name).at(variable.definition.as_ref())
                    );
                }
                let active = self.active.len();
                self.active.push((name.clone(), layer));
                m.job(
                    Kind::VariableEnd(active, name.capacity() + 64),
                    depth,
                    nearest.clone(),
                )?;
                m.job(Kind::Layer(name, variable, layer, 0), depth, nearest)?;
            }
            Kind::LinearEnd(plan, active, entered, _) => {
                let value = m.value();
                self.active.truncate(active);
                if entered {
                    self.binding_context = None;
                }
                m.values.push(plan.finish(value)?);
            }
            Kind::VariableEnd(active, _) => self.active.truncate(active),
            Kind::Layer(name, mut variable, layer, nesting) => {
                if nesting >= MAX_DEPTH {
                    return Err(ExpandError::ExpansionLimit);
                }
                if variable.scope.variable_names {
                    variable.value = self.store.variable_names(false)?;
                    variable.scope.variable_names = false;
                }
                if variable.scope.append_outer
                    && let Some(layer) = layer
                    && let Some((outer_layer, outer)) = self.store.lookup_from(&name, layer + 1)
                {
                    let outer = outer.clone();
                    m.job(Kind::LayerOuter(variable), depth, nearest.clone())?;
                    m.job(
                        Kind::Layer(name, outer, Some(outer_layer), nesting + 1),
                        depth,
                        nearest,
                    )?;
                } else {
                    m.job(Kind::LayerBody(variable, Vec::new()), depth, nearest)?;
                }
            }
            Kind::LayerOuter(variable) => {
                let mut outer = m.value();
                if !outer.is_empty() {
                    push(&mut outer, b" ")?;
                }
                m.job(Kind::LayerBody(variable, outer), depth, nearest)?;
            }
            Kind::LayerBody(variable, outer) => {
                m.job(Kind::LayerJoin(outer), depth, nearest.clone())?;
                if variable.flavor == Flavor::Simple {
                    m.values.push(variable.value);
                } else {
                    let entered = self.binding_context.is_none();
                    if entered {
                        self.binding_context = Some(variable.definition.clone());
                    }
                    let location = variable.definition.clone().or(nearest);
                    m.job(Kind::BindingEnd(entered), depth, location.clone())?;
                    let input = c_string_prefix(&variable.value);
                    let prior = self.logical_depth;
                    self.logical_depth = depth + 1;
                    let direct = if input.len() <= MAX_OUTPUT {
                        self.direct_expression(input)
                    } else {
                        Some(Err(ExpandError::ExpansionLimit))
                    };
                    self.logical_depth = prior;
                    if let Some(result) = direct {
                        let mut value = result.map_err(|e| e.at(location.as_ref()))?;
                        if let Some(nul) = value.iter().position(|b| *b == 0) {
                            value.truncate(nul);
                        }
                        m.values.push(value);
                    } else {
                        let source = m.owned(variable.value, depth + 1)?;
                        m.read(source, depth + 1, location)?;
                    }
                }
            }
            Kind::LayerJoin(mut outer) => {
                let value = m.value();
                push(&mut outer, &value)?;
                m.values.push(outer);
            }
            Kind::BindingEnd(entered) => {
                if entered {
                    self.binding_context = None;
                }
            }
            Kind::ScopeEnd(start, _) => self.bindings.truncate(start),
            Kind::Function(name, args) => match name.as_slice() {
                b"if" => {
                    let condition = args[0].trimmed();
                    m.job(Kind::If(args), depth, nearest.clone())?;
                    m.read(condition, depth + 1, nearest)?;
                }
                b"or" | b"and" => m.job(Kind::Boolean(name == b"and", args, 0), depth, nearest)?,
                b"intcmp" => {
                    let left = args[0].clone();
                    m.job(Kind::Intcmp(args, None), depth, nearest.clone())?;
                    m.read(left, depth + 1, nearest)?;
                }
                b"foreach" => {
                    let name = args[0].clone();
                    m.job(Kind::Foreach(args, None), depth, nearest.clone())?;
                    m.read(name, depth + 1, nearest)?;
                }
                b"let" => {
                    let names = args[0].clone();
                    m.job(Kind::Let(args, None), depth, nearest.clone())?;
                    m.read(names, depth + 1, nearest)?;
                }
                b"shell"
                    if !self.context.has_shell() || matches!(self.store, StoreAccess::Read(_)) =>
                {
                    return Err(ExpandError::Unsupported(name));
                }
                _ => m.job(Kind::Args(name, args, 0, Vec::new()), depth, nearest)?,
            },
            Kind::Args(name, args, mut index, mut values) => {
                let storage = name.capacity()
                    + args.capacity() * std::mem::size_of::<Source<'_>>()
                    + values.capacity() * std::mem::size_of::<Vec<u8>>()
                    + values.iter().map(Vec::capacity).sum::<usize>();
                let mut prepared = if depth >= MAX_DEPTH {
                    Some(m.extra.reserve(storage)?)
                } else {
                    None
                };
                while index < args.len() {
                    if let Some(value) = self.pure_bytes(args[index].bytes()) {
                        let value = value?;
                        if let Some(ticket) = &mut prepared {
                            ticket.grow(value.capacity() + 2 * std::mem::size_of::<Vec<u8>>())?;
                        }
                        values.push(value);
                        index += 1;
                    } else {
                        let source = args[index].clone();
                        drop(prepared);
                        m.job(
                            Kind::ArgValue(name, args, index, values),
                            depth,
                            nearest.clone(),
                        )?;
                        m.read(source, depth + 1, nearest)?;
                        return Ok(());
                    }
                }
                if self.context.is_cancelled() {
                    return Err(ExpandError::Cancelled);
                }
                if name == b"call" {
                    self.machine_call(m, values, depth, nearest)?;
                } else {
                    m.values.push(self.eager_function(&name, &values)?);
                }
            }
            Kind::ArgValue(name, args, index, mut values) => {
                values.push(m.value());
                m.job(Kind::Args(name, args, index + 1, values), depth, nearest)?;
            }
            Kind::If(args) => {
                let value = m.value();
                let index = if value.is_empty() { 2 } else { 1 };
                if let Some(body) = args.get(index) {
                    m.read(body.clone(), depth + 1, nearest)?;
                } else {
                    m.values.push(Vec::new());
                }
            }
            Kind::Boolean(and, args, index) => {
                if let Some(arg) = args.get(index) {
                    let source = arg.trimmed();
                    m.job(Kind::BooleanValue(and, args, index), depth, nearest.clone())?;
                    m.read(source, depth + 1, nearest)?;
                } else {
                    m.values.push(Vec::new());
                }
            }
            Kind::BooleanValue(and, args, index) => {
                let value = m.value();
                if value.is_empty() == and || index + 1 == args.len() {
                    m.values.push(value);
                } else {
                    m.job(Kind::Boolean(and, args, index + 1), depth, nearest)?;
                }
            }
            Kind::Intcmp(args, left) => {
                let value = m.value();
                if let Some(left) = left {
                    let l = Decimal::parse(&left, false)?;
                    let r = Decimal::parse(&value, true)?;
                    let order = l.compare(&r);
                    if args.len() == 2 {
                        m.values.push(if order == std::cmp::Ordering::Equal {
                            let mut out = Vec::new();
                            if l.negative {
                                push(&mut out, b"-")?;
                            }
                            push(&mut out, l.digits)?;
                            out
                        } else {
                            Vec::new()
                        });
                    } else {
                        let index = match order {
                            std::cmp::Ordering::Less => 2,
                            std::cmp::Ordering::Equal => 3,
                            std::cmp::Ordering::Greater => {
                                if args.len() == 5 {
                                    4
                                } else {
                                    3
                                }
                            }
                        };
                        if let Some(body) = args.get(index) {
                            m.read(body.clone(), depth + 1, nearest)?;
                        } else {
                            m.values.push(Vec::new());
                        }
                    }
                } else {
                    let right = args[1].clone();
                    m.job(Kind::Intcmp(args, Some(value)), depth, nearest.clone())?;
                    m.read(right, depth + 1, nearest)?;
                }
            }
            Kind::Foreach(args, name) => {
                let value = m.value();
                if let Some(name) = name {
                    let name = words(&name).next().unwrap_or_default().to_vec();
                    m.job(
                        Kind::Iterate(Loop {
                            name,
                            list: Arc::new(value),
                            cursor: 0,
                            index: 0,
                            body: args[2].clone(),
                            output: Vec::new(),
                        }),
                        depth,
                        nearest,
                    )?;
                } else {
                    let list = args[1].clone();
                    m.job(Kind::Foreach(args, Some(value)), depth, nearest.clone())?;
                    m.read(list, depth + 1, nearest)?;
                }
            }
            Kind::Let(args, names) => {
                let value = m.value();
                if let Some(names) = names {
                    let names: Vec<_> = words(&names).collect();
                    let mut spans = word_spans(&value);
                    if names.len() > 1 && word_spans(&value).next().is_none() {
                        return Err(ExpandError::Safety(crate::safety::SafetyError::EmptyLet));
                    }
                    let mut bindings = BTreeMap::new();
                    for (index, name) in names.iter().enumerate() {
                        let v = match spans.next() {
                            Some((start, _)) if index + 1 == names.len() => value[start..].to_vec(),
                            Some((start, end)) => value[start..end].to_vec(),
                            None => Vec::new(),
                        };
                        bindings.insert(name.to_vec(), local(v));
                    }
                    let start = self.bindings.len();
                    self.bindings.push(LocalFrame {
                        values: bindings,
                        call: false,
                    });
                    m.job(
                        Kind::ScopeEnd(start, scope_storage(&self.bindings[start..])),
                        depth,
                        nearest.clone(),
                    )?;
                    m.read(args[2].clone(), depth + 1, nearest)?;
                } else {
                    let values = args[1].clone();
                    m.job(Kind::Let(args, Some(value)), depth, nearest.clone())?;
                    m.read(values, depth + 1, nearest)?;
                }
            }
            Kind::Iterate(mut state) => {
                let next = word_spans(&state.list[state.cursor..]).next();
                if let Some((a, b)) = next {
                    let value = state.list[state.cursor + a..state.cursor + b].to_vec();
                    state.cursor += b;
                    let start = self.bindings.len();
                    self.bindings.push(LocalFrame {
                        values: BTreeMap::from([(state.name.clone(), local(value))]),
                        call: false,
                    });
                    let body = state.body.clone();
                    m.job(Kind::Iterated(state), depth, nearest.clone())?;
                    m.job(
                        Kind::ScopeEnd(start, scope_storage(&self.bindings[start..])),
                        depth,
                        nearest.clone(),
                    )?;
                    m.read(body, depth + 1, nearest)?;
                } else {
                    m.values.push(state.output);
                }
            }
            Kind::Iterated(mut state) => {
                let value = m.value();
                if state.index != 0 {
                    push(&mut state.output, b" ")?;
                }
                push(&mut state.output, &value)?;
                state.index += 1;
                m.job(Kind::Iterate(state), depth, nearest)?;
            }
        }
        Ok(())
    }

    fn machine_call<'a>(
        &mut self,
        m: &mut Machine<'a>,
        expanded: Vec<Vec<u8>>,
        depth: usize,
        nearest: Option<Arc<Location>>,
    ) -> Result<(), ExpandError> {
        let name_text = trim_function_start(&expanded[0]);
        let name = name_text.split(gnu_whitespace).next().unwrap_or_default();
        if let Some((minimum, maximum)) = arity(name) {
            let mut values = expanded[1..].to_vec();
            if values.len() < minimum {
                return Err(too_few_arguments(name, values.len()));
            }
            if let Some(max) = maximum {
                values.truncate(max);
            }
            if matches!(
                name,
                b"if" | b"or" | b"and" | b"intcmp" | b"foreach" | b"let" | b"call"
            ) {
                let args = values
                    .into_iter()
                    .map(|value| m.owned(value, depth + 1))
                    .collect::<Result<Vec<_>, _>>()?;
                m.job(Kind::Function(name.to_vec(), args), depth, nearest)?;
            } else {
                m.values.push(self.eager_function(name, &values)?);
            }
            return Ok(());
        }
        let Some(variable) = self.lookup(name).cloned() else {
            if !name.is_empty() && self.context.warns_undefined() {
                self.warn_missing(name)?;
            }
            m.values.push(Vec::new());
            return Ok(());
        };
        let layer = if self
            .bindings
            .iter()
            .rev()
            .any(|scope| scope.contains_key(name))
        {
            None
        } else {
            self.store.lookup_from(name, 0).map(|(layer, _)| layer)
        };
        let mut bindings = BTreeMap::new();
        // Each call frame already contains placeholders for all older call
        // parameters. The latest one is the complete mask; lexical scopes are
        // deliberately excluded.
        if let Some(scope) = self.bindings.iter().rev().find(|scope| scope.call) {
            for key in scope.keys() {
                if !key.is_empty() && key.iter().all(u8::is_ascii_digit) {
                    bindings.insert(key.clone(), local(Vec::new()));
                }
            }
        }
        let end = expanded[0].len() - name_text.len() + name.len();
        for (index, value) in expanded.iter().enumerate() {
            let value = if index == 0 { &value[..end] } else { value };
            bindings.insert(index.to_string().into_bytes(), local(value.to_vec()));
        }
        let start = self.bindings.len();
        self.bindings.push(LocalFrame {
            values: bindings,
            call: true,
        });
        let location = variable.definition.clone().or(nearest);
        m.job(
            Kind::ScopeEnd(start, scope_storage(&self.bindings[start..])),
            depth,
            location.clone(),
        )?;
        if variable.flavor == Flavor::Simple {
            m.values.push(variable.value);
        } else {
            m.job(
                Kind::Layer(name.to_vec(), variable, layer, 0),
                depth,
                location,
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    fn define(store: &mut VariableStore, name: &[u8], body: &[u8]) {
        store
            .define(name, body, Assignment::Recursive, Origin::File)
            .unwrap();
    }
    fn chain(store: &mut VariableStore, depth: usize, kind: &str) {
        for i in 0..depth {
            let reference = format!("$(R{})", i + 1);
            let body = match kind {
                "strip" => format!("$(strip [{reference}])"),
                "let" => format!("$(let x,inner,[{reference}])"),
                "foreach" => format!("$(foreach x,one,[{reference}])"),
                "file" => format!("$(file >>seen,E)[{reference}]"),
                _ => unreachable!(),
            };
            define(store, format!("R{i}").as_bytes(), body.as_bytes());
        }
    }
    fn wrapped(depth: usize, value: &[u8]) -> Vec<u8> {
        let mut out = vec![b'['; depth];
        out.extend_from_slice(value);
        out.extend(std::iter::repeat_n(b']', depth));
        out
    }

    #[test]
    fn general_function_bodies_use_heap_continuations() {
        let mut store = VariableStore::new();
        chain(&mut store, 1024, "strip");
        store
            .define_literal(b"R1024", b"end", Origin::File)
            .unwrap();
        assert_eq!(store.expand(b"$(R0)").unwrap(), wrapped(1024, b"end"));
    }
    #[test]
    fn recursive_call_bodies_keep_numeric_scope_without_native_recursion() {
        let mut store = VariableStore::new();
        define(
            &mut store,
            b"walk",
            b"$(if $(1),x$(call walk,$(wordlist 2,9999,$(1))))",
        );
        let list = std::iter::repeat_n("word", 512)
            .collect::<Vec<_>>()
            .join(" ");
        let input = format!("$(call walk,{list})");
        assert_eq!(store.expand(input.as_bytes()).unwrap(), vec![b'x'; 512]);
    }
    #[test]
    fn effects_between_variable_links_are_never_replayed() {
        let mut store = VariableStore::new();
        chain(&mut store, 1024, "file");
        store
            .define_literal(b"R1024", b"end", Origin::File)
            .unwrap();
        let mut writes = 0;
        let mut filesystem = |effect: FilesystemEffect<'_>| -> Result<Vec<u8>, FilesystemError> {
            assert!(matches!(
                effect,
                FilesystemEffect::WriteFile {
                    path: b"seen",
                    append: true,
                    ..
                }
            ));
            writes += 1;
            Ok(Vec::new())
        };
        assert_eq!(
            store
                .expand_with(
                    b"$(R0)",
                    ExpansionContext::pure().with_filesystem(&mut filesystem)
                )
                .unwrap(),
            wrapped(1024, b"end")
        );
        assert_eq!(writes, 1024);
    }
    #[test]
    fn deep_lexical_error_restores_bindings_active_keys_and_context() {
        let mut store = VariableStore::new();
        chain(&mut store, 512, "let");
        define(&mut store, b"R512", b"$(error stop)");
        store.define_literal(b"x", b"outer", Origin::File).unwrap();
        let mut expansion = store.expansion(ExpansionContext::pure());
        assert!(matches!(
            expansion.expand(b"$(R0)"),
            Err(ExpandError::ContextualUserError { .. })
        ));
        assert!(expansion.active.is_empty());
        assert!(expansion.bindings.is_empty());
        assert!(expansion.binding_context.is_none());
        assert_eq!(expansion.expand(b"$(x)").unwrap(), b"outer");
    }
    #[test]
    fn foreach_separator_limit_keeps_precedence_over_a_failed_body() {
        let mut store = VariableStore::new();
        store
            .define_literal(b"BIG", &vec![b'x'; MAX_OUTPUT], Origin::File)
            .unwrap();
        assert_eq!(
            store.expand(b"$(foreach x,a b,$(if $(filter a,$(x)),$(BIG),$(error body)))"),
            Err(ExpandError::ExpansionLimit)
        );
    }
    #[test]
    fn deep_cancellation_restores_all_suspended_state() {
        let mut store = VariableStore::new();
        chain(&mut store, 512, "file");
        define(&mut store, b"R512", b"end");
        let flag = AtomicBool::new(false);
        let mut writes = 0;
        let mut filesystem = |_: FilesystemEffect<'_>| -> Result<Vec<u8>, FilesystemError> {
            writes += 1;
            if writes == 400 {
                flag.store(true, Ordering::Relaxed);
            }
            Ok(Vec::new())
        };
        {
            let mut expansion = store.expansion(
                ExpansionContext::pure()
                    .with_cancellation(&flag)
                    .with_filesystem(&mut filesystem),
            );
            assert_eq!(expansion.expand(b"$(R0)"), Err(ExpandError::Cancelled));
            assert!(expansion.active.is_empty());
            assert!(expansion.bindings.is_empty());
            assert!(expansion.binding_context.is_none());
        }
        assert_eq!(writes, 400);
    }
    #[test]
    fn completed_iterations_release_additional_heap_storage() {
        let mut store = VariableStore::new();
        chain(&mut store, 192, "foreach");
        let list = std::iter::repeat_n("x", 10000)
            .collect::<Vec<_>>()
            .join(" ");
        define(
            &mut store,
            b"R192",
            format!("$(foreach x,{list},$(x))").as_bytes(),
        );
        assert_eq!(
            store.expand(b"$(R0)").unwrap(),
            wrapped(192, list.as_bytes())
        );
    }
    #[test]
    fn extra_heap_storage_exhaustion_is_safe_and_precedes_the_leaf_effect() {
        let mut store = VariableStore::new();
        chain(&mut store, 10000, "strip");
        define(&mut store, b"R10000", b"$(file >forbidden,E)");
        let mut filesystem = |_: FilesystemEffect<'_>| -> Result<Vec<u8>, FilesystemError> {
            panic!("leaf after exhausted continuation budget")
        };
        let mut expansion =
            store.expansion(ExpansionContext::pure().with_filesystem(&mut filesystem));
        assert_eq!(expansion.expand(b"$(R0)"), Err(ExpandError::ExpansionLimit));
        assert!(expansion.active.is_empty());
        assert!(expansion.bindings.is_empty());
    }
}
