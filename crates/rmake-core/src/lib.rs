// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! GNU Make compatibility semantics, independent of operating-system effects.
//!
//! This crate is under development; the version identifies the intended
//! compatibility target, not a claim that it is already implemented.

pub const COMPATIBILITY_VERSION: &str = "4.4.1";

pub mod parser;
pub mod reader;
pub mod variables;
