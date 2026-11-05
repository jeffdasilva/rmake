// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Owned diagnostic text with lossless transport for Makefile bytes.

use std::fmt;

/// Source identity preserves the native ABI distinction between NULL and a
/// non-NULL pointer to an empty filename. Clones share named source bytes.
#[repr(transparent)]
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct SourceName(Option<std::sync::Arc<[u8]>>);

impl SourceName {
    pub const fn null() -> Self {
        Self(None)
    }

    pub fn is_null(&self) -> bool {
        self.0.is_none()
    }

    pub fn as_option(&self) -> Option<&[u8]> {
        self.0.as_deref()
    }
}

impl AsRef<[u8]> for SourceName {
    fn as_ref(&self) -> &[u8] {
        self.0.as_deref().unwrap_or_default()
    }
}

impl std::ops::Deref for SourceName {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        self.as_ref()
    }
}

impl From<std::sync::Arc<[u8]>> for SourceName {
    fn from(name: std::sync::Arc<[u8]>) -> Self {
        Self(Some(name))
    }
}
impl From<&[u8]> for SourceName {
    fn from(name: &[u8]) -> Self {
        Self(Some(name.into()))
    }
}
impl<const N: usize> From<&[u8; N]> for SourceName {
    fn from(name: &[u8; N]) -> Self {
        Self::from(name.as_slice())
    }
}
impl From<Vec<u8>> for SourceName {
    fn from(name: Vec<u8>) -> Self {
        Self(Some(name.into()))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Location {
    /// Immutable source spelling shared by locations from the same read.
    /// Sharing bytes does not cache file contents, lookup results or effects.
    pub file: SourceName,
    pub line: usize,
}

/// An output emitted synchronously while reading or expanding make syntax.
/// The receiver controls I/O; byte slices remain valid for this callback only.
#[derive(Clone, Copy, Debug)]
pub enum Output<'a> {
    Info(&'a [u8]),
    Warning {
        location: Option<&'a Location>,
        message: &'a [u8],
    },
    /// Typed system data remains distinct from user and foreign warning bytes.
    SystemWarning {
        location: Option<&'a Location>,
        message: crate::messages::WarningMessage<'a>,
    },
}

/// Borrowed reader observations; hosts choose whether and how to display them.
#[derive(Clone, Copy, Debug)]
pub enum ReaderEvent<'a> {
    Reading {
        name: &'a [u8],
        search: bool,
        optional: bool,
        no_default_goal: bool,
        no_tilde: bool,
    },
    Bom(&'a [u8]),
}

/// Output receivers can expose live diagnostic policy without growing every
/// expansion context. Ordinary closures retain the default quiet policy.
pub trait OutputReceiver {
    fn emit(&mut self, event: Output<'_>);
    fn reader_event(&mut self, _event: ReaderEvent<'_>) {}
    fn loader_event(&mut self, _event: crate::native::LoaderEvent<'_>) {}
    fn guile_callback(&mut self, _callback: crate::native::GuileCallback<'_>) {}
    /// An application may freeze a system message under the active locale/domain.
    /// None keeps the pure core's English identity behavior.
    fn format_system_message(
        &mut self,
        _message: crate::messages::MessageRef<'_>,
    ) -> Option<Result<Vec<u8>, Vec<u8>>> {
        None
    }

    fn warns_undefined(&self) -> bool {
        false
    }
}

impl<F: FnMut(Output<'_>)> OutputReceiver for F {
    fn emit(&mut self, event: Output<'_>) {
        self(event);
    }
}

pub struct WarningOutput<'a> {
    output: &'a mut dyn FnMut(Output<'_>),
    enabled: &'a std::cell::Cell<bool>,
}

impl<'a> WarningOutput<'a> {
    pub fn new(output: &'a mut dyn FnMut(Output<'_>), enabled: &'a std::cell::Cell<bool>) -> Self {
        Self { output, enabled }
    }
}

impl OutputReceiver for WarningOutput<'_> {
    fn emit(&mut self, event: Output<'_>) {
        (self.output)(event);
    }
    fn warns_undefined(&self) -> bool {
        self.enabled.get()
    }
}

/// A filesystem operation requested by expansion. Filenames and contents are
/// raw Linux bytes; the runtime owns filesystem access and error formatting.
#[derive(Clone, Copy, Debug)]
pub enum FilesystemEffect<'a> {
    Wildcard(&'a [u8]),
    /// Interned rule filenames differ from the wildcard function's archive
    /// results. Return NUL-separated complete names to preserve member blanks.
    ArchiveGlob(&'a [u8]),
    /// Rule search keeps unmatched patterns and transports names with NULs.
    RuleGlob(&'a [u8]),
    /// Include search keeps unmatched patterns and complete names, with no
    /// archive-member interpretation.
    IncludeGlob(&'a [u8]),
    /// Account/environment fallback; make's nonempty HOME is expanded in core.
    Tilde(&'a [u8]),
    /// Register a directory's cached identity/open state without enumerating
    /// its entries. VPATH construction performs this during actual reading.
    DirectoryName(&'a [u8]),
    Realpath(&'a [u8]),
    ReadFile(&'a [u8]),
    WriteFile {
        path: &'a [u8],
        text: Option<&'a [u8]>,
        append: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FilesystemError {
    Caller(Vec<u8>),
    Unlocated(Vec<u8>),
}

impl From<Vec<u8>> for FilesystemError {
    fn from(message: Vec<u8>) -> Self {
        Self::Caller(message)
    }
}

impl FilesystemError {
    pub fn into_message(self) -> Vec<u8> {
        match self {
            Self::Caller(message) | Self::Unlocated(message) => message,
        }
    }
}

type FilesystemReceiver<'a> =
    dyn FnMut(FilesystemEffect<'_>) -> Result<Vec<u8>, FilesystemError> + 'a;

/// A makefile read is distinct from $(file <...): an absent file is recorded
/// for the remake phase, and successful reads retain their resolved filename.
pub struct IncludedFile {
    pub name: Vec<u8>,
    pub input: crate::reader::Input,
}

/// Retain the failed search candidate and OS error number until remake reports
/// it. Host adapters can also supply arbitrary, untranslated detail bytes.
pub struct IncludeFailure {
    pub resolved_name: Option<Vec<u8>>,
    pub detail: IncludeError,
}

#[derive(Clone, Debug)]
pub enum IncludeError {
    Errno(i32),
    Bytes(Vec<u8>),
}

impl From<Vec<u8>> for IncludeFailure {
    fn from(detail: Vec<u8>) -> Self {
        Self {
            resolved_name: None,
            detail: IncludeError::Bytes(detail),
        }
    }
}

type IncludeReceiver<'a> = dyn FnMut(&[u8]) -> Result<IncludedFile, IncludeFailure> + 'a;

pub enum EnvironmentValue {
    Set(Vec<u8>),
    Remove,
    Original,
}

pub struct ShellRequest<'a> {
    pub command: &'a [u8],
    pub shell: &'a [u8],
    pub flags: &'a [u8],
    pub ifs: Option<&'a [u8]>,
    /// Ordered set/remove operations, or an original-process fallback for an
    /// active recursive export. Interpretation performs no implicit OS reads.
    pub environment: &'a [(Vec<u8>, EnvironmentValue)],
    pub original_shell: Option<&'a [u8]>,
}

pub struct ShellResult {
    pub stdout: Vec<u8>,
    pub status: i32,
}

type ShellReceiver<'a> = dyn FnMut(ShellRequest<'_>) -> Result<ShellResult, Vec<u8>> + 'a;

/// Global MAKEFLAGS assignment is an interpreter effect. Expand the value
/// before invoking the receiver, so nested eval can report its own assignment
/// without retaining a mutable borrow of host option state.
pub struct MakeflagsRequest<'a> {
    pub value: &'a [u8],
    pub origin: crate::variables::Origin,
}
pub struct FlagAssignment {
    pub argument: Vec<u8>,
    pub environment_override: bool,
    pub position: usize,
}

/// A nonfatal Make-owned option value error, emitted at the decode cursor.
#[derive(Clone, Copy, Debug)]
pub enum FlagNotice {
    EmptyOption(&'static [u8]),
    InvalidJobs,
}

#[derive(Clone, Copy, Debug)]
pub struct FlagNoticeAt {
    pub position: usize,
    pub notice: FlagNotice,
}

#[derive(Default)]
pub struct FlagReplay<'a> {
    pub warning_positions: &'a [usize],
    pub warning_entry: bool,
    pub notices: &'a [FlagNoticeAt],
}

/// Canonical flags selected after include metadata effects have completed.
pub struct MakeflagsMetadata {
    pub eval_reference: bool,
    pub value: Vec<u8>,
    pub origin: crate::variables::Origin,
    pub environment_override: bool,
}

pub struct MakeflagsResult {
    pub eval_reference: bool,
    pub value: Vec<u8>,
    pub origin: crate::variables::Origin,
    pub environment_override: bool,
    pub include_dirs: Vec<Vec<u8>>,
    /// Effectful search construction runs after the argument cursor is retired.
    pub prepare_includes: Option<Box<dyn IncludePreparation>>,
    /// Own the resolved search until all metadata effects succeed. Finalization
    /// publishes that search and selects current flags after any nested reset.
    /// Dropping or refreshing a result discards its unused transaction.
    pub publish_includes: Option<Box<dyn FnOnce() -> MakeflagsMetadata>>,
    pub assignments: Vec<FlagAssignment>,
    /// Input positions of warning-enabling options, excluding serialized
    /// preexisting flags. They share the definition stream's owned cursor.
    pub warning_positions: Vec<usize>,
    pub notices: Vec<FlagNoticeAt>,
    pub warning_entry: bool,
    pub next_argument: usize,
}

/// Owns search traversal without borrowing host state across HOME effects.
pub trait IncludePreparation {
    fn next(&mut self) -> Option<Vec<u8>>;
    fn directory(&mut self, value: Vec<u8>);
    fn finish(self: Box<Self>) -> PreparedIncludes;
}

pub struct PreparedIncludes {
    pub directories: Vec<Vec<u8>>,
    pub publish: Box<dyn FnOnce() -> MakeflagsMetadata>,
}

/// A resumable host decoder owns argv and returns before interpreter effects.
pub trait FlagDecoder {
    fn next(&mut self, cursor: usize) -> Result<Option<FlagStep>, Vec<u8>>;
    fn filename(&mut self, value: Vec<u8>) -> Result<(), Vec<u8>>;
    fn finish(self: Box<Self>) -> Result<MakeflagsResult, Vec<u8>>;
    fn terminal(&self) -> usize;
    fn requested_jobs(&self) -> Option<usize> {
        None
    }
}

pub struct DecodedFlags {
    pub result: MakeflagsResult,
    pub names: Vec<Vec<u8>>,
    pub jobs: Option<usize>,
}

pub struct FlagStep {
    pub position: usize,
    pub environment_override: bool,
    pub following_definition: bool,
    pub action: FlagAction,
    pub notice: Option<FlagNotice>,
}

pub enum FlagAction {
    Policy,
    Definition(Vec<u8>),
    Filename(Vec<u8>),
}

pub trait MakeflagsReceiver {
    fn begin(
        &mut self,
        _request: MakeflagsRequest<'_>,
        _passive: bool,
    ) -> Result<Option<Box<dyn FlagDecoder>>, Vec<u8>> {
        Ok(None)
    }
    fn sync_decoded_flags(&mut self) {}
    /// Validate effects selected by the completed live source, after its
    /// definitions and before publishing metadata or expanding include paths.
    fn complete_live_flags(&mut self) -> Result<(), Vec<u8>> {
        Ok(())
    }
    fn decode(&mut self, request: MakeflagsRequest<'_>) -> Result<MakeflagsResult, Vec<u8>>;
    /// Apply an ordered policy step without decoding or repeating I/O. Quiet
    /// frontends can retain the default; warning-aware hosts share their cell.
    fn set_warning_policy(&mut self, _enabled: bool) {}
    fn report_flag_notice(&mut self, _notice: FlagNotice) {}
}

impl<F: FnMut(MakeflagsRequest<'_>) -> Result<MakeflagsResult, Vec<u8>>> MakeflagsReceiver for F {
    fn decode(&mut self, request: MakeflagsRequest<'_>) -> Result<MakeflagsResult, Vec<u8>> {
        self(request)
    }
}

/// Host stack ownership for synchronous parser reentry. A runtime adapter
/// executes the callback exactly once, on the same thread, or rejects before
/// invoking it when additional stack storage cannot be reserved.
pub trait EvaluationStack {
    fn run(
        &self,
        action: &mut dyn FnMut() -> Result<(), crate::parser::ParseError>,
    ) -> Option<Result<(), crate::parser::ParseError>>;
}

/// Explicit caller context and output receiver. This is never stored in or
/// shared by cloned variable stores; each evaluation chooses its own receiver.
pub struct ExpansionContext<'a> {
    pub location: Option<&'a Location>,
    cancellation: Option<&'a std::sync::atomic::AtomicBool>,
    output: Option<&'a mut dyn OutputReceiver>,
    filesystem: Option<&'a mut FilesystemReceiver<'a>>,
    includes: Option<&'a mut IncludeReceiver<'a>>,
    shell: Option<&'a mut ShellReceiver<'a>>,
    makeflags: Option<&'a mut dyn MakeflagsReceiver>,
    pub(crate) evaluation_stack: Option<&'a dyn EvaluationStack>,
    native_runtime: Option<std::rc::Rc<dyn crate::native::NativeRuntime>>,
    makeflags_depth: usize,
    makeflags_terminal: usize,
    makeflags_next_is_definition: bool,
    makeflags_cursor: Option<std::rc::Rc<std::cell::Cell<usize>>>,
}

impl<'a> ExpansionContext<'a> {
    pub fn new<F: FnMut(Output<'_>)>(location: Option<&'a Location>, output: &'a mut F) -> Self {
        Self::with_receiver(location, output)
    }

    pub fn with_receiver(
        location: Option<&'a Location>,
        output: &'a mut dyn OutputReceiver,
    ) -> Self {
        Self {
            location,
            cancellation: None,
            output: Some(output),
            filesystem: None,
            includes: None,
            shell: None,
            makeflags: None,
            evaluation_stack: None,
            native_runtime: None,
            makeflags_depth: 0,
            makeflags_terminal: 0,
            makeflags_next_is_definition: false,
            makeflags_cursor: None,
        }
    }

    /// Pure evaluation rejects output functions instead of silently losing them.
    pub fn pure() -> Self {
        Self {
            location: None,
            cancellation: None,
            output: None,
            filesystem: None,
            includes: None,
            shell: None,
            makeflags: None,
            evaluation_stack: None,
            native_runtime: None,
            makeflags_depth: 0,
            makeflags_terminal: 0,
            makeflags_next_is_definition: false,
            makeflags_cursor: None,
        }
    }

    pub fn with_evaluation_stack(mut self, stack: Option<&'a dyn EvaluationStack>) -> Self {
        self.evaluation_stack = stack;
        self
    }

    pub fn with_native_runtime(
        mut self,
        runtime: std::rc::Rc<dyn crate::native::NativeRuntime>,
    ) -> Self {
        self.native_runtime = Some(runtime);
        self
    }

    pub fn native_runtime(&self) -> Option<std::rc::Rc<dyn crate::native::NativeRuntime>> {
        self.native_runtime.clone()
    }

    /// Classification returns an owned registration without cloning its runtime.
    pub fn lookup_native_function(
        &self,
        name: &[u8],
    ) -> Option<std::rc::Rc<crate::native::NativeFunction>> {
        self.native_runtime.as_ref()?.lookup(name)
    }

    pub fn with_makeflags(mut self, receiver: &'a mut dyn MakeflagsReceiver) -> Self {
        self.makeflags = Some(receiver);
        self
    }

    pub fn has_makeflags(&self) -> bool {
        self.makeflags.is_some()
    }

    pub fn update_makeflags(
        &mut self,
        database: &mut crate::parser::Database,
        origin: crate::variables::Origin,
        mut value: Vec<u8>,
        encoded_overrides: bool,
    ) -> Result<(), crate::variables::ExpandError> {
        use crate::variables::{Assignment, ExpandError};
        if self.makeflags.is_none() {
            return Ok(());
        }
        if self.is_cancelled() {
            return Err(ExpandError::Cancelled);
        }
        if self.makeflags_depth >= 128 {
            return Err(ExpandError::ExpansionLimit);
        }
        if encoded_overrides
            && let Some(boundary) = value.windows(4).position(|part| part == b" -- ")
        {
            value.truncate(boundary);
        }
        let has_input = value
            .iter()
            .any(|byte| !matches!(byte, b' ' | b'\t'..=b'\r'));
        let decoder = self
            .makeflags
            .as_mut()
            .expect("configured receiver")
            .begin(
                MakeflagsRequest {
                    value: &value,
                    origin,
                },
                false,
            )
            .map_err(ExpandError::UnlocatedError)?;
        let mut result = if let Some(decoder) = decoder {
            self.drive_flag_decoder(database, decoder, origin)?.result
        } else {
            let mut result = self
                .makeflags
                .as_mut()
                .expect("configured receiver")
                .decode(MakeflagsRequest {
                    value: &value,
                    origin,
                })
                .map_err(ExpandError::UnlocatedError)?;
            database
                .variables
                .set_environment_override(result.environment_override);
            if !result.assignments.is_empty()
                || !result.warning_positions.is_empty()
                || !result.notices.is_empty()
            {
                self.apply_flag_steps(
                    database,
                    &result.assignments,
                    origin,
                    result.next_argument,
                    FlagReplay {
                        warning_positions: &result.warning_positions,
                        warning_entry: result.warning_entry,
                        notices: &result.notices,
                    },
                )?;
                // Nested definitions can change switches, but refreshing does not
                // repeat the outer assignment stream or reset its owned cursor.
                if !result.assignments.is_empty() || !result.notices.is_empty() {
                    result = self
                        .makeflags
                        .as_mut()
                        .expect("configured receiver")
                        .decode(MakeflagsRequest { value: b"", origin })
                        .map_err(ExpandError::UnlocatedError)?;
                }
            } else if has_input && let Some(cursor) = &self.makeflags_cursor {
                // A nested single short-option word rewinds GNU's process-global
                // cursor into an already-applied outer assignment. Refuse the
                // recorded nonterminating class before replaying those effects.
                // The first outer definition can also leave short-option
                // state referring into a retired nested argument buffer; normal
                // exits in this class can activate unrequested switches.
                let token = value.trim_ascii();
                let retired_short_state = cursor.get() > 2
                    || (cursor.get() == 2
                        && self.makeflags_terminal > 3
                        && self.makeflags_next_is_definition);
                if self.makeflags_depth > 0
                    && self.makeflags_terminal > 2
                    && retired_short_state
                    && result.next_argument == 2
                    && token.starts_with(b"-")
                    && token.len() > 1
                    && !token.starts_with(b"--")
                {
                    return Err(ExpandError::Safety(crate::safety::SafetyError::NestedFlags));
                }
                cursor.set(result.next_argument);
            }
            result
        };
        if has_input {
            self.makeflags
                .as_mut()
                .expect("configured receiver")
                .complete_live_flags()
                .map_err(ExpandError::UnlocatedError)?;
        }
        if let Some(mut preparation) = result.prepare_includes.take() {
            // Argument-driving depth, terminal and following-definition state
            // have been restored. Search effects retain any enclosing frame,
            // but must not impersonate a completed decoder's argv scope.
            let mut bytes = 0usize;
            while let Some(raw) = preparation.next() {
                if self.is_cancelled() {
                    return Err(ExpandError::Cancelled);
                }
                bytes = bytes.saturating_add(raw.len().max(1));
                if bytes > crate::variables::MAX_OUTPUT {
                    return Err(ExpandError::ExpansionLimit);
                }
                let value = self.flag_tilde(database, &raw)?;
                bytes = bytes.saturating_add(value.len());
                if bytes > crate::variables::MAX_OUTPUT {
                    return Err(ExpandError::ExpansionLimit);
                }
                preparation.directory(value);
            }
            let prepared = preparation.finish();
            result.include_dirs = prepared.directories;
            result.publish_includes = Some(prepared.publish);
        }
        if let Some(history) = database.database_history() {
            for directory in &result.include_dirs {
                history.intern(directory);
            }
        }
        database.variables.define_literal(
            b".INCLUDE_DIRS",
            b"",
            crate::variables::Origin::Default,
        )?;
        for directory in result.include_dirs {
            database.define_variable_with(
                crate::parser::AssignmentSpec {
                    name: b".INCLUDE_DIRS",
                    value: &directory,
                    kind: Assignment::Append,
                },
                crate::variables::Origin::Default,
                None,
                self.reborrow(),
            )?;
        }
        if self.is_cancelled() {
            return Err(ExpandError::Cancelled);
        }
        if let Some(publish) = result.publish_includes {
            let metadata = publish();
            result.eval_reference = metadata.eval_reference;
            result.value = metadata.value;
            result.origin = metadata.origin;
            result.environment_override = metadata.environment_override;
        }
        database.update_mflags(&result.value, result.environment_override)?;
        if result.eval_reference {
            result.value.extend_from_slice(b" $(-*-eval-flags-*-)");
        }
        let overrides_name = database.command_overrides_name();
        let overrides_reference = database.command_overrides_reference();
        let variables = database
            .evaluation_scope
            .as_mut()
            .unwrap_or(&mut database.variables);
        if variables
            .database_lookup(overrides_name)
            .is_some_and(|variable| !variable.raw_value().is_empty())
        {
            result.value.extend_from_slice(overrides_reference);
        }
        variables.set_environment_override(result.environment_override);
        variables.define_runtime_metadata(b"MAKEFLAGS", &result.value, result.origin)?;
        Ok(())
    }

    /// Drive an owned decoder without retaining a host borrow across effects.
    pub fn drive_flag_decoder(
        &mut self,
        database: &mut crate::parser::Database,
        mut decoder: Box<dyn FlagDecoder>,
        origin: crate::variables::Origin,
    ) -> Result<DecodedFlags, crate::variables::ExpandError> {
        use crate::variables::{ExpandError, Origin};
        if self.makeflags_depth >= 128 {
            return Err(ExpandError::ExpansionLimit);
        }
        let terminal = decoder.terminal();
        let cursor = self
            .makeflags_cursor
            .get_or_insert_with(Default::default)
            .clone();
        if terminal > 1 {
            cursor.set(1);
        }
        let depth = self.makeflags_depth;
        self.makeflags_depth += 1;
        let old_terminal = std::mem::replace(&mut self.makeflags_terminal, terminal);
        let old_following = self.makeflags_next_is_definition;
        let driven = (|| {
            let mut names = Vec::new();
            for _ in 0..128_usize.saturating_mul(terminal.max(1)) {
                if self.is_cancelled() {
                    return Err(ExpandError::Cancelled);
                }
                let Some(step) = decoder
                    .next(cursor.get())
                    .map_err(ExpandError::UnlocatedError)?
                else {
                    let jobs = decoder.requested_jobs();
                    let result = decoder.finish().map_err(ExpandError::UnlocatedError)?;
                    cursor.set(cursor.get().max(result.next_argument));
                    self.makeflags
                        .as_mut()
                        .expect("configured receiver")
                        .sync_decoded_flags();
                    return Ok(DecodedFlags {
                        result,
                        names,
                        jobs,
                    });
                };
                cursor.set(step.position + 1);
                self.makeflags_next_is_definition = step.following_definition;
                self.makeflags
                    .as_mut()
                    .expect("configured receiver")
                    .sync_decoded_flags();
                database
                    .variables
                    .set_environment_override(step.environment_override);
                if let Some(notice) = step.notice {
                    self.makeflags
                        .as_mut()
                        .expect("configured receiver")
                        .report_flag_notice(notice);
                }
                match step.action {
                    FlagAction::Policy => {}
                    FlagAction::Definition(argument) => {
                        let effective =
                            if origin == Origin::Environment && step.environment_override {
                                Origin::EnvironmentOverride
                            } else {
                                origin
                            };
                        if let Some(name) =
                            database.define_argument_with(&argument, effective, self.reborrow())?
                        {
                            names.push(name);
                        }
                    }
                    FlagAction::Filename(raw) => {
                        let value = self.flag_filename(database, &raw)?;
                        decoder
                            .filename(value)
                            .map_err(ExpandError::UnlocatedError)?;
                        self.makeflags
                            .as_mut()
                            .expect("configured receiver")
                            .sync_decoded_flags();
                    }
                }
            }
            Err(ExpandError::Safety(crate::safety::SafetyError::NestedFlags))
        })();
        self.makeflags
            .as_mut()
            .expect("configured receiver")
            .sync_decoded_flags();
        self.makeflags_depth = depth;
        self.makeflags_terminal = old_terminal;
        self.makeflags_next_is_definition = old_following;
        driven
    }

    fn flag_filename(
        &mut self,
        database: &mut crate::parser::Database,
        raw: &[u8],
    ) -> Result<Vec<u8>, crate::variables::ExpandError> {
        let mut expanded = self.flag_tilde(database, raw)?;
        let prefix = expanded.len() - crate::filenames::without_dot_prefix(&expanded).len();
        expanded.drain(..prefix);
        Ok(expanded)
    }

    fn flag_tilde(
        &mut self,
        database: &mut crate::parser::Database,
        raw: &[u8],
    ) -> Result<Vec<u8>, crate::variables::ExpandError> {
        use crate::variables::ExpandError;
        let mut expanded = Vec::new();
        if raw.starts_with(b"~") {
            if raw.len() == 1 || raw[1] == b'/' {
                let warning = self.warns_undefined();
                self.makeflags
                    .as_mut()
                    .expect("configured receiver")
                    .set_warning_policy(false);
                let result = database.expand_with(b"$(HOME)", self.reborrow());
                self.makeflags
                    .as_mut()
                    .expect("configured receiver")
                    .set_warning_policy(warning);
                let mut home = result?;
                if let Some(end) = home.iter().position(|b| *b == 0) {
                    home.truncate(end);
                }
                if !home.is_empty() {
                    if home.len().saturating_add(raw.len() - 1) > crate::variables::MAX_OUTPUT {
                        return Err(ExpandError::ExpansionLimit);
                    }
                    expanded = [home.as_slice(), &raw[1..]].concat();
                }
            }
            if expanded.is_empty() {
                expanded = self
                    .perform_filesystem(FilesystemEffect::Tilde(raw))
                    .unwrap_or_else(|| Ok(Vec::new()))
                    .map_err(|e| match e {
                        FilesystemError::Caller(v) => ExpandError::FileError(v),
                        FilesystemError::Unlocated(v) => ExpandError::UnlocatedError(v),
                    })?;
            }
        }
        Ok(if expanded.is_empty() {
            raw.to_vec()
        } else {
            expanded
        })
    }

    /// Replay option definitions in argument order. A nested MAKEFLAGS decode
    /// shares GNU's observable argument cursor, but owns all input storage.
    pub fn apply_flag_assignments(
        &mut self,
        database: &mut crate::parser::Database,
        assignments: &[FlagAssignment],
        origin: crate::variables::Origin,
        terminal: usize,
    ) -> Result<Vec<Vec<u8>>, crate::variables::ExpandError> {
        self.apply_flag_steps(
            database,
            assignments,
            origin,
            terminal,
            FlagReplay::default(),
        )
    }

    /// Replay definitions and warning policies with the same owned cursor.
    /// Passive callers supply their policy before preliminary flag inspection.
    pub fn apply_flag_steps(
        &mut self,
        database: &mut crate::parser::Database,
        assignments: &[FlagAssignment],
        origin: crate::variables::Origin,
        terminal: usize,
        replay: FlagReplay<'_>,
    ) -> Result<Vec<Vec<u8>>, crate::variables::ExpandError> {
        use crate::variables::{ExpandError, Origin};
        let FlagReplay {
            warning_positions,
            warning_entry,
            notices,
        } = replay;
        if self.makeflags_depth >= 128 {
            return Err(ExpandError::ExpansionLimit);
        }
        if !warning_positions.is_empty() {
            self.makeflags
                .as_mut()
                .expect("configured policy receiver")
                .set_warning_policy(warning_entry);
        }
        let cursor = self
            .makeflags_cursor
            .get_or_insert_with(Default::default)
            .clone();
        cursor.set(1);
        let mut names = Vec::new();
        let depth = self.makeflags_depth;
        let previous_terminal = std::mem::replace(&mut self.makeflags_terminal, terminal);
        let previous_following = self.makeflags_next_is_definition;
        self.makeflags_depth += 1;
        let applied = (|| {
            for _ in 0..128_usize.saturating_mul(
                assignments
                    .len()
                    .saturating_add(warning_positions.len())
                    .saturating_add(notices.len())
                    .max(1),
            ) {
                let index =
                    assignments.partition_point(|assignment| assignment.position < cursor.get());
                let next_warning = warning_positions
                    .get(warning_positions.partition_point(|position| *position < cursor.get()));
                let next_notice =
                    notices.get(notices.partition_point(|entry| entry.position < cursor.get()));
                if let Some(entry) = next_notice
                    && next_warning.is_none_or(|position| entry.position < *position)
                    && assignments
                        .get(index)
                        .is_none_or(|assignment| entry.position < assignment.position)
                {
                    cursor.set(entry.position + 1);
                    self.makeflags
                        .as_mut()
                        .expect("configured receiver")
                        .report_flag_notice(entry.notice);
                    continue;
                }
                if let Some(position) = next_warning
                    && assignments
                        .get(index)
                        .is_none_or(|assignment| *position < assignment.position)
                {
                    cursor.set(position + 1);
                    self.makeflags
                        .as_mut()
                        .expect("configured policy receiver")
                        .set_warning_policy(true);
                    continue;
                }
                let Some(assignment) = assignments.get(index) else {
                    cursor.set(cursor.get().max(terminal));
                    return Ok(());
                };
                cursor.set(assignment.position + 1);
                self.makeflags_next_is_definition = assignment.position == 1
                    && assignments
                        .get(index + 1)
                        .is_some_and(|next| next.position == assignment.position + 1);
                database
                    .variables
                    .set_environment_override(assignment.environment_override);
                let effective = if origin == Origin::Environment && assignment.environment_override
                {
                    Origin::EnvironmentOverride
                } else {
                    origin
                };
                if let Some(name) = database.define_argument_with(
                    &assignment.argument,
                    effective,
                    self.reborrow(),
                )? {
                    names.push(name);
                }
            }
            Err(ExpandError::Safety(crate::safety::SafetyError::NestedFlags))
        })();
        self.makeflags_depth = depth;
        self.makeflags_terminal = previous_terminal;
        self.makeflags_next_is_definition = previous_following;
        applied?;
        Ok(names)
    }

    pub fn warns_undefined(&self) -> bool {
        self.output
            .as_ref()
            .is_some_and(|output| output.warns_undefined())
    }

    pub fn with_cancellation(mut self, cancellation: &'a std::sync::atomic::AtomicBool) -> Self {
        self.cancellation = Some(cancellation);
        self
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancellation
            .is_some_and(|flag| flag.load(std::sync::atomic::Ordering::Relaxed))
    }

    pub fn with_filesystem(mut self, filesystem: &'a mut FilesystemReceiver<'a>) -> Self {
        self.filesystem = Some(filesystem);
        self
    }

    pub fn with_includes(mut self, includes: &'a mut IncludeReceiver<'a>) -> Self {
        self.includes = Some(includes);
        self
    }

    pub fn reader_event(&mut self, event: ReaderEvent<'_>) {
        if let Some(output) = self.output.as_mut() {
            output.reader_event(event);
        }
    }

    pub(crate) fn loader_event(&mut self, event: crate::native::LoaderEvent<'_>) {
        if let Some(output) = self.output.as_mut() {
            output.loader_event(event);
        }
    }

    pub(crate) fn guile_callback(&mut self, callback: crate::native::GuileCallback<'_>) {
        if !self.is_cancelled()
            && let Some(output) = self.output.as_mut()
        {
            output.guile_callback(callback);
        }
    }

    pub fn read_include(&mut self, name: &[u8]) -> Option<Result<IncludedFile, IncludeFailure>> {
        if self.is_cancelled() {
            return Some(Err(b"interrupted".to_vec().into()));
        }
        self.includes.as_mut().map(|receiver| receiver(name))
    }

    pub fn with_shell(mut self, shell: &'a mut ShellReceiver<'a>) -> Self {
        self.shell = Some(shell);
        self
    }

    pub fn has_shell(&self) -> bool {
        self.shell.is_some()
    }

    pub fn run_shell(&mut self, request: ShellRequest<'_>) -> Option<Result<ShellResult, Vec<u8>>> {
        if self.is_cancelled() {
            return Some(Err(b"interrupted".to_vec()));
        }
        self.shell.as_mut().map(|receiver| receiver(request))
    }

    pub fn perform_filesystem(
        &mut self,
        effect: FilesystemEffect<'_>,
    ) -> Option<Result<Vec<u8>, FilesystemError>> {
        if self.is_cancelled() {
            return Some(Err(FilesystemError::Unlocated(b"interrupted".to_vec())));
        }
        self.filesystem.as_mut().map(|receiver| receiver(effect))
    }

    pub fn emit(&mut self, output: Output<'_>) -> bool {
        if self.is_cancelled() {
            return false;
        }
        if let Some(receiver) = &mut self.output {
            if let Output::SystemWarning { location, message } = output {
                let message = message.message();
                let rendered = match receiver
                    .format_system_message(crate::messages::MessageRef::Borrowed(message))
                {
                    Some(Ok(bytes)) => bytes,
                    Some(Err(bytes)) => {
                        receiver.emit(Output::Warning {
                            location,
                            message: &bytes,
                        });
                        return false;
                    }
                    None => message.english(),
                };
                receiver.emit(Output::Warning {
                    location,
                    message: &rendered,
                });
            } else {
                receiver.emit(output);
            }
            true
        } else {
            false
        }
    }

    pub fn emit_diagnostic_warning(&mut self, location: &Location, message: &Diagnostic) {
        // Reader warnings can wait in a declaration until publication. Resolve
        // typed templates at that point, using the active locale and domain.
        let formatted = if let Some(message) = message.system_message() {
            if self.is_cancelled() {
                return;
            }
            self.output.as_mut().and_then(|receiver| {
                receiver.format_system_message(crate::messages::MessageRef::Owned(message))
            })
        } else {
            None
        };
        let bytes = match &formatted {
            Some(Ok(bytes) | Err(bytes)) => bytes.as_slice(),
            None => message.as_bytes(),
        };
        self.emit(Output::Warning {
            location: Some(location),
            message: bytes,
        });
    }

    pub fn freeze_diagnostic(&mut self, message: Diagnostic) -> Diagnostic {
        if message.frozen_stop().is_some() {
            return message;
        }
        let body = message.system_message().and_then(|message| {
            self.output.as_mut().and_then(|receiver| {
                receiver.format_system_message(crate::messages::MessageRef::Owned(message))
            })
        });
        let stop = self.output.as_mut().and_then(|receiver| {
            receiver.format_system_message(crate::messages::MessageRef::Borrowed(
                crate::messages::Message::Stop,
            ))
        });
        match stop {
            Some(Ok(stop) | Err(stop)) => Diagnostic::frozen(
                match body {
                    Some(Ok(bytes) | Err(bytes)) => bytes,
                    None => message.as_bytes().to_vec(),
                },
                stop,
                message.is_unlocated(),
            ),
            None => message,
        }
    }

    pub fn freeze_error(
        &mut self,
        error: crate::variables::ExpandError,
    ) -> crate::variables::ExpandError {
        if error.is_rendered() {
            return error;
        }
        let body = error.system_message().and_then(|message| {
            self.output
                .as_mut()
                .and_then(|receiver| receiver.format_system_message(message))
        });
        let stop = self.output.as_mut().and_then(|receiver| {
            receiver.format_system_message(crate::messages::MessageRef::Borrowed(
                crate::messages::Message::Stop,
            ))
        });
        match stop {
            Some(Ok(stop) | Err(stop)) => {
                let message = match body {
                    Some(Ok(bytes) | Err(bytes)) => bytes,
                    None => error.diagnostic().as_bytes().to_vec(),
                };
                let diagnostic = Diagnostic::frozen(message, stop, error.is_unlocated());
                crate::variables::ExpandError::RenderedSystem {
                    error: Box::new(error),
                    diagnostic,
                }
            }
            None => error,
        }
    }

    pub fn at<'b>(&'b mut self, location: Option<&'b Location>) -> ExpansionContext<'b> {
        ExpansionContext {
            location,
            cancellation: self.cancellation,
            output: match &mut self.output {
                Some(output) => Some(&mut **output),
                None => None,
            },
            filesystem: match &mut self.filesystem {
                Some(filesystem) => Some(&mut **filesystem),
                None => None,
            },
            includes: match &mut self.includes {
                Some(includes) => Some(&mut **includes),
                None => None,
            },
            shell: match &mut self.shell {
                Some(shell) => Some(&mut **shell),
                None => None,
            },
            makeflags: match &mut self.makeflags {
                Some(receiver) => Some(&mut **receiver),
                None => None,
            },
            evaluation_stack: self.evaluation_stack,
            native_runtime: self.native_runtime.clone(),
            makeflags_depth: self.makeflags_depth,
            makeflags_terminal: self.makeflags_terminal,
            makeflags_next_is_definition: self.makeflags_next_is_definition,
            makeflags_cursor: self.makeflags_cursor.clone(),
        }
    }

    pub fn reborrow(&mut self) -> ExpansionContext<'_> {
        self.at(self.location)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Diagnostic {
    Frozen(Box<FrozenDiagnostic>),
    System(Box<SystemDiagnostic>),
    Text(String),
    Bytes(Vec<u8>),
    /// The selected error has no source/caller location; later eval transport
    /// must preserve that decision rather than attaching a binding locus.
    Unlocated(Vec<u8>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrozenDiagnostic {
    body: Vec<u8>,
    stop: Vec<u8>,
    unlocated: bool,
}

#[derive(Clone, Debug)]
pub struct SystemDiagnostic {
    message: crate::messages::OwnedMessage,
    unlocated: bool,
    english: std::sync::OnceLock<Vec<u8>>,
}
impl PartialEq for SystemDiagnostic {
    fn eq(&self, other: &Self) -> bool {
        self.message == other.message && self.unlocated == other.unlocated
    }
}
impl Eq for SystemDiagnostic {}

impl Diagnostic {
    pub fn frozen(body: Vec<u8>, stop: Vec<u8>, unlocated: bool) -> Self {
        Self::Frozen(Box::new(FrozenDiagnostic {
            body,
            stop,
            unlocated,
        }))
    }
    pub fn frozen_stop(&self) -> Option<&[u8]> {
        match self {
            Self::Frozen(value) => Some(&value.stop),
            _ => None,
        }
    }
    pub fn system(message: crate::messages::Message<'_>) -> Self {
        Self::system_owned(message.owned())
    }
    pub(crate) fn unlocated_system(message: crate::messages::Message<'_>) -> Self {
        Self::with_system_location(message.owned(), true)
    }
    pub(crate) fn system_owned(message: crate::messages::OwnedMessage) -> Self {
        Self::with_system_location(message, false)
    }
    fn with_system_location(message: crate::messages::OwnedMessage, unlocated: bool) -> Self {
        Self::System(Box::new(SystemDiagnostic {
            message,
            unlocated,
            english: std::sync::OnceLock::new(),
        }))
    }
    pub fn system_message(&self) -> Option<&crate::messages::OwnedMessage> {
        match self {
            Self::System(value) => Some(&value.message),
            _ => None,
        }
    }
    pub fn is_unlocated(&self) -> bool {
        match self {
            Self::Frozen(value) => value.unlocated,
            Self::System(value) => value.unlocated,
            _ => matches!(self, Self::Unlocated(_)),
        }
    }
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Self::Frozen(value) => &value.body,
            Self::System(value) => value.english.get_or_init(|| value.message.english()),
            Self::Text(text) => text.as_bytes(),
            Self::Bytes(bytes) | Self::Unlocated(bytes) => bytes,
        }
    }
}

impl From<String> for Diagnostic {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<&str> for Diagnostic {
    fn from(value: &str) -> Self {
        Self::Text(value.into())
    }
}

impl From<Vec<u8>> for Diagnostic {
    fn from(value: Vec<u8>) -> Self {
        Self::Bytes(value)
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Formatting is for human-readable contexts only. CLI diagnostics must
        // write as_bytes() directly to retain arbitrary input bytes.
        output.write_str(&String::from_utf8_lossy(self.as_bytes()))
    }
}

#[cfg(test)]
mod tests {
    use super::Diagnostic;

    #[test]
    fn cancellation_survives_reborrowing_and_location_changes() {
        use std::sync::atomic::{AtomicBool, Ordering};
        let flag = AtomicBool::new(false);
        let mut context = super::ExpansionContext::pure().with_cancellation(&flag);
        assert!(!context.reborrow().is_cancelled());
        flag.store(true, Ordering::Relaxed);
        assert!(context.at(None).is_cancelled());
    }

    #[test]
    fn raw_diagnostic_transport_does_not_use_lossy_display() {
        let message = Diagnostic::from(b"bad '\xff'".to_vec());
        assert_eq!(message.as_bytes(), b"bad '\xff'");
        assert_eq!(message.to_string(), "bad '\u{fffd}'");
        assert_eq!(Diagnostic::from("text").as_bytes(), b"text");
    }
}

#[cfg(test)]
mod flag_filename_tests {
    use super::*;
    use std::cell::Cell;

    struct Policy<'a>(&'a Cell<bool>);
    impl MakeflagsReceiver for Policy<'_> {
        fn decode(&mut self, _: MakeflagsRequest<'_>) -> Result<MakeflagsResult, Vec<u8>> {
            unreachable!("filename preparation must not replay decoding")
        }
        fn set_warning_policy(&mut self, enabled: bool) {
            self.0.set(enabled);
        }
    }

    #[test]
    fn home_warning_mask_restores_on_success_and_failure() {
        for failing in [false, true] {
            let mut database = crate::parser::Database::default();
            database
                .read(
                    if failing {
                        b"HOME = $(MISSING)$(error stop)\n"
                    } else {
                        b"HOME = $(MISSING)hdir\n"
                    },
                    b"Makefile",
                )
                .unwrap();
            let enabled = Cell::new(true);
            let mut warnings = Vec::new();
            {
                let mut emit = |event: Output<'_>| {
                    if let Output::Warning { message, .. } = event {
                        warnings.push(message.to_vec());
                    }
                };
                let mut output = WarningOutput::new(&mut emit, &enabled);
                let mut receiver = Policy(&enabled);
                let mut context = ExpansionContext::with_receiver(None, &mut output)
                    .with_makeflags(&mut receiver);
                let result = context.flag_filename(&mut database, b"~/inc");
                if failing {
                    assert!(result.is_err());
                } else {
                    assert_eq!(result.unwrap(), b"hdir/inc");
                }
                assert!(enabled.get());
                database
                    .expand_with(b"$(AFTER_HOME)", context.reborrow())
                    .unwrap();
            }
            assert_eq!(warnings.len(), 1);
            assert!(
                warnings[0]
                    .windows(b"AFTER_HOME".len())
                    .any(|s| s == b"AFTER_HOME")
            );
        }
    }
}

#[cfg(test)]
mod source_name_tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn null_and_empty_sources_keep_distinct_provenance_without_location_growth() {
        let absent = SourceName::null();
        let empty = SourceName::from(b"".as_slice());
        assert_ne!(absent, empty);
        assert!(absent.is_null());
        assert!(!empty.is_null());
        assert_eq!(absent.as_ref(), empty.as_ref());
        assert_eq!(absent.as_option(), None);
        assert_eq!(empty.as_option(), Some(b"".as_slice()));
        assert_eq!(absent.clone(), absent);
        assert_eq!(empty.clone(), empty);
        assert_eq!(
            std::mem::size_of::<SourceName>(),
            std::mem::size_of::<Arc<[u8]>>()
        );
        assert_eq!(
            std::mem::size_of::<Location>(),
            std::mem::size_of::<Arc<[u8]>>() + std::mem::size_of::<usize>()
        );
    }

    #[test]
    fn named_sources_share_owned_byte_storage_including_non_utf8() {
        let bytes: Arc<[u8]> = b"source\xff.mk".as_slice().into();
        let name = SourceName::from(bytes.clone());
        let copied = name.clone();
        assert!(Arc::ptr_eq(name.0.as_ref().unwrap(), &bytes));
        assert!(Arc::ptr_eq(copied.0.as_ref().unwrap(), &bytes));
        assert_eq!(copied.as_option(), Some(b"source\xff.mk".as_slice()));
    }
}
