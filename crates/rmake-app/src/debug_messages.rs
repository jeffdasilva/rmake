// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Static GNU catalog keys used by the Rust debug renderer. Build validation
//! and event rendering share these keys and their checked argument shapes.
use crate::system_messages::Argument;
use std::ffi::CStr;

#[derive(Clone, Copy)]
pub(crate) struct Key {
    pub msgid: &'static CStr,
    pub sample: &'static [Argument<'static>],
}

const B: Argument<'static> = Argument::Bytes(b"bytes");
const P: Argument<'static> = Argument::Unsigned(3);
const A: Argument<'static> = Argument::Pointer(0x1000);

macro_rules! keys {
    ($($name:ident: $msgid:expr, [$($sample:expr),*];)+) => {
        $(pub(crate) const $name: Key = Key { msgid: $msgid, sample: &[$($sample),*] };)+
        pub(crate) const ALL: &[Key] = &[$($name),+];
    };
}

keys! {
    READING: c"Reading makefiles...\n", [];
    UPDATING_MAKEFILES: c"Updating makefiles....\n", [];
    UPDATING_GOALS: c"Updating goal targets....\n", [];
    CONSIDERING: c"Considering target file '%s'.\n", [B];
    MISSING: c"File '%s' does not exist.\n", [B];
    FINISHED: c"Finished prerequisites of target file '%s'.\n", [B];
    MUST_REMAKE: c"Must remake target '%s'.\n", [B];
    REMADE: c"Successfully remade target file '%s'.\n", [B];
    FAILED: c"Failed to remake target file '%s'.\n", [B];
    NO_NEED: c"No need to remake target '%s'", [B];
    USING_VPATH: c"; using VPATH name '%s'", [B];
    ALREADY: c"File '%s' was considered already.\n", [B];
    PRUNING: c"Pruning file '%s'.\n", [B];
    GIVE_UP: c"Giving up on target file '%s'.\n", [B];
    DOUBLE_WITHOUT_PREREQUISITES: c"Target '%s' is double-colon and has no prerequisites.\n", [B];
    NO_RECIPE_NO_CHANGES: c"No recipe for '%s' and no prerequisites actually changed.\n", [B];
    ALWAYS_MAKE: c"Making '%s' due to always-make flag.\n", [B];
    LOOKING: c"Looking for an implicit rule for '%s'.\n", [B];
    LOOKING_ARCHIVE: c"Looking for archive-member implicit rule for '%s'.\n", [B];
    NO_ARCHIVE_RULE: c"No archive-member implicit rule found for '%s'.\n", [B];
    TRYING_RULE: c"Trying pattern rule '%s' with stem '%.*s'.\n", [B, P, B];
    TRYING_LITERAL: c"Trying rule prerequisite '%s'.\n", [B];
    TRYING_IMPLICIT: c"Trying implicit prerequisite '%s'.\n", [B];
    TRYING_HARDER: c"Trying harder.\n", [];
    AVOIDING_RECURSION: c"Avoiding implicit rule recursion for rule '%s'.\n", [B];
    FOUND_PREREQUISITE: c"Found '%s'.\n", [B];
    FOUND_VPATH: c"Found prerequisite '%s' as VPATH '%s'.\n", [B, B];
    MISSING_PREREQUISITE: c"Not found '%s'.\n", [B];
    SEARCHING_EXPLICIT: c"Looking for a rule with explicit file '%s'.\n", [B];
    SEARCHING_INTERMEDIATE: c"Looking for a rule with intermediate file '%s'.\n", [B];
    REJECTED_LITERAL: c"Rejecting rule '%s' due to impossible rule prerequisite '%s'.\n", [B, B];
    REJECTED_IMPLICIT: c"Rejecting rule '%s' due to impossible implicit prerequisite '%s'.\n", [B, B];
    REJECTED_PREREQUISITE: c"Rejecting rule '%s' due to impossible prerequisite '%s'.\n", [B, B];
    NOT_OUGHT_TO_EXIST: c"Prerequisite '%s' of rule '%s' does not qualify as ought to exist.\n", [B, B];
    USING_COMPATIBILITY: c"Using compatibility rule '%s' due to '%s'.\n", [B, B];
    SEARCHING_COMPATIBILITY: c"Searching for a compatibility rule for '%s'.\n", [B];
    REMOVING_INTERMEDIATES: c"Removing intermediate files...\n", [];
    OUGHT_TO_EXIST: c"'%s' ought to exist.\n", [B];
    FOUND_RULE: c"Found implicit rule '%s' for '%s'.\n", [B, B];
    NO_RULE: c"No implicit rule found for '%s'.\n", [B];
    READING_FILE: c"Reading makefile '%s'", [B];
    NO_DEFAULT_GOAL: c" (no default goal)", [];
    SEARCH_PATH: c" (search path)", [];
    OPTIONAL: c" (don't care)", [];
    NO_TILDE: c" (no ~ expansion)", [];
    REEXECUTING: c"Re-executing[%u]:", [P];
    PREREQ_OLDER: c"Prerequisite '%s' is older than target '%s'.\n", [B, B];
    PREREQ_NEWER: c"Prerequisite '%s' is newer than target '%s'.\n", [B, B];
    PREREQ_MISSING: c"Prerequisite '%s' of target '%s' does not exist.\n", [B, B];
    PREREQ_ORDER_ONLY: c"Prerequisite '%s' is order-only for target '%s'.\n", [B, B];
    SKIPPING_BOM: c"Skipping UTF-8 BOM in makefile '%s'\n", [B];
    JOB_CONTROLLER: c"Using jobserver controller %s\n", [B];
    JOB_OBTAINED: c"Obtained token for child %p (%s).\n", [A, B];
    JOB_PUT: c"Putting child %p (%s) PID %s%s on the chain.\n", [A, B, B, B];
    JOB_LIVE: c"Live child %p (%s) PID %s %s\n", [A, B, B, B];
    JOB_WIN: c"Reaping winning child %p PID %s %s\n", [A, B, B];
    JOB_LOSE: c"Reaping losing child %p PID %s %s\n", [A, B, B];
    JOB_REMOVE: c"Removing child %p PID %s%s from chain.\n", [A, B, B];
    JOB_RELEASED: c"Released token for child %p (%s).\n", [A, B];
    BUILT_FOR: c"%sBuilt for %s\n", [B, B];
    LICENSE: c"%sLicense GPLv3+: GNU GPL version 3 or later <https://gnu.org/licenses/gpl.html>\n%sThis is free software: you are free to change and redistribute it.\n%sThere is NO WARRANTY, to the extent permitted by law.\n", [B, B, B];
}
