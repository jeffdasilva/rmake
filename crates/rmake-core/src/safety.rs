// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Explicit compatibility exceptions for recorded GNU Make 4.4.1 hazards.
//! These errors are tested separately from exact oracle conformance.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SafetyError {
    EmptyLet,
    DefaultGoalDeletion,
    ActiveExportDeletion,
    NestedFlags,
    OneShellConfiguration,
}

impl SafetyError {
    pub fn message(self) -> &'static str {
        match self {
            Self::EmptyLet => {
                "rmake safety error [S001]: multiple let names with an empty value list"
            }
            Self::DefaultGoalDeletion => {
                "rmake safety error [S002]: deleting live .DEFAULT_GOAL metadata"
            }
            Self::ActiveExportDeletion => {
                "rmake safety error [S003]: deleting a binding while exporting its value"
            }
            Self::NestedFlags => {
                "rmake safety error [S004]: unsafe reentrant MAKEFLAGS argument decoding"
            }
            Self::OneShellConfiguration => {
                "rmake safety error [S005]: unsafe .ONESHELL shell configuration"
            }
        }
    }
}
