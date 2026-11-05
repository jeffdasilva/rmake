// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva
//! Typed system-message declarations and checked, byte-preserving formatting.
//! These identifiers are compatibility data; user and foreign text is separate.

use std::ffi::CStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageId {
    Stop,
    MissingSeparator,
    MissingSeparatorTabs,
    UnterminatedFunction,
    IntcmpFirst,
    IntcmpSecond,
    IntcmpFirstEmpty,
    IntcmpSecondEmpty,
    UndefinedVariable,
    NoRule,
    NoRuleNeeded,
    TargetNotRemade,
    RecipeError,
    Ignored,
    NoTargets,
    NoMakefile,
    MultipleDefaultGoals,
    OrphanRecipe,
    InvalidConditional,
    MissingEndif,
    EmptyVariableName,
    MissingEndef,
    UnterminatedReference,
    RecursiveVariable,
    OverrideRecipe,
    IgnoreOldRecipe,
    EnterDirectory,
    LeaveDirectory,
    EnterUnknownDirectory,
    LeaveUnknownDirectory,
    EnterDirectoryAtDepth,
    LeaveDirectoryAtDepth,
    EnterUnknownDirectoryAtDepth,
    LeaveUnknownDirectoryAtDepth,
    OsError,
    SourceError,
    FailedMakefileRemake,
    NothingToBeDone,
    UpToDate,
    CircularDependency,
    MissingTargetPattern,
    MultipleTargetPatterns,
    TargetPatternWithoutPercent,
    DirectiveTail,
    DefineTail,
    EndefTail,
    MissingFileName,
    ExtraFileArgument,
    InvalidFileOperation,
    LoadFailed,
    EmptyLoadSymbol,
    MissingNativeLicense,
    NativeSymbolFailure,
    EmptyNativeFunction,
    InvalidNativeFunction,
    LongNativeFunction,
    NativeMinimumArguments,
    NativeMaximumArguments,
}

impl MessageId {
    pub const ALL: [Self; 58] = [
        Self::Stop,
        Self::MissingSeparator,
        Self::MissingSeparatorTabs,
        Self::UnterminatedFunction,
        Self::IntcmpFirst,
        Self::IntcmpSecond,
        Self::IntcmpFirstEmpty,
        Self::IntcmpSecondEmpty,
        Self::UndefinedVariable,
        Self::NoRule,
        Self::NoRuleNeeded,
        Self::TargetNotRemade,
        Self::RecipeError,
        Self::Ignored,
        Self::NoTargets,
        Self::NoMakefile,
        Self::MultipleDefaultGoals,
        Self::OrphanRecipe,
        Self::InvalidConditional,
        Self::MissingEndif,
        Self::EmptyVariableName,
        Self::MissingEndef,
        Self::UnterminatedReference,
        Self::RecursiveVariable,
        Self::OverrideRecipe,
        Self::IgnoreOldRecipe,
        Self::EnterDirectory,
        Self::LeaveDirectory,
        Self::EnterUnknownDirectory,
        Self::LeaveUnknownDirectory,
        Self::EnterDirectoryAtDepth,
        Self::LeaveDirectoryAtDepth,
        Self::EnterUnknownDirectoryAtDepth,
        Self::LeaveUnknownDirectoryAtDepth,
        Self::OsError,
        Self::SourceError,
        Self::FailedMakefileRemake,
        Self::NothingToBeDone,
        Self::UpToDate,
        Self::CircularDependency,
        Self::MissingTargetPattern,
        Self::MultipleTargetPatterns,
        Self::TargetPatternWithoutPercent,
        Self::DirectiveTail,
        Self::DefineTail,
        Self::EndefTail,
        Self::MissingFileName,
        Self::ExtraFileArgument,
        Self::InvalidFileOperation,
        Self::LoadFailed,
        Self::EmptyLoadSymbol,
        Self::MissingNativeLicense,
        Self::NativeSymbolFailure,
        Self::EmptyNativeFunction,
        Self::InvalidNativeFunction,
        Self::LongNativeFunction,
        Self::NativeMinimumArguments,
        Self::NativeMaximumArguments,
    ];
    pub fn sample(self) -> Message<'static> {
        match self {
            Self::Stop => Message::Stop,
            Self::MissingSeparator | Self::MissingSeparatorTabs => Message::MissingSeparator {
                tabs: self == Self::MissingSeparatorTabs,
            },
            Self::UnterminatedFunction => Message::UnterminatedFunction {
                name: b"function",
                close: b')',
            },
            Self::IntcmpFirst
            | Self::IntcmpSecond
            | Self::IntcmpFirstEmpty
            | Self::IntcmpSecondEmpty => Message::InvalidIntcmp {
                second: matches!(self, Self::IntcmpSecond | Self::IntcmpSecondEmpty),
                empty: matches!(self, Self::IntcmpFirstEmpty | Self::IntcmpSecondEmpty),
                value: b"bad",
            },
            Self::UndefinedVariable => Message::UndefinedVariable(b"variable"),
            Self::NoRule | Self::NoRuleNeeded => Message::NoRule {
                prefix: b"",
                target: b"target",
                parent: (self == Self::NoRuleNeeded).then_some(b"parent".as_slice()),
                suffix: b"",
            },
            Self::TargetNotRemade => Message::TargetNotRemade(b"target"),
            Self::RecipeError => Message::RecipeError {
                prefix: b"*** ",
                source: b"source:1",
                target: b"target",
                code: 7,
                ignored: b"",
                suffix: b"",
            },
            Self::Ignored => Message::Ignored,
            Self::NoTargets | Self::NoMakefile => Message::NoTargets {
                makefile_found: self == Self::NoTargets,
            },
            Self::MultipleDefaultGoals => Message::MultipleDefaultGoals,
            Self::OrphanRecipe => Message::OrphanRecipe,
            Self::InvalidConditional => Message::InvalidConditional,
            Self::MissingEndif => Message::MissingEndif,
            Self::EmptyVariableName => Message::EmptyVariableName,
            Self::MissingEndef => Message::MissingEndef,
            Self::UnterminatedReference => Message::UnterminatedReference,
            Self::RecursiveVariable => Message::RecursiveVariable(b"variable"),
            Self::OverrideRecipe | Self::IgnoreOldRecipe => Message::DuplicateRecipe {
                ignoring_previous: self == Self::IgnoreOldRecipe,
                target: b"target",
            },
            Self::EnterDirectory
            | Self::LeaveDirectory
            | Self::EnterUnknownDirectory
            | Self::LeaveUnknownDirectory
            | Self::EnterDirectoryAtDepth
            | Self::LeaveDirectoryAtDepth
            | Self::EnterUnknownDirectoryAtDepth
            | Self::LeaveUnknownDirectoryAtDepth => Message::Directory {
                entering: matches!(
                    self,
                    Self::EnterDirectory
                        | Self::EnterUnknownDirectory
                        | Self::EnterDirectoryAtDepth
                        | Self::EnterUnknownDirectoryAtDepth
                ),
                program: b"make",
                depth: if matches!(
                    self,
                    Self::EnterDirectoryAtDepth
                        | Self::LeaveDirectoryAtDepth
                        | Self::EnterUnknownDirectoryAtDepth
                        | Self::LeaveUnknownDirectoryAtDepth
                ) {
                    2
                } else {
                    0
                },
                path: matches!(
                    self,
                    Self::EnterDirectory
                        | Self::LeaveDirectory
                        | Self::EnterDirectoryAtDepth
                        | Self::LeaveDirectoryAtDepth
                )
                .then_some(b"/directory".as_slice()),
            },
            Self::OsError => Message::OsError {
                action: b"getcwd",
                path: b"",
                detail: b"File not found",
            },
            Self::SourceError => Message::SourceError {
                path: b"Makefile",
                detail: b"I/O error",
            },
            Self::FailedMakefileRemake => Message::FailedMakefileRemake(b"Makefile"),
            Self::NothingToBeDone => Message::NothingToBeDone(b"target"),
            Self::UpToDate => Message::UpToDate(b"target"),
            Self::CircularDependency => Message::CircularDependency {
                target: b"target",
                prerequisite: b"input",
            },
            Self::MissingTargetPattern => Message::MissingTargetPattern,
            Self::MultipleTargetPatterns => Message::MultipleTargetPatterns,
            Self::TargetPatternWithoutPercent => Message::TargetPatternWithoutPercent,
            Self::DirectiveTail => Message::DirectiveTail(b"ifeq"),
            Self::DefineTail => Message::DefineTail,
            Self::EndefTail => Message::EndefTail,
            Self::MissingFileName => Message::MissingFileName,
            Self::ExtraFileArgument => Message::ExtraFileArgument,
            Self::InvalidFileOperation => Message::InvalidFileOperation(b"invalid"),
            Self::LoadFailed => Message::LoadFailed(b"library.so"),
            Self::EmptyLoadSymbol => Message::EmptyLoadSymbol(b"library.so()"),
            Self::MissingNativeLicense => Message::MissingNativeLicense(b"library.so"),
            Self::NativeSymbolFailure => Message::NativeSymbolFailure {
                symbol: b"setup",
                object: b"library.so",
                detail: b"loader error",
            },
            Self::EmptyNativeFunction => Message::EmptyNativeFunction,
            Self::InvalidNativeFunction => Message::InvalidNativeFunction(b"name"),
            Self::LongNativeFunction => Message::LongNativeFunction(b"name"),
            Self::NativeMinimumArguments => Message::NativeArgumentCount {
                minimum: true,
                count: 256,
                name: b"function",
            },
            Self::NativeMaximumArguments => Message::NativeArgumentCount {
                minimum: false,
                count: 256,
                name: b"function",
            },
        }
    }
    pub fn msgid(self) -> &'static CStr {
        match self {
            Self::Stop => c".  Stop.\n",
            Self::MissingSeparator => c"missing separator",
            Self::MissingSeparatorTabs => {
                c"missing separator (did you mean TAB instead of 8 spaces?)"
            }
            Self::UnterminatedFunction => c"unterminated call to function '%s': missing '%c'",
            Self::IntcmpFirst => c"non-numeric first argument to 'intcmp' function: '%s'",
            Self::IntcmpSecond => c"non-numeric second argument to 'intcmp' function: '%s'",
            Self::IntcmpFirstEmpty => {
                c"non-numeric first argument to 'intcmp' function: empty value"
            }
            Self::IntcmpSecondEmpty => {
                c"non-numeric second argument to 'intcmp' function: empty value"
            }
            Self::UndefinedVariable => c"warning: undefined variable '%.*s'",
            Self::NoRule => c"%sNo rule to make target '%s'%s",
            Self::NoRuleNeeded => c"%sNo rule to make target '%s', needed by '%s'%s",
            Self::TargetNotRemade => c"Target '%s' not remade because of errors.",
            Self::RecipeError => c"%s[%s: %s] Error %d%s%s",
            Self::Ignored => c" (ignored)",
            Self::NoTargets => c"No targets",
            Self::NoMakefile => c"No targets specified and no makefile found",
            Self::MultipleDefaultGoals => c".DEFAULT_GOAL contains more than one target",
            Self::OrphanRecipe => c"recipe commences before first target",
            Self::InvalidConditional => c"invalid syntax in conditional",
            Self::MissingEndif => c"missing 'endif'",
            Self::EmptyVariableName => c"empty variable name",
            Self::MissingEndef => c"missing 'endef', unterminated 'define'",
            Self::UnterminatedReference => c"unterminated variable reference",
            Self::RecursiveVariable => c"Recursive variable '%s' references itself (eventually)",
            Self::OverrideRecipe => c"warning: overriding recipe for target '%s'",
            Self::IgnoreOldRecipe => c"warning: ignoring old recipe for target '%s'",
            Self::EnterDirectory => c"%s: Entering directory '%s'\n",
            Self::LeaveDirectory => c"%s: Leaving directory '%s'\n",
            Self::EnterUnknownDirectory => c"%s: Entering an unknown directory\n",
            Self::LeaveUnknownDirectory => c"%s: Leaving an unknown directory\n",
            Self::EnterDirectoryAtDepth => c"%s[%u]: Entering directory '%s'\n",
            Self::LeaveDirectoryAtDepth => c"%s[%u]: Leaving directory '%s'\n",
            Self::EnterUnknownDirectoryAtDepth => c"%s[%u]: Entering an unknown directory\n",
            Self::LeaveUnknownDirectoryAtDepth => c"%s[%u]: Leaving an unknown directory\n",
            Self::OsError => c"%s%s: %s",
            Self::SourceError => c"%s: %s",
            Self::FailedMakefileRemake => c"Failed to remake makefile '%s'.",
            Self::NothingToBeDone => c"Nothing to be done for '%s'.",
            Self::UpToDate => c"'%s' is up to date.",
            Self::CircularDependency => c"Circular %s <- %s dependency dropped.",
            Self::MissingTargetPattern => c"missing target pattern",
            Self::MultipleTargetPatterns => c"multiple target patterns",
            Self::TargetPatternWithoutPercent => c"target pattern contains no '%%'",
            Self::DirectiveTail => c"extraneous text after '%s' directive",
            Self::DefineTail => c"extraneous text after 'define' directive",
            Self::EndefTail => c"extraneous text after 'endef' directive",
            Self::MissingFileName => c"file: missing filename",
            Self::ExtraFileArgument => c"file: too many arguments",
            Self::InvalidFileOperation => c"file: invalid file operation: %s",
            Self::LoadFailed => c"%s: failed to load",
            Self::EmptyLoadSymbol => c"Empty symbol name for load: %s",
            Self::MissingNativeLicense => c"Loaded object %s is not declared to be GPL compatible",
            Self::NativeSymbolFailure => c"Failed to load symbol %s from %s: %s",
            Self::EmptyNativeFunction => c"Empty function name",
            Self::InvalidNativeFunction => c"Invalid function name: %s",
            Self::LongNativeFunction => c"Function name too long: %s",
            Self::NativeMinimumArguments => c"Invalid minimum argument count (%u) for function %s",
            Self::NativeMaximumArguments => c"Invalid maximum argument count (%u) for function %s",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Argument<'a> {
    Bytes(&'a [u8]),
    Signed(i64),
    Unsigned(u64),
    Character(u8),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OwnedArgument {
    Bytes(Vec<u8>),
    Signed(i64),
    Unsigned(u64),
    Character(u8),
}
impl OwnedArgument {
    fn borrowed(&self) -> Argument<'_> {
        match self {
            Self::Bytes(bytes) => Argument::Bytes(bytes),
            Self::Signed(value) => Argument::Signed(*value),
            Self::Unsigned(value) => Argument::Unsigned(*value),
            Self::Character(value) => Argument::Character(*value),
        }
    }
}
impl From<Argument<'_>> for OwnedArgument {
    fn from(value: Argument<'_>) -> Self {
        match value {
            Argument::Bytes(bytes) => Self::Bytes(bytes.to_vec()),
            Argument::Signed(value) => Self::Signed(value),
            Argument::Unsigned(value) => Self::Unsigned(value),
            Argument::Character(value) => Self::Character(value),
        }
    }
}

/// Borrowed typed system data, valid only for its synchronous output call.
#[derive(Clone, Copy, Debug)]
pub enum Message<'a> {
    Stop,
    MissingSeparator {
        tabs: bool,
    },
    UnterminatedFunction {
        name: &'a [u8],
        close: u8,
    },
    InvalidIntcmp {
        second: bool,
        empty: bool,
        value: &'a [u8],
    },
    UndefinedVariable(&'a [u8]),
    NoRule {
        prefix: &'a [u8],
        target: &'a [u8],
        parent: Option<&'a [u8]>,
        suffix: &'a [u8],
    },
    TargetNotRemade(&'a [u8]),
    RecipeError {
        prefix: &'a [u8],
        source: &'a [u8],
        target: &'a [u8],
        code: i32,
        ignored: &'a [u8],
        suffix: &'a [u8],
    },
    Ignored,
    NoTargets {
        makefile_found: bool,
    },
    MultipleDefaultGoals,
    OrphanRecipe,
    InvalidConditional,
    MissingEndif,
    EmptyVariableName,
    MissingEndef,
    UnterminatedReference,
    RecursiveVariable(&'a [u8]),
    DuplicateRecipe {
        ignoring_previous: bool,
        target: &'a [u8],
    },
    Directory {
        entering: bool,
        program: &'a [u8],
        depth: u32,
        path: Option<&'a [u8]>,
    },
    OsError {
        action: &'a [u8],
        path: &'a [u8],
        detail: &'a [u8],
    },
    SourceError {
        path: &'a [u8],
        detail: &'a [u8],
    },
    FailedMakefileRemake(&'a [u8]),
    NothingToBeDone(&'a [u8]),
    UpToDate(&'a [u8]),
    CircularDependency {
        target: &'a [u8],
        prerequisite: &'a [u8],
    },
    MissingTargetPattern,
    MultipleTargetPatterns,
    TargetPatternWithoutPercent,
    DirectiveTail(&'a [u8]),
    DefineTail,
    EndefTail,
    MissingFileName,
    ExtraFileArgument,
    InvalidFileOperation(&'a [u8]),
    LoadFailed(&'a [u8]),
    EmptyLoadSymbol(&'a [u8]),
    MissingNativeLicense(&'a [u8]),
    NativeSymbolFailure {
        symbol: &'a [u8],
        object: &'a [u8],
        detail: &'a [u8],
    },
    EmptyNativeFunction,
    InvalidNativeFunction(&'a [u8]),
    LongNativeFunction(&'a [u8]),
    NativeArgumentCount {
        minimum: bool,
        count: u32,
        name: &'a [u8],
    },
}
impl<'a> Message<'a> {
    pub fn id(self) -> MessageId {
        match self {
            Self::Stop => MessageId::Stop,
            Self::MissingSeparator { tabs: false } => MessageId::MissingSeparator,
            Self::MissingSeparator { tabs: true } => MessageId::MissingSeparatorTabs,
            Self::UnterminatedFunction { .. } => MessageId::UnterminatedFunction,
            Self::InvalidIntcmp {
                second: false,
                empty: false,
                ..
            } => MessageId::IntcmpFirst,
            Self::InvalidIntcmp {
                second: true,
                empty: false,
                ..
            } => MessageId::IntcmpSecond,
            Self::InvalidIntcmp {
                second: false,
                empty: true,
                ..
            } => MessageId::IntcmpFirstEmpty,
            Self::InvalidIntcmp {
                second: true,
                empty: true,
                ..
            } => MessageId::IntcmpSecondEmpty,
            Self::UndefinedVariable(_) => MessageId::UndefinedVariable,
            Self::NoRule { parent: None, .. } => MessageId::NoRule,
            Self::NoRule {
                parent: Some(_), ..
            } => MessageId::NoRuleNeeded,
            Self::TargetNotRemade(_) => MessageId::TargetNotRemade,
            Self::RecipeError { .. } => MessageId::RecipeError,
            Self::Ignored => MessageId::Ignored,
            Self::NoTargets {
                makefile_found: true,
            } => MessageId::NoTargets,
            Self::NoTargets {
                makefile_found: false,
            } => MessageId::NoMakefile,
            Self::MultipleDefaultGoals => MessageId::MultipleDefaultGoals,
            Self::OrphanRecipe => MessageId::OrphanRecipe,
            Self::InvalidConditional => MessageId::InvalidConditional,
            Self::MissingEndif => MessageId::MissingEndif,
            Self::EmptyVariableName => MessageId::EmptyVariableName,
            Self::MissingEndef => MessageId::MissingEndef,
            Self::UnterminatedReference => MessageId::UnterminatedReference,
            Self::RecursiveVariable(_) => MessageId::RecursiveVariable,
            Self::DuplicateRecipe {
                ignoring_previous: false,
                ..
            } => MessageId::OverrideRecipe,
            Self::DuplicateRecipe {
                ignoring_previous: true,
                ..
            } => MessageId::IgnoreOldRecipe,
            Self::Directory {
                entering,
                depth,
                path,
                ..
            } => match (entering, depth != 0, path.is_some()) {
                (true, false, true) => MessageId::EnterDirectory,
                (false, false, true) => MessageId::LeaveDirectory,
                (true, false, false) => MessageId::EnterUnknownDirectory,
                (false, false, false) => MessageId::LeaveUnknownDirectory,
                (true, true, true) => MessageId::EnterDirectoryAtDepth,
                (false, true, true) => MessageId::LeaveDirectoryAtDepth,
                (true, true, false) => MessageId::EnterUnknownDirectoryAtDepth,
                (false, true, false) => MessageId::LeaveUnknownDirectoryAtDepth,
            },
            Self::OsError { .. } => MessageId::OsError,
            Self::SourceError { .. } => MessageId::SourceError,
            Self::FailedMakefileRemake(_) => MessageId::FailedMakefileRemake,
            Self::NothingToBeDone(_) => MessageId::NothingToBeDone,
            Self::UpToDate(_) => MessageId::UpToDate,
            Self::CircularDependency { .. } => MessageId::CircularDependency,
            Self::MissingTargetPattern => MessageId::MissingTargetPattern,
            Self::MultipleTargetPatterns => MessageId::MultipleTargetPatterns,
            Self::TargetPatternWithoutPercent => MessageId::TargetPatternWithoutPercent,
            Self::DirectiveTail(_) => MessageId::DirectiveTail,
            Self::DefineTail => MessageId::DefineTail,
            Self::EndefTail => MessageId::EndefTail,
            Self::MissingFileName => MessageId::MissingFileName,
            Self::ExtraFileArgument => MessageId::ExtraFileArgument,
            Self::InvalidFileOperation(_) => MessageId::InvalidFileOperation,
            Self::LoadFailed(_) => MessageId::LoadFailed,
            Self::EmptyLoadSymbol(_) => MessageId::EmptyLoadSymbol,
            Self::MissingNativeLicense(_) => MessageId::MissingNativeLicense,
            Self::NativeSymbolFailure { .. } => MessageId::NativeSymbolFailure,
            Self::EmptyNativeFunction => MessageId::EmptyNativeFunction,
            Self::InvalidNativeFunction(_) => MessageId::InvalidNativeFunction,
            Self::LongNativeFunction(_) => MessageId::LongNativeFunction,
            Self::NativeArgumentCount { minimum: true, .. } => MessageId::NativeMinimumArguments,
            Self::NativeArgumentCount { minimum: false, .. } => MessageId::NativeMaximumArguments,
        }
    }
    fn arguments(self) -> ([Argument<'a>; 6], usize) {
        let mut values = [Argument::Unsigned(0); 6];
        let mut count = 0;
        let mut push = |value| {
            values[count] = value;
            count += 1;
        };
        match self {
            Self::Stop
            | Self::MissingSeparator { .. }
            | Self::Ignored
            | Self::NoTargets { .. }
            | Self::MultipleDefaultGoals
            | Self::OrphanRecipe
            | Self::InvalidConditional
            | Self::MissingEndif
            | Self::EmptyVariableName
            | Self::MissingEndef
            | Self::UnterminatedReference
            | Self::MissingTargetPattern
            | Self::MultipleTargetPatterns
            | Self::TargetPatternWithoutPercent
            | Self::DefineTail
            | Self::EndefTail
            | Self::MissingFileName
            | Self::ExtraFileArgument
            | Self::EmptyNativeFunction => {}
            Self::RecursiveVariable(name)
            | Self::DuplicateRecipe { target: name, .. }
            | Self::NothingToBeDone(name)
            | Self::UpToDate(name)
            | Self::DirectiveTail(name)
            | Self::InvalidFileOperation(name)
            | Self::FailedMakefileRemake(name)
            | Self::LoadFailed(name)
            | Self::EmptyLoadSymbol(name)
            | Self::MissingNativeLicense(name)
            | Self::InvalidNativeFunction(name)
            | Self::LongNativeFunction(name) => {
                push(Argument::Bytes(name));
            }
            Self::NativeSymbolFailure {
                symbol,
                object,
                detail,
            } => {
                push(Argument::Bytes(symbol));
                push(Argument::Bytes(object));
                push(Argument::Bytes(detail));
            }
            Self::NativeArgumentCount { count, name, .. } => {
                push(Argument::Unsigned(u64::from(count)));
                push(Argument::Bytes(name));
            }
            Self::CircularDependency {
                target,
                prerequisite,
            } => {
                push(Argument::Bytes(target));
                push(Argument::Bytes(prerequisite));
            }
            Self::Directory {
                program,
                depth,
                path,
                ..
            } => {
                push(Argument::Bytes(program));
                if depth != 0 {
                    push(Argument::Unsigned(u64::from(depth)));
                }
                if let Some(path) = path {
                    push(Argument::Bytes(path));
                }
            }
            Self::OsError {
                action,
                path,
                detail,
            } => {
                push(Argument::Bytes(action));
                push(Argument::Bytes(path));
                push(Argument::Bytes(detail));
            }
            Self::SourceError { path, detail } => {
                push(Argument::Bytes(path));
                push(Argument::Bytes(detail));
            }
            Self::UnterminatedFunction { name, close } => {
                push(Argument::Bytes(name));
                push(Argument::Character(close));
            }
            Self::InvalidIntcmp { empty: true, .. } => {}
            Self::InvalidIntcmp { value, .. } => push(Argument::Bytes(value)),
            Self::UndefinedVariable(name) => {
                push(Argument::Unsigned(name.len() as u64));
                push(Argument::Bytes(name));
            }
            Self::NoRule {
                prefix,
                target,
                parent,
                suffix,
            } => {
                push(Argument::Bytes(prefix));
                push(Argument::Bytes(target));
                if let Some(parent) = parent {
                    push(Argument::Bytes(parent));
                }
                push(Argument::Bytes(suffix));
            }
            Self::TargetNotRemade(target) => push(Argument::Bytes(target)),
            Self::RecipeError {
                prefix,
                source,
                target,
                code,
                ignored,
                suffix,
            } => {
                push(Argument::Bytes(prefix));
                push(Argument::Bytes(source));
                push(Argument::Bytes(target));
                push(Argument::Signed(i64::from(code)));
                push(Argument::Bytes(ignored));
                push(Argument::Bytes(suffix));
            }
        }
        (values, count)
    }

    pub fn render(self, template: &[u8]) -> Result<Vec<u8>, FormatError> {
        let (values, count) = self.arguments();
        checked_format(template, &values[..count])
    }
    pub fn english(self) -> Vec<u8> {
        self.render(self.id().msgid().to_bytes())
            .expect("typed English declaration has a valid schema")
    }
    pub fn owned(self) -> OwnedMessage {
        let (values, count) = self.arguments();
        OwnedMessage {
            id: self.id(),
            arguments: values[..count]
                .iter()
                .copied()
                .map(OwnedArgument::from)
                .collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnedMessage {
    pub id: MessageId,
    arguments: Vec<OwnedArgument>,
}
impl OwnedMessage {
    pub fn render(&self, template: &[u8]) -> Result<Vec<u8>, FormatError> {
        let mut values = [Argument::Unsigned(0); 6];
        for (index, value) in self.arguments.iter().enumerate() {
            *values.get_mut(index).ok_or(FormatError::MissingArgument)? = value.borrowed();
        }
        checked_format(template, &values[..self.arguments.len()])
    }
    pub fn english(&self) -> Vec<u8> {
        self.render(self.id.msgid().to_bytes())
            .expect("owned English declaration has a valid schema")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormatError {
    CatalogLookup,
    InvalidSpecifier,
    InvalidPosition,
    MissingArgument,
    ArgumentType,
    MixedPositions,
}

/// A deliberately checked subset of printf compatibility data. No translated
/// bytes or arguments ever reach C varargs, and arbitrary user bytes stay raw.
pub fn checked_format(template: &[u8], arguments: &[Argument<'_>]) -> Result<Vec<u8>, FormatError> {
    let mut output = Vec::with_capacity(template.len());
    let mut cursor = 0;
    let mut next = 0;
    let mut positioned = None;
    fn position(bytes: &[u8], cursor: &mut usize) -> Result<Option<usize>, FormatError> {
        let start = *cursor;
        let mut number = 0usize;
        while let Some(digit) = bytes.get(*cursor).filter(|byte| byte.is_ascii_digit()) {
            number = number
                .checked_mul(10)
                .and_then(|n| n.checked_add(usize::from(*digit - b'0')))
                .ok_or(FormatError::InvalidPosition)?;
            *cursor += 1;
        }
        if *cursor > start && bytes.get(*cursor) == Some(&b'$') {
            *cursor += 1;
            return number
                .checked_sub(1)
                .map(Some)
                .ok_or(FormatError::InvalidPosition);
        }
        *cursor = start;
        Ok(None)
    }
    fn take<'a>(
        arguments: &[Argument<'a>],
        next: &mut usize,
        positioned: &mut Option<bool>,
        position: Option<usize>,
    ) -> Result<Argument<'a>, FormatError> {
        let explicit = position.is_some();
        if positioned.is_some_and(|previous| previous != explicit) {
            return Err(FormatError::MixedPositions);
        }
        *positioned = Some(explicit);
        let index = position.unwrap_or_else(|| {
            let value = *next;
            *next += 1;
            value
        });
        let value = *arguments.get(index).ok_or(FormatError::MissingArgument)?;
        Ok(value)
    }
    while let Some(byte) = template.get(cursor) {
        if *byte != b'%' {
            output.push(*byte);
            cursor += 1;
            continue;
        }
        cursor += 1;
        if template.get(cursor) == Some(&b'%') {
            output.push(b'%');
            cursor += 1;
            continue;
        }
        let value_position = position(template, &mut cursor)?;
        let mut precision = None;
        if template.get(cursor) == Some(&b'.') {
            cursor += 1;
            if template.get(cursor) != Some(&b'*') {
                return Err(FormatError::InvalidSpecifier);
            }
            cursor += 1;
            let precision_position = position(template, &mut cursor)?;
            let Argument::Unsigned(value) =
                take(arguments, &mut next, &mut positioned, precision_position)?
            else {
                return Err(FormatError::ArgumentType);
            };
            precision = Some(usize::try_from(value).map_err(|_| FormatError::InvalidSpecifier)?);
        }
        let specifier = *template.get(cursor).ok_or(FormatError::InvalidSpecifier)?;
        cursor += 1;
        let value = take(arguments, &mut next, &mut positioned, value_position)?;
        match (specifier, value) {
            (b's', Argument::Bytes(bytes)) => {
                let length = bytes
                    .iter()
                    .position(|byte| *byte == 0)
                    .unwrap_or(bytes.len());
                let length = precision.map_or(length, |limit| limit.min(length));
                output.extend_from_slice(&bytes[..length]);
            }
            (b'd', Argument::Signed(value)) if precision.is_none() => {
                output.extend_from_slice(value.to_string().as_bytes())
            }
            (b'u', Argument::Unsigned(value)) if precision.is_none() => {
                output.extend_from_slice(value.to_string().as_bytes())
            }
            (b'c', Argument::Character(value)) if precision.is_none() => output.push(value),
            (b's' | b'd' | b'u' | b'c', _) => return Err(FormatError::ArgumentType),
            _ => return Err(FormatError::InvalidSpecifier),
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn translated_positions_precision_and_raw_bytes_are_checked() {
        assert_eq!(
            checked_format(
                b"%2$s/%1$s/%%",
                &[Argument::Bytes(b"a\xff"), Argument::Bytes(b"b")]
            )
            .unwrap(),
            b"b/a\xff/%"
        );
        assert_eq!(
            checked_format(
                b"%2$.*1$s",
                &[Argument::Unsigned(2), Argument::Bytes(b"a\xffc")]
            )
            .unwrap(),
            b"a\xff"
        );
        assert_eq!(
            checked_format(b"%s", &[Argument::Bytes(b"a\0tail")]).unwrap(),
            b"a"
        );
        assert_eq!(
            checked_format(
                b"%1$s/%1$s",
                &[Argument::Bytes(b"a"), Argument::Bytes(b"unused")]
            )
            .unwrap(),
            b"a/a"
        );
        assert_eq!(
            checked_format(b"fixed", &[Argument::Bytes(b"unused")]).unwrap(),
            b"fixed"
        );
    }
    #[test]
    fn malformed_catalog_schemas_never_reach_varargs() {
        for (template, args, error) in [
            (
                b"%n".as_slice(),
                vec![Argument::Bytes(b"x")],
                FormatError::InvalidSpecifier,
            ),
            (
                b"%s %s",
                vec![Argument::Bytes(b"x")],
                FormatError::MissingArgument,
            ),
            (
                b"%d",
                vec![Argument::Bytes(b"x")],
                FormatError::ArgumentType,
            ),
            (
                b"%0$s",
                vec![Argument::Bytes(b"x")],
                FormatError::InvalidPosition,
            ),
            (
                b"%1$s/%s",
                vec![Argument::Bytes(b"x")],
                FormatError::MixedPositions,
            ),
        ] {
            assert_eq!(checked_format(template, &args), Err(error));
        }
    }
}

/// Either borrowed live data or an owned diagnostic, without another context field.
#[derive(Clone, Copy, Debug)]
pub enum MessageRef<'a> {
    Borrowed(Message<'a>),
    Owned(&'a OwnedMessage),
}
impl MessageRef<'_> {
    pub fn id(self) -> MessageId {
        match self {
            Self::Borrowed(value) => value.id(),
            Self::Owned(value) => value.id,
        }
    }
    pub fn render(self, template: &[u8]) -> Result<Vec<u8>, FormatError> {
        match self {
            Self::Borrowed(value) => value.render(template),
            Self::Owned(value) => value.render(template),
        }
    }
}

#[cfg(test)]
mod boundary_tests {
    use super::*;
    use crate::diagnostic::{ExpansionContext, Location, Output, OutputReceiver};
    use crate::variables::{Assignment, Origin, VariableStore};
    struct Receiver {
        later: bool,
        formatted: usize,
        warnings: Vec<Vec<u8>>,
    }
    impl OutputReceiver for Receiver {
        fn emit(&mut self, event: Output<'_>) {
            match event {
                Output::Warning { message, .. } => self.warnings.push(message.to_vec()),
                Output::Info(_) => {}
                Output::SystemWarning { .. } => panic!("context must freeze system warnings"),
            }
        }
        fn warns_undefined(&self) -> bool {
            true
        }
        fn format_system_message(
            &mut self,
            message: MessageRef<'_>,
        ) -> Option<Result<Vec<u8>, Vec<u8>>> {
            self.formatted += 1;
            let template = match message.id() {
                MessageId::IntcmpFirst => {
                    if self.later {
                        b"LATER:%s".as_slice()
                    } else {
                        b"FIRST:%s"
                    }
                }
                MessageId::UndefinedVariable => b"FIRST:%2$s",
                MessageId::MissingSeparator => b"FIRST:separator",
                MessageId::Stop => {
                    if self.later {
                        b"LATER:stop"
                    } else {
                        b"FIRST:stop"
                    }
                }
                _ => panic!("unexpected system message"),
            };
            Some(message.render(template).map_err(|_| b"schema".to_vec()))
        }
    }
    #[test]
    fn fatal_text_freezes_before_receiver_state_changes_and_keeps_definition_location() {
        let definition = Location {
            file: b"definition.mk".as_slice().into(),
            line: 7,
        };
        let caller = Location {
            file: b"caller.mk".as_slice().into(),
            line: 11,
        };
        let mut store = VariableStore::new();
        store
            .define_at(
                b"FAIL",
                b"$(intcmp bad,0)",
                Assignment::Recursive,
                Origin::File,
                &definition,
            )
            .unwrap();
        let mut receiver = Receiver {
            later: false,
            formatted: 0,
            warnings: vec![],
        };
        let error = store
            .expand_with(
                b"$(FAIL)",
                ExpansionContext::with_receiver(Some(&caller), &mut receiver),
            )
            .unwrap_err();
        receiver.later = true;
        assert_eq!(error.diagnostic().as_bytes(), b"FIRST:bad");
        assert_eq!(error.diagnostic_location(Some(&caller)), Some(&definition));
        assert_eq!(
            error.diagnostic().frozen_stop(),
            Some(b"FIRST:stop".as_slice())
        );
        assert_eq!(receiver.formatted, 2);
    }
    #[test]
    fn warnings_freeze_synchronously_and_user_text_bypasses_catalogs() {
        let store = VariableStore::new();
        let mut receiver = Receiver {
            later: false,
            formatted: 0,
            warnings: vec![],
        };
        store
            .expand_with(
                b"$(MISSING)$(warning missing separator)",
                ExpansionContext::with_receiver(None, &mut receiver),
            )
            .unwrap();
        receiver.later = true;
        assert_eq!(
            receiver.warnings,
            [b"FIRST:MISSING".to_vec(), b"missing separator".to_vec()]
        );
        assert_eq!(receiver.formatted, 1);
        let mut database = crate::parser::Database::default();
        receiver.later = false;
        let error = database
            .read_with(
                b"not a rule\n",
                b"Makefile",
                ExpansionContext::with_receiver(None, &mut receiver),
            )
            .unwrap_err();
        receiver.later = true;
        assert_eq!(error.message.as_bytes(), b"FIRST:separator");
        assert_eq!(error.message.frozen_stop(), Some(b"FIRST:stop".as_slice()));
    }
}

#[cfg(test)]
mod user_error_boundary {
    use super::*;
    use crate::diagnostic::{ExpansionContext, Output, OutputReceiver};
    use crate::variables::VariableStore;
    struct Receiver;
    impl OutputReceiver for Receiver {
        fn emit(&mut self, _: Output<'_>) {
            panic!("unexpected output")
        }
        fn format_system_message(
            &mut self,
            message: MessageRef<'_>,
        ) -> Option<Result<Vec<u8>, Vec<u8>>> {
            assert_eq!(message.id(), MessageId::Stop);
            Some(Ok(b" private-stop\n".to_vec()))
        }
    }
    #[test]
    fn raw_user_fatal_keeps_bytes_and_its_frozen_terminator_through_eval_transport() {
        let mut database = crate::parser::Database::default();
        let error = database
            .read_with(
                b"VALUE := $(error raw\xff)\n",
                b"Makefile",
                ExpansionContext::with_receiver(None, &mut Receiver),
            )
            .unwrap_err();
        assert_eq!(error.message.as_bytes(), b"raw\xff");
        let moved = crate::variables::evaluation_error(error);
        assert_eq!(moved.diagnostic().as_bytes(), b"raw\xff");
        assert_eq!(
            moved.diagnostic().frozen_stop(),
            Some(b" private-stop\n".as_slice())
        );
        let store = VariableStore::new();
        let direct = store
            .expand_with(
                b"$(error text)",
                ExpansionContext::with_receiver(None, &mut Receiver),
            )
            .unwrap_err();
        assert_eq!(direct.diagnostic().as_bytes(), b"text");
    }

    #[test]
    fn raw_file_error_keeps_text_that_matches_a_system_message() {
        let raw = b"file: missing filename";
        let error = crate::variables::ExpandError::FileError(raw.to_vec());
        let diagnostic = ExpansionContext::with_receiver(None, &mut Receiver)
            .freeze_error(error)
            .diagnostic();
        assert_eq!(diagnostic.as_bytes(), raw);
        assert_eq!(
            diagnostic.frozen_stop(),
            Some(b" private-stop\n".as_slice())
        );
    }
}

/// Warning events carry only their own small typed payload, so ordinary info
/// output keeps the pre-NLS event size. Larger fatal/job data stays off that path.
#[derive(Clone, Copy, Debug)]
pub enum WarningMessage<'a> {
    UndefinedVariable(&'a [u8]),
}
impl<'a> WarningMessage<'a> {
    pub fn message(self) -> Message<'a> {
        match self {
            Self::UndefinedVariable(name) => Message::UndefinedVariable(name),
        }
    }
    pub fn english(self) -> Vec<u8> {
        self.message().english()
    }
}

#[cfg(test)]
mod layout_tests {
    #[test]
    fn typed_warnings_do_not_enlarge_ordinary_output_events() {
        assert!(
            std::mem::size_of::<crate::diagnostic::Output<'_>>()
                <= 4 * std::mem::size_of::<usize>()
        );
    }
}
