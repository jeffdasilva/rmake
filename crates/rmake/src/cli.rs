// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

use rmake_core::variables::Origin;
use std::ffi::OsString;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::PathBuf;

#[derive(Clone, Default, Debug)]
pub struct Options {
    pub files: Vec<PathBuf>,
    pub evals: Vec<Vec<u8>>,
    pub jobs: Option<usize>,
    pub command_jobs: Option<usize>,
    pub output_sync: Option<Vec<u8>>,
    pub inherited_output_sync: Option<Vec<u8>>,
    pub sync_mutex: Option<Vec<u8>>,
    pub load_limit: Option<f64>,
    pub inherited_jobs: bool,
    pub jobserver_auth: Option<Vec<u8>>,
    pub jobserver_pipe: bool,
    pub reset_jobserver: bool,
    pub directories: Vec<PathBuf>,
    pub include_directories: Vec<PathBuf>,
    pub include_assignment_positions: Vec<usize>,
    pub goals: Vec<Vec<u8>>,
    pub assignments: Vec<Vec<u8>>,
    pub assignment_context: Vec<(usize, bool)>,
    pub flag_word_count: usize,
    pub dry_run: bool,
    pub silent: bool,
    pub silent_origin: Option<Origin>,
    pub always: bool,
    pub ignore_errors: bool,
    pub keep_going: bool,
    pub keep_going_origin: Option<Origin>,
    pub check_symlink_times: bool,
    pub question: bool,
    pub touch: bool,
    pub environment_override: bool,
    pub no_rules: bool,
    pub no_variables: bool,
    pub trace: bool,
    pub warn_undefined: bool,
    pub startup_missing_makeflags_warning: bool,
    pub no_print_directory: bool,
    pub help: bool,
    pub version: bool,
}

impl Options {
    /// MAKEFLAGS uses make's backslash-separated words, not shell quoting.
    /// Only the CLI subset implemented here is accepted; unsupported inherited
    /// scheduling flags must not silently change a recursive invocation.
    pub fn with_environment(
        args: impl IntoIterator<Item = OsString>,
        makeflags: Option<OsString>,
        gnumakeflags: Option<OsString>,
    ) -> Result<Self, String> {
        let missing_makeflags = makeflags.is_none();
        let mut combined = Vec::new();
        for flags in [gnumakeflags, makeflags].into_iter().flatten() {
            Self::append_flags(&mut combined, flags.as_bytes())?;
        }
        // No inherited tokens means there is no source merge. Decode the
        // explicit arguments once, retaining the same jobserver finalization.
        if combined.is_empty() {
            let result = Self::parse(args)?;
            let jobs = result.jobs;
            return Ok(result.with_command_jobs(jobs));
        }
        let inherited_options = Self::parse(combined.clone())?;
        let inherited = inherited_options.jobs;
        let args: Vec<_> = args.into_iter().collect();
        let explicit_options = Self::parse(args.clone())?;
        let explicit = explicit_options.jobs;
        combined.extend(args);
        let mut result = Self::parse(combined)?;
        result.silent_origin = explicit_options
            .silent_origin
            .or_else(|| inherited_options.silent_origin.map(|_| Origin::Environment));
        result.keep_going_origin = explicit_options.keep_going_origin.or_else(|| {
            inherited_options
                .keep_going_origin
                .map(|_| Origin::Environment)
        });
        result.startup_missing_makeflags_warning =
            missing_makeflags && inherited_options.warn_undefined;
        result.inherited_output_sync = inherited_options.output_sync;
        result.inherited_jobs = inherited.is_some() && explicit.is_none();
        Ok(result.with_command_jobs(explicit))
    }

    fn with_command_jobs(mut self, jobs: Option<usize>) -> Self {
        self.command_jobs = jobs;
        self.reset_jobserver = jobs.is_some() && self.jobserver_auth.is_some();
        if self.reset_jobserver {
            self.jobserver_auth = None;
        }
        self
    }

    fn append_flags(combined: &mut Vec<OsString>, flags: &[u8]) -> Result<(), String> {
        let raw = flags;
        let mut decoded = Vec::with_capacity(raw.len());
        let mut pos = 0;
        while pos < raw.len() {
            decoded.push(raw[pos]);
            if raw[pos] == b'$' && raw.get(pos + 1) == Some(&b'$') {
                pos += 1;
            }
            pos += 1;
        }
        Self::append_expanded_flags(combined, &decoded)
    }

    // Expanded file flags have already consumed their make-dollar quoting.
    // GNU skips C whitespace once at the start, then separates only SP/TAB.
    fn append_expanded_flags(combined: &mut Vec<OsString>, mut bytes: &[u8]) -> Result<(), String> {
        while bytes
            .first()
            .is_some_and(|byte| matches!(byte, b' ' | b'\t'..=b'\r'))
        {
            bytes = &bytes[1..];
        }
        let mut tokens = Vec::new();
        let mut token = Vec::new();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'\\' && i + 1 < bytes.len() {
                i += 1;
                token.push(bytes[i]);
            } else if matches!(bytes[i], b' ' | b'\t') {
                if !token.is_empty() {
                    tokens.push(std::mem::take(&mut token));
                }
            } else {
                token.push(bytes[i]);
            }
            i += 1;
        }
        if !token.is_empty() {
            tokens.push(token);
        }
        if let Some(first) = tokens.first_mut()
            && !first.starts_with(b"-")
            && !first.contains(&b'=')
        {
            first.insert(0, b'-');
        }
        let mut inherited_end = false;
        for token in tokens {
            if token == b"--" {
                inherited_end = true;
                continue;
            }
            if inherited_end && token.starts_with(b"-") {
                return Err("option-like goals in inherited make flags are not implemented".into());
            }
            combined.push(OsString::from_vec(token));
        }
        Ok(())
    }

    /// Decode changed file bindings after reading. Existing switches persist;
    /// later file flags can change them, while explicit CLI -j keeps priority.
    pub fn file_flags(&self, flags: &[Vec<u8>]) -> Result<(Self, Option<usize>), String> {
        self.decode_file_flags(flags, true, Origin::Environment)
    }
    pub fn assigned_flags(
        &self,
        flags: &[Vec<u8>],
        origin: Origin,
    ) -> Result<(Self, Option<usize>), String> {
        self.decode_file_flags(flags, false, origin)
    }
    fn decode_file_flags(
        &self,
        flags: &[Vec<u8>],
        restore_cli_jobs: bool,
        origin: Origin,
    ) -> Result<(Self, Option<usize>), String> {
        let mut incoming = Vec::new();
        for value in flags {
            Self::append_expanded_flags(&mut incoming, value)?;
        }
        let incoming_options = Self::parse_mode::<false>(incoming.clone())?;
        let mut arguments = Vec::new();
        Self::append_flags(&mut arguments, &self.recursive_flags())?;
        if self.jobserver_pipe {
            arguments.push("--jobserver-style=pipe".into());
        }
        let prefix_count = arguments.len();
        let flag_word_count = incoming.len() + 1;
        arguments.extend(incoming);
        let mut parsed = Self::parse_mode::<false>(arguments)?;
        for (position, _) in &mut parsed.assignment_context {
            *position = *position - prefix_count + 1;
        }
        if !self.evals.is_empty() {
            let mut seen: std::collections::HashSet<_> = self.evals.iter().cloned().collect();
            let mut evals = self.evals.clone();
            evals.extend(
                parsed
                    .evals
                    .into_iter()
                    .filter(|value| seen.insert(value.clone())),
            );
            parsed.evals = evals;
        }
        parsed.flag_word_count = flag_word_count;
        // GNU only tracks origins for selected switches. Reconstructing the
        // spelling must not turn an inherited/file setting into a CLI setting.
        if incoming_options.silent_origin.is_some()
            && (origin == Origin::CommandLine || self.silent_origin.is_none_or(|old| origin >= old))
        {
            parsed.silent_origin = Some(origin);
            parsed.silent = incoming_options.silent;
        } else {
            parsed.silent_origin = self.silent_origin;
            parsed.silent = self.silent;
        }
        if incoming_options.keep_going_origin.is_some()
            && (origin == Origin::CommandLine
                || self.keep_going_origin.is_none_or(|old| origin >= old))
        {
            parsed.keep_going_origin = Some(origin);
            parsed.keep_going = incoming_options.keep_going;
        } else {
            parsed.keep_going_origin = self.keep_going_origin;
            parsed.keep_going = self.keep_going;
        }
        parsed.command_jobs = self.command_jobs;
        if restore_cli_jobs && let Some(jobs) = self.command_jobs {
            parsed.jobs = Some(jobs);
        }
        Ok((
            parsed,
            incoming_options
                .jobs
                .filter(|_| self.command_jobs.is_none()),
        ))
    }

    pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Self, String> {
        Self::parse_mode::<true>(args)
    }

    fn parse_mode<const BIG_R_IMPLIES_RULES: bool>(
        args: impl IntoIterator<Item = OsString>,
    ) -> Result<Self, String> {
        let args: Vec<Vec<u8>> = args.into_iter().map(OsString::into_vec).collect();
        let mut result = Self::default();
        let mut evals_seen = std::collections::HashSet::new();
        let mut i = 0;
        let mut end_options = false;
        while i < args.len() {
            let arg = &args[i];
            if !end_options && arg == b"--" {
                end_options = true;
                i += 1;
                continue;
            }
            if !end_options && arg.starts_with(b"--") {
                let split = arg.iter().position(|b| *b == b'=');
                let name = split.map_or(arg.as_slice(), |n| &arg[..n]);
                let value = split.map(|n| arg[n + 1..].to_vec());
                let flag = match name {
                    b"--help" => Some(&mut result.help),
                    b"--version" => Some(&mut result.version),
                    b"--dry-run" | b"--just-print" | b"--recon" => Some(&mut result.dry_run),
                    b"--silent" | b"--quiet" => Some(&mut result.silent),
                    b"--always-make" => Some(&mut result.always),
                    b"--ignore-errors" => Some(&mut result.ignore_errors),
                    b"--keep-going" => Some(&mut result.keep_going),
                    b"--check-symlink-times" => Some(&mut result.check_symlink_times),
                    b"--question" => Some(&mut result.question),
                    b"--touch" => Some(&mut result.touch),
                    b"--trace" => Some(&mut result.trace),
                    b"--warn-undefined-variables" => Some(&mut result.warn_undefined),
                    b"--environment-overrides" => Some(&mut result.environment_override),
                    b"--no-builtin-rules" => Some(&mut result.no_rules),
                    b"--no-builtin-variables" => {
                        if BIG_R_IMPLIES_RULES {
                            result.no_rules = true;
                        }
                        Some(&mut result.no_variables)
                    }
                    _ => None,
                };
                if let Some(flag) = flag {
                    if value.is_some() {
                        return Err(format!(
                            "option '{}' doesn't allow an argument",
                            String::from_utf8_lossy(name)
                        ));
                    }
                    *flag = true;
                    match name {
                        b"--silent" | b"--quiet" => {
                            result.silent_origin = Some(Origin::CommandLine)
                        }
                        b"--keep-going" => result.keep_going_origin = Some(Origin::CommandLine),
                        _ => {}
                    }
                } else {
                    match name {
                        b"--no-silent" | b"--no-keep-going" | b"--stop" => {
                            if value.is_some() {
                                return Err(format!(
                                    "option '{}' doesn't allow an argument",
                                    String::from_utf8_lossy(name)
                                ));
                            }
                            if name == b"--no-silent" {
                                result.silent = false;
                                result.silent_origin = Some(Origin::CommandLine);
                            } else {
                                result.keep_going = false;
                                result.keep_going_origin = Some(Origin::CommandLine);
                            }
                        }
                        b"--jobserver-auth" | b"--jobserver-fds" => {
                            result.jobserver_auth = Some(
                                value
                                    .or_else(|| {
                                        i += 1;
                                        args.get(i).cloned()
                                    })
                                    .ok_or("jobserver authentication requires an argument")?,
                            );
                        }
                        b"--jobserver-style" => {
                            let value = value
                                .or_else(|| {
                                    i += 1;
                                    args.get(i).cloned()
                                })
                                .ok_or("jobserver style requires an argument")?;
                            result.jobserver_pipe = match value.as_slice() {
                                b"fifo" => false,
                                b"pipe" => true,
                                _ => return Err("unknown jobserver style".into()),
                            };
                        }
                        b"--no-print-directory" if value.is_none() => {
                            result.no_print_directory = true
                        }
                        b"--file" | b"--makefile" | b"--directory" | b"--include-dir" => {
                            let value = value
                                .or_else(|| {
                                    i += 1;
                                    args.get(i).cloned()
                                })
                                .ok_or_else(|| {
                                    format!(
                                        "option '{}' requires an argument",
                                        String::from_utf8_lossy(name)
                                    )
                                })?;
                            let path = PathBuf::from(OsString::from_vec(value));
                            if name == b"--include-dir" {
                                result.add_include_directory(path)?;
                            } else if name == b"--directory" {
                                result.directories.push(path);
                            } else {
                                result.files.push(path);
                            }
                        }
                        b"--eval" => {
                            let value = value
                                .or_else(|| {
                                    i += 1;
                                    args.get(i).cloned()
                                })
                                .ok_or("option '--eval' requires an argument")?;
                            if value.is_empty() {
                                return Err(
                                    "the '--eval' option requires a non-empty string argument"
                                        .into(),
                                );
                            }
                            if evals_seen.insert(value.clone()) {
                                result.evals.push(value);
                            }
                        }
                        b"--output-sync" => {
                            result.output_sync = Some(value.unwrap_or_else(|| b"target".to_vec()))
                        }
                        b"--sync-mutex" => {
                            result.sync_mutex = Some(
                                value
                                    .or_else(|| {
                                        i += 1;
                                        args.get(i).cloned()
                                    })
                                    .ok_or("option '--sync-mutex' requires an argument")?,
                            );
                        }
                        b"--load-average" | b"--max-load" => {
                            let value = value.or_else(|| {
                                if args.get(i + 1).is_some_and(|v| {
                                    v.first().is_some_and(|b| b.is_ascii_digit() || *b == b'.')
                                }) {
                                    i += 1;
                                    Some(args[i].clone())
                                } else {
                                    None
                                }
                            });
                            result.load_limit =
                                value.map(|v| rmake_linux::process::load_number(&v));
                        }
                        b"--jobs" => {
                            let value = value.or_else(|| {
                                if args
                                    .get(i + 1)
                                    .is_some_and(|v| v.iter().all(u8::is_ascii_digit))
                                {
                                    i += 1;
                                    Some(args[i].clone())
                                } else {
                                    None
                                }
                            });
                            result.jobs = Some(job_count(value.as_deref().unwrap_or_default())?);
                        }
                        _ => {
                            return Err(format!(
                                "unsupported option '{}'",
                                String::from_utf8_lossy(arg)
                            ));
                        }
                    }
                }
            } else if !end_options && arg.starts_with(b"-") && arg.len() > 1 {
                let mut j = 1;
                while j < arg.len() {
                    match arg[j] {
                        b'h' => result.help = true,
                        b'v' => result.version = true,
                        b'n' => result.dry_run = true,
                        b's' => {
                            result.silent = true;
                            result.silent_origin = Some(Origin::CommandLine);
                        }
                        b'B' => result.always = true,
                        b'i' => result.ignore_errors = true,
                        b'k' => {
                            result.keep_going = true;
                            result.keep_going_origin = Some(Origin::CommandLine);
                        }
                        b'S' => {
                            result.keep_going = false;
                            result.keep_going_origin = Some(Origin::CommandLine);
                        }
                        b'b' | b'm' => {}
                        b'L' => result.check_symlink_times = true,
                        b'q' => result.question = true,
                        b't' => result.touch = true,
                        b'e' => result.environment_override = true,
                        b'r' => result.no_rules = true,
                        b'R' => {
                            result.no_variables = true;
                            if BIG_R_IMPLIES_RULES {
                                result.no_rules = true;
                            }
                        }
                        b'O' => {
                            result.output_sync = Some(if j + 1 < arg.len() {
                                arg[j + 1..].to_vec()
                            } else {
                                b"target".to_vec()
                            });
                            break;
                        }
                        b'l' => {
                            let value = if j + 1 < arg.len() {
                                Some(arg[j + 1..].to_vec())
                            } else if args.get(i + 1).is_some_and(|v| {
                                v.first().is_some_and(|b| b.is_ascii_digit() || *b == b'.')
                            }) {
                                i += 1;
                                Some(args[i].clone())
                            } else {
                                None
                            };
                            result.load_limit =
                                value.map(|v| rmake_linux::process::load_number(&v));
                            break;
                        }
                        option @ (b'f' | b'C' | b'j' | b'I' | b'E') => {
                            let value = if j + 1 < arg.len() {
                                arg[j + 1..].to_vec()
                            } else if option == b'j'
                                && !args.get(i + 1).is_some_and(|v| {
                                    !v.is_empty() && v.iter().all(u8::is_ascii_digit)
                                })
                            {
                                Vec::new()
                            } else {
                                i += 1;
                                args.get(i).cloned().ok_or_else(|| {
                                    format!("option requires an argument -- '{}'", option as char)
                                })?
                            };
                            if option == b'E' {
                                if value.is_empty() {
                                    return Err(
                                        "the '-E' option requires a non-empty string argument"
                                            .into(),
                                    );
                                }
                                if evals_seen.insert(value.clone()) {
                                    result.evals.push(value);
                                }
                            } else if option == b'I' {
                                result.add_include_directory(OsString::from_vec(value).into())?;
                            } else if option == b'j' {
                                result.jobs = Some(job_count(&value)?);
                            } else if option == b'f' {
                                result.files.push(OsString::from_vec(value).into());
                            } else {
                                result.directories.push(OsString::from_vec(value).into());
                            }
                            break;
                        }
                        option => return Err(format!("unsupported option '-{}'", option as char)),
                    }
                    j += 1;
                }
            } else if rmake_core::parser::assignment(arg).is_some() {
                result.assignments.push(arg.clone());
                result
                    .assignment_context
                    .push((i, result.environment_override));
            } else {
                result
                    .goals
                    .push(rmake_core::filenames::without_dot_prefix(arg).to_vec());
            }
            i += 1;
        }
        Ok(result)
    }

    fn add_include_directory(&mut self, path: PathBuf) -> Result<(), String> {
        if path.as_os_str().is_empty() {
            return Err("the '-I' option requires a non-empty string argument".into());
        }
        // Runtime expansion observes only assignments preceding this option.
        self.include_assignment_positions
            .push(self.assignments.len());
        self.include_directories.push(path);
        Ok(())
    }

    /// Stored MAKEFLAGS is recursive: literal option dollars need one layer
    /// of make quoting, separate from its encoded command-variable reference.
    pub fn makeflags(&self) -> Vec<u8> {
        let mut flags = self.recursive_flags();
        if !self.evals.is_empty() {
            flags.extend_from_slice(b" $(-*-eval-flags-*-)");
        }
        flags
    }

    pub fn recursive_flags(&self) -> Vec<u8> {
        let flags = self.flags();
        if !flags.contains(&b'$') {
            return flags;
        }
        let mut value =
            Vec::with_capacity(flags.len() + flags.iter().filter(|byte| **byte == b'$').count());
        for byte in flags {
            if byte == b'$' {
                value.push(b'$');
            }
            value.push(byte);
        }
        value
    }

    pub fn flags(&self) -> Vec<u8> {
        let mut flags = Vec::new();
        for (set, flag) in [
            (self.always, b'B'),
            (self.environment_override, b'e'),
            (self.ignore_errors, b'i'),
            (self.keep_going, b'k'),
            (self.check_symlink_times, b'L'),
            (self.dry_run, b'n'),
            (self.question, b'q'),
            (self.no_rules, b'r'),
            (self.no_variables, b'R'),
            (self.silent, b's'),
            (!self.keep_going && self.keep_going_origin.is_some(), b'S'),
            (self.touch, b't'),
        ] {
            if set {
                flags.push(flag);
            }
        }
        for directory in &self.include_directories {
            flags.extend_from_slice(b" -I");
            for &byte in directory.as_os_str().as_bytes() {
                if byte.is_ascii_whitespace() || byte == b'\\' {
                    flags.push(b'\\');
                }
                flags.push(byte);
            }
        }
        if let Some(jobs) = self.jobs {
            flags.extend_from_slice(b" -j");
            if jobs != usize::MAX {
                flags.extend_from_slice(jobs.to_string().as_bytes());
            }
        }
        if let Some(limit) = self.load_limit.filter(|v| *v != -1.0) {
            flags.extend_from_slice(b" -l");
            flags.extend_from_slice(rmake_linux::process::load_spelling(limit).as_bytes());
        }
        if let Some(mode) = &self.output_sync {
            flags.extend_from_slice(b" -O");
            flags.extend_from_slice(mode);
        }
        if let Some(auth) = &self.jobserver_auth {
            flags.extend_from_slice(b" --jobserver-auth=");
            for byte in auth {
                if byte.is_ascii_whitespace() || *byte == b'\\' {
                    flags.push(b'\\');
                }
                flags.push(*byte);
            }
        }
        if let Some(auth) = &self.sync_mutex {
            flags.extend_from_slice(b" --sync-mutex=");
            for byte in auth {
                if byte.is_ascii_whitespace() || *byte == b'\\' {
                    flags.push(b'\\');
                }
                flags.push(*byte);
            }
        }
        if self.trace {
            flags.extend_from_slice(b" --trace");
        }
        if self.no_print_directory {
            flags.extend_from_slice(b" --no-print-directory");
        }
        if self.warn_undefined {
            flags.extend_from_slice(b" --warn-undefined-variables");
        }
        if !self.silent && self.silent_origin.is_some() {
            flags.extend_from_slice(b" --no-silent");
        }
        flags
    }
}

fn job_count(value: &[u8]) -> Result<usize, String> {
    if value.is_empty() {
        return Ok(usize::MAX);
    }
    std::str::from_utf8(value)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .ok_or_else(|| "the '-j' option requires a positive integer argument".into())
}

pub fn invocation_name(arg: &std::ffi::OsStr) -> Vec<u8> {
    std::path::Path::new(arg)
        .file_name()
        .unwrap_or(arg)
        .as_bytes()
        .to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warning_option_serializes_and_tracks_the_initial_gnu_flag_boundary() {
        let direct =
            Options::with_environment(["--warn-undefined-variables".into()], None, None).unwrap();
        assert!(direct.warn_undefined);
        assert!(!direct.startup_missing_makeflags_warning);
        assert!(
            direct
                .recursive_flags()
                .ends_with(b" --warn-undefined-variables")
        );
        let inherited = Options::with_environment(
            Vec::<OsString>::new(),
            None,
            Some("--warn-undefined-variables".into()),
        )
        .unwrap();
        assert!(inherited.warn_undefined && inherited.startup_missing_makeflags_warning);
        let present = Options::with_environment(
            Vec::<OsString>::new(),
            Some("".into()),
            Some("--warn-undefined-variables".into()),
        )
        .unwrap();
        assert!(present.warn_undefined);
        assert!(!present.startup_missing_makeflags_warning);
    }
    fn parse(args: &[&str]) -> Result<Options, String> {
        Options::parse(args.iter().map(OsString::from))
    }

    #[test]
    fn stored_recursive_flags_preserve_literal_dollars_after_make_expansion() {
        let options = Options::parse(["-Idir$dollar".into()]).unwrap();
        assert_eq!(options.flags(), b" -Idir$dollar");
        let mut variables = rmake_core::variables::VariableStore::new();
        variables
            .define(
                b"MAKEFLAGS",
                &options.recursive_flags(),
                rmake_core::variables::Assignment::Recursive,
                rmake_core::variables::Origin::File,
            )
            .unwrap();
        assert_eq!(variables.expand(b"$(MAKEFLAGS)").unwrap(), options.flags());
        let (assigned, _) = options
            .assigned_flags(&[b"-k".to_vec()], Origin::File)
            .unwrap();
        assert_eq!(assigned.include_directories, options.include_directories);
    }

    #[test]
    fn immediate_flags_expose_requested_jobs_before_final_cli_priority_is_restored() {
        let options = Options::with_environment(["-j1".into()], None, None).unwrap();
        assert_eq!(
            options
                .assigned_flags(&[b"-j3 -Oline".to_vec()], Origin::File)
                .unwrap()
                .0
                .jobs,
            Some(3)
        );
        let final_options = options.file_flags(&[b"-j3 -Oline".to_vec()]).unwrap().0;
        assert_eq!(final_options.jobs, Some(1));
        assert_eq!(
            final_options.output_sync.as_deref(),
            Some(b"line".as_slice())
        );
    }

    #[test]
    fn output_sync_optional_arguments_and_recursive_flags_preserve_explicit_none() {
        for (argument, expected) in [
            ("-O", "target"),
            ("-Oline", "line"),
            ("--output-sync", "target"),
            ("--output-sync=recurse", "recurse"),
            ("--output-sync=none", "none"),
        ] {
            let options = Options::parse([argument.into(), "goal".into()]).unwrap();
            assert_eq!(options.output_sync.as_deref(), Some(expected.as_bytes()));
            assert_eq!(options.goals, [b"goal".to_vec()]);
            assert_eq!(options.flags(), format!(" -O{expected}").as_bytes());
        }
    }

    #[test]
    fn output_sync_inheritance_keeps_the_startup_mode_and_last_explicit_choice() {
        let options = Options::with_environment(
            ["-Onone".into()],
            Some("-Oline --sync-mutex=fnm:path".into()),
            None,
        )
        .unwrap();
        assert_eq!(options.output_sync.as_deref(), Some(b"none".as_slice()));
        assert_eq!(
            options.inherited_output_sync.as_deref(),
            Some(b"line".as_slice())
        );
        assert_eq!(options.flags(), b" -Onone --sync-mutex=fnm:path");
    }

    #[test]
    fn load_options_preserve_c_numeric_prefixes_and_recursive_flag_spelling() {
        for (text, spelling) in [
            ("-l2.5junk", "2.5"),
            ("-l0x10", "16"),
            ("-l-2", "-2"),
            ("-lnan", "nan"),
            ("-linf", "inf"),
            ("-l-0", "-0"),
            ("--max-load=3.1415927", "3.14159"),
        ] {
            let parsed = parse(&[text]).unwrap();
            assert_eq!(parsed.flags(), format!(" -l{spelling}").as_bytes());
        }
        assert_eq!(parse(&["-l2", "-l"]).unwrap().flags(), b"");
        assert_eq!(parse(&["-l-1"]).unwrap().flags(), b"");
        assert_eq!(parse(&["-l", "2.25"]).unwrap().load_limit, Some(2.25));
    }

    #[test]
    fn explicit_load_settings_override_inherited_limits_including_reset() {
        let inherited = Options::with_environment([], Some("rR -l2.5".into()), None).unwrap();
        assert_eq!(inherited.load_limit, Some(2.5));
        let changed =
            Options::with_environment(["--load-average=4".into()], Some("-l2.5".into()), None)
                .unwrap();
        assert_eq!(changed.load_limit, Some(4.0));
        let cleared = Options::with_environment(["-l".into()], Some("-l2.5".into()), None).unwrap();
        assert_eq!(cleared.load_limit, None);
    }

    #[test]
    fn grouped_short_options_and_values() {
        let options = parse(&[
            "-rRs",
            "-finput.mk",
            "-C",
            "nested",
            "-j1",
            "NAME=value",
            "all",
        ])
        .unwrap();
        assert!(options.no_rules && options.no_variables && options.silent);
        assert_eq!(options.files, [PathBuf::from("input.mk")]);
        assert_eq!(options.directories, [PathBuf::from("nested")]);
        assert_eq!(options.goals, [b"all".to_vec()]);
        assert_eq!(options.assignments, [b"NAME=value".to_vec()]);
    }

    #[test]
    fn parallel_options_and_zero_rejection() {
        for args in [&["-j2"][..], &["--jobs"], &["-j", "all"]] {
            assert!(parse(args).unwrap().jobs.is_some());
        }
        assert!(parse(&["--jobs=0"]).is_err());
        assert_eq!(parse(&["--jobs", "1"]).unwrap().jobs, Some(1));
    }

    #[test]
    fn options_end_and_non_utf8_goals() {
        let options =
            Options::parse([OsString::from("--"), OsString::from_vec(b"-\xff".to_vec())]).unwrap();
        assert_eq!(options.goals, [b"-\xff".to_vec()]);
        assert!(parse(&["--silent=yes"]).is_err());
    }

    #[test]
    fn inherited_dry_run_and_escaped_assignments_are_preserved() {
        let options = Options::with_environment(
            [OsString::from("-S"), OsString::from("all")],
            Some("nk -- X=two\\ words".into()),
            None,
        )
        .unwrap();
        assert!(options.dry_run);
        assert!(!options.keep_going);
        assert_eq!(options.assignments, [b"X=two words".to_vec()]);
        assert_eq!(options.goals, [b"all".to_vec()]);
        assert_eq!(
            Options::with_environment([], Some("--jobserver-auth=3,4".into()), None)
                .unwrap()
                .jobserver_auth,
            Some(b"3,4".to_vec())
        );
        assert!(Options::with_environment([], Some("r -- -n".into()), None).is_err());
    }

    #[test]
    fn jobserver_transport_and_explicit_job_reset_are_distinct() {
        let inherited = Options::with_environment(
            [],
            Some("-j --jobserver-auth=fifo:/tmp/server".into()),
            None,
        )
        .unwrap();
        assert!(inherited.inherited_jobs);
        assert!(!inherited.reset_jobserver);
        assert_eq!(inherited.jobserver_auth, Some(b"fifo:/tmp/server".to_vec()));
        let reset =
            Options::with_environment(["-j2".into()], Some("-j --jobserver-auth=3,4".into()), None)
                .unwrap();
        assert!(!reset.inherited_jobs);
        assert!(reset.reset_jobserver);
        assert!(reset.jobserver_auth.is_none());
        let pipe = parse(&["--jobserver-style=pipe", "--jobs=3"]).unwrap();
        assert!(pipe.jobserver_pipe);
        assert_eq!(pipe.jobs, Some(3));
        assert!(parse(&["--jobserver-style=invalid"]).is_err());
    }

    #[test]
    fn no_print_directory_survives_makeflags_with_gnu_spacing() {
        let options = parse(&["--no-print-directory", "--no-print-directory"]).unwrap();
        assert_eq!(options.flags(), b" --no-print-directory");
        let inherited = Options::with_environment(
            [],
            Some(OsString::from_vec(b"rR --no-print-directory".to_vec())),
            None,
        )
        .unwrap();
        assert!(inherited.no_print_directory);
        assert_eq!(inherited.flags(), b"rR --no-print-directory");
    }

    #[test]
    fn symlink_time_options_round_trip_in_gnu_flag_order() {
        let options = parse(&["-sBikLntq", "-rR", "--no-print-directory"]).unwrap();
        assert!(options.check_symlink_times);
        assert_eq!(options.flags(), b"BikLnqrRst --no-print-directory");
        let inherited =
            Options::with_environment([], Some(OsString::from_vec(options.flags())), None).unwrap();
        assert!(inherited.check_symlink_times);
        assert_eq!(inherited.flags(), options.flags());
        let long = parse(&["--check-symlink-times", "-LL"]).unwrap();
        assert_eq!(long.flags(), b"L");
        assert!(parse(&["--check-symlink-times=yes"]).is_err());
    }

    #[test]
    fn include_options_preserve_raw_paths_and_quote_recursive_flags() {
        let options = parse(&[
            "-I-",
            "-Ia/",
            "-Ia",
            "-Ia/",
            "--include-dir=two words",
            "-Iback\\slash",
        ])
        .unwrap();
        assert_eq!(
            options.flags(),
            b" -I- -Ia/ -Ia -Ia/ -Itwo\\ words -Iback\\\\slash"
        );
        let inherited =
            Options::with_environment([], Some(OsString::from_vec(options.flags())), None).unwrap();
        assert_eq!(inherited.flags(), options.flags());
        assert!(parse(&["--include-dir="]).is_err());
        assert!(parse(&["-I"]).is_err());
    }

    #[test]
    fn inherited_include_argument_is_not_scanned_as_short_options() {
        let options =
            Options::with_environment([], Some("-Iconfig -IConfig".into()), None).unwrap();
        assert_eq!(options.include_directories.len(), 2);
        let inherited_file = Options::with_environment([], Some("-fconfig".into()), None).unwrap();
        assert_eq!(inherited_file.files, [PathBuf::from("config")]);
        let non_utf8 = Options::parse([OsString::from_vec(b"-I\xff".to_vec())]).unwrap();
        assert_eq!(non_utf8.flags(), b" -I\xff");
    }
    #[test]
    fn trace_flag_is_retained_in_live_decoding_and_recursive_order() {
        let options = Options::parse(
            [
                "-rR",
                "--trace",
                "--no-print-directory",
                "--warn-undefined-variables",
                "--no-silent",
            ]
            .map(OsString::from),
        )
        .unwrap();
        assert!(options.trace);
        assert_eq!(
            options.recursive_flags(),
            b"rR --trace --no-print-directory --warn-undefined-variables --no-silent"
        );
        let (updated, _) = options.file_flags(&[b"-i".to_vec()]).unwrap();
        assert!(updated.trace);
        assert!(updated.ignore_errors);
        assert!(Options::parse([OsString::from("--trace=all")]).is_err());
    }
    #[test]
    fn eval_arguments_deduplicate_bytes_and_survive_live_flags_without_entering_mflags() {
        let options = Options::parse(
            [
                "-EX=one",
                "--eval",
                "X=two $X",
                "--eval=X=one",
                "-rR",
                "--no-print-directory",
            ]
            .map(OsString::from),
        )
        .unwrap();
        assert_eq!(options.evals, [b"X=one".to_vec(), b"X=two $X".to_vec()]);
        assert_eq!(options.recursive_flags(), b"rR --no-print-directory");
        assert_eq!(
            options.makeflags(),
            b"rR --no-print-directory $(-*-eval-flags-*-)"
        );
        let (updated, _) = options
            .file_flags(&[b"--eval=X=three --eval=X=one -s".to_vec()])
            .unwrap();
        assert_eq!(
            updated.evals,
            [b"X=one".to_vec(), b"X=two $X".to_vec(), b"X=three".to_vec()]
        );
        assert!(updated.silent);
    }
    #[test]
    fn runtime_big_r_defers_its_implied_rule_flag_until_the_post_read_phase() {
        let startup = Options::parse([OsString::from("-R")]).unwrap();
        assert!(startup.no_rules && startup.no_variables);
        let (live, _) = Options::default()
            .assigned_flags(&[b"-R".to_vec()], Origin::File)
            .unwrap();
        assert!(live.no_variables);
        assert!(!live.no_rules);
        assert_eq!(live.flags(), b"R");
        let (explicit, _) = Options::default()
            .assigned_flags(&[b"-Rr".to_vec()], Origin::File)
            .unwrap();
        assert!(explicit.no_rules && explicit.no_variables);
    }
}
