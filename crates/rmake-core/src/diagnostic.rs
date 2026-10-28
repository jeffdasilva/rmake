// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Owned diagnostic text with lossless transport for Makefile bytes.

use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Location {
    /// Immutable source spelling shared by locations from the same read.
    /// Sharing bytes does not cache file contents, lookup results or effects.
    pub file: std::sync::Arc<[u8]>,
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
}

/// Output receivers can expose live diagnostic policy without growing every
/// expansion context. Ordinary closures retain the default quiet policy.
pub trait OutputReceiver {
    fn emit(&mut self, event: Output<'_>);
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
    /// Account/environment fallback; make's nonempty HOME is expanded in core.
    Tilde(&'a [u8]),
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
    pub bytes: Vec<u8>,
}

type IncludeReceiver<'a> = dyn FnMut(&[u8]) -> Result<IncludedFile, Vec<u8>> + 'a;

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

pub struct MakeflagsResult {
    pub value: Vec<u8>,
    pub origin: crate::variables::Origin,
    pub environment_override: bool,
    pub include_dirs: Vec<Vec<u8>>,
    pub assignments: Vec<FlagAssignment>,
    pub next_argument: usize,
}
type MakeflagsReceiver<'a> =
    dyn FnMut(MakeflagsRequest<'_>) -> Result<MakeflagsResult, Vec<u8>> + 'a;

/// Explicit caller context and output receiver. This is never stored in or
/// shared by cloned variable stores; each evaluation chooses its own receiver.
pub struct ExpansionContext<'a> {
    pub location: Option<&'a Location>,
    cancellation: Option<&'a std::sync::atomic::AtomicBool>,
    output: Option<&'a mut dyn OutputReceiver>,
    filesystem: Option<&'a mut FilesystemReceiver<'a>>,
    includes: Option<&'a mut IncludeReceiver<'a>>,
    shell: Option<&'a mut ShellReceiver<'a>>,
    makeflags: Option<&'a mut MakeflagsReceiver<'a>>,
    makeflags_depth: usize,
    makeflags_terminal: usize,
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
            makeflags_depth: 0,
            makeflags_terminal: 0,
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
            makeflags_depth: 0,
            makeflags_terminal: 0,
            makeflags_cursor: None,
        }
    }

    pub fn with_makeflags(mut self, receiver: &'a mut MakeflagsReceiver<'a>) -> Self {
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
        let mut result = self.makeflags.as_mut().expect("configured receiver")(MakeflagsRequest {
            value: &value,
            origin,
        })
        .map_err(ExpandError::UnlocatedError)?;
        database
            .variables
            .set_environment_override(result.environment_override);
        if !result.assignments.is_empty() {
            self.apply_flag_assignments(
                database,
                &result.assignments,
                origin,
                result.next_argument,
            )?;
            // Nested definitions can change switches, but refreshing does not
            // repeat the outer assignment stream or reset its owned cursor.
            result = self.makeflags.as_mut().expect("configured receiver")(MakeflagsRequest {
                value: b"",
                origin,
            })
            .map_err(ExpandError::UnlocatedError)?;
        } else if has_input && let Some(cursor) = &self.makeflags_cursor {
            // A nested single short-option word rewinds GNU's process-global
            // cursor into an already-applied outer assignment. Refuse the
            // recorded nonterminating class before replaying those effects.
            let token = value.trim_ascii();
            if self.makeflags_depth > 0
                && self.makeflags_terminal > 2
                && cursor.get() > 2
                && result.next_argument == 2
                && token.starts_with(b"-")
                && token.len() > 1
                && !token.starts_with(b"--")
            {
                return Err(ExpandError::Safety(crate::safety::SafetyError::NestedFlags));
            }
            cursor.set(result.next_argument);
        }
        database.update_mflags(&result.value, result.environment_override)?;
        let overrides_name = database.command_overrides_name();
        let overrides_reference = database.command_overrides_reference();
        let variables = database
            .evaluation_scope
            .as_mut()
            .unwrap_or(&mut database.variables);
        if variables
            .get(overrides_name)
            .is_some_and(|variable| !variable.raw_value().is_empty())
        {
            result.value.extend_from_slice(overrides_reference);
        }
        variables.set_environment_override(result.environment_override);
        variables.define_runtime_metadata(b"MAKEFLAGS", &result.value, result.origin)?;
        let variables = &mut database.variables;
        variables.define_literal(b".INCLUDE_DIRS", b"", crate::variables::Origin::Default)?;
        for directory in result.include_dirs {
            variables.define_with(
                b".INCLUDE_DIRS",
                &directory,
                Assignment::Append,
                crate::variables::Origin::Default,
                None,
                self.reborrow(),
            )?;
        }
        Ok(())
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
        use crate::variables::{ExpandError, Origin};
        if self.makeflags_depth >= 128 {
            return Err(ExpandError::ExpansionLimit);
        }
        let cursor = self
            .makeflags_cursor
            .get_or_insert_with(Default::default)
            .clone();
        cursor.set(1);
        let mut names = Vec::new();
        let depth = self.makeflags_depth;
        let previous_terminal = std::mem::replace(&mut self.makeflags_terminal, terminal);
        self.makeflags_depth += 1;
        let applied = (|| {
            for _ in 0..128_usize.saturating_mul(assignments.len().max(1)) {
                let index =
                    assignments.partition_point(|assignment| assignment.position < cursor.get());
                let Some(assignment) = assignments.get(index) else {
                    cursor.set(cursor.get().max(terminal));
                    return Ok(());
                };
                cursor.set(assignment.position + 1);
                database
                    .variables
                    .set_environment_override(assignment.environment_override);
                let effective = if origin == Origin::Environment && assignment.environment_override
                {
                    Origin::EnvironmentOverride
                } else {
                    origin
                };
                if let Some(name) =
                    database.define_argument_with(&assignment.argument, effective, self.at(None))?
                {
                    names.push(name);
                }
            }
            Err(ExpandError::Safety(crate::safety::SafetyError::NestedFlags))
        })();
        self.makeflags_depth = depth;
        self.makeflags_terminal = previous_terminal;
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

    pub fn read_include(&mut self, name: &[u8]) -> Option<Result<IncludedFile, Vec<u8>>> {
        if self.is_cancelled() {
            return Some(Err(b"interrupted".to_vec()));
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
            receiver.emit(output);
            true
        } else {
            false
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
            makeflags_depth: self.makeflags_depth,
            makeflags_terminal: self.makeflags_terminal,
            makeflags_cursor: self.makeflags_cursor.clone(),
        }
    }

    pub fn reborrow(&mut self) -> ExpansionContext<'_> {
        self.at(self.location)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Diagnostic {
    Text(String),
    Bytes(Vec<u8>),
}

impl Diagnostic {
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Self::Text(text) => text.as_bytes(),
            Self::Bytes(bytes) => bytes,
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
