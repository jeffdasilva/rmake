// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Byte-preserving variables and expansion with explicit effect receivers.
//!
//! `define` receives an already-expanded variable name and an RHS whose assignment
//! syntax whitespace has been removed by the reader. Expansion never interprets
//! UTF-8. Output and filesystem effects use explicit callbacks; core evaluation
//! performs no operating-system calls. An unknown variable still expands to empty.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, OnceLock};

use crate::diagnostic::{
    Diagnostic, ExpansionContext, FilesystemEffect, FilesystemError, Location, Output, ShellRequest,
};

const MAX_DEPTH: usize = 128;
pub const MAX_OUTPUT: usize = 16 * 1024 * 1024;

#[cfg(test)]
mod shell_effect_tests {
    use super::*;
    use crate::diagnostic::ShellResult;

    #[test]
    fn cancellation_after_output_prevents_the_next_effect() {
        use std::sync::atomic::{AtomicBool, Ordering};
        let flag = AtomicBool::new(false);
        let mut emitted = 0;
        let mut receiver = |_: Output<'_>| {
            emitted += 1;
            flag.store(true, Ordering::Relaxed);
        };
        let mut filesystem = |_: FilesystemEffect<'_>| -> Result<Vec<u8>, FilesystemError> {
            panic!("effect after cancellation")
        };
        let error = VariableStore::new()
            .expand_mut_with(
                b"$(info before)$(file >forbidden)",
                ExpansionContext::new(None, &mut receiver)
                    .with_cancellation(&flag)
                    .with_filesystem(&mut filesystem),
            )
            .unwrap_err();
        assert_eq!(error, ExpandError::Cancelled);
        assert_eq!(emitted, 1);
    }

    #[test]
    fn shell_status_is_an_override_binding_visible_before_the_expansion_finishes() {
        let mut variables = VariableStore::new();
        let mut emit = |_: Output<'_>| {};
        let mut run = |_: ShellRequest<'_>| {
            Ok(ShellResult {
                stdout: b"first\r\nsecond\n\n".to_vec(),
                status: 7,
            })
        };
        assert_eq!(
            variables
                .expand_mut_with(
                    b"$(shell command):$(.SHELLSTATUS):$(origin .SHELLSTATUS)",
                    ExpansionContext::new(None, &mut emit).with_shell(&mut run)
                )
                .unwrap(),
            b"first second:7:override"
        );
        assert_eq!(
            variables
                .expand(b"$(.SHELLSTATUS):$(flavor .SHELLSTATUS)")
                .unwrap(),
            b"7:simple"
        );
    }

    #[test]
    fn empty_shell_command_does_not_launch_or_replace_previous_status() {
        let mut variables = VariableStore::new();
        variables
            .define_literal(b".SHELLSTATUS", b"9", Origin::Override)
            .unwrap();
        let mut emit = |_: Output<'_>| {};
        let mut run = |_: ShellRequest<'_>| -> Result<ShellResult, Vec<u8>> {
            panic!("empty command launched")
        };
        assert_eq!(
            variables
                .expand_mut_with(
                    b"$(shell ):$(.SHELLSTATUS)",
                    ExpansionContext::new(None, &mut emit).with_shell(&mut run)
                )
                .unwrap(),
            b":9"
        );
    }
}

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
    definition: Option<Arc<Location>>,
    scope: ScopeMetadata,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct ScopeMetadata {
    private: bool,
    export: ExportPolicy,
    export_default_reset: bool,
    export_if_set: bool,
    append_outer: bool,
    variable_names: bool,
}

/// Environment preparation remembers inherited policy on the selected binding.
/// Shared scope layers observe that metadata; real binding copies take a value
/// snapshot rather than sharing mutable state with the original definition.
#[derive(Debug, Default)]
struct ExportPolicy(AtomicU8);

impl ExportPolicy {
    fn get(&self) -> Option<bool> {
        match self.0.load(Ordering::Relaxed) {
            1 => Some(true),
            2 => Some(false),
            _ => None,
        }
    }

    fn encoded(value: Option<bool>) -> u8 {
        match value {
            None => 0,
            Some(true) => 1,
            Some(false) => 2,
        }
    }

    fn set(&self, value: Option<bool>) {
        self.0.store(Self::encoded(value), Ordering::Relaxed);
    }

    fn resolve(&self, value: Option<bool>) -> Option<bool> {
        let _ = self.0.compare_exchange(
            0,
            Self::encoded(value),
            Ordering::Relaxed,
            Ordering::Relaxed,
        );
        self.get()
    }
}

impl Clone for ExportPolicy {
    fn clone(&self) -> Self {
        Self(AtomicU8::new(Self::encoded(self.get())))
    }
}

impl PartialEq for ExportPolicy {
    fn eq(&self, other: &Self) -> bool {
        self.get() == other.get()
    }
}

impl Eq for ExportPolicy {}

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

    pub fn definition(&self) -> Option<&Location> {
        self.definition.as_deref()
    }

    pub fn is_private(&self) -> bool {
        self.scope.private
    }

    pub fn export_policy(&self) -> Option<bool> {
        self.scope.export.get().or_else(|| {
            self.scope
                .export_if_set
                .then_some(self.origin != Origin::Default)
        })
    }

    /// A runtime alias reset global policy to GNU's default. The original
    /// inherited-environment name alone must no longer force this binding out.
    pub fn export_default_reset(&self) -> bool {
        self.scope.export_default_reset
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExpandError {
    Cancelled,
    Located {
        location: Arc<Location>,
        error: Box<ExpandError>,
    },
    Unsupported(Vec<u8>),
    FileError(Vec<u8>),
    UnlocatedError(Vec<u8>),
    UserError(Vec<u8>),
    /// A user error inside a recursive binding keeps the first entered binding
    /// context. An explicit absent location must survive diagnostic unwinding.
    ContextualUserError {
        message: Vec<u8>,
        location: Option<Arc<Location>>,
    },
    UnterminatedReference,
    RecursiveVariable(Vec<u8>),
    EmptyVariableName,
    InvalidArguments {
        function: Vec<u8>,
        message: &'static str,
    },
    InvalidInteger {
        second: bool,
        value: Vec<u8>,
        empty: bool,
    },
    InsufficientArguments {
        function: Vec<u8>,
        count: usize,
    },
    WordIndexZero,
    WordIndexError(Vec<u8>),
    ExpansionLimit,
}

impl ExpandError {
    pub fn is_unlocated(&self) -> bool {
        matches!(self, Self::UnlocatedError(_))
    }
    pub fn location(&self) -> Option<&Location> {
        match self {
            Self::Located { location, .. } => Some(location),
            Self::ContextualUserError { location, .. } => location.as_deref(),
            _ => None,
        }
    }

    /// The error function uses the active parse/recipe/export context, while
    /// expansion diagnostics such as intcmp retain the innermost definition.
    /// Without a caller, user errors retain their first binding context (even
    /// an unlocated CLI/environment binding); validation errors use the innermost definition.
    pub fn diagnostic_location<'a>(
        &'a self,
        context: Option<&'a Location>,
    ) -> Option<&'a Location> {
        if matches!(self, Self::UnlocatedError(_)) {
            return None;
        }
        if self.is_user_error() {
            context.or(self.location())
        } else {
            self.location().or(context)
        }
    }

    fn is_user_error(&self) -> bool {
        match self {
            Self::UserError(_) | Self::ContextualUserError { .. } => true,
            Self::Located { error, .. } => error.is_user_error(),
            _ => false,
        }
    }

    fn at(self, location: Option<&Arc<Location>>) -> Self {
        if matches!(
            self,
            Self::ContextualUserError { .. } | Self::UnlocatedError(_)
        ) {
            return self;
        }
        match location {
            Some(location) if self.location().is_none() => Self::Located {
                location: Arc::clone(location),
                error: Box::new(self),
            },
            _ => self,
        }
    }

    pub fn diagnostic(&self) -> Diagnostic {
        match self {
            Self::Located { error, .. } => error.diagnostic(),
            Self::FileError(message)
            | Self::UnlocatedError(message)
            | Self::WordIndexError(message)
            | Self::UserError(message)
            | Self::ContextualUserError { message, .. } => message.clone().into(),
            Self::RecursiveVariable(name) => [
                b"Recursive variable '".as_slice(),
                name,
                b"' references itself (eventually)",
            ]
            .concat()
            .into(),
            Self::InvalidInteger {
                second,
                value,
                empty,
            } => {
                let mut message = b"non-numeric ".to_vec();
                message.extend_from_slice(if *second { b"second" } else { b"first" });
                message.extend_from_slice(b" argument to 'intcmp' function: ");
                if *empty {
                    message.extend_from_slice(b"empty value");
                } else {
                    message.push(b'\'');
                    message.extend_from_slice(value);
                    message.push(b'\'');
                }
                message.into()
            }
            Self::InsufficientArguments { function, count } => {
                let mut message =
                    format!("insufficient number of arguments ({count}) to function '")
                        .into_bytes();
                message.extend_from_slice(function);
                message.push(b'\'');
                message.into()
            }
            _ => self.to_string().into(),
        }
    }
}

impl fmt::Display for ExpandError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Located { error, .. } => error.fmt(output),
            Self::FileError(message)
            | Self::UnlocatedError(message)
            | Self::WordIndexError(message)
            | Self::UserError(message)
            | Self::ContextualUserError { message, .. } => {
                output.write_str(&String::from_utf8_lossy(message))
            }
            Self::Unsupported(name) => write!(
                output,
                "unsupported expansion feature '{}'",
                String::from_utf8_lossy(name)
            ),
            Self::UnterminatedReference => output.write_str("unterminated variable reference"),
            Self::RecursiveVariable(name) => write!(
                output,
                "Recursive variable '{}' references itself (eventually)",
                String::from_utf8_lossy(name)
            ),
            Self::EmptyVariableName => output.write_str("empty variable name"),
            Self::InvalidArguments { function, message } => write!(
                output,
                "invalid arguments to '{}': {message}",
                String::from_utf8_lossy(function)
            ),
            Self::Cancelled => output.write_str("interrupted"),
            Self::ExpansionLimit => output.write_str("variable expansion resource limit exceeded"),
            Self::WordIndexZero => {
                output.write_str("first argument to 'word' function must be greater than 0")
            }
            Self::InvalidInteger { .. } | Self::InsufficientArguments { .. } => {
                write!(output, "{}", self.diagnostic())
            }
        }
    }
}

impl std::error::Error for ExpandError {}

#[derive(Clone, Debug, Default)]
struct NamesState {
    generation: u64,
    value: Vec<u8>,
}
#[derive(Debug, Default)]
struct NamesCache(std::sync::Mutex<NamesState>);
impl Clone for NamesCache {
    fn clone(&self) -> Self {
        Self(std::sync::Mutex::new(
            self.0
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .clone(),
        ))
    }
}

#[derive(Clone, Debug)]
struct Bindings {
    values: BTreeMap<Arc<[u8]>, Variable>,
    order: crate::variable_order::NameOrder,
    reserved_exports: Vec<Vec<u8>>,
    generation: u64,
    names_cache: OnceLock<Arc<NamesCache>>,
}
impl Default for Bindings {
    fn default() -> Self {
        Self::new(1024)
    }
}
impl Bindings {
    fn new(size: usize) -> Self {
        Self {
            values: BTreeMap::new(),
            order: crate::variable_order::NameOrder::deferred(size, size < 1024),
            reserved_exports: Vec::new(),
            generation: 0,
            names_cache: OnceLock::new(),
        }
    }
    fn insert(&mut self, name: &[u8], variable: Variable) {
        match self.values.entry(name.into()) {
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                if entry.get().scope.variable_names
                    && let Some(cache) = self.names_cache.get_mut()
                {
                    Arc::make_mut(cache)
                        .0
                        .lock()
                        .unwrap_or_else(|error| error.into_inner())
                        .value = variable.value.clone();
                }
                entry.insert(variable);
            }
            std::collections::btree_map::Entry::Vacant(entry) => {
                if let Some(cache) = self.names_cache.get_mut() {
                    Arc::make_mut(cache);
                }
                self.generation += 1;
                self.order.insert_owned(entry.key().clone());
                entry.insert(variable);
            }
        }
    }
    fn remove(&mut self, name: &[u8]) -> Option<Variable> {
        let value = self.values.remove(name);
        if value.is_some() {
            if let Some(cache) = self.names_cache.get_mut() {
                Arc::make_mut(cache);
            }
            self.generation += 1;
        }
        self.order.remove(name);
        value
    }
    fn get_mut(&mut self, name: &[u8]) -> Option<&mut Variable> {
        self.values.get_mut(name)
    }
}
impl std::ops::Deref for Bindings {
    type Target = BTreeMap<Arc<[u8]>, Variable>;
    fn deref(&self) -> &Self::Target {
        &self.values
    }
}

#[derive(Clone, Debug)]
pub struct VariableStore {
    variables: Arc<Bindings>,
    layers: Vec<VariableLayer>,
    working_directory: Option<Arc<[u8]>>,
    original_shell: Option<Arc<[u8]>>,
    environment_override: bool,
    scoped: bool,
    file_locals: bool,
}

impl Default for VariableStore {
    fn default() -> Self {
        Self {
            variables: empty_bindings(1024),
            layers: Vec::new(),
            working_directory: None,
            original_shell: None,
            environment_override: false,
            scoped: false,
            file_locals: false,
        }
    }
}

fn empty_bindings(size: usize) -> Arc<Bindings> {
    static GLOBAL: OnceLock<Arc<Bindings>> = OnceLock::new();
    static FILE: OnceLock<Arc<Bindings>> = OnceLock::new();
    static PATTERN: OnceLock<Arc<Bindings>> = OnceLock::new();
    let slot = match size {
        1024 => &GLOBAL,
        32 => &FILE,
        16 => &PATTERN,
        _ => unreachable!("scope size"),
    };
    slot.get_or_init(|| Arc::new(Bindings::new(size))).clone()
}

#[derive(Clone, Debug)]
struct VariableLayer {
    file_locals: bool,
    variables: Arc<Bindings>,
    inherited: bool,
    global: bool,
    pattern: bool,
}

/// Bindings attached to a target or pattern, independent of build rules.
/// Clones share immutable definitions; modifying a declaration uses copy on write.
#[derive(Clone, Debug)]
pub struct TargetScope {
    variables: Arc<Bindings>,
    pattern: bool,
}

impl Default for TargetScope {
    fn default() -> Self {
        Self {
            variables: empty_bindings(32),
            pattern: false,
        }
    }
}

/// One pattern declaration, prepared while reading and applied in specificity
/// and declaration order when a matching target is first considered.
#[derive(Clone, Debug)]
pub struct PatternVariable {
    name: Vec<u8>,
    value: Vec<u8>,
    assignment: Assignment,
    origin: Origin,
    private: bool,
    export: Option<bool>,
    definition: Option<Arc<Location>>,
}

impl VariableStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Invocation data, independent of the visible SHELL binding and eval.
    pub fn set_original_shell(&mut self, value: &[u8]) {
        self.original_shell = Some(value.into());
    }
    pub fn original_shell(&self) -> Option<&[u8]> {
        self.original_shell.as_deref()
    }

    pub fn enable_variable_names(&mut self) -> Result<(), ExpandError> {
        self.define_literal(b".VARIABLES", b"", Origin::Default)?;
        let bindings = Arc::make_mut(&mut self.variables);
        bindings
            .get_mut(b".VARIABLES")
            .expect("builtin binding")
            .scope
            .variable_names = true;
        bindings.names_cache = OnceLock::new();
        Ok(())
    }

    fn variable_names(&self, refresh: bool) -> Result<Vec<u8>, ExpandError> {
        let bindings = if self.scoped {
            self.layers
                .iter()
                .find(|layer| layer.global)
                .map_or(&self.variables, |layer| &layer.variables)
        } else {
            &self.variables
        };
        let cache = bindings.names_cache.get_or_init(Arc::default);
        let mut state = cache.0.lock().unwrap_or_else(|error| error.into_inner());
        if refresh && state.generation != bindings.generation {
            let mut value = Vec::new();
            for name in bindings
                .order
                .names()
                .filter(|name| bindings.values.contains_key(*name))
            {
                if !value.is_empty() {
                    push(&mut value, b" ")?;
                }
                push(&mut value, name)?;
            }
            state.value = value;
            state.generation = bindings.generation;
        }
        Ok(state.value.clone())
    }

    /// GNU normalizes flags in the current file set. An explicit binding in
    /// that file retains its priority; global and inherited sets stay outside
    /// this comparison. Preserve metadata when materializing that local value.
    pub(crate) fn define_runtime_metadata(
        &mut self,
        name: &[u8],
        value: &[u8],
        origin: Origin,
    ) -> Result<(), ExpandError> {
        if self.file_locals
            && !self.variables.contains_key(name)
            && let Some(old) = self
                .layers
                .iter()
                .filter(|layer| !layer.global && !layer.inherited && !layer.pattern)
                .find_map(|layer| layer.variables.get(name))
        {
            if old.origin > origin {
                return Ok(());
            }
            let old = old.clone();
            Arc::make_mut(&mut self.variables).insert(name, old);
        }
        self.define(name, value, Assignment::Recursive, origin)
    }

    /// GNU's legacy switch variable excludes command overrides and retains
    /// recursive make-dollar quoting. File/CLI definitions keep their priority.
    pub fn update_mflags(
        &mut self,
        flags: &[u8],
        environment_override: bool,
    ) -> Result<(), ExpandError> {
        let value = if let Some(complex) = flags.strip_prefix(b" ") {
            complex.to_vec()
        } else if flags.is_empty() {
            Vec::new()
        } else {
            [b"-".as_slice(), flags].concat()
        };
        self.define_runtime_metadata(
            b"MFLAGS",
            &value,
            if environment_override {
                Origin::EnvironmentOverride
            } else {
                Origin::Environment
            },
        )
    }

    /// Preserve startup table occupancy independently of metadata surfaces that
    /// are not implemented yet. Reservations never define or advertise a value.
    pub fn reserve_environment_layout(&mut self, names: &[&[u8]], exported: bool) {
        let bindings = Arc::make_mut(&mut self.variables);
        for name in names {
            bindings.order.insert(name);
            if exported && !bindings.reserved_exports.iter().any(|old| old == name) {
                bindings.reserved_exports.push(name.to_vec());
            }
        }
    }

    pub(crate) fn set_scoped_export_policy(&self, name: &[u8], export: bool) -> bool {
        let Some((index, variable)) = self.lookup_from(name, 0) else {
            return false;
        };
        let global = if index == 0 {
            !self.scoped
        } else {
            self.layers[index - 1].global
        };
        if global {
            return false;
        }
        variable.scope.export.set(Some(export));
        true
    }

    /// Set the actual invocation directory for `abspath`, independently of the
    /// overridable CURDIR variable. This performs no filesystem access.
    pub fn set_working_directory(&mut self, directory: &[u8]) {
        self.working_directory = Some(Arc::from(directory));
    }

    pub fn get(&self, name: &[u8]) -> Option<&Variable> {
        self.lookup_from(name, 0).map(|(_, value)| value)
    }

    pub fn remove(&mut self, name: &[u8]) -> Option<Variable> {
        Arc::make_mut(&mut self.variables).remove(name)
    }

    /// Unlike internal removal, an undefine directive observes origin priority
    /// and promotes an environment binding when -e makes it prevail.
    pub fn undefine(&mut self, name: &[u8], origin: Origin) -> bool {
        self.promote_environment(name, origin);
        if self
            .get(name)
            .is_some_and(|variable| variable.origin > origin)
        {
            return false;
        }
        self.remove(name).is_some()
    }

    pub fn definitions(&self) -> impl Iterator<Item = (&[u8], &Variable)> {
        std::iter::once((0, &self.variables))
            .chain(
                self.layers
                    .iter()
                    .enumerate()
                    .map(|(index, layer)| (index + 1, &layer.variables)),
            )
            .flat_map(move |(index, variables)| {
                variables.iter().filter_map(move |(name, value)| {
                    self.lookup_from(name, 0)
                        .is_some_and(|(selected, _)| selected == index)
                        .then_some((name.as_ref(), value))
                })
            })
    }

    fn lookup_from(&self, name: &[u8], start: usize) -> Option<(usize, &Variable)> {
        if start == 0
            && let Some(value) = self.variables.get(name)
        {
            return Some((0, value));
        }
        self.layers
            .iter()
            .enumerate()
            .skip(start.saturating_sub(1))
            .find_map(|(index, layer)| {
                let value = layer.variables.get(name)?;
                (!layer.inherited || !value.scope.private).then_some((index + 1, value))
            })
    }

    /// GNU promotes imported environment origins on a conflicting global
    /// assignment, not when importing the environment or reading a variable.
    pub fn set_environment_override(&mut self, enabled: bool) {
        self.environment_override = enabled;
    }

    fn promote_environment(&mut self, name: &[u8], origin: Origin) {
        if self.environment_override
            && origin != Origin::Environment
            && self
                .variables
                .get(name)
                .is_some_and(|variable| variable.origin == Origin::Environment)
        {
            Arc::make_mut(&mut self.variables)
                .get_mut(name)
                .unwrap()
                .origin = Origin::EnvironmentOverride;
        }
    }

    /// Qualifiers can change metadata without replacing a binding's value.
    pub fn set_private(&mut self, name: &[u8], private: bool) {
        if let Some(variable) = Arc::make_mut(&mut self.variables).get_mut(name) {
            variable.scope.private = private;
        }
    }

    /// Keep explicit global directives coherent with metadata aliased by a
    /// suppressed target conditional assignment.
    pub fn set_export_policy(&mut self, name: &[u8], export: Option<bool>) {
        if let Some(variable) = Arc::make_mut(&mut self.variables).get_mut(name) {
            variable.scope.export.set(export);
            variable.scope.export_default_reset = export.is_none();
            if export.is_none() {
                variable.scope.export_if_set = false;
            }
        }
    }

    /// Startup policy for MAKEFILES: export once assigned, unless a later
    /// explicit policy changes it. Undefine destroys this binding's policy.
    pub fn set_export_if_set(&mut self, name: &[u8]) {
        if let Some(variable) = Arc::make_mut(&mut self.variables).get_mut(name) {
            variable.scope.export.set(None);
            variable.scope.export_if_set = true;
        }
    }

    /// Enter a target with its matching scopes in low-to-high precedence order.
    /// Ancestor layers stay separate: a private child definition can expose an
    /// earlier ancestor again when entering the grandchild.
    pub fn with_target_scopes(&self, scopes: &[&TargetScope]) -> Self {
        let mut layers = Vec::with_capacity(scopes.len() + self.layers.len() + 1);
        for scope in scopes.iter().rev() {
            if !scope.variables.is_empty() {
                layers.push(VariableLayer {
                    file_locals: false,
                    variables: Arc::clone(&scope.variables),
                    inherited: false,
                    global: false,
                    pattern: scope.pattern,
                });
            }
        }
        if !self.variables.is_empty() {
            layers.push(VariableLayer {
                file_locals: self.file_locals,
                variables: Arc::clone(&self.variables),
                inherited: true,
                global: !self.scoped,
                pattern: false,
            });
        }
        layers.extend(self.layers.iter().map(|layer| VariableLayer {
            file_locals: layer.file_locals,
            variables: Arc::clone(&layer.variables),
            inherited: true,
            global: layer.global,
            pattern: layer.pattern,
        }));
        Self {
            // Empty local maps can share storage. Definitions use Arc::make_mut
            // so writes in one target still leave parents and siblings intact.
            variables: if self.file_locals && self.variables.is_empty() {
                Arc::clone(&self.variables)
            } else {
                empty_bindings(32)
            },
            layers,
            working_directory: self.working_directory.clone(),
            original_shell: self.original_shell.clone(),
            environment_override: false,
            scoped: true,
            file_locals: true,
        }
    }

    /// A file rename can merge explicit bindings after its pattern scope was
    /// activated. Replace only that explicit layer, without replaying effects.
    pub fn refresh_target_scope(&mut self, scope: &TargetScope) {
        if let Some(layer) = self
            .layers
            .iter_mut()
            .find(|layer| !layer.inherited && !layer.global && !layer.pattern)
        {
            layer.variables = Arc::clone(&scope.variables);
        } else if !scope.variables.is_empty() {
            self.layers.insert(
                0,
                VariableLayer {
                    file_locals: false,
                    variables: Arc::clone(&scope.variables),
                    inherited: false,
                    global: false,
                    pattern: false,
                },
            );
        }
    }

    /// Rebase a saved traversal context after an activation changed global
    /// metadata. Explicit scope snapshots remain independent of global changes.
    pub fn refresh_globals(&mut self, globals: &VariableStore) {
        if !self.scoped {
            self.variables = Arc::clone(&globals.variables);
            return;
        }
        let mut found = false;
        for layer in &mut self.layers {
            if layer.global {
                layer.variables = Arc::clone(&globals.variables);
                found = true;
            }
        }
        if !found && !globals.variables.is_empty() {
            self.layers.push(VariableLayer {
                file_locals: false,
                variables: Arc::clone(&globals.variables),
                inherited: true,
                global: true,
                pattern: false,
            });
        }
    }

    /// Environment binding selection treats pattern layers as inherited even
    /// on the matching target. References inside a selected value still expand
    /// in the complete target context; use expand_environment_variable_with.
    pub fn for_environment(&self) -> Self {
        let mut environment = self.clone();
        for layer in &mut environment.layers {
            if layer.pattern {
                layer.inherited = true;
            }
        }
        environment
    }

    /// Environment construction has its own ordered table. Non-global scopes
    /// reserve a name even when it is not exported, while globals only reserve
    /// eligible exports. Keep this independent from lookup and .VARIABLES order.
    pub fn environment_order(
        &self,
        mut exported: impl FnMut(&[u8], &Variable) -> bool,
    ) -> Vec<Vec<u8>> {
        use crate::variable_order::NameOrder;
        let mut sources = vec![(&*self.variables, !self.scoped, self.file_locals, false)];
        sources.extend(self.layers.iter().map(|layer| {
            (
                &*layer.variables,
                layer.global,
                layer.file_locals,
                layer.pattern,
            )
        }));
        let mut table = NameOrder::new(1024, false);
        let mut index = 0;
        while index < sources.len() {
            let (bindings, global, locals, _) = sources[index];
            let paired = locals
                && sources
                    .get(index + 1)
                    .is_some_and(|(_, global, locals, pattern)| !global && !locals && !pattern);
            let merged = paired.then(|| {
                let mut order = sources[index + 1].0.order.clone();
                bindings.order.replay_into(&mut order);
                order
            });
            let order = merged.as_ref().unwrap_or(&bindings.order);
            for name in order.names() {
                if global
                    && bindings
                        .reserved_exports
                        .iter()
                        .any(|reserved| reserved == name)
                    && self.lookup_from(name, 0).is_none()
                {
                    table.insert(name);
                    continue;
                }
                let Some((selected, variable)) = self.lookup_from(name, 0) else {
                    continue;
                };
                if (selected == index || (paired && selected == index + 1))
                    && (!global || exported(name, variable))
                {
                    table.insert(name);
                }
            }
            index += 1 + usize::from(paired);
        }
        table.names().map(<[u8]>::to_vec).collect()
    }

    /// An unspecified local policy inherits the closest visible explicit one,
    /// even when a different binding supplies the variable's value.
    pub fn export_policy(&self, name: &[u8]) -> Option<bool> {
        let mut start = 0;
        while let Some((index, value)) = self.lookup_from(name, start) {
            if let Some(policy) = value.scope.export.get().or_else(|| {
                value.scope.export_if_set.then(|| {
                    self.get(name)
                        .is_some_and(|selected| selected.origin != Origin::Default)
                })
            }) {
                return Some(policy);
            }
            start = index + 1;
        }
        None
    }

    /// Resolve inherited policy during actual child-environment preparation.
    /// GNU remembers the answer on that binding, even if an outer declaration's
    /// metadata changes when another target initializes later.
    pub fn resolve_export_policy(&self, name: &[u8]) -> Option<bool> {
        let policy = self.export_policy(name);
        self.get(name).map_or(policy, |variable| {
            if variable.scope.export_if_set && variable.scope.export.get().is_none() {
                policy
            } else {
                variable.scope.export.resolve(policy)
            }
        })
    }

    /// Define an already-expanded name. Conditional assignment skips an existing
    /// definition, but immediate RHS evaluation precedes origin precedence checks.
    /// This pure convenience form rejects output; runtime callers use define_with.
    pub fn define(
        &mut self,
        name: &[u8],
        value: &[u8],
        assignment: Assignment,
        origin: Origin,
    ) -> Result<(), ExpandError> {
        self.define_with(
            name,
            value,
            assignment,
            origin,
            None,
            ExpansionContext::pure(),
        )
    }

    /// Define a file binding, keeping its source separate from the caller's
    /// immediate evaluation context (notably a multiline define's endef).
    pub fn define_at(
        &mut self,
        name: &[u8],
        value: &[u8],
        assignment: Assignment,
        origin: Origin,
        location: &Location,
    ) -> Result<(), ExpandError> {
        self.define_with(
            name,
            value,
            assignment,
            origin,
            Some(location),
            ExpansionContext::pure(),
        )
    }

    /// Define with explicit definition provenance and immediate evaluation effects.
    pub fn define_with(
        &mut self,
        name: &[u8],
        value: &[u8],
        assignment: Assignment,
        origin: Origin,
        definition: Option<&Location>,
        context: ExpansionContext<'_>,
    ) -> Result<(), ExpandError> {
        self.define_with_using(
            name,
            value,
            assignment,
            origin,
            definition,
            context,
            |store, bytes, shell, context| {
                if shell {
                    store.shell_with(bytes, context)
                } else {
                    store.expand_mut_with(bytes, context)
                }
            },
        )
    }

    /// Appending to GNU's special global flag variable discards its encoded
    /// command-variable suffix; canonicalization reconstructs that suffix.
    pub(crate) fn strip_makeflags_assignments(&mut self, origin: Origin) {
        if let Some(variable) = self.variables.get(b"MAKEFLAGS".as_slice()) {
            let priority = if self.environment_override && variable.origin == Origin::Environment {
                Origin::EnvironmentOverride
            } else {
                variable.origin
            };
            if priority > origin {
                return;
            }
            if let Some(boundary) = variable.value.windows(4).position(|part| part == b" -- ") {
                Arc::make_mut(&mut self.variables)
                    .get_mut(b"MAKEFLAGS".as_slice())
                    .expect("existing binding")
                    .value
                    .truncate(boundary);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn define_with_using(
        &mut self,
        name: &[u8],
        value: &[u8],
        assignment: Assignment,
        origin: Origin,
        definition: Option<&Location>,
        mut context: ExpansionContext<'_>,
        mut expand: impl FnMut(
            &mut VariableStore,
            &[u8],
            bool,
            ExpansionContext<'_>,
        ) -> Result<Vec<u8>, ExpandError>,
    ) -> Result<(), ExpandError> {
        if name.is_empty() {
            return Err(ExpandError::EmptyVariableName);
        }
        if matches!(assignment, Assignment::Append | Assignment::Conditional)
            && self
                .variables
                .get(name)
                .is_some_and(|variable| variable.scope.variable_names)
        {
            let value = self.variable_names(true)?;
            Arc::make_mut(&mut self.variables)
                .get_mut(name)
                .expect("special binding")
                .value = value;
        }
        if assignment == Assignment::Conditional && self.get(name).is_some() {
            return Ok(());
        }
        // An empty append does not reach GNU's definition replacement path, so
        // it neither changes the value nor promotes an environment origin.
        if assignment == Assignment::Append && value.is_empty() && self.variables.contains_key(name)
        {
            return Ok(());
        }
        let promote_environment = self.environment_override
            && self
                .variables
                .get(name)
                .is_some_and(|old| old.origin == Origin::Environment)
            && origin != Origin::Environment;
        if let Some(old) = self.variables.get(name) {
            let old_origin = if promote_environment {
                Origin::EnvironmentOverride
            } else {
                old.origin
            };
            if old_origin > origin {
                // GNU evaluates immediate assignments even when their result is
                // discarded. An append is immediate only for a simple binding
                // already present in this store; recursive appends stay deferred.
                if matches!(
                    assignment,
                    Assignment::Simple | Assignment::ImmediateRecursive | Assignment::Shell
                ) || (assignment == Assignment::Append && old.flavor == Flavor::Simple)
                {
                    expand(
                        self,
                        value,
                        assignment == Assignment::Shell,
                        context.reborrow(),
                    )?;
                }
                if promote_environment {
                    Arc::make_mut(&mut self.variables)
                        .get_mut(name)
                        .unwrap()
                        .origin = Origin::EnvironmentOverride;
                }
                return Ok(());
            }
        }
        let mut variable = match assignment {
            Assignment::Shell => Variable {
                value: expand(self, value, true, context.reborrow())?,
                flavor: Flavor::Recursive,
                origin,
                definition: None,
                scope: ScopeMetadata::default(),
            },
            Assignment::Simple => Variable {
                value: expand(
                    self,
                    value,
                    assignment == Assignment::Shell,
                    context.reborrow(),
                )?,
                flavor: Flavor::Simple,
                origin,
                definition: None,
                scope: ScopeMetadata::default(),
            },
            Assignment::ImmediateRecursive => {
                let expanded = expand(
                    self,
                    value,
                    assignment == Assignment::Shell,
                    context.reborrow(),
                )?;
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
                    definition: None,
                    scope: ScopeMetadata::default(),
                }
            }
            Assignment::Append => {
                if let Some(old) = self.variables.get(name).cloned() {
                    let extra = match old.flavor {
                        Flavor::Recursive => value.to_vec(),
                        Flavor::Simple => expand(
                            self,
                            value,
                            assignment == Assignment::Shell,
                            context.reborrow(),
                        )?,
                    };
                    // Empty appends do not replace the binding, its origin,
                    // or its definition location, even with override.
                    if extra.is_empty() {
                        return Ok(());
                    }
                    let mut combined = old.value.clone();
                    if !combined.is_empty() && !extra.is_empty() {
                        push(&mut combined, b" ")?;
                    }
                    push(&mut combined, &extra)?;
                    Variable {
                        value: combined,
                        flavor: old.flavor,
                        origin,
                        definition: None,
                        scope: ScopeMetadata::default(),
                    }
                } else {
                    Variable {
                        value: value.to_vec(),
                        flavor: Flavor::Recursive,
                        origin,
                        definition: None,
                        scope: ScopeMetadata::default(),
                    }
                }
            }
            Assignment::Recursive | Assignment::Conditional => Variable {
                value: value.to_vec(),
                flavor: Flavor::Recursive,
                origin,
                definition: None,
                scope: ScopeMetadata::default(),
            },
        };
        if variable.value.len() > MAX_OUTPUT {
            return Err(ExpandError::ExpansionLimit);
        }
        variable.definition = definition.cloned().map(Arc::new);
        variable.scope = self
            .variables
            .get(name)
            .map_or_else(ScopeMetadata::default, |old| old.scope.clone());
        if assignment != Assignment::Append {
            variable.scope.append_outer = false;
        }
        Arc::make_mut(&mut self.variables).insert(name, variable);
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
        self.promote_environment(name, origin);
        if self.get(name).is_some_and(|old| old.origin > origin) {
            return Ok(());
        }
        if value.len() > MAX_OUTPUT {
            return Err(ExpandError::ExpansionLimit);
        }
        let scope = self
            .variables
            .get(name)
            .map_or_else(ScopeMetadata::default, |old| old.scope.clone());
        Arc::make_mut(&mut self.variables).insert(
            name,
            Variable {
                value: value.to_vec(),
                flavor: Flavor::Simple,
                origin,
                definition: None,
                scope,
            },
        );
        Ok(())
    }

    /// Append already-expanded metadata bytes, keeping the previous flavor.
    /// Unlike ordinary +=, a simple binding must not evaluate these bytes again.
    pub fn append_literal_at(
        &mut self,
        name: &[u8],
        value: &[u8],
        origin: Origin,
        location: &Location,
    ) -> Result<(), ExpandError> {
        if name.is_empty() {
            return Err(ExpandError::EmptyVariableName);
        }
        self.promote_environment(name, origin);
        if self.get(name).is_some_and(|old| old.origin > origin) {
            return Ok(());
        }
        let (mut combined, flavor) = self.get(name).map_or_else(
            || (Vec::new(), Flavor::Simple),
            |old| (old.value.clone(), old.flavor),
        );
        if !combined.is_empty() {
            push(&mut combined, b" ")?;
        }
        push(&mut combined, value)?;
        let scope = self
            .variables
            .get(name)
            .map_or_else(ScopeMetadata::default, |old| old.scope.clone());
        Arc::make_mut(&mut self.variables).insert(
            name,
            Variable {
                value: combined,
                flavor,
                origin,
                definition: Some(Arc::new(location.clone())),
                scope,
            },
        );
        Ok(())
    }

    /// Evaluate in a pure/test context, rejecting output functions.
    /// Runtime callers must use `expand_with` to preserve observable emissions.
    pub fn expand(&self, input: &[u8]) -> Result<Vec<u8>, ExpandError> {
        self.expand_with(input, ExpansionContext::pure())
    }

    pub fn expand_with(
        &self,
        input: &[u8],
        context: ExpansionContext<'_>,
    ) -> Result<Vec<u8>, ExpandError> {
        self.expansion(context).expand(input)
    }

    pub fn expand_mut_with(
        &mut self,
        input: &[u8],
        context: ExpansionContext<'_>,
    ) -> Result<Vec<u8>, ExpandError> {
        Expansion {
            store: StoreAccess::Write(self),
            bindings: Vec::new(),
            active: Vec::new(),
            depth: 0,
            context,
            binding_context: None,
        }
        .expand(input)
    }

    pub fn shell_with(
        &mut self,
        input: &[u8],
        context: ExpansionContext<'_>,
    ) -> Result<Vec<u8>, ExpandError> {
        let mut expansion = Expansion {
            store: StoreAccess::Write(self),
            bindings: Vec::new(),
            active: Vec::new(),
            depth: 0,
            context,
            binding_context: None,
        };
        if !expansion.context.has_shell() {
            return Err(ExpandError::Unsupported(b"!=".to_vec()));
        }
        let command = expansion.expand(input)?;
        expansion.shell(&command)
    }

    /// Invocation phases have an unlocated outer expansion, as when GNU
    /// decodes MAKEFLAGS. User output/errors retain that outer context while
    /// nested validation errors still carry their binding provenance.
    pub fn expand_at_invocation_with(
        &mut self,
        input: &[u8],
        context: ExpansionContext<'_>,
    ) -> Result<Vec<u8>, ExpandError> {
        let mut expansion = Expansion {
            store: StoreAccess::Write(self),
            bindings: Vec::new(),
            active: Vec::new(),
            depth: 0,
            context,
            binding_context: None,
        };
        expansion.binding_context = Some(None);
        expansion.expand(input)
    }

    /// Pure/test convenience wrapper that rejects output functions.
    pub fn expand_variable(&self, name: &[u8]) -> Result<Vec<u8>, ExpandError> {
        self.expand_variable_with(name, ExpansionContext::pure())
    }

    /// Expand a named binding with provenance, without encoding arbitrary name
    /// bytes as a variable-reference expression.
    pub fn expand_variable_with(
        &self,
        name: &[u8],
        context: ExpansionContext<'_>,
    ) -> Result<Vec<u8>, ExpandError> {
        self.expansion(context).variable(name)
    }

    pub fn expand_environment_variable_with(
        &self,
        name: &[u8],
        context: ExpansionContext<'_>,
    ) -> Result<Vec<u8>, ExpandError> {
        let environment = self.for_environment();
        let Some((layer, variable)) = environment.lookup_from(name, 0) else {
            return Ok(Vec::new());
        };
        let mut expansion = self.expansion(context);
        if variable.flavor == Flavor::Recursive && variable.scope.append_outer {
            // GNU constructs an inherited append by searching the entire file
            // context again. Environment selection may have skipped a private
            // pattern binding that this second, ordinary lookup can see.
            let Some((first_layer, first)) = self.lookup_from(name, 0) else {
                return Ok(Vec::new());
            };
            expansion.active.push((name.to_vec(), Some(layer)));
            expansion.binding_context = Some(variable.definition.clone());
            expansion.expand_layer(name, first, Some(first_layer), 0)
        } else {
            expansion.selected_variable(name, variable, Some(layer))
        }
    }

    pub fn expand_environment_variable_mut_with(
        &mut self,
        name: &[u8],
        context: ExpansionContext<'_>,
    ) -> Result<Vec<u8>, ExpandError> {
        let environment = self.for_environment();
        let Some((layer, variable)) = environment.lookup_from(name, 0) else {
            return Ok(Vec::new());
        };
        let mut expansion = Expansion {
            store: StoreAccess::Write(self),
            bindings: Vec::new(),
            active: Vec::new(),
            depth: 0,
            context,
            binding_context: None,
        };
        if variable.flavor == Flavor::Recursive && variable.scope.append_outer {
            // GNU constructs an inherited append by searching the entire file
            // context again. Environment selection may have skipped a private
            // pattern binding that this second, ordinary lookup can see.
            let Some((first_layer, first)) = expansion.store.lookup_from(name, 0) else {
                return Ok(Vec::new());
            };
            let first = first.clone();
            expansion.active.push((name.to_vec(), Some(layer)));
            expansion.binding_context = Some(variable.definition.clone());
            expansion.expand_layer(name, &first, Some(first_layer), 0)
        } else {
            expansion.selected_variable(name, variable, Some(layer))
        }
    }

    fn expansion<'a, 'b>(&'a self, context: ExpansionContext<'b>) -> Expansion<'a, 'b> {
        Expansion {
            store: StoreAccess::Read(self),
            bindings: Vec::new(),
            active: Vec::new(),
            depth: 0,
            context,
            binding_context: None,
        }
    }
}

impl PatternVariable {
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_with(
        globals: &VariableStore,
        name: &[u8],
        value: &[u8],
        assignment: Assignment,
        mut origin: Origin,
        private: bool,
        export: Option<bool>,
        definition: Option<&Location>,
        context: ExpansionContext<'_>,
    ) -> Result<Self, ExpandError> {
        if name.is_empty() {
            return Err(ExpandError::EmptyVariableName);
        }
        let mut value = if assignment == Assignment::Simple {
            globals.expand_with(value, context)?
        } else {
            value.to_vec()
        };
        if globals
            .get(name)
            .is_some_and(|variable| variable.scope.variable_names)
        {
            globals.variable_names(true)?;
        }
        if origin != Origin::Override
            && let Some(global) = globals.get(name)
            && matches!(
                global.origin,
                Origin::CommandLine | Origin::EnvironmentOverride
            )
        {
            value = global.value.clone();
            origin = global.origin;
        }
        Ok(Self {
            name: name.to_vec(),
            value,
            assignment,
            origin,
            private,
            export,
            definition: definition.cloned().map(Arc::new),
        })
    }

    pub fn name(&self) -> &[u8] {
        &self.name
    }
}

impl TargetScope {
    /// File-record merging retains bindings already present on the destination.
    pub fn merge_missing(&mut self, other: &Self) {
        let variables = Arc::make_mut(&mut self.variables);
        for name in other.variables.order.names() {
            if !variables.contains_key(name) {
                variables.insert(name, other.variables.get(name).unwrap().clone());
            }
        }
    }

    /// The read context sees this target's earlier declarations over the current
    /// global definitions. Ancestors do not exist until dependency traversal.
    pub fn context(&self, globals: &VariableStore) -> VariableStore {
        let mut context = globals.with_target_scopes(&[]);
        context.variables = Arc::clone(&self.variables);
        context.file_locals = false;
        context
    }

    pub fn get(&self, name: &[u8]) -> Option<&Variable> {
        self.variables.get(name)
    }

    /// Apply a prepared pattern declaration to this target's single accumulated
    /// pattern layer. Return true when a suppressed conditional changed global
    /// metadata, allowing the owner to reconcile its export-name indexes.
    pub fn apply_pattern_with(
        &mut self,
        globals: &mut VariableStore,
        prepared: &PatternVariable,
        context: ExpansionContext<'_>,
    ) -> Result<bool, ExpandError> {
        if !self.pattern && self.variables.is_empty() {
            self.variables = empty_bindings(16);
        }
        self.pattern = true;
        let mut scope = self.context(globals);
        // Pattern initialization shares the global lookup boundary. A private
        // global is visible here even though recipes cannot inherit its value.
        for layer in &mut scope.layers {
            if layer.global {
                layer.inherited = false;
            }
        }
        let name = prepared.name.as_slice();
        let aliases_global = prepared.assignment == Assignment::Conditional
            && !self.variables.contains_key(name)
            && globals.get(name).is_some();
        if prepared.assignment == Assignment::Simple {
            // This value was expanded when read. Install literal bytes without
            // evaluating them again or comparing against outer-scope origins.
            if scope
                .variables
                .get(name)
                .is_none_or(|old| old.origin <= prepared.origin)
            {
                let metadata = scope
                    .variables
                    .get(name)
                    .map_or_else(ScopeMetadata::default, |old| old.scope.clone());
                Arc::make_mut(&mut scope.variables).insert(
                    name,
                    Variable {
                        value: prepared.value.clone(),
                        flavor: Flavor::Simple,
                        origin: prepared.origin,
                        definition: prepared.definition.clone(),
                        scope: metadata,
                    },
                );
            }
        } else {
            let conditional_exists =
                prepared.assignment == Assignment::Conditional && scope.get(name).is_some();
            let append_outer =
                prepared.assignment == Assignment::Append && !self.variables.contains_key(name);
            scope.define_with(
                name,
                &prepared.value,
                prepared.assignment,
                prepared.origin,
                prepared.definition.as_deref(),
                context,
            )?;
            if !conditional_exists
                && let Some(variable) = Arc::make_mut(&mut scope.variables).get_mut(name)
            {
                variable.scope.append_outer = prepared.assignment == Assignment::Append
                    && (variable.scope.append_outer || append_outer);
            }
        }
        let variable = if aliases_global {
            Arc::make_mut(&mut globals.variables).get_mut(name)
        } else {
            Arc::make_mut(&mut scope.variables).get_mut(name)
        };
        if let Some(variable) = variable {
            variable.scope.private = prepared.private;
            variable.scope.export.set(prepared.export);
            variable.scope.export_default_reset = aliases_global && prepared.export.is_none();
        }
        self.variables = scope.variables;
        Ok(aliases_global)
    }

    /// Apply an explicit target declaration. Origin precedence applies inside
    /// this scope; global override values do not outrank a target definition.
    #[allow(clippy::too_many_arguments)]
    pub fn define_with(
        &mut self,
        globals: &mut VariableStore,
        name: &[u8],
        value: &[u8],
        assignment: Assignment,
        origin: Origin,
        private: bool,
        export: Option<bool>,
        definition: Option<&Location>,
        context: ExpansionContext<'_>,
    ) -> Result<(), ExpandError> {
        if name.is_empty() {
            return Err(ExpandError::EmptyVariableName);
        }
        let mut scope = self.context(globals);
        if assignment == Assignment::Conditional
            && scope
                .get(name)
                .is_some_and(|variable| variable.scope.variable_names)
        {
            globals.variable_names(true)?;
        }
        if assignment == Assignment::Conditional && scope.get(name).is_some() {
            // GNU's suppressed conditional returns the existing binding itself.
            // Qualifiers can consequently mutate even global binding metadata.
            let variable = if self.variables.contains_key(name) {
                Arc::make_mut(&mut self.variables).get_mut(name)
            } else {
                Arc::make_mut(&mut globals.variables).get_mut(name)
            };
            if let Some(variable) = variable {
                variable.scope.private = private;
                if export.is_some() {
                    variable.scope.export.set(export);
                }
            }
            return Ok(());
        }
        let append_outer = assignment == Assignment::Append && !self.variables.contains_key(name);
        scope.define_with(name, value, assignment, origin, definition, context)?;
        let variable = Arc::make_mut(&mut scope.variables).get_mut(name).unwrap();
        variable.scope.append_outer =
            assignment == Assignment::Append && (variable.scope.append_outer || append_outer);
        if globals
            .get(name)
            .is_some_and(|variable| variable.scope.variable_names)
        {
            globals.variable_names(true)?;
        }
        if let Some(global) = globals.get(name)
            && matches!(
                global.origin,
                Origin::CommandLine | Origin::EnvironmentOverride
            )
            && global.origin > variable.origin
        {
            // Preserve a snapshot: a later global override must not change the
            // command-line/environment binding selected for this target.
            let policy = variable.scope.export.get();
            *variable = global.clone();
            if policy.is_some() {
                variable.scope.export.set(policy);
            }
        }
        variable.scope.private = private;
        if export.is_some() {
            variable.scope.export.set(export);
        }
        self.variables = scope.variables;
        Ok(())
    }
}

enum StoreAccess<'a> {
    Read(&'a VariableStore),
    Write(&'a mut VariableStore),
    Database(&'a mut crate::parser::Database),
    Scoped {
        database: &'a mut crate::parser::Database,
        variables: &'a mut VariableStore,
    },
}

impl std::ops::Deref for StoreAccess<'_> {
    type Target = VariableStore;
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Read(store) => store,
            Self::Write(store) => store,
            Self::Database(db) => &db.variables,
            Self::Scoped { variables, .. } => variables,
        }
    }
}

impl StoreAccess<'_> {
    fn begin_filename_session(&mut self, needed: bool) -> bool {
        match self {
            Self::Database(db) | Self::Scoped { database: db, .. } => {
                db.begin_filename_session(needed)
            }
            _ => false,
        }
    }

    fn end_filename_session(&mut self, active: bool) {
        match self {
            Self::Database(db) | Self::Scoped { database: db, .. } => {
                db.end_filename_session(active)
            }
            _ => {}
        }
    }

    fn write_filename(&mut self, name: &[u8], offset: usize) -> Vec<u8> {
        match self {
            Self::Database(db) | Self::Scoped { database: db, .. } => {
                db.write_filename(name, offset)
            }
            _ => name.to_vec(),
        }
    }

    fn filename_suffix(&self, fallback: &[u8]) -> Vec<u8> {
        match self {
            Self::Database(db) | Self::Scoped { database: db, .. } => db.filename_suffix(fallback),
            _ => fallback.get(1..).unwrap_or_default().to_vec(),
        }
    }

    fn writable(&mut self) -> Option<&mut VariableStore> {
        match self {
            Self::Read(_) => None,
            Self::Write(store) => Some(store),
            Self::Database(db) => Some(&mut db.variables),
            Self::Scoped { variables, .. } => Some(variables),
        }
    }
}

pub(crate) fn expand_scoped_database(
    db: &mut crate::parser::Database,
    variables: &mut VariableStore,
    input: &[u8],
    context: ExpansionContext<'_>,
) -> Result<Vec<u8>, ExpandError> {
    let bindings = db.evaluation_bindings.clone();
    Expansion {
        store: StoreAccess::Scoped {
            database: db,
            variables,
        },
        bindings,
        active: Vec::new(),
        depth: 0,
        context,
        binding_context: None,
    }
    .expand(input)
}

pub(crate) fn expand_environment_database(
    db: &mut crate::parser::Database,
    variables: &mut VariableStore,
    name: &[u8],
    context: ExpansionContext<'_>,
) -> Result<Vec<u8>, ExpandError> {
    let environment = variables.for_environment();
    let Some((layer, variable)) = environment.lookup_from(name, 0) else {
        return Ok(Vec::new());
    };
    let bindings = db.evaluation_bindings.clone();
    let mut expansion = Expansion {
        store: StoreAccess::Scoped {
            database: db,
            variables,
        },
        bindings,
        active: Vec::new(),
        depth: 0,
        context,
        binding_context: None,
    };
    if variable.flavor == Flavor::Recursive && variable.scope.append_outer {
        let Some((first_layer, first)) = expansion.store.lookup_from(name, 0) else {
            return Ok(Vec::new());
        };
        let first = first.clone();
        expansion.active.push((name.to_vec(), Some(layer)));
        expansion.binding_context = Some(variable.definition.clone());
        expansion.expand_layer(name, &first, Some(first_layer), 0)
    } else {
        expansion.selected_variable(name, variable, Some(layer))
    }
}

pub(crate) fn expand_database(
    db: &mut crate::parser::Database,
    input: &[u8],
    context: ExpansionContext<'_>,
) -> Result<Vec<u8>, ExpandError> {
    let bindings = db.evaluation_bindings.clone();
    Expansion {
        store: StoreAccess::Database(db),
        bindings,
        active: Vec::new(),
        depth: 0,
        context,
        binding_context: None,
    }
    .expand(input)
}

pub(crate) fn shell_database(
    db: &mut crate::parser::Database,
    input: &[u8],
    context: ExpansionContext<'_>,
) -> Result<Vec<u8>, ExpandError> {
    let bindings = db.evaluation_bindings.clone();
    let mut scope = db.evaluation_scope.take();
    if let Some(scope) = &mut scope {
        scope.refresh_globals(&db.variables);
    }
    let result = {
        let store = if let Some(variables) = &mut scope {
            StoreAccess::Scoped {
                database: &mut *db,
                variables,
            }
        } else {
            StoreAccess::Database(&mut *db)
        };
        let mut expansion = Expansion {
            store,
            bindings,
            active: Vec::new(),
            depth: 0,
            context,
            binding_context: None,
        };
        if !expansion.context.has_shell() {
            Err(ExpandError::Unsupported(b"!=".to_vec()))
        } else {
            expansion
                .expand(input)
                .and_then(|command| expansion.shell(&command))
        }
    };
    db.evaluation_scope = scope;
    result
}

struct Expansion<'a, 'b> {
    store: StoreAccess<'a>,
    bindings: Vec<BTreeMap<Vec<u8>, Variable>>,
    active: Vec<(Vec<u8>, Option<usize>)>,
    depth: usize,
    context: ExpansionContext<'b>,
    // Outer None means no binding entered; Some(None) means an unlocated one.
    binding_context: Option<Option<Arc<Location>>>,
}

impl Expansion<'_, '_> {
    fn expand_binding(&mut self, variable: &Variable) -> Result<Vec<u8>, ExpandError> {
        let entered = self.binding_context.is_none();
        if entered {
            self.binding_context = Some(variable.definition.clone());
        }
        let result = self.expand(&variable.value);
        if entered {
            self.binding_context = None;
        }
        result.map_err(|error| error.at(variable.definition.as_ref()))
    }

    fn lookup(&self, name: &[u8]) -> Option<&Variable> {
        self.bindings
            .iter()
            .rev()
            .find_map(|scope| scope.get(name))
            .or_else(|| self.store.get(name))
    }

    fn expand(&mut self, input: &[u8]) -> Result<Vec<u8>, ExpandError> {
        if self.context.is_cancelled() {
            return Err(ExpandError::Cancelled);
        }
        // GNU consumes a C string at each expansion boundary. A NUL returned
        // by a file read does not skip later effects in this same expansion;
        // truncate only after evaluating all remaining source text.
        let input = if input.contains(&0) {
            &input[..input
                .iter()
                .position(|byte| *byte == 0)
                .expect("contains NUL")]
        } else {
            input
        };
        if self.depth >= MAX_DEPTH || input.len() > MAX_OUTPUT {
            return Err(ExpandError::ExpansionLimit);
        }
        self.depth += 1;
        let result = self.expand_inner(input).map(|mut result| {
            if result.contains(&0) {
                let nul = result
                    .iter()
                    .position(|byte| *byte == 0)
                    .expect("contains NUL");
                result.truncate(nul);
            }
            result
        });
        self.depth -= 1;
        if self.context.is_cancelled() {
            Err(ExpandError::Cancelled)
        } else {
            result
        }
    }

    fn expand_inner(&mut self, input: &[u8]) -> Result<Vec<u8>, ExpandError> {
        let mut result = Vec::new();
        let mut cursor = 0;
        while cursor < input.len() {
            if input[cursor] != b'$' {
                let end = input[cursor..]
                    .iter()
                    .position(|byte| *byte == b'$')
                    .map_or(input.len(), |offset| cursor + offset);
                push(&mut result, &input[cursor..end])?;
                cursor = end;
                continue;
            }
            if cursor + 1 == input.len() {
                push(&mut result, b"$")?;
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
        let binding = self.bindings.iter().rev().find_map(|scope| scope.get(name));
        let (variable, layer) = if let Some(variable) = binding {
            (variable, None)
        } else if let Some((layer, variable)) = self.store.lookup_from(name, 0) {
            (variable, Some(layer))
        } else {
            return Ok(Vec::new());
        };
        if variable.scope.variable_names {
            let value = self.store.variable_names(true)?;
            if variable.flavor == Flavor::Simple {
                return Ok(value);
            }
            let mut dynamic = variable.clone();
            dynamic.scope.variable_names = false;
            dynamic.value = value;
            return self.selected_variable(name, &dynamic, layer);
        }
        if variable.flavor == Flavor::Simple {
            // Simple values need neither definition context nor a temporary
            // owned binding. Copy their bytes once, directly from the store.
            return Ok(variable.value.clone());
        }
        let variable = variable.clone();
        self.selected_variable(name, &variable, layer)
    }

    fn selected_variable(
        &mut self,
        name: &[u8],
        variable: &Variable,
        layer: Option<usize>,
    ) -> Result<Vec<u8>, ExpandError> {
        if variable.scope.variable_names {
            let value = self.store.variable_names(true)?;
            if variable.flavor == Flavor::Simple {
                return Ok(value);
            }
            let mut dynamic = variable.clone();
            dynamic.scope.variable_names = false;
            dynamic.value = value;
            return self.selected_variable(name, &dynamic, layer);
        }
        if variable.flavor == Flavor::Simple {
            // Already-expanded values need no temporary definition context.
            return Ok(variable.value.clone());
        }
        if self
            .active
            .iter()
            .any(|(active, active_layer)| active == name && *active_layer == layer)
        {
            return Err(
                ExpandError::RecursiveVariable(name.to_vec()).at(variable.definition.as_ref())
            );
        }
        self.active.push((name.to_vec(), layer));
        let result = self.expand_layer(name, variable, layer, 0);
        self.active.pop();
        result
    }

    fn expand_layer(
        &mut self,
        name: &[u8],
        variable: &Variable,
        layer: Option<usize>,
        depth: usize,
    ) -> Result<Vec<u8>, ExpandError> {
        if variable.scope.variable_names {
            // Outer appends use the stored global value, without performing a
            // second special lookup or refreshing its membership snapshot.
            let mut dynamic = variable.clone();
            dynamic.scope.variable_names = false;
            dynamic.value = self.store.variable_names(false)?;
            return self.expand_layer(name, &dynamic, layer, depth);
        }
        if depth >= MAX_DEPTH {
            return Err(ExpandError::ExpansionLimit);
        }
        let mut value = Vec::new();
        if variable.scope.append_outer
            && let Some(layer) = layer
            && let Some((outer_layer, outer)) = self.store.lookup_from(name, layer + 1)
        {
            let outer = outer.clone();
            value = self.expand_layer(name, &outer, Some(outer_layer), depth + 1)?;
            if !value.is_empty() {
                push(&mut value, b" ")?;
            }
        }
        if variable.flavor == Flavor::Simple {
            push(&mut value, &variable.value)?;
        } else {
            push(&mut value, &self.expand_binding(variable)?)?;
        }
        Ok(value)
    }

    fn reference(&mut self, body: &[u8], delimiter: u8) -> Result<Vec<u8>, ExpandError> {
        if let Some(end) = body.iter().position(gnu_whitespace) {
            let name = &body[..end];
            if let Some((minimum, maximum)) = arity(name) {
                let argument_text = trim_function_start(&body[end..]);
                let args = split_args(argument_text, delimiter, maximum)?;
                if args.len() < minimum && !matches!(name, b"word" | b"wordlist") {
                    return Err(too_few_arguments(name, args.len()));
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
            b"intcmp" => {
                // Both expansions precede either numeric validation.
                let left = self.expand(args[0])?;
                let right = self.expand(args[1])?;
                let left = Decimal::parse(&left, false)?;
                let right = Decimal::parse(&right, true)?;
                let ordering = left.compare(&right);
                if args.len() == 2 {
                    return if ordering == std::cmp::Ordering::Equal {
                        let mut result = Vec::new();
                        if left.negative {
                            push(&mut result, b"-")?;
                        }
                        push(&mut result, left.digits)?;
                        Ok(result)
                    } else {
                        Ok(Vec::new())
                    };
                }
                let index = match ordering {
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
                args.get(index)
                    .map_or_else(|| Ok(Vec::new()), |body| self.expand(body))
            }
            b"if" => {
                let condition = self.expand(trim(args[0]))?;
                let index = if condition.is_empty() { 2 } else { 1 };
                args.get(index)
                    .map_or_else(|| Ok(Vec::new()), |body| self.expand(body))
            }
            b"or" => {
                for arg in args {
                    let value = self.expand(trim(arg))?;
                    if !value.is_empty() {
                        return Ok(value);
                    }
                }
                Ok(Vec::new())
            }
            b"and" => {
                let mut last = Vec::new();
                for arg in args {
                    last = self.expand(trim(arg))?;
                    if last.is_empty() {
                        break;
                    }
                }
                Ok(last)
            }
            b"foreach" => {
                let variable = self.expand(args[0])?;
                let variable = words(&variable).next().unwrap_or_default().to_vec();
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
                let mut value_spans = word_spans(&values);
                let mut bindings = BTreeMap::new();
                for (index, name) in names.iter().enumerate() {
                    let value = match value_spans.next() {
                        Some((start, _)) if index + 1 == names.len() => values[start..].to_vec(),
                        Some((start, end)) => values[start..end].to_vec(),
                        None => Vec::new(),
                    };
                    bindings.insert(name.to_vec(), local(value));
                }
                self.bindings.push(bindings);
                let result = self.expand(args[2]);
                self.bindings.pop();
                result
            }
            b"call" => self.call(args),
            b"shell" => {
                if !self.context.has_shell() || matches!(self.store, StoreAccess::Read(_)) {
                    return Err(ExpandError::Unsupported(name.to_vec()));
                }
                let command = self.expand(args[0])?;
                self.shell(&command)
            }
            b"guile" => Err(ExpandError::Unsupported(name.to_vec())),
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
        // GNU takes the first whitespace-delimited word as the function or
        // variable name, even when a longer variable name is defined.
        let name_text = trim_function_start(&expanded[0]);
        let name = name_text.split(gnu_whitespace).next().unwrap_or_default();
        if let Some((minimum, maximum)) = arity(name) {
            let mut values = expanded[1..].to_vec();
            if values.len() < minimum {
                return Err(too_few_arguments(name, values.len()));
            }
            if let Some(maximum) = maximum {
                values.truncate(maximum);
            }
            // Builtins normally receive either eager values or raw arguments.
            // `call` itself is eager, but lazy builtins must still perform their
            // own evaluation (notably foreach/let bodies with escaped dollars).
            if matches!(
                name,
                b"if" | b"or" | b"and" | b"intcmp" | b"foreach" | b"let" | b"call"
            ) {
                let refs: Vec<&[u8]> = values.iter().map(Vec::as_slice).collect();
                return self.function(name, &refs);
            }
            return self.eager_function(name, &values);
        }
        let Some(variable) = self.lookup(name).cloned() else {
            return Ok(Vec::new());
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
        // Inner calls hide missing outer positional parameters as well.
        for scope in &self.bindings {
            for key in scope.keys() {
                if !key.is_empty() && key.iter().all(u8::is_ascii_digit) {
                    bindings.insert(key.clone(), local(Vec::new()));
                }
            }
        }
        // Parameter zero retains leading whitespace from the expanded name,
        // but GNU terminates it at the end of the lookup token.
        let zero_end = expanded[0].len() - name_text.len() + name.len();
        for (index, value) in expanded.iter().enumerate() {
            let value = if index == 0 {
                &value[..zero_end]
            } else {
                value
            };
            bindings.insert(index.to_string().into_bytes(), local(value.to_vec()));
        }
        self.bindings.push(bindings);
        let result = if variable.flavor == Flavor::Simple {
            Ok(variable.value)
        } else {
            self.expand_layer(name, &variable, layer, 0)
        };
        self.bindings.pop();
        result.map_err(|error| error.at(variable.definition.as_ref()))
    }

    fn filesystem(
        &mut self,
        name: &[u8],
        effect: FilesystemEffect<'_>,
    ) -> Result<Vec<u8>, ExpandError> {
        match self.context.perform_filesystem(effect) {
            None => Err(ExpandError::Unsupported(name.to_vec())),
            Some(Err(FilesystemError::Unlocated(message))) => {
                Err(ExpandError::UnlocatedError(message))
            }
            Some(Err(FilesystemError::Caller(message))) => Err(ExpandError::ContextualUserError {
                message,
                location: self.context.location.cloned().map(Arc::new),
            }),
            Some(Ok(bytes)) if bytes.len() > MAX_OUTPUT => Err(ExpandError::ExpansionLimit),
            Some(Ok(bytes)) => Ok(bytes),
        }
    }

    fn environment_variable(&mut self, name: &[u8]) -> Result<Vec<u8>, ExpandError> {
        let environment = self.store.for_environment();
        let Some((layer, variable)) = environment.lookup_from(name, 0) else {
            return Ok(Vec::new());
        };
        if variable.flavor == Flavor::Recursive && variable.scope.append_outer {
            let Some((first_layer, first)) = self.store.lookup_from(name, 0) else {
                return Ok(Vec::new());
            };
            let first = first.clone();
            self.active.push((name.to_vec(), Some(layer)));
            let prior = self.binding_context.replace(variable.definition.clone());
            let result = self.expand_layer(name, &first, Some(first_layer), 0);
            self.binding_context = prior;
            self.active.pop();
            result
        } else {
            self.selected_variable(name, variable, Some(layer))
        }
    }

    fn shell(&mut self, command: &[u8]) -> Result<Vec<u8>, ExpandError> {
        if matches!(self.store, StoreAccess::Read(_)) {
            return Err(ExpandError::Unsupported(b"shell".to_vec()));
        }
        if command
            .iter()
            .all(|byte| matches!(byte, b' ' | b'\t' | b'\n'))
        {
            return Ok(Vec::new());
        }
        let shell = self.variable(b"SHELL")?;
        let flags_raw = self
            .store
            .get(b".SHELLFLAGS")
            .map_or_else(Vec::new, |v| v.value.clone());
        let flags = self.expand(&flags_raw)?;
        let ifs = if self.lookup(b"IFS").is_some() {
            Some(self.variable(b"IFS")?)
        } else {
            None
        };
        let export_all = match &self.store {
            StoreAccess::Database(db) | StoreAccess::Scoped { database: db, .. } => {
                db.export_all_for_shell()
            }
            _ => false,
        };
        let snapshot = self.store.for_environment();
        let names = snapshot.environment_order(|name, variable| {
            if name == b"SHELL" && variable.export_policy().is_none() {
                return false;
            }
            let valid = name
                .first()
                .is_some_and(|byte| byte.is_ascii_alphabetic() || *byte == b'_')
                && name
                    .iter()
                    .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'_');
            self.store.resolve_export_policy(name).unwrap_or(
                valid
                    && (matches!(
                        variable.origin,
                        Origin::Environment | Origin::EnvironmentOverride | Origin::CommandLine
                    ) || (export_all
                        && !matches!(variable.origin, Origin::Default | Origin::Automatic))),
            )
        });
        let names = names
            .into_iter()
            .filter(|name| snapshot.get(name).is_some());
        let mut environment = Vec::new();
        for name in names {
            let current = self.store.for_environment();
            let Some(variable) = current.get(&name).cloned() else {
                environment.push((name, crate::diagnostic::EnvironmentValue::Remove));
                continue;
            };
            if name == b"SHELL" && variable.export_policy().is_none() {
                continue;
            }
            let valid = name
                .first()
                .is_some_and(|byte| byte.is_ascii_alphabetic() || *byte == b'_')
                && name
                    .iter()
                    .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'_');
            let export = self.store.resolve_export_policy(&name).unwrap_or(
                valid
                    && (matches!(
                        variable.origin,
                        Origin::Environment | Origin::EnvironmentOverride | Origin::CommandLine
                    ) || (export_all
                        && !matches!(variable.origin, Origin::Default | Origin::Automatic))),
            );
            if !export {
                environment.push((name, crate::diagnostic::EnvironmentValue::Remove));
                continue;
            }
            // GNU uses the original process environment when constructing an
            // exported recursive variable would re-enter the current binding.
            if self.active.iter().any(|(active, _)| *active == name) {
                environment.push((name, crate::diagnostic::EnvironmentValue::Original));
                continue;
            }
            let value = if matches!(
                variable.origin,
                Origin::Environment | Origin::EnvironmentOverride
            ) {
                variable.value
            } else {
                self.environment_variable(&name)?
            };
            environment.push((name, crate::diagnostic::EnvironmentValue::Set(value)));
        }
        let original_shell = self.store.original_shell.clone();
        let result = self
            .context
            .run_shell(ShellRequest {
                command,
                shell: &shell,
                flags: &flags,
                ifs: ifs.as_deref(),
                environment: &environment,
                original_shell: original_shell.as_deref(),
            })
            .ok_or_else(|| ExpandError::Unsupported(b"shell".to_vec()))?
            .map_err(ExpandError::UnlocatedError)?;
        let status = result.status.to_string();
        self.store
            .writable()
            .expect("mutable shell host")
            .define_literal(b".SHELLSTATUS", status.as_bytes(), Origin::Override)?;
        if result.stdout.len() > MAX_OUTPUT {
            return Err(ExpandError::ExpansionLimit);
        }
        let mut output = Vec::new();
        let mut index = 0;
        while index < result.stdout.len() {
            let byte = result.stdout[index];
            if byte == 0 {
                break;
            }
            if byte == b'\r' && result.stdout.get(index + 1) == Some(&b'\n') {
                index += 1;
            }
            output.push(if result.stdout[index] == b'\n' {
                b' '
            } else {
                byte
            });
            index += 1;
        }
        // Strip only folded trailing newlines, retaining literal trailing spaces.
        let mut end = result
            .stdout
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(result.stdout.len());
        let mut folded = 0;
        while end > 0 && result.stdout[end - 1] == b'\n' {
            folded += 1;
            end -= 1;
            if end > 0 && result.stdout[end - 1] == b'\r' {
                end -= 1;
            }
        }
        output.truncate(output.len().saturating_sub(folded));
        Ok(output)
    }

    fn eager_function(&mut self, name: &[u8], args: &[Vec<u8>]) -> Result<Vec<u8>, ExpandError> {
        if matches!(name, b"word" | b"wordlist") {
            let minimum = if name == b"word" { 2 } else { 3 };
            if args.len() < minimum {
                return Err(too_few_arguments(name, args.len()));
            }
        }
        let first = args.first().map(Vec::as_slice).unwrap_or_default();
        match name {
            b"shell" => self.shell(first),
            b"eval" => {
                let location = self.context.location.cloned();
                let result = match &mut self.store {
                    StoreAccess::Database(db) => db.evaluate_in(
                        first,
                        location.as_ref(),
                        None,
                        self.bindings.clone(),
                        self.context.reborrow(),
                    ),
                    StoreAccess::Scoped {
                        database,
                        variables,
                    } => {
                        let result = database.evaluate_in(
                            first,
                            location.as_ref(),
                            Some(*variables),
                            self.bindings.clone(),
                            self.context.reborrow(),
                        );
                        variables.refresh_globals(&database.variables);
                        result
                    }
                    _ => return Err(ExpandError::Unsupported(name.to_vec())),
                };
                result.map_err(|error| {
                    if error.unlocated {
                        // Without a caller, an eval syntax/phase error still
                        // inherits its enclosing binding's validation locus.
                        ExpandError::FileError(error.message.as_bytes().to_vec())
                    } else {
                        ExpandError::Located {
                            location: Arc::new(error.location),
                            error: Box::new(ExpandError::FileError(
                                error.message.as_bytes().to_vec(),
                            )),
                        }
                    }
                })?;
                Ok(Vec::new())
            }

            b"wildcard" => {
                let session = self.store.begin_filename_session(first.contains(&b'~'));
                let result = (|| {
                    let mut output = Vec::new();
                    for (mut pattern, emit, offset) in crate::filenames::sequence_events(first) {
                        if session {
                            pattern = self.store.write_filename(&pattern, offset);
                        }
                        if !emit {
                            continue;
                        }
                        if pattern.starts_with(b"~") {
                            let expanded = if pattern.len() == 1 || pattern[1] == b'/' {
                                let home = self.variable(b"HOME")?;
                                let suffix = self.store.filename_suffix(&pattern);
                                if home.is_empty() {
                                    let mut expanded =
                                        self.filesystem(name, FilesystemEffect::Tilde(b"~"))?;
                                    if !expanded.is_empty() {
                                        push(&mut expanded, &suffix)?;
                                    }
                                    expanded
                                } else {
                                    let mut expanded = home;
                                    push(&mut expanded, &suffix)?;
                                    expanded
                                }
                            } else {
                                self.filesystem(name, FilesystemEffect::Tilde(&pattern))?
                            };
                            if !expanded.is_empty() {
                                pattern = expanded;
                            }
                        }
                        if let Some((_, member)) = crate::filenames::archive_parts(&pattern)
                            && member.starts_with(b"(")
                            && member.ends_with(b")")
                        {
                            return Err(ExpandError::UnlocatedError(
                                [
                                    b"attempt to use unsupported feature: '".as_slice(),
                                    &pattern,
                                    b"'",
                                ]
                                .concat(),
                            ));
                        }
                        let matches =
                            self.filesystem(name, FilesystemEffect::Wildcard(&pattern))?;
                        if !matches.is_empty() {
                            if !output.is_empty() {
                                push(&mut output, b" ")?;
                            }
                            push(&mut output, &matches)?;
                        }
                    }
                    Ok(output)
                })();
                self.store.end_filename_session(session);
                result
            }
            b"file" => {
                let (offset, read, append) = match first {
                    [b'>', b'>', ..] => (2, false, true),
                    [b'>', ..] => (1, false, false),
                    [b'<', ..] => (1, true, false),
                    _ => {
                        return Err(ExpandError::FileError(
                            [b"file: invalid file operation: ".as_slice(), first].concat(),
                        ));
                    }
                };
                let path = words(&first[offset..])
                    .next()
                    .ok_or_else(|| ExpandError::FileError(b"file: missing filename".to_vec()))?;
                if read {
                    if args.len() > 1 {
                        return Err(ExpandError::FileError(b"file: too many arguments".to_vec()));
                    }
                    self.filesystem(name, FilesystemEffect::ReadFile(path))
                } else {
                    self.filesystem(
                        name,
                        FilesystemEffect::WriteFile {
                            path,
                            text: args.get(1).map(Vec::as_slice),
                            append,
                        },
                    )?;
                    Ok(Vec::new())
                }
            }
            b"realpath" => {
                let mut result = Vec::new();
                for path in words(first) {
                    let canonical = self.filesystem(name, FilesystemEffect::Realpath(path))?;
                    if !canonical.is_empty() {
                        if !result.is_empty() {
                            push(&mut result, b" ")?;
                        }
                        push(&mut result, &canonical)?;
                    }
                }
                Ok(result)
            }
            b"error" => args.first().map_or_else(
                || Ok(Vec::new()),
                |message| {
                    Err(match &self.binding_context {
                        Some(location) => ExpandError::ContextualUserError {
                            message: message.clone(),
                            location: location.clone(),
                        },
                        None => ExpandError::UserError(message.clone()),
                    })
                },
            ),
            b"info" | b"warning" => {
                if let Some(message) = args.first() {
                    let output = if name == b"info" {
                        Output::Info(message)
                    } else {
                        Output::Warning {
                            location: self.context.location.or(self
                                .binding_context
                                .as_ref()
                                .and_then(|location| location.as_deref())),
                            message,
                        }
                    };
                    if !self.context.emit(output) {
                        return Err(ExpandError::Unsupported(name.to_vec()));
                    }
                }
                Ok(Vec::new())
            }
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
                // The pinned Linux GNU 4.4.1 profile compares the first byte
                // as a signed char, then the remaining string as unsigned
                // bytes. This observable ordering is not ordinary byte sort.
                list.sort_unstable_by(|left, right| {
                    (left[0] as i8)
                        .cmp(&(right[0] as i8))
                        .then_with(|| left.cmp(right))
                });
                list.dedup();
                join_words(list)
            }
            b"words" => Ok(words(first).count().to_string().into_bytes()),
            b"word" => {
                let index = word_index(first, name, false)?;
                if index <= 0 {
                    return Err(ExpandError::WordIndexZero);
                }
                Ok(words(&args[1])
                    .nth(usize::try_from(index).unwrap_or(usize::MAX) - 1)
                    .unwrap_or_default()
                    .to_vec())
            }
            b"wordlist" => {
                let start = word_index(first, name, false)?;
                if start < 1 {
                    return Err(word_index_error(
                        name,
                        false,
                        Some(start.to_string().as_bytes()),
                        false,
                    ));
                }
                let end = word_index(&args[1], name, true)?;
                if end < 0 {
                    return Err(word_index_error(
                        name,
                        true,
                        Some(end.to_string().as_bytes()),
                        false,
                    ));
                }
                if end < start {
                    return Ok(Vec::new());
                }
                let start = usize::try_from(start).unwrap_or(usize::MAX);
                let end = usize::try_from(end).unwrap_or(usize::MAX);
                // Unlike most word functions, wordlist retains the whitespace
                // between selected words from the expanded input.
                let mut spans = word_spans(&args[2]);
                let Some((start_byte, first_end)) = spans.nth(start - 1) else {
                    return Ok(Vec::new());
                };
                let end_byte = spans
                    .take(end - start)
                    .last()
                    .map_or(first_end, |(_, end)| end);
                Ok(args[2][start_byte..end_byte].to_vec())
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
            b"origin" | b"flavor" | b"value" => {
                let variable = self.lookup(first);
                let special = variable.is_some_and(|variable| variable.scope.variable_names);
                let dynamic = if special {
                    Some(self.store.variable_names(true)?)
                } else {
                    None
                };
                match name {
                    b"origin" => Ok(variable
                        .map_or(b"undefined".as_slice(), |value| value.origin.name())
                        .to_vec()),
                    b"flavor" => Ok(match variable.map(|value| value.flavor) {
                        None => b"undefined".to_vec(),
                        Some(Flavor::Recursive) => b"recursive".to_vec(),
                        Some(Flavor::Simple) => b"simple".to_vec(),
                    }),
                    _ => Ok(dynamic.unwrap_or_else(|| {
                        variable.map_or_else(Vec::new, |value| value.value.clone())
                    })),
                }
            }
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
        definition: None,
        scope: ScopeMetadata::default(),
    }
}

fn invalid(function: &[u8], message: &'static str) -> ExpandError {
    ExpandError::InvalidArguments {
        function: function.to_vec(),
        message,
    }
}

fn too_few_arguments(function: &[u8], count: usize) -> ExpandError {
    if matches!(function, b"intcmp" | b"file" | b"word" | b"wordlist") {
        ExpandError::InsufficientArguments {
            function: function.to_vec(),
            count,
        }
    } else {
        invalid(function, "too few arguments")
    }
}

struct Decimal<'a> {
    negative: bool,
    digits: &'a [u8],
}

impl<'a> Decimal<'a> {
    fn parse(original: &'a [u8], second: bool) -> Result<Self, ExpandError> {
        // GNU's numeric whitespace also includes VT, unlike Rust's ASCII
        // whitespace predicate. Keep this local to the numeric grammar.
        let start = original
            .iter()
            .position(|byte| !gnu_whitespace(byte))
            .unwrap_or(original.len());
        let end = original
            .iter()
            .rposition(|byte| !gnu_whitespace(byte))
            .map_or(start, |index| index + 1);
        let value = &original[start..end];
        let (negative, digits) = match value.first() {
            Some(b'-') => (true, &value[1..]),
            Some(b'+') => (false, &value[1..]),
            _ => (false, value),
        };
        if digits.is_empty() || !digits.iter().all(u8::is_ascii_digit) {
            return Err(ExpandError::InvalidInteger {
                second,
                value: original.to_vec(),
                empty: value.is_empty(),
            });
        }
        let nonzero = digits.iter().position(|byte| *byte != b'0');
        Ok(match nonzero {
            Some(start) => Self {
                negative,
                digits: &digits[start..],
            },
            None => Self {
                negative: false,
                digits: b"0",
            },
        })
    }

    fn compare(&self, other: &Self) -> std::cmp::Ordering {
        // Pinned GNU Make 4.4.1 compares magnitudes in the same direction for
        // two negative operands: -19 compares greater than -7. Preserve this
        // observed quirk rather than applying mathematical signed ordering.
        other
            .negative
            .cmp(&self.negative)
            .then_with(|| self.digits.len().cmp(&other.digits.len()))
            .then_with(|| self.digits.cmp(other.digits))
    }
}

fn push(output: &mut Vec<u8>, value: &[u8]) -> Result<(), ExpandError> {
    if output.len().saturating_add(value.len()) > MAX_OUTPUT {
        return Err(ExpandError::ExpansionLimit);
    }
    output.extend_from_slice(value);
    Ok(())
}

fn gnu_whitespace(byte: &u8) -> bool {
    byte.is_ascii_whitespace() || *byte == 0x0b
}

fn trim_function_start(mut value: &[u8]) -> &[u8] {
    while value.first().is_some_and(gnu_whitespace) {
        value = &value[1..];
    }
    value
}

fn trim_start(mut value: &[u8]) -> &[u8] {
    while value.first().is_some_and(gnu_whitespace) {
        value = &value[1..];
    }
    value
}

fn trim(value: &[u8]) -> &[u8] {
    let mut value = trim_start(value);
    while value.last().is_some_and(gnu_whitespace) {
        value = &value[..value.len() - 1];
    }
    value
}

fn words(value: &[u8]) -> impl Iterator<Item = &[u8]> {
    value.split(gnu_whitespace).filter(|word| !word.is_empty())
}

fn word_spans(value: &[u8]) -> impl Iterator<Item = (usize, usize)> {
    let mut position = 0;
    std::iter::from_fn(move || {
        while position < value.len() && gnu_whitespace(&value[position]) {
            position += 1;
        }
        if position == value.len() {
            return None;
        }
        let start = position;
        while position < value.len() && !gnu_whitespace(&value[position]) {
            position += 1;
        }
        Some((start, position))
    })
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

pub(crate) fn reference_end(input: &[u8], opening: usize) -> Result<usize, ExpandError> {
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
        b"intcmp" => Some((2, Some(5))),
        b"error" | b"info" | b"warning" | b"realpath" => Some((0, Some(1))),
        b"or" | b"and" | b"call" => Some((1, None)),
        b"strip" | b"sort" | b"words" | b"firstword" | b"lastword" | b"dir" | b"notdir"
        | b"suffix" | b"basename" | b"origin" | b"flavor" | b"value" | b"abspath" | b"wildcard"
        | b"shell" | b"eval" | b"guile" => Some((1, Some(1))),
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

fn word_index_error(
    function: &[u8],
    second: bool,
    value: Option<&[u8]>,
    out_of_range: bool,
) -> ExpandError {
    let mut message = b"invalid ".to_vec();
    message.extend_from_slice(if second { b"second" } else { b"first" });
    message.extend_from_slice(b" argument to '");
    message.extend_from_slice(function);
    message.extend_from_slice(b"' function: ");
    match value {
        None => message.extend_from_slice(b"empty value"),
        Some(value) => {
            message.push(b'\'');
            message.extend_from_slice(value);
            message.push(b'\'');
            if out_of_range {
                message.extend_from_slice(b" out of range");
            }
        }
    }
    ExpandError::WordIndexError(message)
}

fn word_index(value: &[u8], function: &[u8], second: bool) -> Result<i64, ExpandError> {
    let trimmed = trim(value);
    if trimmed.is_empty() {
        return Err(word_index_error(function, second, None, false));
    }
    let (negative, digits) = match trimmed[0] {
        b'-' => (true, &trimmed[1..]),
        b'+' => (false, &trimmed[1..]),
        _ => (false, trimmed),
    };
    let limit = if negative {
        1u64 << 63
    } else {
        i64::MAX as u64
    };
    let mut magnitude = 0u64;
    let mut count = 0;
    for digit in digits.iter().take_while(|byte| byte.is_ascii_digit()) {
        let digit = u64::from(*digit - b'0');
        if magnitude > (limit - digit) / 10 {
            // Numeric-prefix overflow wins even when nonnumeric suffixes follow.
            return Err(word_index_error(function, second, Some(value), true));
        }
        magnitude = magnitude * 10 + digit;
        count += 1;
    }
    if count == 0 || count != digits.len() {
        return Err(word_index_error(function, second, Some(value), false));
    }
    Ok(if negative && magnitude == 1u64 << 63 {
        i64::MIN
    } else if negative {
        -(magnitude as i64)
    } else {
        magnitude as i64
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pattern {
    bytes: Vec<u8>,
    wildcard: Option<usize>,
}

impl Pattern {
    pub fn new(input: &[u8]) -> Self {
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

    pub fn has_wildcard(&self) -> bool {
        self.wildcard.is_some()
    }

    pub(crate) fn fixed_len(&self) -> usize {
        self.bytes.len() - usize::from(self.wildcard.is_some())
    }

    pub fn literal_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub(crate) fn matches<'a>(&self, word: &'a [u8]) -> Option<&'a [u8]> {
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

    pub fn replace(&self, stem: &[u8]) -> Result<Vec<u8>, ExpandError> {
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
    if pattern.bytes.is_empty() {
        push(&mut output, text)?;
        if text.last().is_none_or(gnu_whitespace) {
            push(&mut output, &replacement.bytes)?;
        }
        return Ok(output);
    }
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
            // An explicitly empty replacement removes its matched word.
            // A '%' replacement with an empty stem still contributes a word
            // position, which preserves the separator around that empty value.
            if pattern.matches(word).is_some() && replacement.bytes.is_empty() {
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

    fn source(line: usize) -> Location {
        Location {
            file: b"source.mk".as_slice().into(),
            line,
        }
    }

    #[test]
    fn nested_variable_and_call_errors_keep_the_innermost_definition() {
        let mut store = VariableStore::new();
        store
            .define_at(
                b"INNER",
                b"$(intcmp bad,0)",
                Assignment::Recursive,
                Origin::File,
                &source(2),
            )
            .unwrap();
        store
            .define_at(
                b"OUTER",
                b"$(call INNER)",
                Assignment::Recursive,
                Origin::File,
                &source(7),
            )
            .unwrap();
        for input in [b"$(OUTER)".as_slice(), b"$(call OUTER)"] {
            assert_eq!(
                store.expand(input).unwrap_err().location(),
                Some(&source(2))
            );
        }
        store
            .define_at(
                b"INNER",
                b"ok",
                Assignment::Recursive,
                Origin::File,
                &source(3),
            )
            .unwrap();
        store
            .define_at(
                b"OUTER",
                b"$(call INNER)$(intcmp bad,0)",
                Assignment::Recursive,
                Origin::File,
                &source(8),
            )
            .unwrap();
        assert_eq!(
            store.expand(b"$(OUTER)").unwrap_err().location(),
            Some(&source(8))
        );
    }

    #[test]
    fn bindings_without_sources_inherit_the_surrounding_expansion_context() {
        let mut store = VariableStore::new();
        store
            .define(
                b"CLI",
                b"$(intcmp bad,0)",
                Assignment::Recursive,
                Origin::CommandLine,
            )
            .unwrap();
        assert_eq!(store.expand_variable(b"CLI").unwrap_err().location(), None);
        store
            .define_at(
                b"OUTER",
                b"$(CLI)",
                Assignment::Recursive,
                Origin::File,
                &source(4),
            )
            .unwrap();
        assert_eq!(
            store.expand_variable(b"OUTER").unwrap_err().location(),
            Some(&source(4))
        );
        store
            .define_literal(b"OUTER", b"literal", Origin::Automatic)
            .unwrap();
        assert_eq!(store.get(b"OUTER").unwrap().definition(), None);
    }

    #[test]
    fn definition_metadata_changes_only_when_the_binding_changes() {
        let mut store = VariableStore::new();
        store
            .define_at(
                b"V",
                b"$(intcmp bad,0)",
                Assignment::Recursive,
                Origin::File,
                &source(1),
            )
            .unwrap();
        let cloned = store.clone();
        assert!(Arc::ptr_eq(
            store.get(b"V").unwrap().definition.as_ref().unwrap(),
            cloned.get(b"V").unwrap().definition.as_ref().unwrap()
        ));
        store
            .define_at(b"V", b"", Assignment::Append, Origin::Override, &source(2))
            .unwrap();
        assert_eq!(store.get(b"V").unwrap().origin(), Origin::File);
        store
            .define_at(
                b"V",
                b"ignored",
                Assignment::Conditional,
                Origin::File,
                &source(3),
            )
            .unwrap();
        assert_eq!(store.get(b"V").unwrap().definition(), Some(&source(1)));
        store
            .define_at(
                b"V",
                b"$(EMPTY)",
                Assignment::Append,
                Origin::File,
                &source(4),
            )
            .unwrap();
        assert_eq!(
            store.expand_variable(b"V").unwrap_err().location(),
            Some(&source(4))
        );
        store
            .define_at(b"S", b"text", Assignment::Simple, Origin::File, &source(5))
            .unwrap();
        store
            .define_at(
                b"S",
                b"$(EMPTY)",
                Assignment::Append,
                Origin::Override,
                &source(6),
            )
            .unwrap();
        assert_eq!(store.get(b"S").unwrap().origin(), Origin::File);
        assert_eq!(store.get(b"S").unwrap().definition(), Some(&source(5)));
    }

    #[test]
    fn call_argument_errors_precede_the_callee_definition_context() {
        let mut store = VariableStore::new();
        store
            .define_at(
                b"CHECK",
                b"$(intcmp $(1),0)",
                Assignment::Recursive,
                Origin::File,
                &source(2),
            )
            .unwrap();
        assert_eq!(
            store
                .expand(b"$(call CHECK,$(intcmp bad,0))")
                .unwrap_err()
                .location(),
            None
        );
        assert_eq!(
            store.expand(b"$(call CHECK,bad)").unwrap_err().location(),
            Some(&source(2))
        );
    }

    #[test]
    fn cycle_diagnostic_uses_reentered_binding_source_and_raw_name_bytes() {
        let mut store = VariableStore::new();
        store
            .define_at(
                b"\xff",
                b"$(OTHER)",
                Assignment::Recursive,
                Origin::File,
                &source(2),
            )
            .unwrap();
        store
            .define_at(
                b"OTHER",
                b"$(\xff)",
                Assignment::Recursive,
                Origin::File,
                &source(3),
            )
            .unwrap();
        let error = store.expand_variable(b"\xff").unwrap_err();
        assert_eq!(error.location(), Some(&source(2)));
        assert_eq!(
            error.diagnostic().as_bytes(),
            b"Recursive variable '\xff' references itself (eventually)"
        );
        assert_eq!(
            store.expand_variable(b"OTHER").unwrap_err().location(),
            Some(&source(3))
        );
    }

    #[test]
    fn intcmp_defaults_normalize_numbers_and_distinguish_empty_branches() {
        for (input, expected) in [
            (b"$(intcmp +0003,003)".as_slice(), b"3".as_slice()),
            (b"$(intcmp -0003,-003)", b"-3"),
            (b"$(intcmp -000,+0)", b"0"),
            (b"$(intcmp 1,2)", b""),
            (b"$(intcmp 3,3,)", b""),
            (b"$(intcmp 1,2, L ,E,G)", b" L "),
            (b"$(intcmp 2,2,L,E,G)", b"E"),
            (b"$(intcmp 3,2,L,E)", b"E"),
            (b"$(intcmp 3,2,L,E,)", b""),
            (b"$(intcmp 3,2,L)", b""),
            (b"$(intcmp 3,2,L,E,G,extra)", b"G,extra"),
        ] {
            assert_eq!(expand(input), expected, "{input:?}");
        }
    }

    #[test]
    fn intcmp_preserves_gnu_441_negative_magnitude_ordering() {
        for (input, expected) in [
            (b"$(intcmp -19,-7,L,E,G)".as_slice(), b"G".as_slice()),
            (b"$(intcmp -7,-19,L,E,G)", b"L"),
            (b"$(intcmp -9,-8,L,E,G)", b"G"),
            (b"$(intcmp -1,0,L,E,G)", b"L"),
            (b"$(intcmp -0,-1,L,E,G)", b"G"),
            (b"$(intcmp -0001,-1,L,E,G)", b"E"),
        ] {
            assert_eq!(expand(input), expected, "{input:?}");
        }
    }

    #[test]
    fn intcmp_accepts_numbers_beyond_machine_integer_limits() {
        let digits = vec![b'9'; 10_000];
        let expression = [b"$(intcmp +000".as_slice(), &digits, b",", &digits, b")"].concat();
        assert_eq!(expand(&expression), digits);
        let expression = [b"$(intcmp ".as_slice(), &digits, b",1", &digits, b",L,E,G)"].concat();
        assert_eq!(expand(&expression), b"L");
    }

    #[test]
    fn intcmp_numeric_whitespace_includes_vertical_tabs_but_not_non_ascii() {
        let mut store = VariableStore::new();
        for byte in [b' ', b'\t', b'\n', b'\r', 0x0b, 0x0c] {
            store
                .define_literal(b"N", &[byte, b'+', b'0', b'7', byte], Origin::File)
                .unwrap();
            assert_eq!(store.expand(b"$(intcmp $(N),7)").unwrap(), b"7");
        }
        for value in [b"\xff ".as_slice(), b" + ", b"1 2", b"0x10", b"\xc2\xa01"] {
            store.define_literal(b"N", value, Origin::File).unwrap();
            let error = store.expand(b"$(intcmp 1,$(N))").unwrap_err();
            assert_eq!(
                error.diagnostic().as_bytes(),
                [
                    b"non-numeric second argument to 'intcmp' function: '".as_slice(),
                    value,
                    b"'"
                ]
                .concat()
            );
        }
        let error = store.expand(b"$(intcmp \x0b\x0c,0)").unwrap_err();
        assert_eq!(
            error.diagnostic().as_bytes(),
            b"non-numeric first argument to 'intcmp' function: empty value"
        );
    }

    #[test]
    fn intcmp_expands_both_operands_before_validation_and_only_selected_branch() {
        let store = VariableStore::new();
        assert_eq!(
            store.expand(b"$(intcmp bad,$(shell unused),L,E,G)"),
            Err(ExpandError::Unsupported(b"shell".to_vec()))
        );
        assert_eq!(
            store
                .expand(b"$(intcmp 1,1,$(shell unused),equal,$(shell unused))")
                .unwrap(),
            b"equal"
        );
        assert_eq!(
            store.expand(b"$(intcmp 1,2,$(shell selected),E,G)"),
            Err(ExpandError::Unsupported(b"shell".to_vec()))
        );
        let error = store
            .expand(b"$(intcmp lhs,rhs,$(shell unused))")
            .unwrap_err();
        assert_eq!(
            error.diagnostic().as_bytes(),
            b"non-numeric first argument to 'intcmp' function: 'lhs'"
        );
    }

    #[test]
    fn call_intcmp_is_eager_then_reexpands_operands_and_selected_branch() {
        let mut store = VariableStore::new();
        define(&mut store, b"N", b"3", Assignment::Recursive);
        define(&mut store, b"VALUE", b"selected", Assignment::Recursive);
        define(&mut store, b"intcmp", b"shadow", Assignment::Recursive);
        assert_eq!(store.expand(b"$(intcmp)").unwrap(), b"shadow");
        assert_eq!(
            store
                .expand(b"$(call intcmp,$$(N),3,$$(shell unused),$$(VALUE),$$(shell unused),extra)")
                .unwrap(),
            b"selected"
        );
        assert_eq!(
            store.expand(b"$(call intcmp,1,1,L,E,G,$(shell extra))"),
            Err(ExpandError::Unsupported(b"shell".to_vec()))
        );
        assert_eq!(
            store.expand(b"$(call intcmp,1,1,$(shell unselected),E,G)"),
            Err(ExpandError::Unsupported(b"shell".to_vec()))
        );
    }

    #[test]
    fn intcmp_arity_errors_include_the_actual_count_before_direct_expansion() {
        let store = VariableStore::new();
        for (input, count) in [
            (b"$(intcmp )".as_slice(), 1),
            (b"$(intcmp $(shell unused))", 1),
            (b"$(call intcmp)", 0),
            (b"$(call intcmp,1)", 1),
        ] {
            assert_eq!(
                store.expand(input),
                Err(ExpandError::InsufficientArguments {
                    function: b"intcmp".to_vec(),
                    count
                })
            );
        }
        assert_eq!(
            store.expand(b"$(call intcmp,$(shell first))"),
            Err(ExpandError::Unsupported(b"shell".to_vec()))
        );
    }

    #[test]
    fn function_names_use_gnu_whitespace_and_call_uses_the_first_word() {
        let mut store = VariableStore::new();
        define(&mut store, b"user", b"short", Assignment::Recursive);
        define(&mut store, b"user name", b"long", Assignment::Recursive);
        for input in [
            b"$(intcmp\x0b1,2,L,E,G)".as_slice(),
            b"$(call\x0bintcmp,1,2,L,E,G)",
            b"$(call \x0bintcmp\x0bignored,1,2,L,E,G)",
            b"$(call intcmp ignored,1,2,L,E,G)",
        ] {
            assert_eq!(store.expand(input).unwrap(), b"L", "{input:?}");
        }
        assert_eq!(store.expand(b"$(call user name)").unwrap(), b"short");
        assert_eq!(store.expand(b"$(call user\x0bname)").unwrap(), b"short");
        define(&mut store, b"user", b"[$(0)]/$(1)", Assignment::Recursive);
        store
            .define_literal(b"NAME", b" \x0buser ignored", Origin::File)
            .unwrap();
        assert_eq!(
            store.expand(b"$(call $(NAME),a)").unwrap(),
            b"[ \x0buser]/a"
        );
    }

    #[test]
    fn parsed_zero_word_index_has_gnu_diagnostic() {
        for input in [b"$(word 0,x)".as_slice(), b"$(word +00,x)"] {
            let error = VariableStore::new().expand(input).unwrap_err();
            assert_eq!(error, ExpandError::WordIndexZero);
            assert_eq!(
                error.diagnostic().as_bytes(),
                b"first argument to 'word' function must be greater than 0"
            );
        }
        assert_eq!(
            VariableStore::new().expand(b"$(word -1,x)"),
            Err(ExpandError::WordIndexZero)
        );
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
    fn precedence_preserves_origin_and_skips_deferred_rhs_evaluation() {
        let mut store = VariableStore::new();
        store
            .define(b"X", b"command", Assignment::Recursive, Origin::CommandLine)
            .unwrap();
        store
            .define(
                b"X",
                b"$(shell forbidden)",
                Assignment::Recursive,
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
    fn losing_immediate_assignments_still_expand_rhs_without_replacing_value() {
        // Oracle: BROKEN=$(BROKEN); X:=$(BROKEN) (or :::=) fails even with
        // X=chosen on the command line, or a preceding override definition.
        for origin in [Origin::CommandLine, Origin::Override] {
            for assignment in [Assignment::Simple, Assignment::ImmediateRecursive] {
                let mut store = VariableStore::new();
                define(&mut store, b"BROKEN", b"$(BROKEN)", Assignment::Recursive);
                store
                    .define(b"X", b"chosen", Assignment::Recursive, origin)
                    .unwrap();
                assert_eq!(
                    store.define(b"X", b"$(BROKEN)", assignment, Origin::File),
                    Err(ExpandError::RecursiveVariable(b"BROKEN".to_vec()))
                );
                assert_eq!(store.get(b"X").unwrap().raw_value(), b"chosen");
                assert_eq!(store.get(b"X").unwrap().origin(), origin);
                assert_eq!(
                    store.define(b"X", b"$(unterminated", assignment, Origin::File),
                    Err(ExpandError::UnterminatedReference)
                );
                store
                    .define(b"X", b"$(subst x,y,x)", assignment, Origin::File)
                    .unwrap();
                assert_eq!(store.expand(b"$(X)").unwrap(), b"chosen");
            }
        }
    }

    #[test]
    fn losing_append_evaluates_only_when_existing_binding_is_simple() {
        // Oracle: X+=$(BROKEN) fails with command-line X:=chosen, but stays
        // unevaluated with X=chosen. Existing flavor controls append timing.
        for (existing, evaluates) in [
            (Assignment::Simple, true),
            (Assignment::Recursive, false),
            (Assignment::ImmediateRecursive, false),
        ] {
            let mut store = VariableStore::new();
            define(&mut store, b"BROKEN", b"$(BROKEN)", Assignment::Recursive);
            store
                .define(b"X", b"chosen", existing, Origin::CommandLine)
                .unwrap();
            let result = store.define(b"X", b"$(BROKEN)", Assignment::Append, Origin::File);
            if evaluates {
                assert_eq!(
                    result,
                    Err(ExpandError::RecursiveVariable(b"BROKEN".to_vec()))
                );
            } else {
                result.unwrap();
            }
            assert_eq!(store.expand(b"$(X)").unwrap(), b"chosen");
            assert_eq!(store.get(b"X").unwrap().origin(), Origin::CommandLine);
        }
    }

    #[test]
    fn conditional_assignment_skips_rhs_for_any_existing_flavor_or_origin() {
        for origin in [Origin::File, Origin::CommandLine, Origin::Override] {
            for existing in [Assignment::Simple, Assignment::Recursive] {
                let mut store = VariableStore::new();
                define(&mut store, b"BROKEN", b"$(BROKEN)", Assignment::Recursive);
                store.define(b"X", b"", existing, origin).unwrap();
                store
                    .define(
                        b"X",
                        b"$(BROKEN)",
                        Assignment::Conditional,
                        Origin::Override,
                    )
                    .unwrap();
                assert_eq!(store.expand(b"$(X)").unwrap(), b"");
                assert_eq!(store.get(b"X").unwrap().origin(), origin);
            }
        }
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
        assert_eq!(expand(b"$(sort \xff \x80 a \xff)"), b"\x80 \xff a");
        assert_eq!(expand(b"$(subst \xff,z,\x80\xff)"), b"\x80z");
    }

    #[test]
    fn sort_uses_signed_first_byte_but_unsigned_remaining_bytes() {
        assert_eq!(expand(b"$(sort a\xff aA a\x80)"), b"aA a\x80 a\xff");
        assert_eq!(expand(b"$(sort \xffa \x80b a)"), b"\x80b \xffa a");
        assert_eq!(
            expand(b"$(sort \xff\xff \xffa \xff\x80)"),
            b"\xffa \xff\x80 \xff\xff"
        );
    }

    #[test]
    fn empty_patsubst_pattern_appends_unquoted_replacement() {
        assert_eq!(expand(b"$(patsubst ,x, a )"), b" a x");
        assert_eq!(expand(br"$(patsubst ,a\%b, a )"), b" a a%b");
        assert_eq!(expand(br"$(patsubst ,a\\%b, a )"), br" a a\%b");
        assert_eq!(expand(b"$(patsubst ,%, a )"), b" a %");
        assert_eq!(expand(b"$(patsubst ,x,)"), b"x");
        assert_eq!(expand(b"$(patsubst ,x,a)"), b"a");
        assert_eq!(expand(b"$(patsubst ,x, a)"), b" a");
        assert_eq!(expand(b"$(patsubst ,x,a )"), b"a x");
        assert_eq!(expand(b"$(patsubst ,x,  )"), b"  x");
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
            b"$(word x,a)",
            b"$(wordlist 0,1,a)",
            b"$(word -1,a)",
        ] {
            assert!(VariableStore::new().expand(input).is_err());
        }
    }

    #[test]
    fn word_indices_accept_signed_decimal_zero_and_i64_limits_exactly() {
        for (expression, expected) in [
            (b"$(word +0002,a b c)".as_slice(), b"b".as_slice()),
            (b"$(word \x0b+2\r\n,a b c)", b"b"),
            (b"$(word 9223372036854775807,a b c)", b""),
            (b"$(wordlist 1,+9223372036854775807,a\tb c)", b"a\tb c"),
            (
                b"$(wordlist 9223372036854775807,9223372036854775807,a b)",
                b"",
            ),
            (b"$(wordlist 1,-0000,a b)", b""),
            (b"$(wordlist +0002,0003,a b c d)", b"b c"),
        ] {
            assert_eq!(expand(expression), expected);
        }
        let zeros = vec![b'0'; 100];
        let expression = [b"$(word ".as_slice(), &zeros, b"2,a b c)"].concat();
        assert_eq!(expand(&expression), b"b");
    }

    #[test]
    fn word_index_errors_distinguish_domain_syntax_and_numeric_prefix_overflow() {
        for (expression, expected) in [
            (b"$(word -0001,a b)".as_slice(), b"first argument to 'word' function must be greater than 0".as_slice()),
            (b"$(word -9223372036854775808,a b)", b"first argument to 'word' function must be greater than 0"),
            (b"$(wordlist -0001,2,a b)", b"invalid first argument to 'wordlist' function: '-1'"),
            (b"$(wordlist -0,bad,a b)", b"invalid first argument to 'wordlist' function: '0'"),
            (b"$(wordlist 1,-0002,a b)", b"invalid second argument to 'wordlist' function: '-2'"),
            (b"$(wordlist 1,-9223372036854775808,a b)", b"invalid second argument to 'wordlist' function: '-9223372036854775808'"),
            (b"$(word 9223372036854775808x,a b)", b"invalid first argument to 'word' function: '9223372036854775808x' out of range"),
            (b"$(word x9223372036854775808,a b)", b"invalid first argument to 'word' function: 'x9223372036854775808'"),
            (b"$(wordlist 1,-9223372036854775809x,a b)", b"invalid second argument to 'wordlist' function: '-9223372036854775809x' out of range"),
            (b"$(wordlist 1,18446744073709551615,a b)", b"invalid second argument to 'wordlist' function: '18446744073709551615' out of range"),
            (b"$(word ,a b)", b"invalid first argument to 'word' function: empty value"),
            (b"$(wordlist 1,\x0b\r\n,a b)", b"invalid second argument to 'wordlist' function: empty value"),
        ] {
            let error = VariableStore::new().expand(expression).unwrap_err();
            assert_eq!(error.diagnostic().as_bytes(), expected);
        }
    }

    #[test]
    fn word_index_diagnostics_keep_untrimmed_non_utf8_bytes() {
        let mut store = VariableStore::new();
        store
            .define_literal(b"N", b" \t\xffbad\xa0\x0b ", Origin::File)
            .unwrap();
        let error = store.expand(b"$(word $(N),a b)").unwrap_err();
        assert_eq!(
            error.diagnostic().as_bytes(),
            b"invalid first argument to 'word' function: ' \t\xffbad\xa0\x0b '"
        );
        store
            .define_literal(b"N", b" \t9223372036854775808x\x0b ", Origin::File)
            .unwrap();
        let error = store.expand(b"$(wordlist 1,$(N),a b)").unwrap_err();
        assert_eq!(error.diagnostic().as_bytes(), b"invalid second argument to 'wordlist' function: ' \t9223372036854775808x\x0b ' out of range");
    }

    #[test]
    fn word_functions_expand_provided_arguments_before_arity_or_numeric_validation() {
        let store = VariableStore::new();
        for (expression, messages, diagnostic) in [
            (
                b"$(word $(info first))".as_slice(),
                vec![b"first".as_slice()],
                b"insufficient number of arguments (1) to function 'word'".as_slice(),
            ),
            (
                b"$(wordlist $(info first),$(info second))",
                vec![b"first".as_slice(), b"second".as_slice()],
                b"insufficient number of arguments (2) to function 'wordlist'",
            ),
            (
                b"$(wordlist 0,$(info second)bad,$(info text))",
                vec![b"second".as_slice(), b"text".as_slice()],
                b"invalid first argument to 'wordlist' function: '0'",
            ),
            (
                b"$(wordlist 1,$(info second)bad,$(info text))",
                vec![b"second".as_slice(), b"text".as_slice()],
                b"invalid second argument to 'wordlist' function: 'bad'",
            ),
        ] {
            let mut outputs = Vec::new();
            let error = store
                .expand_with(
                    expression,
                    ExpansionContext::new(None, &mut |output| {
                        if let Output::Info(message) = output {
                            outputs.push(message.to_vec());
                        }
                    }),
                )
                .unwrap_err();
            assert_eq!(outputs, messages);
            assert_eq!(error.diagnostic().as_bytes(), diagnostic);
        }
        assert_eq!(
            store.expand(b"$(word $(error before-arity))"),
            Err(ExpandError::UserError(b"before-arity".to_vec()))
        );
        assert_eq!(
            store.expand(b"$(wordlist 0,bad,$(error before-validation))"),
            Err(ExpandError::UserError(b"before-validation".to_vec()))
        );
    }

    #[test]
    fn word_index_errors_keep_the_recursive_binding_definition() {
        let mut store = VariableStore::new();
        let definition = source(2);
        let caller = source(9);
        store
            .define_at(
                b"F",
                b"$(word \xff,a b)",
                Assignment::Recursive,
                Origin::File,
                &definition,
            )
            .unwrap();
        let error = store
            .expand_with(b"$(F)", ExpansionContext::new(Some(&caller), &mut |_| {}))
            .unwrap_err();
        assert_eq!(error.diagnostic_location(Some(&caller)), Some(&definition));
        assert_eq!(
            error.diagnostic().as_bytes(),
            b"invalid first argument to 'word' function: '\xff'"
        );
    }

    #[test]
    fn word_functions_share_c_whitespace_without_splitting_other_bytes() {
        for separator in b" \t\n\r\x0b\x0c" {
            let list = [
                b"b.c".as_slice(),
                &[*separator],
                b"a.h",
                &[*separator],
                b"b.c",
            ]
            .concat();
            for (function, expected) in [
                (b"strip".as_slice(), b"b.c a.h b.c".as_slice()),
                (b"sort", b"a.h b.c"),
                (b"words", b"3"),
                (b"firstword", b"b.c"),
                (b"lastword", b"b.c"),
                (b"suffix", b".c .h .c"),
                (b"basename", b"b a b"),
                (b"notdir", b"b.c a.h b.c"),
                (b"dir", b"./ ./ ./"),
            ] {
                let expression = [b"$(".as_slice(), function, b" ", &list, b")"].concat();
                assert_eq!(expand(&expression), expected, "{function:?}, {separator}");
            }
        }
        assert_eq!(expand(b"$(words a\x1cb\x85c\xa0d)"), b"1");
        assert_eq!(expand(b"$(strip a\x1cb\x85c\xa0d)"), b"a\x1cb\x85c\xa0d");
        assert_eq!(expand(b"$(word \x0b2\x0b,a\x0bb\x0bc)"), b"b");
        assert_eq!(expand(b"$(join a\x0bb,c\x0bd)"), b"ac bd");
        assert_eq!(expand(b"$(filter a\x0bb,a\x0bc\x0bb)"), b"a b");
        assert_eq!(expand(b"$(filter-out a\x0bb,a\x0bc\x0bb)"), b"c");
        assert_eq!(expand(b"$(addprefix p,a\x0bb)"), b"pa pb");
        assert_eq!(expand(b"$(addsuffix s,a\x0bb)"), b"as bs");
    }

    #[test]
    fn word_selection_preserves_interior_bytes_and_string_functions_stay_literal() {
        assert_eq!(expand(b"$(wordlist 2,3,a\x0bb\r\nc\x0cd)"), b"b\r\nc");
        assert_eq!(expand(b"$(patsubst a,z,\x0ba\r\nb\x0c)"), b"\x0bz\r\nb\x0c");
        assert_eq!(expand(b"$(patsubst %,z%,\x0ba\r\nb\x0c)"), b"za zb");
        assert_eq!(expand(b"$(patsubst ,z,a\x0b)"), b"a\x0bz");
        assert_eq!(expand(b"$(subst a,z,\x0ba\r\nb\x0c)"), b"\x0bz\r\nb\x0c");
        assert_eq!(expand(b"$(findstring a\x0bb,x a\x0bb y)"), b"a\x0bb");
        let mut store = VariableStore::new();
        store.set_working_directory(b"/project");
        define(&mut store, b"a", b"value", Assignment::Recursive);
        assert_eq!(
            store.expand(b"$(abspath a\x0bb)").unwrap(),
            b"/project/a /project/b"
        );
        assert_eq!(
            store
                .expand(b"$(origin a\x0b):$(flavor a\x0b):$(value a\x0b)")
                .unwrap(),
            b"undefined:undefined:"
        );
    }

    #[test]
    fn lazy_conditions_trim_source_whitespace_but_retain_expanded_whitespace() {
        let mut store = VariableStore::new();
        define(&mut store, b"W", b" \t\x0b\r\n\x0c ", Assignment::Simple);
        for (expression, expected) in [
            (
                b"$(if \x0b$(MISSING)\x0b,yes,no)".as_slice(),
                b"no".as_slice(),
            ),
            (b"$(if $(W),yes,no)", b"yes"),
            (b"$(or \x0b, \tselected\x0b ,$(shell never))", b"selected"),
            (b"$(and yes,\x0b,$(shell never))", b""),
            (b"$(and yes, \tselected\x0b )", b"selected"),
            (b"$(or $(W),$(shell never))", b" \t\x0b\r\n\x0c "),
            (b"$(and yes,$(W))", b" \t\x0b\r\n\x0c "),
            (b"$(call or,$(W),fallback)", b"fallback"),
        ] {
            assert_eq!(store.expand(expression).unwrap(), expected);
        }
    }

    #[test]
    fn foreach_binds_the_first_expanded_name_token_and_restores_it() {
        let mut store = VariableStore::new();
        define(&mut store, b"x", b"outer", Assignment::Recursive);
        define(
            &mut store,
            b"NAME",
            b"\x0bx \tignored\x0c",
            Assignment::Simple,
        );
        assert_eq!(
            store
                .expand(b"$(foreach $(NAME),a\x0bb,[$(x):$(origin x):$(flavor x)])|$(x)")
                .unwrap(),
            b"[a:automatic:simple] [b:automatic:simple]|outer"
        );
    }

    #[test]
    fn let_retains_the_exact_last_remainder_and_handles_missing_values_safely() {
        assert_eq!(
            expand(b"$(let a\x0bb,\tfirst\x0bsecond\r\nthird\x0c,[$(a)][$(b)])"),
            b"[first][second\r\nthird\x0c]"
        );
        assert_eq!(
            expand(b"$(let a,\x0bfirst\tsecond \r,[$(a)])"),
            b"[first\tsecond \r]"
        );
        assert_eq!(expand(b"$(let ,ignored,body)"), b"body");
        // GNU 4.4.1 crashes on this shortage of words; retain a safe empty binding.
        assert_eq!(expand(b"$(let a b, \t\x0b,[$(a)][$(b)])"), b"[][]");
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
    fn percent_replacement_with_empty_stem_keeps_word_positions() {
        assert_eq!(expand(b"$(patsubst a%,%,a other a)"), b" other ");
        assert_eq!(expand(b"$(patsubst a%,,a other a)"), b"other");
        assert_eq!(expand(b"$(patsubst a%,%,a a)"), b" ");
        assert_eq!(expand(b"$(patsubst a%,,a a)"), b"");
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
    fn expansion_limit_preserves_effect_order_and_terminal_dollar() {
        let mut store = VariableStore::new();
        store
            .define_literal(b"BIG", &vec![b'x'; MAX_OUTPUT - 1], Origin::File)
            .unwrap();
        let mut messages = Vec::new();
        let mut output = |event: Output<'_>| {
            if let Output::Info(message) = event {
                messages.push(message.to_vec());
            }
        };
        let result = store
            .expand_with(
                b"$(info before)$(BIG)q$(info boundary)",
                ExpansionContext::new(None, &mut output),
            )
            .unwrap();
        assert_eq!(result.len(), MAX_OUTPUT);
        assert_eq!(result.last(), Some(&b'q'));
        assert_eq!(messages, [b"before".to_vec(), b"boundary".to_vec()]);
        messages.clear();
        assert_eq!(
            store.expand_with(
                b"$(info before)$(BIG)qr$(info forbidden)",
                ExpansionContext::new(None, &mut |event| {
                    if let Output::Info(message) = event {
                        messages.push(message.to_vec());
                    }
                }),
            ),
            Err(ExpandError::ExpansionLimit)
        );
        assert_eq!(messages, [b"before".to_vec()]);
        let result = store.expand(b"$(BIG)$").unwrap();
        assert_eq!(result.len(), MAX_OUTPUT);
        assert_eq!(result.last(), Some(&b'$'));
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
    fn error_distinguishes_empty_invocation_from_variable_and_zero_argument_call() {
        let mut store = VariableStore::new();
        define(&mut store, b"error", b"variable", Assignment::Recursive);
        assert_eq!(store.expand(b"$(error)").unwrap(), b"variable");
        assert_eq!(store.expand(b"$(call error)").unwrap(), b"");
        for input in [b"$(error )".as_slice(), b"$(call error,)"] {
            assert_eq!(store.expand(input), Err(ExpandError::UserError(Vec::new())));
        }
    }

    #[test]
    fn error_message_preserves_bytes_commas_whitespace_and_literal_call_dollars() {
        let store = VariableStore::new();
        for (input, message) in [
            (
                b"$(error   raw\xff,bytes  )".as_slice(),
                b"raw\xff,bytes  ".as_slice(),
            ),
            (b"$(call error, $$(VALUE) ,ignored)", b" $(VALUE) "),
            (b"$(error line\nnext)", b"line\nnext"),
        ] {
            let error = store.expand(input).unwrap_err();
            assert_eq!(error.diagnostic().as_bytes(), message);
        }
    }

    #[test]
    fn error_obeys_lazy_branches_and_eager_call_arguments() {
        let store = VariableStore::new();
        assert_eq!(
            store
                .expand(
                    b"$(if ,$(error unused),ok):$(or yes,$(error unused)):$(and ,$(error unused))"
                )
                .unwrap(),
            b"ok:yes:"
        );
        assert_eq!(
            store.expand(b"$(call error,outer,$(error inner))"),
            Err(ExpandError::UserError(b"inner".to_vec()))
        );
        assert_eq!(
            store.expand(b"$(call if,,ok,$(error eager))"),
            Err(ExpandError::UserError(b"eager".to_vec()))
        );
        assert_eq!(
            store.expand(b"$(error $(word 0,anything))"),
            Err(ExpandError::WordIndexZero)
        );
    }

    #[test]
    fn user_errors_prefer_active_context_without_changing_numeric_error_provenance() {
        let mut store = VariableStore::new();
        let definition = source(2);
        let caller = source(8);
        store
            .define_at(
                b"FAIL",
                b"$(error fatal)",
                Assignment::Recursive,
                Origin::File,
                &definition,
            )
            .unwrap();
        for input in [b"$(FAIL)".as_slice(), b"$(call FAIL)"] {
            let error = store.expand(input).unwrap_err();
            assert_eq!(error.diagnostic_location(Some(&caller)), Some(&caller));
            assert_eq!(error.diagnostic_location(None), Some(&definition));
        }
        store
            .define_at(
                b"FAIL",
                b"$(error $(intcmp bad,0))",
                Assignment::Recursive,
                Origin::File,
                &definition,
            )
            .unwrap();
        let error = store.expand(b"$(FAIL)").unwrap_err();
        assert_eq!(error.diagnostic_location(Some(&caller)), Some(&definition));
    }

    fn capture_output(output: Output<'_>, captured: &mut Vec<(bool, Option<Location>, Vec<u8>)>) {
        match output {
            Output::Info(message) => captured.push((false, None, message.to_vec())),
            Output::Warning { location, message } => {
                captured.push((true, location.cloned(), message.to_vec()))
            }
        }
    }

    #[test]
    fn output_is_synchronous_and_retained_before_later_fatal_expansion() {
        let store = VariableStore::default();
        let mut captured = Vec::new();
        let result = store.expand_with(
            b"$(info first$(warning nested))$(call info,second,$(info extra))$(error fatal)$(info absent)",
            ExpansionContext::new(None, &mut |output| capture_output(output, &mut captured)),
        );
        assert_eq!(result, Err(ExpandError::UserError(b"fatal".to_vec())));
        assert_eq!(
            captured,
            vec![
                (true, None, b"nested".to_vec()),
                (false, None, b"first".to_vec()),
                (false, None, b"extra".to_vec()),
                (false, None, b"second".to_vec())
            ]
        );
    }

    #[test]
    fn output_arity_literal_bytes_and_lazy_branches_match_control_functions() {
        let mut store = VariableStore::default();
        define(&mut store, b"info", b"named", Assignment::Recursive);
        let mut captured = Vec::new();
        let result = store.expand_with(
            b"$(info)|$(call info)$(call warning)$(call info,)$(warning )$(info a,b,\xff)$(call warning,$$(error literal))$(if yes,ok,$(info absent))",
            ExpansionContext::new(None, &mut |output| capture_output(output, &mut captured)),
        ).unwrap();
        assert_eq!(result, b"named|ok");
        assert_eq!(
            captured,
            vec![
                (false, None, Vec::new()),
                (true, None, Vec::new()),
                (false, None, b"a,b,\xff".to_vec()),
                (true, None, b"$(error literal)".to_vec())
            ]
        );
    }

    #[test]
    fn warnings_use_caller_or_first_binding_context_and_restore_nested_context() {
        let mut store = VariableStore::default();
        let outer = Location {
            file: b"outer".as_slice().into(),
            line: 2,
        };
        let inner = Location {
            file: b"inner\xff".as_slice().into(),
            line: 4,
        };
        let caller = Location {
            file: b"recipe".as_slice().into(),
            line: 9,
        };
        store
            .define_at(
                b"OUTER",
                b"$(warning before)$(call INNER)$(warning after)",
                Assignment::Recursive,
                Origin::File,
                &outer,
            )
            .unwrap();
        store
            .define_at(
                b"INNER",
                b"$(warning inner)",
                Assignment::Recursive,
                Origin::File,
                &inner,
            )
            .unwrap();
        for (context, expected) in [
            (None, vec![outer; 3]),
            (Some(&caller), vec![caller.clone(); 3]),
        ] {
            let mut captured = Vec::new();
            store
                .expand_variable_with(
                    b"OUTER",
                    ExpansionContext::new(context, &mut |output| {
                        capture_output(output, &mut captured)
                    }),
                )
                .unwrap();
            assert_eq!(
                captured
                    .into_iter()
                    .map(|(_, location, _)| location.unwrap())
                    .collect::<Vec<_>>(),
                expected
            );
        }
    }

    #[test]
    fn first_unlocated_binding_locks_warning_and_user_error_but_not_validation_location() {
        let mut store = VariableStore::default();
        let inner = source(1);
        store
            .define(
                b"OUTER",
                b"$(INNER)",
                Assignment::Recursive,
                Origin::CommandLine,
            )
            .unwrap();
        for (body, validation) in [
            (b"$(warning first)$(error fatal)".as_slice(), false),
            (b"$(warning first)$(intcmp bad,0)", true),
        ] {
            store
                .define_at(b"INNER", body, Assignment::Recursive, Origin::File, &inner)
                .unwrap();
            let mut captured = Vec::new();
            let error = store
                .expand_with(
                    b"$(call OUTER)",
                    ExpansionContext::new(None, &mut |output| {
                        capture_output(output, &mut captured)
                    }),
                )
                .unwrap_err();
            assert_eq!(captured, vec![(true, None, b"first".to_vec())]);
            assert_eq!(
                error.diagnostic_location(None),
                if validation { Some(&inner) } else { None }
            );
        }
        store
            .define_at(
                b"INNER",
                b"$(warning separate)",
                Assignment::Recursive,
                Origin::File,
                &inner,
            )
            .unwrap();
        let mut captured = Vec::new();
        store
            .expand_with(
                b"$(OUTER)$(INNER)",
                ExpansionContext::new(None, &mut |output| capture_output(output, &mut captured)),
            )
            .unwrap();
        assert_eq!(
            captured,
            vec![
                (true, None, b"separate".to_vec()),
                (true, Some(inner), b"separate".to_vec())
            ]
        );
    }

    #[test]
    fn literal_updates_retain_binding_metadata_until_undefine() {
        let mut store = VariableStore::new();
        define(&mut store, b"X", b"old", Assignment::Recursive);
        store.set_private(b"X", true);
        store.set_export_policy(b"X", Some(true));
        store
            .define_literal(b"X", b"$(untouched)", Origin::File)
            .unwrap();
        store
            .append_literal_at(b"X", b"$tail", Origin::File, &source(1))
            .unwrap();
        let variable = store.get(b"X").unwrap();
        assert_eq!(variable.raw_value(), b"$(untouched) $tail");
        assert_eq!(variable.flavor(), Flavor::Simple);
        assert!(variable.is_private());
        assert_eq!(variable.export_policy(), Some(true));
        assert!(store.undefine(b"X", Origin::File));
        store
            .define_literal(b"X", b"replacement", Origin::File)
            .unwrap();
        let variable = store.get(b"X").unwrap();
        assert!(!variable.is_private());
        assert_eq!(variable.export_policy(), None);
    }

    #[test]
    fn literal_metadata_append_keeps_flavor_and_bytes_without_running_rhs() {
        let mut store = VariableStore::default();
        for assignment in [Assignment::Recursive, Assignment::Simple] {
            store.remove(b"LIST");
            let raw = if assignment == Assignment::Simple {
                b"$$(warning untouched)old".as_slice()
            } else {
                b"$(warning untouched)old"
            };
            store
                .define(b"LIST", raw, assignment, Origin::File)
                .unwrap();
            let flavor = store.get(b"LIST").unwrap().flavor();
            store
                .append_literal_at(b"LIST", b"$(literal)\xff", Origin::File, &source(1))
                .unwrap();
            let variable = store.get(b"LIST").unwrap();
            assert_eq!(
                variable.raw_value(),
                b"$(warning untouched)old $(literal)\xff"
            );
            assert_eq!(variable.flavor(), flavor);
            assert_eq!(variable.definition(), Some(&source(1)));
        }
        store
            .define_literal(b"LIST", b"cli", Origin::CommandLine)
            .unwrap();
        store
            .append_literal_at(b"LIST", b"ignored", Origin::File, &source(9))
            .unwrap();
        assert_eq!(store.get(b"LIST").unwrap().raw_value(), b"cli");
        assert_eq!(store.get(b"LIST").unwrap().definition(), None);
    }

    #[test]
    fn pure_wrappers_reject_observable_output_but_skip_unselected_branches() {
        let mut store = VariableStore::default();
        assert_eq!(
            store.expand(b"$(info x)"),
            Err(ExpandError::Unsupported(b"info".to_vec()))
        );
        assert_eq!(
            store.expand(b"$(if yes,ok,$(warning skipped))").unwrap(),
            b"ok"
        );
        assert_eq!(store.expand(b"$(call warning)").unwrap(), b"");
        assert_eq!(
            store.define(b"X", b"$(warning x)", Assignment::Simple, Origin::File),
            Err(ExpandError::Unsupported(b"warning".to_vec()))
        );
    }

    #[test]
    fn filesystem_effects_preserve_raw_paths_and_distinguish_omitted_text() {
        let store = VariableStore::new();
        let mut operations = Vec::new();
        let mut handler = |effect: FilesystemEffect<'_>| match effect {
            FilesystemEffect::WriteFile { path, text, append } => {
                operations.push((path.to_vec(), text.map(Vec::from), append));
                Ok(Vec::new())
            }
            _ => panic!("unexpected filesystem operation"),
        };
        let result = store
            .expand_with(
                b"$(file >a\xff ignored)$(file >>b,)$(file >c,$$(literal),tail)",
                ExpansionContext::pure().with_filesystem(&mut handler),
            )
            .unwrap();
        assert!(result.is_empty());
        assert_eq!(
            operations,
            [
                (b"a\xff".to_vec(), None, false),
                (b"b".to_vec(), Some(Vec::new()), true),
                (b"c".to_vec(), Some(b"$(literal),tail".to_vec()), false),
            ]
        );
    }

    #[test]
    fn filesystem_tokens_include_vertical_tab_separators() {
        let store = VariableStore::new();
        let mut paths = Vec::new();
        let mut handler = |effect: FilesystemEffect<'_>| {
            match effect {
                FilesystemEffect::ReadFile(path) => paths.push(path.to_vec()),
                FilesystemEffect::WriteFile { path, .. } => paths.push(path.to_vec()),
                FilesystemEffect::Realpath(path) => paths.push(path.to_vec()),
                FilesystemEffect::Wildcard(_)
                | FilesystemEffect::ArchiveGlob(_)
                | FilesystemEffect::RuleGlob(_)
                | FilesystemEffect::Tilde(_) => {
                    panic!("unexpected wildcard")
                }
            }
            Ok(Vec::new())
        };
        let result = store
            .expand_with(
                b"$(file <\x0bin\x0bignored)$(file >\x0bout\x0bignored,text)$(realpath a\x0bb)",
                ExpansionContext::pure().with_filesystem(&mut handler),
            )
            .unwrap();
        assert!(result.is_empty());
        assert_eq!(
            paths,
            [
                b"in".to_vec(),
                b"out".to_vec(),
                b"a".to_vec(),
                b"b".to_vec()
            ]
        );
    }

    #[test]
    fn filesystem_validation_runs_after_all_argument_effects() {
        let store = VariableStore::new();
        assert_eq!(
            store.expand(b"$(call file)"),
            Err(ExpandError::InsufficientArguments {
                function: b"file".to_vec(),
                count: 0,
            })
        );
        for (input, expected) in [
            (
                b"$(file >,$(info rhs))".as_slice(),
                b"file: missing filename".as_slice(),
            ),
            (
                b"$(file invalid,$(info rhs))".as_slice(),
                b"file: invalid file operation: invalid".as_slice(),
            ),
            (
                b"$(file <in,$(info rhs))".as_slice(),
                b"file: too many arguments".as_slice(),
            ),
        ] {
            let mut outputs = Vec::new();
            let mut calls = 0;
            let mut handler = |_effect: FilesystemEffect<'_>| {
                calls += 1;
                Ok(Vec::new())
            };
            let error = store
                .expand_with(
                    input,
                    ExpansionContext::new(None, &mut |output| {
                        if let Output::Info(message) = output {
                            outputs.push(message.to_vec());
                        }
                    })
                    .with_filesystem(&mut handler),
                )
                .unwrap_err();
            assert_eq!(error.diagnostic().as_bytes(), expected);
            assert_eq!(outputs, [b"rhs".to_vec()]);
            assert_eq!(calls, 0);
        }
    }

    #[test]
    fn filesystem_io_errors_use_active_context_while_usage_uses_binding_source() {
        let mut store = VariableStore::new();
        store
            .define_at(
                b"F",
                b"$(file >missing/out,x)",
                Assignment::Recursive,
                Origin::File,
                &source(1),
            )
            .unwrap();
        for active in [None, Some(source(4))] {
            let mut handler = |_effect: FilesystemEffect<'_>| {
                Err(b"open: missing/out: No such file or directory"
                    .to_vec()
                    .into())
            };
            let error = store
                .expand_with(
                    b"$(F)",
                    ExpansionContext::new(active.as_ref(), &mut |_| {})
                        .with_filesystem(&mut handler),
                )
                .unwrap_err();
            assert_eq!(error.diagnostic_location(active.as_ref()), active.as_ref());
        }
        store
            .define_at(
                b"F",
                b"$(file >,x)",
                Assignment::Recursive,
                Origin::File,
                &source(1),
            )
            .unwrap();
        let active = source(4);
        let error = store
            .expand_with(b"$(F)", ExpansionContext::new(Some(&active), &mut |_| {}))
            .unwrap_err();
        assert_eq!(error.diagnostic_location(Some(&active)), Some(&source(1)));
    }

    #[test]
    fn realpath_joins_successes_preserves_duplicates_and_accepts_empty_call() {
        let store = VariableStore::new();
        assert_eq!(
            store.expand(b"$(call realpath)|$(realpath )").unwrap(),
            b"|"
        );
        let mut paths = Vec::new();
        let mut handler = |effect: FilesystemEffect<'_>| {
            let FilesystemEffect::Realpath(path) = effect else {
                panic!("wrong operation")
            };
            paths.push(path.to_vec());
            Ok(if path == b"missing" {
                Vec::new()
            } else {
                [b"/root/".as_slice(), path].concat()
            })
        };
        assert_eq!(
            store
                .expand_with(
                    b"$(realpath a missing a\xff a)",
                    ExpansionContext::pure().with_filesystem(&mut handler)
                )
                .unwrap(),
            b"/root/a /root/a\xff /root/a"
        );
        assert_eq!(
            paths,
            [
                b"a".to_vec(),
                b"missing".to_vec(),
                b"a\xff".to_vec(),
                b"a".to_vec()
            ]
        );
        assert!(
            matches!(store.expand(b"$(file >out,text)"), Err(ExpandError::Unsupported(name)) if name == b"file")
        );
    }

    #[test]
    fn file_reads_retain_nul_until_each_expansion_consumer_finishes() {
        let mut store = VariableStore::new();
        store
            .define(
                b"F",
                b"before$(file <data)after",
                Assignment::Recursive,
                Origin::File,
            )
            .unwrap();
        let mut handler = |effect: FilesystemEffect<'_>| {
            assert!(matches!(effect, FilesystemEffect::ReadFile(b"data")));
            Ok(b"a\0b".to_vec())
        };
        let mut messages = Vec::new();
        assert_eq!(
            store
                .expand_with(
                    b"$(info before$(file <data)$(info suffix)after)",
                    ExpansionContext::new(None, &mut |output| {
                        if let Output::Info(message) = output {
                            messages.push(message.to_vec());
                        }
                    })
                    .with_filesystem(&mut handler)
                )
                .unwrap(),
            b""
        );
        assert_eq!(messages, [b"suffix".to_vec(), b"beforea".to_vec()]);
        for (input, expected) in [
            (b"$(F):tail".as_slice(), b"beforea:tail".as_slice()),
            (
                b"$(subst a,z,before$(file <data)after):tail".as_slice(),
                b"beforez:tail".as_slice(),
            ),
            (
                b"$(if yes,$(file <data)after)outside".as_slice(),
                b"aoutside".as_slice(),
            ),
        ] {
            assert_eq!(
                store
                    .expand_with(
                        input,
                        ExpansionContext::pure().with_filesystem(&mut handler)
                    )
                    .unwrap(),
                expected
            );
        }
        assert_eq!(
            store.expand(b"left\0$(error must-not-expand)").unwrap(),
            b"left"
        );
    }

    #[test]
    fn unimplemented_effects_are_explicitly_unsupported() {
        for name in ["shell", "eval", "wildcard", "realpath", "guile"] {
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

    fn scoped_define(
        scope: &mut TargetScope,
        globals: &mut VariableStore,
        name: &[u8],
        value: &[u8],
        assignment: Assignment,
    ) {
        scope
            .define_with(
                globals,
                name,
                value,
                assignment,
                Origin::File,
                false,
                None,
                Some(&source(4)),
                ExpansionContext::pure(),
            )
            .unwrap();
    }

    #[test]
    fn target_context_layers_share_globals_and_isolate_automatic_mutations() {
        let mut globals = VariableStore::new();
        define(&mut globals, b"X", b"global", Assignment::Recursive);
        let first = globals.with_target_scopes(&[]);
        let mut second = first.with_target_scopes(&[]);
        assert!(Arc::ptr_eq(&globals.variables, &second.layers[0].variables));
        assert_eq!(second.layers.len(), 1);
        second
            .define_literal(b"@", b"target", Origin::Automatic)
            .unwrap();
        assert!(globals.get(b"@").is_none());
        assert!(first.get(b"@").is_none());
        assert_eq!(second.expand(b"$@:$(X)").unwrap(), b"target:global");
        assert_eq!(second.definitions().count(), 2);
    }

    #[test]
    fn target_immediate_values_use_local_parse_context_and_keep_literal_dollars() {
        let mut globals = VariableStore::new();
        define(&mut globals, b"K", b"global", Assignment::Recursive);
        let mut scope = TargetScope::default();
        scoped_define(
            &mut scope,
            &mut globals,
            b"K",
            b"local",
            Assignment::Recursive,
        );
        scoped_define(
            &mut scope,
            &mut globals,
            b"X",
            b"$(K)-$$K",
            Assignment::Simple,
        );
        scoped_define(
            &mut scope,
            &mut globals,
            b"K",
            b"later",
            Assignment::Recursive,
        );
        let context = globals.with_target_scopes(&[&scope]);
        assert_eq!(context.expand(b"$(X)|$(K)").unwrap(), b"local-$K|later");
        assert_eq!(globals.expand(b"$(K)|$(X)").unwrap(), b"global|");
    }

    #[test]
    fn scope_selection_precedes_origin_and_private_child_exposes_ancestor_again() {
        let mut globals = VariableStore::new();
        globals
            .define(b"X", b"global", Assignment::Recursive, Origin::Override)
            .unwrap();
        let mut parent = TargetScope::default();
        parent
            .define_with(
                &mut globals,
                b"X",
                b"parent",
                Assignment::Recursive,
                Origin::Override,
                false,
                None,
                None,
                ExpansionContext::pure(),
            )
            .unwrap();
        let mut child = TargetScope::default();
        child
            .define_with(
                &mut globals,
                b"X",
                b"child",
                Assignment::Recursive,
                Origin::File,
                true,
                None,
                None,
                ExpansionContext::pure(),
            )
            .unwrap();
        let parent_context = globals.with_target_scopes(&[&parent]);
        let child_context = parent_context.with_target_scopes(&[&child]);
        assert_eq!(
            child_context.expand(b"$(X) $(origin X)").unwrap(),
            b"child file"
        );
        let grandchild = child_context.with_target_scopes(&[]);
        assert_eq!(
            grandchild.expand(b"$(X) $(origin X)").unwrap(),
            b"parent override"
        );
        assert_eq!(
            grandchild
                .definitions()
                .filter(|(name, _)| *name == b"X")
                .count(),
            1
        );
    }

    #[test]
    fn inherited_append_keeps_each_suffix_and_expands_outer_in_current_context() {
        let mut globals = VariableStore::new();
        define(&mut globals, b"K", b"old", Assignment::Recursive);
        define(&mut globals, b"X", b"base-$(K)", Assignment::Recursive);
        let mut parent = TargetScope::default();
        scoped_define(
            &mut parent,
            &mut globals,
            b"X",
            b"parent-$(K)",
            Assignment::Append,
        );
        let mut child = TargetScope::default();
        scoped_define(
            &mut child,
            &mut globals,
            b"X",
            b"child-$(K)",
            Assignment::Append,
        );
        scoped_define(
            &mut child,
            &mut globals,
            b"K",
            b"child",
            Assignment::Recursive,
        );
        let context = globals
            .with_target_scopes(&[&parent])
            .with_target_scopes(&[&child]);
        assert_eq!(
            context.expand(b"$(X)|$(value X)|$(flavor X)").unwrap(),
            b"base-child parent-child child-child|child-$(K)|recursive"
        );
    }

    #[test]
    fn first_target_append_is_recursive_even_over_global_simple_and_preserves_empty_suffix() {
        let mut globals = VariableStore::new();
        define(&mut globals, b"K", b"old", Assignment::Recursive);
        define(&mut globals, b"X", b"base-$(K)", Assignment::Simple);
        let mut parent = TargetScope::default();
        scoped_define(&mut parent, &mut globals, b"X", b"", Assignment::Append);
        let mut child = TargetScope::default();
        scoped_define(
            &mut child,
            &mut globals,
            b"X",
            b"tail-$(K)",
            Assignment::Append,
        );
        define(&mut globals, b"K", b"new", Assignment::Recursive);
        let context = globals
            .with_target_scopes(&[&parent])
            .with_target_scopes(&[&child]);
        assert_eq!(
            context.expand(b"$(X)|$(value X)|$(flavor X)").unwrap(),
            b"base-old  tail-new|tail-$(K)|recursive"
        );
    }

    #[test]
    fn replacing_local_append_discards_its_outer_value() {
        let mut globals = VariableStore::new();
        define(&mut globals, b"X", b"global", Assignment::Recursive);
        let mut scope = TargetScope::default();
        scoped_define(
            &mut scope,
            &mut globals,
            b"X",
            b"append",
            Assignment::Append,
        );
        scoped_define(
            &mut scope,
            &mut globals,
            b"X",
            b"replacement",
            Assignment::Recursive,
        );
        assert_eq!(
            globals
                .with_target_scopes(&[&scope])
                .expand(b"$(X)")
                .unwrap(),
            b"replacement"
        );
    }

    #[test]
    fn losing_target_assignment_resets_append_metadata_except_for_conditional() {
        for assignment in [
            Assignment::Recursive,
            Assignment::Simple,
            Assignment::Conditional,
        ] {
            let mut globals = VariableStore::new();
            define(&mut globals, b"X", b"global", Assignment::Recursive);
            let mut scope = TargetScope::default();
            scope
                .define_with(
                    &mut globals,
                    b"X",
                    b"suffix",
                    Assignment::Append,
                    Origin::Override,
                    false,
                    None,
                    None,
                    ExpansionContext::pure(),
                )
                .unwrap();
            scoped_define(&mut scope, &mut globals, b"X", b"ignored", assignment);
            let expected = if assignment == Assignment::Conditional {
                b"global suffix|suffix".as_slice()
            } else {
                b"suffix|suffix".as_slice()
            };
            assert_eq!(
                globals
                    .with_target_scopes(&[&scope])
                    .expand(b"$(X)|$(value X)")
                    .unwrap(),
                expected
            );
        }
    }

    #[test]
    fn call_and_function_local_bindings_work_with_target_appends() {
        let mut globals = VariableStore::new();
        define(&mut globals, b"X", b"global-$(1)", Assignment::Recursive);
        let mut scope = TargetScope::default();
        scoped_define(
            &mut scope,
            &mut globals,
            b"X",
            b"local-$(1)",
            Assignment::Append,
        );
        let context = globals.with_target_scopes(&[&scope]);
        assert_eq!(
            context
                .expand(b"$(call X,arg)|$(foreach X,a b,$(X))")
                .unwrap(),
            b"global-arg local-arg|a b"
        );
    }

    #[test]
    fn target_append_detects_recursive_reference_and_preserves_provenance() {
        let mut globals = VariableStore::new();
        globals
            .define_at(
                b"X",
                b"$(X)",
                Assignment::Recursive,
                Origin::File,
                &source(2),
            )
            .unwrap();
        let mut scope = TargetScope::default();
        scoped_define(
            &mut scope,
            &mut globals,
            b"X",
            b"suffix",
            Assignment::Append,
        );
        let error = globals
            .with_target_scopes(&[&scope])
            .expand(b"$(X)")
            .unwrap_err();
        assert!(
            error
                .diagnostic()
                .to_string()
                .contains("Recursive variable 'X'")
        );
        assert_eq!(error.location(), Some(&source(4)));
    }

    #[test]
    fn target_command_line_snapshot_is_distinct_from_suppressed_conditional() {
        let mut globals = VariableStore::new();
        globals
            .define(b"X", b"cli", Assignment::Simple, Origin::CommandLine)
            .unwrap();
        let mut normal = TargetScope::default();
        scoped_define(
            &mut normal,
            &mut globals,
            b"X",
            b"ignored",
            Assignment::Recursive,
        );
        let mut conditional = TargetScope::default();
        scoped_define(
            &mut conditional,
            &mut globals,
            b"X",
            b"ignored",
            Assignment::Conditional,
        );
        globals
            .define(b"X", b"later", Assignment::Recursive, Origin::Override)
            .unwrap();
        assert_eq!(
            globals
                .with_target_scopes(&[&normal])
                .expand(b"$(X) $(origin X) $(flavor X)")
                .unwrap(),
            b"cli command line simple"
        );
        assert_eq!(
            globals
                .with_target_scopes(&[&conditional])
                .expand(b"$(X)")
                .unwrap(),
            b"later"
        );
        assert!(conditional.get(b"X").is_none());
    }

    #[test]
    fn target_first_append_does_not_evaluate_losing_suffix_for_simple_cli_binding() {
        let mut globals = VariableStore::new();
        globals
            .define(b"X", b"cli", Assignment::Simple, Origin::CommandLine)
            .unwrap();
        let mut scope = TargetScope::default();
        scoped_define(
            &mut scope,
            &mut globals,
            b"X",
            b"$(error unused)",
            Assignment::Append,
        );
        assert_eq!(
            globals
                .with_target_scopes(&[&scope])
                .expand(b"$(X)")
                .unwrap(),
            b"cli"
        );
    }

    #[test]
    fn suppressed_target_conditional_can_mark_global_private_without_copying_it() {
        let mut globals = VariableStore::new();
        define(&mut globals, b"X", b"global", Assignment::Recursive);
        let mut scope = TargetScope::default();
        scope
            .define_with(
                &mut globals,
                b"X",
                b"ignored",
                Assignment::Conditional,
                Origin::File,
                true,
                None,
                None,
                ExpansionContext::pure(),
            )
            .unwrap();
        assert!(scope.get(b"X").is_none());
        assert!(globals.get(b"X").unwrap().is_private());
        assert_eq!(globals.expand(b"$(X)").unwrap(), b"global");
        assert_eq!(
            globals
                .with_target_scopes(&[&scope])
                .expand(b"$(X)")
                .unwrap(),
            b""
        );
        define(&mut globals, b"X", b"later", Assignment::Recursive);
        assert!(globals.get(b"X").unwrap().is_private());
        scoped_define(
            &mut scope,
            &mut globals,
            b"X",
            b"created",
            Assignment::Conditional,
        );
        assert_eq!(
            globals
                .with_target_scopes(&[&scope])
                .expand(b"$(X)")
                .unwrap(),
            b"created"
        );
    }

    #[test]
    fn unqualified_losing_target_assignment_resets_privacy_but_keeps_export_policy() {
        let mut globals = VariableStore::new();
        let mut scope = TargetScope::default();
        scope
            .define_with(
                &mut globals,
                b"X",
                b"first",
                Assignment::Recursive,
                Origin::Override,
                true,
                Some(false),
                None,
                ExpansionContext::pure(),
            )
            .unwrap();
        scoped_define(
            &mut scope,
            &mut globals,
            b"X",
            b"ignored",
            Assignment::Conditional,
        );
        let child = globals
            .with_target_scopes(&[&scope])
            .with_target_scopes(&[]);
        assert_eq!(child.expand(b"$(X)").unwrap(), b"first");
        assert_eq!(child.export_policy(b"X"), Some(false));
    }

    #[test]
    fn scoped_export_policies_inherit_independently_from_selected_values() {
        let mut globals = VariableStore::new();
        let mut parent = TargetScope::default();
        parent
            .define_with(
                &mut globals,
                b"X",
                b"parent",
                Assignment::Recursive,
                Origin::File,
                false,
                Some(false),
                None,
                ExpansionContext::pure(),
            )
            .unwrap();
        let mut child = TargetScope::default();
        scoped_define(
            &mut child,
            &mut globals,
            b"X",
            b"child",
            Assignment::Recursive,
        );
        let context = globals
            .with_target_scopes(&[&parent])
            .with_target_scopes(&[&child]);
        assert_eq!(context.expand(b"$(X)").unwrap(), b"child");
        assert_eq!(context.export_policy(b"X"), Some(false));
        child
            .define_with(
                &mut globals,
                b"X",
                b"private",
                Assignment::Recursive,
                Origin::File,
                true,
                Some(true),
                None,
                ExpansionContext::pure(),
            )
            .unwrap();
        let context = globals
            .with_target_scopes(&[&parent])
            .with_target_scopes(&[&child]);
        assert_eq!(context.export_policy(b"X"), Some(true));
        assert_eq!(
            context.with_target_scopes(&[]).export_policy(b"X"),
            Some(false)
        );
    }

    #[test]
    fn environment_override_promotes_only_on_global_collision_after_rhs_evaluation() {
        let mut globals = VariableStore::new();
        globals.set_environment_override(true);
        globals
            .define(b"X", b"env", Assignment::Recursive, Origin::Environment)
            .unwrap();
        let mut early = TargetScope::default();
        scoped_define(
            &mut early,
            &mut globals,
            b"X",
            b"local",
            Assignment::Recursive,
        );
        define(&mut globals, b"X", b"unused", Assignment::Conditional);
        assert_eq!(globals.get(b"X").unwrap().origin(), Origin::Environment);
        let mut outputs = Vec::new();
        globals
            .define_with(
                b"X",
                b"$(info $(origin X))ignored",
                Assignment::Simple,
                Origin::File,
                None,
                ExpansionContext::new(None, &mut |output| {
                    if let Output::Info(bytes) = output {
                        outputs.push(bytes.to_vec());
                    }
                }),
            )
            .unwrap();
        assert_eq!(outputs, vec![b"environment".to_vec()]);
        assert_eq!(
            globals.get(b"X").unwrap().origin(),
            Origin::EnvironmentOverride
        );
        let mut late = TargetScope::default();
        scoped_define(
            &mut late,
            &mut globals,
            b"X",
            b"ignored",
            Assignment::Recursive,
        );
        assert_eq!(
            globals
                .with_target_scopes(&[&early])
                .expand(b"$(X) $(origin X)")
                .unwrap(),
            b"local file"
        );
        assert_eq!(
            globals
                .with_target_scopes(&[&late])
                .expand(b"$(X) $(origin X)")
                .unwrap(),
            b"env environment override"
        );
    }

    #[test]
    fn literal_initialization_and_metadata_append_promote_environment_collisions() {
        for origin in [Origin::Default, Origin::File] {
            let mut globals = VariableStore::new();
            globals.set_environment_override(true);
            globals
                .define(b"X", b"env$Y", Assignment::Recursive, Origin::Environment)
                .unwrap();
            globals.define_literal(b"X", b"default", origin).unwrap();
            let variable = globals.get(b"X").unwrap();
            assert_eq!(variable.raw_value(), b"env$Y");
            assert_eq!(variable.origin(), Origin::EnvironmentOverride);
            assert_eq!(variable.flavor(), Flavor::Recursive);
        }
        let mut globals = VariableStore::new();
        globals.set_environment_override(true);
        globals
            .define(b"X", b"env$Y", Assignment::Recursive, Origin::Environment)
            .unwrap();
        globals
            .append_literal_at(b"X", b"file", Origin::File, &source(1))
            .unwrap();
        assert_eq!(globals.get(b"X").unwrap().raw_value(), b"env$Y");
        assert_eq!(
            globals.get(b"X").unwrap().origin(),
            Origin::EnvironmentOverride
        );
        assert_eq!(globals.get(b"X").unwrap().definition(), None);
    }

    #[test]
    fn environment_origin_survives_empty_append_but_undefine_promotes_it() {
        let mut globals = VariableStore::new();
        globals.set_environment_override(true);
        globals
            .define(b"X", b"env", Assignment::Recursive, Origin::Environment)
            .unwrap();
        define(&mut globals, b"X", b"", Assignment::Append);
        assert_eq!(globals.get(b"X").unwrap().origin(), Origin::Environment);
        assert!(!globals.undefine(b"X", Origin::File));
        assert_eq!(
            globals.get(b"X").unwrap().origin(),
            Origin::EnvironmentOverride
        );
        assert_eq!(globals.get(b"X").unwrap().raw_value(), b"env");
        assert!(globals.undefine(b"X", Origin::Override));
        assert!(globals.get(b"X").is_none());
    }

    #[test]
    fn global_export_directive_replaces_policy_aliased_by_target_conditional() {
        let mut globals = VariableStore::new();
        define(&mut globals, b"X", b"global", Assignment::Recursive);
        let mut scope = TargetScope::default();
        scope
            .define_with(
                &mut globals,
                b"X",
                b"ignored",
                Assignment::Conditional,
                Origin::File,
                false,
                Some(true),
                None,
                ExpansionContext::pure(),
            )
            .unwrap();
        assert_eq!(globals.export_policy(b"X"), Some(true));
        globals.set_export_policy(b"X", Some(false));
        assert_eq!(
            globals.with_target_scopes(&[&scope]).export_policy(b"X"),
            Some(false)
        );
    }

    fn prepare_pattern(
        globals: &VariableStore,
        name: &[u8],
        value: &[u8],
        assignment: Assignment,
    ) -> PatternVariable {
        PatternVariable::prepare_with(
            globals,
            name,
            value,
            assignment,
            Origin::File,
            false,
            None,
            Some(&source(5)),
            ExpansionContext::pure(),
        )
        .unwrap()
    }

    fn apply_pattern(
        scope: &mut TargetScope,
        globals: &mut VariableStore,
        variable: &PatternVariable,
    ) {
        scope
            .apply_pattern_with(globals, variable, ExpansionContext::pure())
            .unwrap();
    }

    #[test]
    fn pattern_simple_prepares_immediately_but_immediate_recursive_waits_for_activation() {
        let mut globals = VariableStore::new();
        define(&mut globals, b"K", b"early", Assignment::Recursive);
        let simple = prepare_pattern(&globals, b"S", b"$(K)-$$X", Assignment::Simple);
        let deferred = prepare_pattern(&globals, b"D", b"$(K)-$$X", Assignment::ImmediateRecursive);
        define(&mut globals, b"K", b"late", Assignment::Recursive);
        let mut scope = TargetScope::default();
        apply_pattern(&mut scope, &mut globals, &simple);
        apply_pattern(&mut scope, &mut globals, &deferred);
        assert_eq!(
            globals
                .with_target_scopes(&[&scope])
                .expand(b"$(S)|$(D)|$(value D)")
                .unwrap(),
            b"early-$X|late-$X|late-$$X"
        );
    }

    #[test]
    fn pattern_append_uses_accumulated_pattern_values_and_final_globals() {
        let mut globals = VariableStore::new();
        define(&mut globals, b"K", b"early", Assignment::Recursive);
        let simple = prepare_pattern(&globals, b"X", b"base-$(K)", Assignment::Simple);
        let local = prepare_pattern(&globals, b"N", b"pattern", Assignment::Recursive);
        let append = prepare_pattern(&globals, b"X", b"tail-$(K)-$(N)-$@", Assignment::Append);
        define(&mut globals, b"K", b"late", Assignment::Recursive);
        let mut scope = TargetScope::default();
        for declaration in [&simple, &local, &append] {
            apply_pattern(&mut scope, &mut globals, declaration);
        }
        let mut exact = TargetScope::default();
        scoped_define(
            &mut exact,
            &mut globals,
            b"K",
            b"exact",
            Assignment::Recursive,
        );
        let mut context = globals.with_target_scopes(&[&scope, &exact]);
        context
            .define_literal(b"@", b"target", Origin::Automatic)
            .unwrap();
        assert_eq!(
            context.expand(b"$(X)|$(flavor X)").unwrap(),
            b"base-early tail-late-pattern-|simple"
        );
    }

    #[test]
    fn pattern_conditionals_use_activation_globals_and_alias_metadata() {
        let mut globals = VariableStore::new();
        let mut prepared = prepare_pattern(&globals, b"X", b"pattern", Assignment::Conditional);
        prepared.private = true;
        define(&mut globals, b"X", b"late", Assignment::Recursive);
        globals.set_export_policy(b"X", Some(true));
        let mut parent = globals.with_target_scopes(&[]);
        let mut scope = TargetScope::default();
        assert!(
            scope
                .apply_pattern_with(&mut globals, &prepared, ExpansionContext::pure())
                .unwrap()
        );
        assert!(scope.get(b"X").is_none());
        assert!(globals.get(b"X").unwrap().is_private());
        assert_eq!(globals.export_policy(b"X"), None);
        parent.refresh_globals(&globals);
        assert_eq!(parent.expand(b"$(X)").unwrap(), b"");
        prepared.private = false;
        assert!(
            scope
                .apply_pattern_with(&mut globals, &prepared, ExpansionContext::pure())
                .unwrap()
        );
        parent.refresh_globals(&globals);
        assert_eq!(parent.expand(b"$(X)").unwrap(), b"late");
    }

    #[test]
    fn pattern_global_alias_marks_default_export_reset_and_explicit_policy_clears_it() {
        let mut globals = VariableStore::new();
        globals
            .define(b"X", b"env", Assignment::Recursive, Origin::Environment)
            .unwrap();
        define(&mut globals, b"X", b"file", Assignment::Recursive);
        assert!(!globals.get(b"X").unwrap().export_default_reset());
        let mut prepared = prepare_pattern(&globals, b"X", b"unused", Assignment::Conditional);
        let mut scope = TargetScope::default();
        assert!(
            scope
                .apply_pattern_with(&mut globals, &prepared, ExpansionContext::pure())
                .unwrap()
        );
        assert!(globals.get(b"X").unwrap().export_default_reset());
        assert_eq!(globals.export_policy(b"X"), None);
        for policy in [true, false] {
            prepared.export = Some(policy);
            assert!(
                scope
                    .apply_pattern_with(&mut globals, &prepared, ExpansionContext::pure())
                    .unwrap()
            );
            assert!(!globals.get(b"X").unwrap().export_default_reset());
            assert_eq!(globals.export_policy(b"X"), Some(policy));
        }
        globals.set_export_policy(b"X", None);
        assert!(globals.get(b"X").unwrap().export_default_reset());
        globals.set_export_policy(b"X", Some(true));
        assert!(!globals.get(b"X").unwrap().export_default_reset());
    }

    #[test]
    fn local_pattern_policy_reset_does_not_mark_global_export_reset() {
        let mut globals = VariableStore::new();
        globals
            .define(b"X", b"env", Assignment::Recursive, Origin::Environment)
            .unwrap();
        let prepared = prepare_pattern(&globals, b"X", b"pattern", Assignment::Recursive);
        let mut scope = TargetScope::default();
        apply_pattern(&mut scope, &mut globals, &prepared);
        assert!(!globals.get(b"X").unwrap().export_default_reset());
        assert!(!scope.get(b"X").unwrap().export_default_reset());
    }

    #[test]
    fn pattern_conditional_can_create_after_global_definition_removed() {
        let mut globals = VariableStore::new();
        define(&mut globals, b"X", b"old", Assignment::Recursive);
        let prepared = prepare_pattern(&globals, b"X", b"pattern", Assignment::Conditional);
        globals.remove(b"X");
        let mut scope = TargetScope::default();
        apply_pattern(&mut scope, &mut globals, &prepared);
        assert_eq!(
            globals
                .with_target_scopes(&[&scope])
                .expand(b"$(X)")
                .unwrap(),
            b"pattern"
        );
    }

    #[test]
    fn pattern_cli_snapshot_keeps_original_operator_instead_of_cli_flavor() {
        for (assignment, expected) in [
            (Assignment::Recursive, b"cli-new|recursive".as_slice()),
            (Assignment::Simple, b"cli-$(K)|simple".as_slice()),
            (
                Assignment::ImmediateRecursive,
                b"cli-new|recursive".as_slice(),
            ),
            (Assignment::Conditional, b"later|recursive".as_slice()),
            (Assignment::Append, b"later cli-new|recursive".as_slice()),
        ] {
            let mut globals = VariableStore::new();
            globals
                .define(
                    b"X",
                    b"cli-$(K)",
                    Assignment::Recursive,
                    Origin::CommandLine,
                )
                .unwrap();
            let prepared = prepare_pattern(&globals, b"X", b"pattern", assignment);
            globals
                .define(b"X", b"later", Assignment::Recursive, Origin::Override)
                .unwrap();
            define(&mut globals, b"K", b"new", Assignment::Recursive);
            let mut scope = TargetScope::default();
            apply_pattern(&mut scope, &mut globals, &prepared);
            assert_eq!(
                globals
                    .with_target_scopes(&[&scope])
                    .expand(b"$(X)|$(flavor X)")
                    .unwrap(),
                expected
            );
        }
    }

    #[test]
    fn inherited_append_chain_honors_retained_marker_on_simple_pattern_binding() {
        let mut globals = VariableStore::new();
        define(&mut globals, b"X", b"global", Assignment::Recursive);
        let first = prepare_pattern(&globals, b"X", b"first", Assignment::Append);
        let simple = prepare_pattern(&globals, b"X", b"simple-$$X", Assignment::Simple);
        let mut parent = TargetScope::default();
        apply_pattern(&mut parent, &mut globals, &first);
        let mut child = TargetScope::default();
        apply_pattern(&mut child, &mut globals, &first);
        apply_pattern(&mut child, &mut globals, &simple);
        let inherited = globals.with_target_scopes(&[&parent]);
        assert_eq!(
            inherited
                .with_target_scopes(&[&child])
                .expand(b"$(X)|$(flavor X)")
                .unwrap(),
            b"simple-$X|simple"
        );
        let mut exact = TargetScope::default();
        scoped_define(&mut exact, &mut globals, b"X", b"exact", Assignment::Append);
        assert_eq!(
            inherited
                .with_target_scopes(&[&child, &exact])
                .expand(b"$(X)|$(value X)")
                .unwrap(),
            b"global first simple-$X exact|exact"
        );
    }

    #[test]
    fn environment_append_restart_preserves_simple_pattern_inherited_prefixes() {
        let mut globals = VariableStore::new();
        define(&mut globals, b"X", b"global", Assignment::Recursive);
        let first = prepare_pattern(&globals, b"X", b"first", Assignment::Append);
        let mut simple = prepare_pattern(&globals, b"X", b"simple", Assignment::Simple);
        simple.private = true;
        let mut parent = TargetScope::default();
        apply_pattern(&mut parent, &mut globals, &first);
        let mut child = TargetScope::default();
        apply_pattern(&mut child, &mut globals, &first);
        apply_pattern(&mut child, &mut globals, &simple);
        let context = globals
            .with_target_scopes(&[&parent])
            .with_target_scopes(&[&child]);
        assert_eq!(context.expand(b"$(X)").unwrap(), b"simple");
        assert_eq!(
            context
                .expand_environment_variable_with(b"X", ExpansionContext::pure())
                .unwrap(),
            b"global first simple"
        );
    }

    #[test]
    fn pattern_global_override_does_not_block_new_local_simple_definition() {
        let mut globals = VariableStore::new();
        globals
            .define(b"X", b"global", Assignment::Recursive, Origin::Override)
            .unwrap();
        let prepared = prepare_pattern(&globals, b"X", b"pattern-$$X", Assignment::Simple);
        let mut scope = TargetScope::default();
        apply_pattern(&mut scope, &mut globals, &prepared);
        assert_eq!(
            globals
                .with_target_scopes(&[&scope])
                .expand(b"$(X)|$(origin X)")
                .unwrap(),
            b"pattern-$X|file"
        );
    }

    #[test]
    fn pattern_declarations_accumulate_in_one_scope_and_reset_unspecified_export() {
        let mut globals = VariableStore::new();
        define(&mut globals, b"X", b"global", Assignment::Recursive);
        let mut first = prepare_pattern(&globals, b"X", b"one", Assignment::Append);
        first.export = Some(true);
        let mut second = prepare_pattern(&globals, b"X", b"two", Assignment::Append);
        second.private = true;
        let mut scope = TargetScope::default();
        apply_pattern(&mut scope, &mut globals, &first);
        apply_pattern(&mut scope, &mut globals, &second);
        let context = globals.with_target_scopes(&[&scope]);
        assert_eq!(
            context.expand(b"$(X)|$(value X)").unwrap(),
            b"global one two|one two"
        );
        assert_eq!(scope.get(b"X").unwrap().export_policy(), None);
        assert_eq!(
            context.with_target_scopes(&[]).expand(b"$(X)").unwrap(),
            b"global"
        );
    }

    #[test]
    fn private_pattern_environment_selects_outer_value_and_expands_in_full_context() {
        let mut globals = VariableStore::new();
        define(&mut globals, b"X", b"global-$(Z)", Assignment::Recursive);
        define(&mut globals, b"Y", b"$(X)", Assignment::Recursive);
        define(&mut globals, b"Z", b"globalz", Assignment::Recursive);
        let mut pattern_x = prepare_pattern(&globals, b"X", b"pattern", Assignment::Recursive);
        pattern_x.private = true;
        pattern_x.export = Some(true);
        let pattern_z = prepare_pattern(&globals, b"Z", b"targetz", Assignment::Recursive);
        let mut scope = TargetScope::default();
        apply_pattern(&mut scope, &mut globals, &pattern_x);
        apply_pattern(&mut scope, &mut globals, &pattern_z);
        let context = globals.with_target_scopes(&[&scope]);
        assert_eq!(context.expand(b"$(X)|$(Y)").unwrap(), b"pattern|pattern");
        assert_eq!(
            context.for_environment().get(b"X").unwrap().raw_value(),
            b"global-$(Z)"
        );
        assert_eq!(
            context
                .expand_environment_variable_with(b"X", ExpansionContext::pure())
                .unwrap(),
            b"global-targetz"
        );
        assert_eq!(
            context
                .expand_environment_variable_with(b"Y", ExpansionContext::pure())
                .unwrap(),
            b"pattern"
        );
    }

    #[test]
    fn exported_inherited_append_restarts_lookup_with_private_pattern_visible() {
        let mut globals = VariableStore::new();
        define(&mut globals, b"X", b"global", Assignment::Recursive);
        let mut ancestor = TargetScope::default();
        scoped_define(
            &mut ancestor,
            &mut globals,
            b"X",
            b"ancestor",
            Assignment::Append,
        );
        let mut prepared = prepare_pattern(&globals, b"X", b"pattern", Assignment::Recursive);
        prepared.private = true;
        let mut pattern = TargetScope::default();
        apply_pattern(&mut pattern, &mut globals, &prepared);
        let context = globals
            .with_target_scopes(&[&ancestor])
            .with_target_scopes(&[&pattern]);
        assert_eq!(
            context.for_environment().get(b"X").unwrap().raw_value(),
            b"ancestor"
        );
        assert_eq!(
            context
                .expand_environment_variable_with(b"X", ExpansionContext::pure())
                .unwrap(),
            b"pattern"
        );
    }

    #[test]
    fn environment_policy_resolution_persists_on_shared_binding_but_not_value_copies() {
        for policy in [true, false] {
            let mut globals = VariableStore::new();
            define(&mut globals, b"X", b"global", Assignment::Recursive);
            globals.set_export_policy(b"X", Some(policy));
            let mut parent = TargetScope::default();
            scoped_define(
                &mut parent,
                &mut globals,
                b"X",
                b"parent",
                Assignment::Recursive,
            );
            let copied_binding = parent.get(b"X").unwrap().clone();
            let mut context = globals.with_target_scopes(&[&parent]);
            assert_eq!(parent.get(b"X").unwrap().export_policy(), None);
            assert_eq!(
                context.for_environment().resolve_export_policy(b"X"),
                Some(policy)
            );
            assert_eq!(parent.get(b"X").unwrap().export_policy(), Some(policy));
            assert_eq!(copied_binding.export_policy(), None);
            globals.set_private(b"X", true);
            context.refresh_globals(&globals);
            assert_eq!(
                context.for_environment().resolve_export_policy(b"X"),
                Some(policy)
            );
        }
    }

    #[test]
    fn environment_self_named_reference_can_resolve_distinct_private_pattern_binding() {
        let mut globals = VariableStore::new();
        define(&mut globals, b"X", b"outer-$(X)", Assignment::Recursive);
        let mut pattern = prepare_pattern(&globals, b"X", b"inner", Assignment::Recursive);
        pattern.private = true;
        let mut scope = TargetScope::default();
        apply_pattern(&mut scope, &mut globals, &pattern);
        let context = globals.with_target_scopes(&[&scope]);
        assert_eq!(
            context
                .expand_environment_variable_with(b"X", ExpansionContext::pure())
                .unwrap(),
            b"outer-inner"
        );
    }

    #[test]
    fn rebase_global_layer_preserves_explicit_scope_snapshots_and_empty_root_identity() {
        let mut globals = VariableStore::new();
        let mut scope = TargetScope::default();
        scoped_define(
            &mut scope,
            &mut globals,
            b"X",
            b"local",
            Assignment::Recursive,
        );
        let mut context = globals.with_target_scopes(&[&scope]);
        define(&mut globals, b"X", b"global", Assignment::Recursive);
        define(&mut globals, b"Y", b"new", Assignment::Recursive);
        context.refresh_globals(&globals);
        assert_eq!(context.expand(b"$(X)|$(Y)").unwrap(), b"local|new");
        globals.set_private(b"Y", true);
        context.refresh_globals(&globals);
        assert_eq!(context.expand(b"$(X)|$(Y)").unwrap(), b"local|");
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

#[cfg(test)]
mod environment_order_tests {
    use super::*;

    #[test]
    fn runtime_export_directive_changes_the_selected_scope_only() {
        let mut globals = VariableStore::new();
        globals
            .define_literal(b"C", b"global", Origin::File)
            .unwrap();
        globals.set_export_policy(b"C", Some(true));
        let mut target = TargetScope::default();
        target
            .define_with(
                &mut globals,
                b"C",
                b"local",
                Assignment::Recursive,
                Origin::File,
                false,
                Some(true),
                None,
                ExpansionContext::pure(),
            )
            .unwrap();
        let current = globals.with_target_scopes(&[&target]);
        assert!(current.set_scoped_export_policy(b"C", false));
        assert_eq!(current.resolve_export_policy(b"C"), Some(false));
        assert_eq!(globals.resolve_export_policy(b"C"), Some(true));
        assert!(!globals.set_scoped_export_policy(b"C", false));
        assert_eq!(globals.resolve_export_policy(b"C"), Some(true));
    }

    #[test]
    fn layout_reservations_do_not_define_or_resurrect_metadata() {
        let mut store = VariableStore::new();
        store.reserve_environment_layout(&[b"MFLAGS"], true);
        assert!(store.get(b"MFLAGS").is_none());
        assert_eq!(store.definitions().count(), 0);
        assert_eq!(store.environment_order(|_, _| false), [b"MFLAGS".to_vec()]);
        store.remove(b"MFLAGS");
        assert!(store.environment_order(|_, _| false).is_empty());
        store
            .define_literal(b"MFLAGS", b"user", Origin::File)
            .unwrap();
        store.set_export_policy(b"MFLAGS", Some(false));
        assert!(store.environment_order(|_, _| false).is_empty());
        assert_eq!(store.get(b"MFLAGS").unwrap().raw_value(), b"user");
    }
}

#[cfg(test)]
mod variable_names_tests {
    use super::*;

    #[test]
    fn membership_cache_isolated_across_copy_on_write_snapshots() {
        let mut original = VariableStore::new();
        original.enable_variable_names().unwrap();
        original.define_literal(b"A", b"one", Origin::File).unwrap();
        let before = original.expand(b"$(.VARIABLES)").unwrap();
        let mut changed = original.clone();
        changed.define_literal(b"B", b"two", Origin::File).unwrap();
        assert_eq!(original.expand(b"$(.VARIABLES)").unwrap(), before);
        let members = changed.expand(b"$(.VARIABLES)").unwrap();
        assert!(words(&members).any(|word| word == b"B"));
        changed
            .define(
                b".VARIABLES",
                b"custom",
                Assignment::Recursive,
                Origin::File,
            )
            .unwrap();
        assert_eq!(changed.expand(b"$(.VARIABLES)").unwrap(), b"custom");
        assert_eq!(original.expand(b"$(.VARIABLES)").unwrap(), before);
    }

    #[test]
    fn recursive_metadata_effects_repeat_until_membership_invalidates_raw_value() {
        let mut variables = VariableStore::new();
        variables.enable_variable_names().unwrap();
        variables.expand(b"$(.VARIABLES)").unwrap();
        variables
            .define(
                b".VARIABLES",
                b"$(info effect)value",
                Assignment::Recursive,
                Origin::File,
            )
            .unwrap();
        let mut emitted = Vec::new();
        let mut output = |event: Output<'_>| {
            if let Output::Info(bytes) = event {
                emitted.push(bytes.to_vec());
            }
        };
        for _ in 0..2 {
            assert_eq!(
                variables
                    .expand_mut_with(b"$(.VARIABLES)", ExpansionContext::new(None, &mut output))
                    .unwrap(),
                b"value"
            );
        }
        assert_eq!(emitted, [b"effect".to_vec(), b"effect".to_vec()]);
        variables.define_literal(b"NEW", b"", Origin::File).unwrap();
        assert!(words(&variables.expand(b"$(.VARIABLES)").unwrap()).any(|word| word == b"NEW"));
    }

    #[test]
    fn automatic_scope_names_are_excluded_from_global_metadata() {
        let mut globals = VariableStore::new();
        globals.enable_variable_names().unwrap();
        globals
            .define_literal(b"GLOBAL", b"", Origin::File)
            .unwrap();
        let mut scoped = globals.with_target_scopes(&[]);
        scoped
            .define_literal(b"LOCAL", b"", Origin::Automatic)
            .unwrap();
        let names = scoped.expand(b"$(.VARIABLES)").unwrap();
        assert!(words(&names).any(|word| word == b"GLOBAL"));
        assert!(!words(&names).any(|word| word == b"LOCAL"));
    }
}

#[cfg(test)]
mod runtime_metadata_scope_tests {
    use super::*;

    #[test]
    fn normalization_preserves_this_files_value_and_shadows_ancestors() {
        let mut globals = VariableStore::new();
        globals
            .define_literal(b"MFLAGS", b"global", Origin::Override)
            .unwrap();
        let mut target = TargetScope::default();
        target
            .define_with(
                &mut globals,
                b"MFLAGS",
                b"target",
                Assignment::Recursive,
                Origin::File,
                false,
                None,
                None,
                ExpansionContext::pure(),
            )
            .unwrap();
        let mut parent = globals.with_target_scopes(&[&target]);
        parent.update_mflags(b"s", false).unwrap();
        assert_eq!(
            parent.expand(b"$(origin MFLAGS)|$(MFLAGS)").unwrap(),
            b"file|target"
        );
        let mut child = parent.with_target_scopes(&[]);
        child.update_mflags(b"k", false).unwrap();
        assert_eq!(
            child.expand(b"$(origin MFLAGS)|$(MFLAGS)").unwrap(),
            b"environment|-k"
        );
        assert_eq!(
            parent.expand(b"$(origin MFLAGS)|$(MFLAGS)").unwrap(),
            b"file|target"
        );
        assert_eq!(
            globals.expand(b"$(origin MFLAGS)|$(MFLAGS)").unwrap(),
            b"override|global"
        );
    }
}
