// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Byte-preserving variables and expansion without filesystem or process effects.
//!
//! `define` receives an already-expanded variable name and an RHS whose assignment
//! syntax whitespace has been removed by the reader. Expansion never interprets
//! UTF-8. Effects and filesystem-dependent functions are explicit errors until an
//! effect interface is implemented; an unknown variable still expands to empty.

use std::collections::BTreeMap;
use std::fmt;

const MAX_DEPTH: usize = 128;
const MAX_OUTPUT: usize = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Assignment {
    Recursive,
    Simple,
    ImmediateRecursive,
    Append,
    Conditional,
    Shell,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Origin {
    Default,
    Environment,
    File,
    EnvironmentOverride,
    CommandLine,
    Override,
    Automatic,
}

impl Origin {
    fn name(self) -> &'static [u8] {
        match self {
            Self::Default => b"default",
            Self::Environment => b"environment",
            Self::File => b"file",
            Self::EnvironmentOverride => b"environment override",
            Self::CommandLine => b"command line",
            Self::Override => b"override",
            Self::Automatic => b"automatic",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flavor {
    Recursive,
    Simple,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Variable {
    value: Vec<u8>,
    flavor: Flavor,
    origin: Origin,
}

impl Variable {
    pub fn raw_value(&self) -> &[u8] {
        &self.value
    }

    pub fn flavor(&self) -> Flavor {
        self.flavor
    }

    pub fn origin(&self) -> Origin {
        self.origin
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExpandError {
    Unsupported(Vec<u8>),
    UnterminatedReference,
    RecursiveVariable(Vec<u8>),
    EmptyVariableName,
    InvalidArguments {
        function: Vec<u8>,
        message: &'static str,
    },
    ExpansionLimit,
}

impl fmt::Display for ExpandError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported(name) => write!(
                output,
                "unsupported expansion feature '{}'",
                String::from_utf8_lossy(name)
            ),
            Self::UnterminatedReference => output.write_str("unterminated variable reference"),
            Self::RecursiveVariable(name) => write!(
                output,
                "recursive variable '{}' references itself (eventually)",
                String::from_utf8_lossy(name)
            ),
            Self::EmptyVariableName => output.write_str("empty variable name"),
            Self::InvalidArguments { function, message } => write!(
                output,
                "invalid arguments to '{}': {message}",
                String::from_utf8_lossy(function)
            ),
            Self::ExpansionLimit => output.write_str("variable expansion resource limit exceeded"),
        }
    }
}

impl std::error::Error for ExpandError {}

#[derive(Clone, Debug, Default)]
pub struct VariableStore {
    variables: BTreeMap<Vec<u8>, Variable>,
    working_directory: Option<Vec<u8>>,
}

impl VariableStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the actual invocation directory for `abspath`, independently of the
    /// overridable CURDIR variable. This performs no filesystem access.
    pub fn set_working_directory(&mut self, directory: &[u8]) {
        self.working_directory = Some(directory.to_vec());
    }

    pub fn get(&self, name: &[u8]) -> Option<&Variable> {
        self.variables.get(name)
    }

    pub fn remove(&mut self, name: &[u8]) -> Option<Variable> {
        self.variables.remove(name)
    }

    pub fn definitions(&self) -> impl Iterator<Item = (&[u8], &Variable)> {
        self.variables
            .iter()
            .map(|(name, value)| (name.as_slice(), value))
    }

    /// Define an already-expanded name. A lower-precedence definition is ignored
    /// before evaluating its RHS, including effectful or otherwise invalid RHSs.
    pub fn define(
        &mut self,
        name: &[u8],
        value: &[u8],
        assignment: Assignment,
        origin: Origin,
    ) -> Result<(), ExpandError> {
        if name.is_empty() {
            return Err(ExpandError::EmptyVariableName);
        }
        if let Some(old) = self.get(name)
            && (assignment == Assignment::Conditional || old.origin > origin)
        {
            return Ok(());
        }
        let variable = match assignment {
            Assignment::Shell => return Err(ExpandError::Unsupported(b"!=".to_vec())),
            Assignment::Simple => Variable {
                value: self.expand(value)?,
                flavor: Flavor::Simple,
                origin,
            },
            Assignment::ImmediateRecursive => {
                let expanded = self.expand(value)?;
                let mut escaped = Vec::new();
                for byte in expanded {
                    push(&mut escaped, &[byte])?;
                    if byte == b'$' {
                        push(&mut escaped, b"$")?;
                    }
                }
                Variable {
                    value: escaped,
                    flavor: Flavor::Recursive,
                    origin,
                }
            }
            Assignment::Append => {
                if let Some(old) = self.get(name) {
                    let extra = match old.flavor {
                        Flavor::Recursive => value.to_vec(),
                        Flavor::Simple => self.expand(value)?,
                    };
                    let mut combined = old.value.clone();
                    if !combined.is_empty() && !extra.is_empty() {
                        push(&mut combined, b" ")?;
                    }
                    push(&mut combined, &extra)?;
                    Variable {
                        value: combined,
                        flavor: old.flavor,
                        origin,
                    }
                } else {
                    Variable {
                        value: value.to_vec(),
                        flavor: Flavor::Recursive,
                        origin,
                    }
                }
            }
            Assignment::Recursive | Assignment::Conditional => Variable {
                value: value.to_vec(),
                flavor: Flavor::Recursive,
                origin,
            },
        };
        if variable.value.len() > MAX_OUTPUT {
            return Err(ExpandError::ExpansionLimit);
        }
        self.variables.insert(name.to_vec(), variable);
        Ok(())
    }

    /// Install an already-expanded simple value, retaining literal dollar bytes.
    /// Use this for automatic variables and invocation metadata, not recursive
    /// environment-variable imports (which use `Assignment::Recursive`).
    pub fn define_literal(
        &mut self,
        name: &[u8],
        value: &[u8],
        origin: Origin,
    ) -> Result<(), ExpandError> {
        if name.is_empty() {
            return Err(ExpandError::EmptyVariableName);
        }
        if self.get(name).is_some_and(|old| old.origin > origin) {
            return Ok(());
        }
        if value.len() > MAX_OUTPUT {
            return Err(ExpandError::ExpansionLimit);
        }
        self.variables.insert(
            name.to_vec(),
            Variable {
                value: value.to_vec(),
                flavor: Flavor::Simple,
                origin,
            },
        );
        Ok(())
    }

    pub fn expand(&self, input: &[u8]) -> Result<Vec<u8>, ExpandError> {
        Expansion {
            store: self,
            bindings: Vec::new(),
            active: Vec::new(),
            depth: 0,
        }
        .expand(input)
    }
}

struct Expansion<'a> {
    store: &'a VariableStore,
    bindings: Vec<BTreeMap<Vec<u8>, Variable>>,
    active: Vec<Vec<u8>>,
    depth: usize,
}

impl Expansion<'_> {
    fn lookup(&self, name: &[u8]) -> Option<&Variable> {
        self.bindings
            .iter()
            .rev()
            .find_map(|scope| scope.get(name))
            .or_else(|| self.store.get(name))
    }

    fn expand(&mut self, input: &[u8]) -> Result<Vec<u8>, ExpandError> {
        if self.depth >= MAX_DEPTH || input.len() > MAX_OUTPUT {
            return Err(ExpandError::ExpansionLimit);
        }
        self.depth += 1;
        let result = self.expand_inner(input);
        self.depth -= 1;
        result
    }

    fn expand_inner(&mut self, input: &[u8]) -> Result<Vec<u8>, ExpandError> {
        let mut result = Vec::new();
        let mut cursor = 0;
        while cursor < input.len() {
            if input[cursor] != b'$' || cursor + 1 == input.len() {
                push(&mut result, &input[cursor..cursor + 1])?;
                cursor += 1;
                continue;
            }
            match input[cursor + 1] {
                b'$' => {
                    push(&mut result, b"$")?;
                    cursor += 2;
                }
                b'(' | b'{' => {
                    let end = reference_end(input, cursor + 1)?;
                    let expanded = self.reference(&input[cursor + 2..end], input[cursor + 1])?;
                    push(&mut result, &expanded)?;
                    cursor = end + 1;
                }
                _ => {
                    let expanded = self.variable(&input[cursor + 1..cursor + 2])?;
                    push(&mut result, &expanded)?;
                    cursor += 2;
                }
            }
        }
        Ok(result)
    }

    fn variable(&mut self, name: &[u8]) -> Result<Vec<u8>, ExpandError> {
        let Some(variable) = self.lookup(name).cloned() else {
            return Ok(Vec::new());
        };
        if variable.flavor == Flavor::Simple {
            return Ok(variable.value);
        }
        if self.active.iter().any(|active| active == name) {
            return Err(ExpandError::RecursiveVariable(name.to_vec()));
        }
        self.active.push(name.to_vec());
        let result = self.expand(&variable.value);
        self.active.pop();
        result
    }

    fn reference(&mut self, body: &[u8], delimiter: u8) -> Result<Vec<u8>, ExpandError> {
        if let Some(end) = body.iter().position(|byte| byte.is_ascii_whitespace()) {
            let name = &body[..end];
            if let Some((minimum, maximum)) = arity(name) {
                let argument_text = trim_start(&body[end..]);
                let args = split_args(argument_text, delimiter, maximum)?;
                if args.len() < minimum {
                    return Err(invalid(name, "too few arguments"));
                }
                return self.function(name, &args);
            }
        }
        let name = self.expand(body)?;
        if let Some(colon) = name.iter().position(|byte| *byte == b':')
            && let Some(equals) = name[colon + 1..].iter().position(|byte| *byte == b'=')
        {
            let equals = colon + 1 + equals;
            let value = self.variable(&name[..colon])?;
            let from = &name[colon + 1..equals];
            let to = &name[equals + 1..];
            if Pattern::new(from).wildcard.is_some() {
                return pattern_substitute(from, to, &value);
            }
            let mut pattern = b"%".to_vec();
            pattern.extend_from_slice(from);
            let mut replacement = b"%".to_vec();
            replacement.extend_from_slice(to);
            return pattern_substitute(&pattern, &replacement, &value);
        }
        self.variable(&name)
    }

    fn function(&mut self, name: &[u8], args: &[&[u8]]) -> Result<Vec<u8>, ExpandError> {
        match name {
            b"if" => {
                let condition = self.expand(trim(args[0]))?;
                let index = if condition.is_empty() { 2 } else { 1 };
                args.get(index)
                    .map_or_else(|| Ok(Vec::new()), |body| self.expand(body))
            }
            b"or" => {
                for arg in args {
                    let value = self.expand(arg)?;
                    if !value.is_empty() {
                        return Ok(value);
                    }
                }
                Ok(Vec::new())
            }
            b"and" => {
                let mut last = Vec::new();
                for arg in args {
                    last = self.expand(arg)?;
                    if last.is_empty() {
                        break;
                    }
                }
                Ok(last)
            }
            b"foreach" => {
                let variable = self.expand(args[0])?;
                let variable = trim(&variable).to_vec();
                let list = self.expand(args[1])?;
                let mut output = Vec::new();
                for (index, word) in words(&list).enumerate() {
                    let bindings = BTreeMap::from([(variable.clone(), local(word.to_vec()))]);
                    self.bindings.push(bindings);
                    let expanded = self.expand(args[2]);
                    self.bindings.pop();
                    if index != 0 {
                        push(&mut output, b" ")?;
                    }
                    push(&mut output, &expanded?)?;
                }
                Ok(output)
            }
            b"let" => {
                let names = self.expand(args[0])?;
                let values = self.expand(args[1])?;
                let names: Vec<_> = words(&names).collect();
                let values: Vec<_> = words(&values).collect();
                let mut bindings = BTreeMap::new();
                for (index, name) in names.iter().enumerate() {
                    let value = if index + 1 == names.len() {
                        join_words(values.get(index..).unwrap_or_default().iter().copied())?
                    } else {
                        values.get(index).copied().unwrap_or_default().to_vec()
                    };
                    bindings.insert(name.to_vec(), local(value));
                }
                self.bindings.push(bindings);
                let result = self.expand(args[2]);
                self.bindings.pop();
                result
            }
            b"call" => self.call(args),
            b"shell" | b"eval" | b"file" | b"info" | b"warning" | b"error" | b"wildcard"
            | b"realpath" | b"guile" => Err(ExpandError::Unsupported(name.to_vec())),
            _ => {
                let expanded = args
                    .iter()
                    .map(|arg| self.expand(arg))
                    .collect::<Result<Vec<_>, _>>()?;
                self.eager_function(name, &expanded)
            }
        }
    }

    fn call(&mut self, args: &[&[u8]]) -> Result<Vec<u8>, ExpandError> {
        let expanded = args
            .iter()
            .map(|arg| self.expand(arg))
            .collect::<Result<Vec<_>, _>>()?;
        let name = trim(&expanded[0]);
        if let Some((minimum, maximum)) = arity(name) {
            let mut values = expanded[1..].to_vec();
            if values.len() < minimum {
                return Err(invalid(name, "too few arguments"));
            }
            if let Some(maximum) = maximum {
                values.truncate(maximum);
            }
            // Builtins normally receive either eager values or raw arguments.
            // `call` itself is eager, but lazy builtins must still perform their
            // own evaluation (notably foreach/let bodies with escaped dollars).
            if matches!(name, b"if" | b"or" | b"and" | b"foreach" | b"let" | b"call") {
                let refs: Vec<&[u8]> = values.iter().map(Vec::as_slice).collect();
                return self.function(name, &refs);
            }
            return self.eager_function(name, &values);
        }
        let Some(variable) = self.lookup(name).cloned() else {
            return Ok(Vec::new());
        };
        let mut bindings = BTreeMap::new();
        // Inner calls hide missing outer positional parameters as well.
        for scope in &self.bindings {
            for key in scope.keys() {
                if !key.is_empty() && key.iter().all(u8::is_ascii_digit) {
                    bindings.insert(key.clone(), local(Vec::new()));
                }
            }
        }
        for (index, value) in expanded.iter().enumerate() {
            bindings.insert(index.to_string().into_bytes(), local(value.clone()));
        }
        self.bindings.push(bindings);
        let result = if variable.flavor == Flavor::Simple {
            Ok(variable.value)
        } else {
            self.expand(&variable.value)
        };
        self.bindings.pop();
        result
    }

    fn eager_function(&mut self, name: &[u8], args: &[Vec<u8>]) -> Result<Vec<u8>, ExpandError> {
        let first = args.first().map(Vec::as_slice).unwrap_or_default();
        match name {
            b"strip" => join_words(words(first)),
            b"subst" => substitute(first, &args[1], &args[2]),
            b"patsubst" => pattern_substitute(first, &args[1], &args[2]),
            b"findstring" => Ok(
                if first.is_empty() || args[1].windows(first.len()).any(|word| word == first) {
                    first.to_vec()
                } else {
                    Vec::new()
                },
            ),
            b"filter" | b"filter-out" => {
                let patterns: Vec<_> = words(first).map(Pattern::new).collect();
                join_words(words(&args[1]).filter(|word| {
                    patterns
                        .iter()
                        .any(|pattern| pattern.matches(word).is_some())
                        == (name == b"filter")
                }))
            }
            b"sort" => {
                let mut list: Vec<_> = words(first).collect();
                list.sort_unstable();
                list.dedup();
                join_words(list)
            }
            b"words" => Ok(words(first).count().to_string().into_bytes()),
            b"word" => {
                let index = number(first, name)?;
                if index == 0 {
                    return Err(invalid(name, "word index must be greater than zero"));
                }
                Ok(words(&args[1]).nth(index - 1).unwrap_or_default().to_vec())
            }
            b"wordlist" => {
                let start = number(first, name)?;
                let end = number(&args[1], name)?;
                if start == 0 {
                    return Err(invalid(name, "start index must be greater than zero"));
                }
                if end < start {
                    return Ok(Vec::new());
                }
                // Unlike most word functions, wordlist retains the whitespace
                // between selected words from the expanded input.
                let spans = word_spans(&args[2]);
                let Some((start_byte, _)) = spans.get(start - 1) else {
                    return Ok(Vec::new());
                };
                let end_byte = spans[end.min(spans.len()) - 1].1;
                Ok(args[2][*start_byte..end_byte].to_vec())
            }
            b"firstword" => Ok(words(first).next().unwrap_or_default().to_vec()),
            b"lastword" => Ok(words(first).last().unwrap_or_default().to_vec()),
            b"dir" | b"notdir" | b"suffix" | b"basename" => {
                let mut output = Vec::new();
                let mut count = 0;
                for word in words(first) {
                    let slash = word.iter().rposition(|byte| *byte == b'/');
                    let filename_start = slash.map_or(0, |position| position + 1);
                    let dot = word[filename_start..]
                        .iter()
                        .rposition(|byte| *byte == b'.')
                        .map(|position| position + filename_start);
                    let item: &[u8] = match name {
                        b"dir" => slash.map_or(b"./".as_slice(), |position| &word[..position + 1]),
                        b"notdir" => &word[filename_start..],
                        b"suffix" => {
                            let Some(position) = dot else { continue };
                            &word[position..]
                        }
                        _ => &word[..dot.unwrap_or(word.len())],
                    };
                    if count != 0 {
                        push(&mut output, b" ")?;
                    }
                    push(&mut output, item)?;
                    count += 1;
                }
                Ok(output)
            }
            b"addprefix" | b"addsuffix" => {
                let mut output = Vec::new();
                for (index, word) in words(&args[1]).enumerate() {
                    if index != 0 {
                        push(&mut output, b" ")?;
                    }
                    if name == b"addprefix" {
                        push(&mut output, first)?;
                    }
                    push(&mut output, word)?;
                    if name == b"addsuffix" {
                        push(&mut output, first)?;
                    }
                }
                Ok(output)
            }
            b"join" => {
                let left: Vec<_> = words(first).collect();
                let right: Vec<_> = words(&args[1]).collect();
                let mut output = Vec::new();
                for index in 0..left.len().max(right.len()) {
                    if index != 0 {
                        push(&mut output, b" ")?;
                    }
                    push(&mut output, left.get(index).copied().unwrap_or_default())?;
                    push(&mut output, right.get(index).copied().unwrap_or_default())?;
                }
                Ok(output)
            }
            b"origin" => Ok(self
                .lookup(first)
                .map_or(b"undefined".as_slice(), |value| value.origin.name())
                .to_vec()),
            b"flavor" => Ok(match self.lookup(first).map(|value| value.flavor) {
                None => b"undefined".to_vec(),
                Some(Flavor::Recursive) => b"recursive".to_vec(),
                Some(Flavor::Simple) => b"simple".to_vec(),
            }),
            b"value" => Ok(self
                .lookup(first)
                .map_or_else(Vec::new, |value| value.value.clone())),
            b"abspath" => {
                let directory = self.store.working_directory.as_deref().ok_or_else(|| {
                    ExpandError::Unsupported(b"abspath without invocation directory".to_vec())
                })?;
                let paths = words(first)
                    .map(|path| absolute_path(directory, path))
                    .collect::<Vec<_>>();
                join_words(paths.iter().map(Vec::as_slice))
            }
            _ => Err(ExpandError::Unsupported(name.to_vec())),
        }
    }
}

fn local(value: Vec<u8>) -> Variable {
    Variable {
        value,
        flavor: Flavor::Simple,
        origin: Origin::Automatic,
    }
}

fn invalid(function: &[u8], message: &'static str) -> ExpandError {
    ExpandError::InvalidArguments {
        function: function.to_vec(),
        message,
    }
}

fn push(output: &mut Vec<u8>, value: &[u8]) -> Result<(), ExpandError> {
    if output.len().saturating_add(value.len()) > MAX_OUTPUT {
        return Err(ExpandError::ExpansionLimit);
    }
    output.extend_from_slice(value);
    Ok(())
}

fn trim_start(mut value: &[u8]) -> &[u8] {
    while value.first().is_some_and(u8::is_ascii_whitespace) {
        value = &value[1..];
    }
    value
}

fn trim(value: &[u8]) -> &[u8] {
    let mut value = trim_start(value);
    while value.last().is_some_and(u8::is_ascii_whitespace) {
        value = &value[..value.len() - 1];
    }
    value
}

fn words(value: &[u8]) -> impl Iterator<Item = &[u8]> {
    value
        .split(u8::is_ascii_whitespace)
        .filter(|word| !word.is_empty())
}

fn word_spans(value: &[u8]) -> Vec<(usize, usize)> {
    let mut result = Vec::new();
    let mut position = 0;
    while position < value.len() {
        if value[position].is_ascii_whitespace() {
            position += 1;
            continue;
        }
        let start = position;
        while position < value.len() && !value[position].is_ascii_whitespace() {
            position += 1;
        }
        result.push((start, position));
    }
    result
}

fn join_words<'a>(values: impl IntoIterator<Item = &'a [u8]>) -> Result<Vec<u8>, ExpandError> {
    let mut output = Vec::new();
    for (index, word) in values.into_iter().enumerate() {
        if index != 0 {
            push(&mut output, b" ")?;
        }
        push(&mut output, word)?;
    }
    Ok(output)
}

fn reference_end(input: &[u8], opening: usize) -> Result<usize, ExpandError> {
    let open = input[opening];
    let close = if open == b'(' { b')' } else { b'}' };
    let mut depth = 1;
    for (offset, byte) in input[opening + 1..].iter().enumerate() {
        if *byte == open {
            depth += 1;
        }
        if *byte == close {
            depth -= 1;
            if depth == 0 {
                return Ok(opening + offset + 1);
            }
        }
    }
    Err(ExpandError::UnterminatedReference)
}

fn arity(name: &[u8]) -> Option<(usize, Option<usize>)> {
    match name {
        b"subst" | b"patsubst" | b"wordlist" | b"foreach" | b"let" => Some((3, Some(3))),
        b"findstring" | b"filter" | b"filter-out" | b"word" | b"addprefix" | b"addsuffix"
        | b"join" => Some((2, Some(2))),
        b"if" => Some((2, Some(3))),
        b"or" | b"and" | b"call" => Some((1, None)),
        b"strip" | b"sort" | b"words" | b"firstword" | b"lastword" | b"dir" | b"notdir"
        | b"suffix" | b"basename" | b"origin" | b"flavor" | b"value" | b"abspath" | b"realpath"
        | b"wildcard" | b"shell" | b"eval" | b"info" | b"warning" | b"error" | b"guile" => {
            Some((1, Some(1)))
        }
        b"file" => Some((1, Some(2))),
        _ => None,
    }
}

fn split_args(
    input: &[u8],
    delimiter: u8,
    maximum: Option<usize>,
) -> Result<Vec<&[u8]>, ExpandError> {
    let close = if delimiter == b'(' { b')' } else { b'}' };
    let mut result = Vec::new();
    let mut start = 0;
    let mut position = 0;
    let mut nesting = 0;
    while position < input.len() {
        if input[position] == b'$'
            && input
                .get(position + 1)
                .is_some_and(|b| *b == b'(' || *b == b'{')
        {
            position = reference_end(input, position + 1)? + 1;
            continue;
        }
        if input[position] == delimiter {
            nesting += 1;
        }
        if input[position] == close && nesting > 0 {
            nesting -= 1;
        }
        if input[position] == b','
            && nesting == 0
            && maximum.is_none_or(|max| result.len() + 1 < max)
        {
            result.push(&input[start..position]);
            start = position + 1;
        }
        position += 1;
    }
    result.push(&input[start..]);
    Ok(result)
}

fn number(value: &[u8], function: &[u8]) -> Result<usize, ExpandError> {
    let value = trim(value);
    let digits = value.strip_prefix(b"+").unwrap_or(value);
    if digits.is_empty() || !digits.iter().all(u8::is_ascii_digit) {
        return Err(invalid(function, "expected a nonnegative integer"));
    }
    digits.iter().try_fold(0usize, |result, digit| {
        result
            .checked_mul(10)
            .and_then(|result| result.checked_add(usize::from(digit - b'0')))
            .ok_or_else(|| invalid(function, "integer is too large"))
    })
}

fn substitute(from: &[u8], to: &[u8], text: &[u8]) -> Result<Vec<u8>, ExpandError> {
    let mut output = Vec::new();
    if from.is_empty() {
        push(&mut output, text)?;
        push(&mut output, to)?;
        return Ok(output);
    }
    let mut position = 0;
    while position < text.len() {
        if text[position..].starts_with(from) {
            push(&mut output, to)?;
            position += from.len();
        } else {
            push(&mut output, &text[position..position + 1])?;
            position += 1;
        }
    }
    Ok(output)
}

struct Pattern {
    bytes: Vec<u8>,
    wildcard: Option<usize>,
}

impl Pattern {
    fn new(input: &[u8]) -> Self {
        let mut bytes = Vec::new();
        let mut wildcard = None;
        let mut position = 0;
        while position < input.len() {
            if input[position] == b'\\' {
                let start = position;
                while input.get(position) == Some(&b'\\') {
                    position += 1;
                }
                let count = position - start;
                if input.get(position) == Some(&b'%') && wildcard.is_none() {
                    bytes.extend(std::iter::repeat_n(b'\\', count / 2));
                    if count % 2 == 0 {
                        wildcard = Some(bytes.len());
                    }
                    bytes.push(b'%');
                    position += 1;
                } else {
                    bytes.extend(std::iter::repeat_n(b'\\', count));
                }
                continue;
            }
            if input[position] == b'%' && wildcard.is_none() {
                wildcard = Some(bytes.len());
            }
            bytes.push(input[position]);
            position += 1;
        }
        Self { bytes, wildcard }
    }

    fn matches<'a>(&self, word: &'a [u8]) -> Option<&'a [u8]> {
        match self.wildcard {
            None => (word == self.bytes).then_some(&word[word.len()..]),
            Some(position) => {
                let prefix = &self.bytes[..position];
                let suffix = &self.bytes[position + 1..];
                if word.len() >= prefix.len() + suffix.len()
                    && word.starts_with(prefix)
                    && word.ends_with(suffix)
                {
                    Some(&word[prefix.len()..word.len() - suffix.len()])
                } else {
                    None
                }
            }
        }
    }

    fn replace(&self, stem: &[u8]) -> Result<Vec<u8>, ExpandError> {
        if let Some(position) = self.wildcard {
            let mut output = self.bytes[..position].to_vec();
            push(&mut output, stem)?;
            push(&mut output, &self.bytes[position + 1..])?;
            Ok(output)
        } else {
            Ok(self.bytes.clone())
        }
    }
}

fn pattern_substitute(from: &[u8], to: &[u8], text: &[u8]) -> Result<Vec<u8>, ExpandError> {
    let pattern = Pattern::new(from);
    let replacement = Pattern::new(to);
    let mut output = Vec::new();
    if pattern.wildcard.is_none() {
        let mut previous = 0;
        for (start, end) in word_spans(text) {
            push(&mut output, &text[previous..start])?;
            push(
                &mut output,
                if pattern.matches(&text[start..end]).is_some() {
                    &replacement.bytes
                } else {
                    &text[start..end]
                },
            )?;
            previous = end;
        }
        push(&mut output, &text[previous..])?;
    } else {
        let mut first = true;
        for word in words(text) {
            let value = pattern
                .matches(word)
                .map_or_else(|| Ok(word.to_vec()), |stem| replacement.replace(stem))?;
            // Empty substitutions remove a word, including its separator.
            if value.is_empty() {
                continue;
            }
            if !first {
                push(&mut output, b" ")?;
            }
            first = false;
            push(&mut output, &value)?;
        }
    }
    Ok(output)
}

fn absolute_path(directory: &[u8], path: &[u8]) -> Vec<u8> {
    let combined = if path.starts_with(b"/") {
        path.to_vec()
    } else {
        let mut combined = directory.to_vec();
        combined.push(b'/');
        combined.extend_from_slice(path);
        combined
    };
    let mut parts = Vec::new();
    for part in combined.split(|byte| *byte == b'/') {
        match part {
            b"" | b"." => {}
            b".." => {
                parts.pop();
            }
            _ => parts.push(part),
        }
    }
    let mut result = vec![b'/'];
    for (index, part) in parts.iter().enumerate() {
        if index != 0 {
            result.push(b'/');
        }
        result.extend_from_slice(part);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn define(store: &mut VariableStore, name: &[u8], value: &[u8], assignment: Assignment) {
        store.define(name, value, assignment, Origin::File).unwrap();
    }

    fn expand(input: &[u8]) -> Vec<u8> {
        VariableStore::new().expand(input).unwrap()
    }

    #[test]
    fn preserves_non_utf8_and_literal_dollars() {
        let mut store = VariableStore::new();
        define(&mut store, b"\xff", b"\xfe", Assignment::Recursive);
        assert_eq!(
            store.expand(b"\x80$\xff/$${literal}/tail$").unwrap(),
            b"\x80\xfe/${literal}/tail$"
        );
    }

    #[test]
    fn single_character_braces_and_unknown_variables() {
        let mut store = VariableStore::new();
        define(&mut store, b"X", b"yes", Assignment::Recursive);
        assert_eq!(
            store.expand(b"$X:${X}:$(X):$Y:$(missing)").unwrap(),
            b"yes:yes:yes::"
        );
    }

    #[test]
    fn nested_names_expand_before_lookup() {
        let mut store = VariableStore::new();
        define(&mut store, b"KIND", b"blue", Assignment::Recursive);
        define(&mut store, b"blue_COUNT", b"seven", Assignment::Recursive);
        assert_eq!(store.expand(b"$($(KIND)_COUNT)").unwrap(), b"seven");
    }

    #[test]
    fn names_with_spaces_are_not_implicitly_trimmed_or_function_calls() {
        let mut store = VariableStore::new();
        define(&mut store, b"odd name", b"literal", Assignment::Recursive);
        define(&mut store, b"strip", b"variable", Assignment::Recursive);
        assert_eq!(
            store
                .expand(b"$(odd name):$(strip):$( strip):$(strip )")
                .unwrap(),
            b"literal:variable::"
        );
    }

    #[test]
    fn simple_values_do_not_expand_again() {
        let mut store = VariableStore::new();
        define(&mut store, b"X", b"$$(missing)", Assignment::Simple);
        assert_eq!(store.expand(b"$(X)").unwrap(), b"$(missing)");
    }

    #[test]
    fn immediate_and_recursive_assignment_observe_different_times() {
        let mut store = VariableStore::new();
        define(&mut store, b"BASE", b"before", Assignment::Recursive);
        define(&mut store, b"SIMPLE", b"$(BASE)", Assignment::Simple);
        define(&mut store, b"LATE", b"$(BASE)", Assignment::Recursive);
        define(&mut store, b"BASE", b"after", Assignment::Recursive);
        assert_eq!(store.expand(b"$(SIMPLE):$(LATE)").unwrap(), b"before:after");
    }

    #[test]
    fn immediate_recursive_escapes_results_but_appends_lazily() {
        let mut store = VariableStore::new();
        define(
            &mut store,
            b"X",
            b"$$(LATE)",
            Assignment::ImmediateRecursive,
        );
        define(&mut store, b"LATE", b"first", Assignment::Recursive);
        define(&mut store, b"X", b"$(LATE)", Assignment::Append);
        define(&mut store, b"LATE", b"last", Assignment::Recursive);
        assert_eq!(
            store.expand(b"$(X):$(flavor X)").unwrap(),
            b"$(LATE) last:recursive"
        );
    }

    #[test]
    fn append_respects_existing_flavor_and_empty_values() {
        let mut store = VariableStore::new();
        define(&mut store, b"BASE", b"first", Assignment::Recursive);
        define(&mut store, b"R", b"start", Assignment::Recursive);
        define(&mut store, b"S", b"start", Assignment::Simple);
        define(&mut store, b"R", b"$(BASE)", Assignment::Append);
        define(&mut store, b"S", b"$(BASE)", Assignment::Append);
        define(&mut store, b"U", b"$(BASE)", Assignment::Append);
        define(&mut store, b"BASE", b"last", Assignment::Recursive);
        define(&mut store, b"R", b"", Assignment::Append);
        assert_eq!(
            store.expand(b"$(R):$(S):$(U)").unwrap(),
            b"start last:start first:last"
        );
        define(&mut store, b"E", b"", Assignment::Recursive);
        define(&mut store, b"E", b"next", Assignment::Append);
        assert_eq!(store.expand(b"$(E)").unwrap(), b"next");
    }

    #[test]
    fn conditional_distinguishes_defined_empty_from_undefined() {
        let mut store = VariableStore::new();
        define(&mut store, b"EMPTY", b"", Assignment::Recursive);
        define(
            &mut store,
            b"EMPTY",
            b"$(shell forbidden)",
            Assignment::Conditional,
        );
        define(&mut store, b"NEW", b"fallback", Assignment::Conditional);
        assert_eq!(store.expand(b"[$(EMPTY)]:$(NEW)").unwrap(), b"[]:fallback");
    }

    #[test]
    fn precedence_blocks_rhs_evaluation_and_preserves_origin() {
        let mut store = VariableStore::new();
        store
            .define(b"X", b"command", Assignment::Recursive, Origin::CommandLine)
            .unwrap();
        store
            .define(
                b"X",
                b"$(shell forbidden)",
                Assignment::Simple,
                Origin::File,
            )
            .unwrap();
        assert_eq!(
            store.expand(b"$(X):$(origin X)").unwrap(),
            b"command:command line"
        );
        store
            .define(b"X", b"forced", Assignment::Recursive, Origin::Override)
            .unwrap();
        assert_eq!(
            store.expand(b"$(X):$(origin X)").unwrap(),
            b"forced:override"
        );
    }

    #[test]
    fn environment_precedence_and_remove() {
        let mut store = VariableStore::new();
        store
            .define(b"X", b"env", Assignment::Recursive, Origin::Environment)
            .unwrap();
        define(&mut store, b"X", b"file", Assignment::Recursive);
        assert_eq!(store.expand(b"$(X)").unwrap(), b"file");
        store
            .define(
                b"X",
                b"env-override",
                Assignment::Recursive,
                Origin::EnvironmentOverride,
            )
            .unwrap();
        define(&mut store, b"X", b"ignored", Assignment::Recursive);
        assert_eq!(
            store.expand(b"$(X):$(origin X)").unwrap(),
            b"env-override:environment override"
        );
        assert_eq!(store.definitions().count(), 1);
        assert!(store.remove(b"X").is_some());
        assert_eq!(store.expand(b"$(origin X)").unwrap(), b"undefined");
    }

    #[test]
    fn recursion_and_unterminated_references_are_errors() {
        let mut store = VariableStore::new();
        define(&mut store, b"A", b"$(B)", Assignment::Recursive);
        define(&mut store, b"B", b"$(A)", Assignment::Recursive);
        assert_eq!(
            store.expand(b"$(A)"),
            Err(ExpandError::RecursiveVariable(b"A".to_vec()))
        );
        assert_eq!(
            store.expand(b"${missing"),
            Err(ExpandError::UnterminatedReference)
        );
    }

    #[test]
    fn failed_definition_leaves_prior_value_intact() {
        let mut store = VariableStore::new();
        define(&mut store, b"X", b"before", Assignment::Simple);
        assert!(
            store
                .define(b"X", b"$(shell command)", Assignment::Simple, Origin::File)
                .is_err()
        );
        assert_eq!(store.expand(b"$(X)").unwrap(), b"before");
    }

    #[test]
    fn pure_string_and_list_functions() {
        let cases: &[(&[u8], &[u8])] = &[
            (b"$(strip  a\t b   c  )", b"a b c"),
            (b"$(subst a,b,aa,aa)", b"bb,bb"),
            (b"$(subst ,x,abc)", b"abcx"),
            (b"$(subst a, b,a)", b" b"),
            (b"$(findstring ab,zabc)", b"ab"),
            (b"$(findstring missing,word)", b""),
            (b"$(filter a% a,a ab ba a)", b"a ab a"),
            (b"$(filter-out %.c,one.c two.h three.c)", b"two.h"),
            (b"$(sort z a z b a)", b"a b z"),
            (b"$(words   a b c )", b"3"),
            (b"$(word +2,a b c)", b"b"),
            (b"$(word 9,a b)", b""),
            (b"$(wordlist 2,3,a  b\tc d)", b"b\tc"),
            (b"$(wordlist 3,1,a b c)", b""),
            (b"$(wordlist 1,0,a)", b""),
            (b"$(firstword  a b )", b"a"),
            (b"$(lastword  a b )", b"b"),
            (b"$(join a b,1 2 3)", b"a1 b2 3"),
            (b"$(addprefix src/,a.c b.c)", b"src/a.c src/b.c"),
            (b"$(addsuffix .o,a b)", b"a.o b.o"),
        ];
        for (input, expected) in cases {
            assert_eq!(
                expand(input),
                *expected,
                "{}",
                String::from_utf8_lossy(input)
            );
        }
    }

    #[test]
    fn pure_functions_do_not_decode_utf8() {
        assert_eq!(expand(b"$(sort \xff \x80 a \xff)"), b"a \x80 \xff");
        assert_eq!(expand(b"$(subst \xff,z,\x80\xff)"), b"\x80z");
    }

    #[test]
    fn fixed_arity_last_argument_retains_commas() {
        assert_eq!(expand(b"$(subst a,b,(a,a))"), b"(b,b)");
        assert_eq!(expand(b"$(word 2,a b,c)"), b"b,c");
        assert_eq!(expand(b"$(subst {,x,{)"), b"x");
    }

    #[test]
    fn nested_function_arguments_are_not_split_at_inner_commas() {
        assert_eq!(expand(b"$(subst a,z,$(subst b,a,bb))"), b"zz");
        assert_eq!(expand(b"$(subst a,z,${subst b,a,bb})"), b"zz");
    }

    #[test]
    fn invalid_arity_and_indices_are_errors() {
        for input in [
            b"$(subst a,b)".as_slice(),
            b"$(word 0,a)",
            b"$(word x,a)",
            b"$(wordlist 0,1,a)",
            b"$(word -1,a)",
        ] {
            assert!(matches!(
                VariableStore::new().expand(input),
                Err(ExpandError::InvalidArguments { .. })
            ));
        }
    }

    #[test]
    fn pattern_replacement_and_literal_replacement_have_distinct_whitespace() {
        assert_eq!(
            expand(b"$(patsubst %.c,%.o,  a.c\tb.c  c.h  )"),
            b"a.o b.o c.h"
        );
        assert_eq!(expand(b"$(patsubst a,b,  a\tb  )"), b"  b\tb  ");
        assert_eq!(expand(b"$(patsubst %.c,,a.c other b.c)"), b"other");
    }

    #[test]
    fn percent_escaping_and_suffix_references() {
        assert_eq!(expand(br"$(patsubst a\%b,ok,a%b aXXb)"), b"ok aXXb");
        assert_eq!(expand(br"$(filter a\%b a%,a%b abc z)"), b"a%b abc");
        let mut store = VariableStore::new();
        define(&mut store, b"X", b" a.c  b.c x.h ", Assignment::Recursive);
        assert_eq!(store.expand(b"$(X:.c=.o)").unwrap(), b"a.o b.o x.h");
        assert_eq!(
            store.expand(b"$(X:%.c=obj/%.o)").unwrap(),
            b"obj/a.o obj/b.o x.h"
        );
    }

    #[test]
    fn filename_functions_keep_their_distinct_empty_word_behavior() {
        assert_eq!(expand(b"$(dir src/a.c b.c nested/)"), b"src/ ./ nested/");
        assert_eq!(expand(b"$(notdir a/ b.c c/)"), b" b.c ");
        assert_eq!(
            expand(b"$(basename a/.profile dir.with.dot/foo file. a..b)"),
            b"a/ dir.with.dot/foo file a."
        );
        assert_eq!(
            expand(b"$(suffix a/.profile dir.with.dot/foo file. a..b)"),
            b".profile . .b"
        );
    }

    #[test]
    fn abspath_uses_actual_directory_not_curdir_variable() {
        let mut store = VariableStore::new();
        store.set_working_directory(b"/work/project");
        define(&mut store, b"CURDIR", b"/pretend", Assignment::Recursive);
        assert_eq!(
            store
                .expand(b"$(abspath ./a ../b /x/../y ../../../../z)")
                .unwrap(),
            b"/work/project/a /work/b /y /z"
        );
    }

    #[test]
    fn metadata_functions_do_not_expand_values_or_trim_names() {
        let mut store = VariableStore::new();
        define(&mut store, b"X", b"$(shell never)", Assignment::Recursive);
        assert_eq!(
            store
                .expand(b"$(value X):$(flavor X):$(origin X):$(origin X )")
                .unwrap(),
            b"$(shell never):recursive:file:undefined"
        );
    }

    #[test]
    fn lazy_boolean_functions_skip_unsupported_effects() {
        assert_eq!(expand(b"$(if yes,good,$(shell never))"), b"good");
        assert_eq!(expand(b"$(if ,$(error never),good)"), b"good");
        assert_eq!(expand(b"$(or ,first,$(shell never))"), b"first");
        assert_eq!(expand(b"$(and ,$(shell never))"), b"");
        assert_eq!(expand(b"$(and first,second,third)"), b"third");
    }

    #[test]
    fn foreach_bindings_are_simple_automatic_and_restored() {
        let mut store = VariableStore::new();
        define(&mut store, b"x", b"outer", Assignment::Recursive);
        assert_eq!(
            store
                .expand(b"$(foreach x,a b,$(x):$(origin x):$(flavor x))|$(x)")
                .unwrap(),
            b"a:automatic:simple b:automatic:simple|outer"
        );
        assert_eq!(store.expand(b"$(foreach x,,$(shell never))").unwrap(), b"");
    }

    #[test]
    fn foreach_retains_separators_for_empty_body_results() {
        assert_eq!(expand(b"[$(foreach x,a b c,)]"), b"[  ]");
    }

    #[test]
    fn nested_calls_mask_missing_parameters_and_restore_outer_scope() {
        let mut store = VariableStore::new();
        define(&mut store, b"f", b"$(1):$(2)", Assignment::Recursive);
        define(
            &mut store,
            b"g",
            b"$(call f,inner)|$(1):$(2)",
            Assignment::Recursive,
        );
        assert_eq!(
            store.expand(b"$(call g,outer,second)").unwrap(),
            b"inner:|outer:second"
        );
    }

    #[test]
    fn call_arguments_expand_once_and_builtin_names_take_precedence() {
        let mut store = VariableStore::new();
        define(&mut store, b"f", b"$(1)", Assignment::Recursive);
        define(&mut store, b"D", b"$$hi", Assignment::Recursive);
        define(&mut store, b"sort", b"shadowed", Assignment::Recursive);
        assert_eq!(
            store
                .expand(b"$(call f,$(D)):$(call strip,$(D)):$(call sort,z a)")
                .unwrap(),
            b"$hi:$hi:a z"
        );
    }

    #[test]
    fn call_expands_arguments_even_when_function_does_not_use_them() {
        let mut store = VariableStore::new();
        define(&mut store, b"f", b"constant", Assignment::Recursive);
        assert!(matches!(
            store.expand(b"$(call f,$(shell forbidden))"),
            Err(ExpandError::Unsupported(_))
        ));
    }

    #[test]
    fn recursive_calls_are_bounded_but_not_confused_with_variable_cycles() {
        let mut store = VariableStore::new();
        define(
            &mut store,
            b"walk",
            b"$(if $(1),$(firstword $(1)) $(call walk,$(wordlist 2,99,$(1))))",
            Assignment::Recursive,
        );
        assert_eq!(store.expand(b"$(call walk,a b c)").unwrap(), b"a b c ");
        define(&mut store, b"loop", b"$(call loop)", Assignment::Recursive);
        assert_eq!(
            store.expand(b"$(call loop)"),
            Err(ExpandError::ExpansionLimit)
        );
    }

    #[test]
    fn let_distributes_words_and_restores_scope() {
        let mut store = VariableStore::new();
        define(&mut store, b"a", b"outer", Assignment::Recursive);
        assert_eq!(
            store
                .expand(b"$(let a b,one two three,$(a):$(b):$(flavor b))|$(a)")
                .unwrap(),
            b"one:two three:simple|outer"
        );
    }

    #[test]
    fn known_effects_are_explicitly_unsupported() {
        for name in [
            "shell", "eval", "file", "info", "warning", "error", "wildcard", "realpath", "guile",
        ] {
            let input = format!("$({name} value)");
            assert_eq!(
                VariableStore::new().expand(input.as_bytes()),
                Err(ExpandError::Unsupported(name.as_bytes().to_vec()))
            );
        }
        assert!(matches!(
            VariableStore::new().define(b"X", b"command", Assignment::Shell, Origin::File),
            Err(ExpandError::Unsupported(_))
        ));
    }

    #[test]
    fn foreach_trims_the_expanded_binding_name() {
        assert_eq!(
            expand(b"$(foreach x ,a b,[$(x)]-[$(x )])"),
            b"[a]-[] [b]-[]"
        );
    }

    #[test]
    fn call_to_lazy_builtin_still_expands_its_body() {
        assert_eq!(expand(b"$(call foreach,x,a b,$$(x))"), b"a b");
        assert_eq!(expand(b"$(call let,x,a b,$$(x))"), b"a b");
    }

    #[test]
    fn literal_values_preserve_dollars_without_rhs_evaluation() {
        let mut store = VariableStore::new();
        store
            .define_literal(b"@", b"output$X", Origin::Automatic)
            .unwrap();
        assert_eq!(
            store.expand(b"$@ $(flavor @) $(origin @)").unwrap(),
            b"output$X simple automatic"
        );
        store
            .define_literal(b"@", b"ignored", Origin::File)
            .unwrap();
        assert_eq!(store.expand(b"$@").unwrap(), b"output$X");
    }

    #[test]
    fn clones_are_independent_and_expose_raw_metadata() {
        let mut store = VariableStore::new();
        define(&mut store, b"X", b"original", Assignment::Simple);
        let mut copied = store.clone();
        define(&mut copied, b"X", b"changed", Assignment::Recursive);
        assert_eq!(store.get(b"X").unwrap().raw_value(), b"original");
        assert_eq!(store.get(b"X").unwrap().flavor(), Flavor::Simple);
        assert_eq!(store.get(b"X").unwrap().origin(), Origin::File);
    }
}
