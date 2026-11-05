// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

use rmake_core::variables::Origin;
use std::ffi::OsString;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::PathBuf;

// The compatibility spelling order is also observable in an ambiguous-option
// diagnostic. Aliases follow the primary switch names.
const GNU_LONG_OPTIONS: &[&[u8]] = &[
    b"--always-make",
    b"--environment-overrides",
    b"--eval",
    b"--help",
    b"--ignore-errors",
    b"--keep-going",
    b"--check-symlink-times",
    b"--just-print",
    b"--print-data-base",
    b"--question",
    b"--no-builtin-rules",
    b"--no-builtin-variables",
    b"--silent",
    b"--no-keep-going",
    b"--touch",
    b"--version",
    b"--print-directory",
    b"--directory",
    b"--file",
    b"--include-dir",
    b"--jobs",
    b"--load-average",
    b"--old-file",
    b"--output-sync",
    b"--what-if",
    b"--debug",
    b"--jobserver-auth",
    b"--trace",
    b"--no-print-directory",
    b"--warn-undefined-variables",
    b"--sync-mutex",
    b"--no-silent",
    b"--jobserver-fds",
    b"--temp-stdin",
    b"--shuffle",
    b"--jobserver-style",
    b"--quiet",
    b"--stop",
    b"--new-file",
    b"--assume-new",
    b"--assume-old",
    b"--max-load",
    b"--dry-run",
    b"--recon",
    b"--makefile",
];

// Resolve only the fallback path: exact options retain their existing fast
// dispatch. Include the complete GNU namespace so unsupported or alias spellings
// still prevent accepting an ambiguous prefix. No parser/global cursor is shared.
#[cold]
fn expand_unique_long_option(argument: &[u8]) -> Option<Vec<u8>> {
    let end = argument
        .iter()
        .position(|&byte| byte == b'=')
        .unwrap_or(argument.len());
    let name = &argument[..end];
    let mut matches = GNU_LONG_OPTIONS
        .iter()
        .copied()
        .filter(|candidate| candidate.starts_with(name));
    let selected = matches.next()?;
    if selected == name || matches.next().is_some() {
        return None;
    }
    let mut expanded = Vec::with_capacity(selected.len() + argument.len() - end);
    expanded.extend_from_slice(selected);
    expanded.extend_from_slice(&argument[end..]);
    Some(expanded)
}

/// Recoverable startup syntax failures are emitted at their argument, while
/// the surrounding source still runs its later definitions and flags.
#[derive(Clone)]
pub(crate) enum UsageIssue {
    Empty(&'static [u8]),
    Jobs,
    MissingShort(u8),
    UnknownShort(u8),
    MissingLong(Vec<u8>),
    UnexpectedLong(Vec<u8>),
    UnknownLong(Vec<u8>),
}

impl UsageIssue {
    pub(crate) fn notice(&self) -> Option<rmake_core::diagnostic::FlagNotice> {
        use rmake_core::diagnostic::FlagNotice;
        match self {
            Self::Empty(name) => Some(FlagNotice::EmptyOption(name)),
            Self::Jobs => Some(FlagNotice::InvalidJobs),
            _ => None,
        }
    }

    pub(crate) fn getopt_line(&self, invocation: &[u8], localized: bool) -> Option<Vec<u8>> {
        use rmake_core::messages::Argument::{Bytes, Character};

        let short = |key, letter| {
            crate::usage::getopt(key, &[Bytes(invocation), Character(letter)], localized)
        };
        let long = |key, name: &[u8]| {
            let operand = name.strip_prefix(b"--").expect("long option spelling");
            crate::usage::getopt(
                key,
                &[Bytes(invocation), Bytes(b"--"), Bytes(operand)],
                localized,
            )
        };
        Some(match self {
            Self::Empty(_) | Self::Jobs => return None,
            Self::MissingShort(letter) => {
                short(c"%s: option requires an argument -- '%c'\n", *letter)
            }
            Self::UnknownShort(letter) => short(c"%s: invalid option -- '%c'\n", *letter),
            Self::MissingLong(name) => long(c"%s: option '%s%s' requires an argument\n", name),
            Self::UnexpectedLong(name) => {
                long(c"%s: option '%s%s' doesn't allow an argument\n", name)
            }
            Self::UnknownLong(arg) => {
                let name = arg.split(|byte| *byte == b'=').next().unwrap_or_default();
                let mut names = GNU_LONG_OPTIONS
                    .iter()
                    .filter(|candidate| candidate.starts_with(name));
                if let (Some(first), Some(second)) = (names.next(), names.next()) {
                    let mut result = long(c"%s: option '%s%s' is ambiguous; possibilities:", arg);
                    for candidate in [first, second].into_iter().chain(names) {
                        result.extend_from_slice(b" '");
                        result.extend_from_slice(candidate);
                        result.push(b'\'');
                    }
                    result.push(b'\n');
                    result
                } else {
                    long(c"%s: unrecognized option '%s%s'\n", arg)
                }
            }
        })
    }

    pub(crate) fn message(&self) -> Vec<u8> {
        match self {
            Self::Empty(option) => crate::usage::empty(option),
            Self::Jobs => crate::usage::jobs(),
            _ => self
                .getopt_line(b"", false)
                .and_then(|line| {
                    line.strip_prefix(b": ")?
                        .strip_suffix(b"\n")
                        .map(<[u8]>::to_vec)
                })
                .expect("English getopt error framing"),
        }
    }
}

/// A decoded operand or policy event, at its argument position.
#[derive(Clone)]
pub(crate) enum Argument {
    Definition(usize),
    File(usize),
    OldFile(usize),
    NewFile(usize),
    Directory(usize),
    Include(usize),
    Goal(usize),
    Warning(usize),
    Usage(usize, UsageIssue),
}

/// Explicit selectors retain their source; automatic reporting is evaluated
/// against the current silent policy and the invocation's startup context.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DirectoryChoice {
    #[default]
    Automatic,
    On,
    Off,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct StdinArgument {
    pub(crate) argument: usize,
    pub(crate) consumed: usize,
    pub(crate) prefix: usize,
}

#[derive(Clone, Default, Debug)]
pub struct Options {
    pub files: Vec<PathBuf>,
    pub(crate) temp_stdin_file: Option<usize>,
    pub(crate) stdin_argument: Option<StdinArgument>,
    pub old_files: Vec<PathBuf>,
    pub new_files: Vec<PathBuf>,
    pub evals: Vec<Vec<u8>>,
    pub jobs: Option<usize>,
    pub command_jobs: Option<usize>,
    pub output_sync: Option<Vec<u8>>,
    pub shuffle: Option<Vec<u8>>,
    pub inherited_output_sync: Option<Vec<u8>>,
    pub sync_mutex: Option<Vec<u8>>,
    pub load_limit: Option<f64>,
    pub inherited_jobs: bool,
    pub jobserver_auth: Option<Vec<u8>>,
    pub jobserver_pipe: bool,
    pub(crate) unknown_jobserver_style: Option<Vec<u8>>,
    pub reset_jobserver: bool,
    pub directories: Vec<PathBuf>,
    pub include_directories: Vec<PathBuf>,
    pub include_assignment_positions: Vec<usize>,
    pub goals: Vec<Vec<u8>>,
    pub assignments: Vec<Vec<u8>>,
    pub assignment_context: Vec<(usize, bool)>,
    pub assignment_warnings: Vec<bool>,
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
    pub print_database: bool,
    pub debug: crate::debug::Requests,
    pub debug_mask: crate::debug::Mask,
    pub warn_undefined: bool,
    pub startup_missing_makeflags_warning: bool,
    pub directory_choice: DirectoryChoice,
    pub directory_origin: Option<Origin>,
    pub help: bool,
    pub version: bool,
    pub(crate) invalid_usage: bool,
    pub(crate) flag_notices: Vec<rmake_core::diagnostic::FlagNoticeAt>,
}

/// An owned argument cursor. A yielded operand leaves no borrowed argv,
/// option state or parser callback on the stack.
#[derive(Default)]
pub(crate) struct Decoder {
    args: Vec<Vec<u8>>,
    pub(crate) options: Options,
    pub(crate) cursor: usize,
    end_options: bool,
    offset: usize,
    track_jobs: bool,
    evals_seen: std::collections::HashSet<Vec<u8>>,
}

#[derive(Default)]
struct IncomingSwitches {
    silent: Option<bool>,
    keep_going: Option<bool>,
    jobs: Option<usize>,
    directory: Option<DirectoryChoice>,
}

pub(crate) struct DecodedArgument {
    pub(crate) argument: Option<Argument>,
    pub(crate) position: usize,
    pub(crate) usage: Vec<UsageIssue>,
}

impl Decoder {
    pub(crate) fn terminal(&self) -> usize {
        self.args.len() + 1
    }
    pub(crate) fn following_definition(&self) -> bool {
        self.cursor == 1
            && self
                .args
                .get(self.cursor)
                .is_some_and(|arg| rmake_core::parser::assignment(arg).is_some())
    }

    pub(crate) fn new(
        args: impl IntoIterator<Item = OsString>,
        options: Options,
        offset: usize,
        track_jobs: bool,
    ) -> Self {
        Self {
            args: args.into_iter().map(OsString::into_vec).collect(),
            evals_seen: options.evals.iter().cloned().collect(),
            options,
            offset,
            track_jobs,
            ..Self::default()
        }
    }

    pub(crate) fn advance<const STARTUP: bool>(
        &mut self,
    ) -> Result<Option<DecodedArgument>, String> {
        self.advance_in_sources::<STARTUP>(&[])
    }

    fn advance_in_sources<const STARTUP: bool>(
        &mut self,
        source_ends: &[usize],
    ) -> Result<Option<DecodedArgument>, String> {
        let mut args = std::mem::take(&mut self.args);
        let mut evals_seen = std::mem::take(&mut self.evals_seen);
        let result = &mut self.options;
        let mut i = self.cursor;
        let mut end_options = self.end_options;
        let offset = self.offset;
        let track_jobs = self.track_jobs;
        let mut expanded_long = None;
        let mut usage = Vec::new();
        let mut ends = source_ends[source_ends.partition_point(|end| *end < i)..].iter();
        let mut source_end = ends.next().copied().unwrap_or(args.len());
        macro_rules! pause {
            ($event:expr) => {{
                let step = DecodedArgument {
                    argument: $event,
                    position: i + offset,
                    usage,
                };
                self.args = args;
                self.evals_seen = evals_seen;
                self.cursor = i + 1;
                self.end_options = end_options;
                return Ok(Some(step));
            }};
        }
        while i < args.len() {
            while i == source_end {
                end_options = false;
                source_end = ends.next().copied().unwrap_or(args.len());
            }
            let argument_index = i;
            let source_args = &args[..source_end];
            let arg = &source_args[i];
            if !end_options && arg == b"--" {
                end_options = true;
                i += 1;
                continue;
            }
            if !end_options && arg.starts_with(b"--") {
                let split = arg.iter().position(|b| *b == b'=');
                let name = split.map_or(arg.as_slice(), |n| &arg[..n]);
                let value = split.map(|n| arg[n + 1..].to_vec());
                // A missing required operand is an invalid getopt result.
                // Live decoding skips it without applying a switch or calling
                // an operand receiver. Startup still reports the usage error.
                if value.is_none()
                    && i + 1 == source_end
                    && matches!(
                        name,
                        b"--file"
                            | b"--makefile"
                            | b"--temp-stdin"
                            | b"--directory"
                            | b"--include-dir"
                            | b"--old-file"
                            | b"--assume-old"
                            | b"--new-file"
                            | b"--assume-new"
                            | b"--what-if"
                            | b"--eval"
                            | b"--jobserver-auth"
                            | b"--jobserver-fds"
                            | b"--jobserver-style"
                            | b"--sync-mutex"
                    )
                {
                    if STARTUP {
                        usage.push(UsageIssue::MissingLong(name.to_vec()));
                        pause!(None);
                    }
                    i += 1;
                    continue;
                }
                if value.as_deref() == Some(b"".as_slice()) {
                    let option = match name {
                        b"--output-sync" => Some(b"-O".as_slice()),
                        b"--debug" => Some(b"--debug".as_slice()),
                        b"--shuffle" => Some(b"--shuffle".as_slice()),
                        b"--jobserver-auth" => Some(b"--jobserver-auth".as_slice()),
                        b"--jobserver-fds" => Some(b"--jobserver-fds".as_slice()),
                        b"--jobserver-style" => Some(b"--jobserver-style".as_slice()),
                        b"--sync-mutex" => Some(b"--sync-mutex".as_slice()),
                        _ => None,
                    };
                    if let Some(option) = option {
                        usage.push(UsageIssue::Empty(option));
                        pause!(None);
                    }
                }
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
                    b"--print-data-base" => Some(&mut result.print_database),
                    b"--warn-undefined-variables" => Some(&mut result.warn_undefined),
                    b"--environment-overrides" => Some(&mut result.environment_override),
                    b"--no-builtin-rules" => Some(&mut result.no_rules),
                    b"--no-builtin-variables" => Some(&mut result.no_variables),
                    _ => None,
                };
                if let Some(flag) = flag {
                    if value.is_some() {
                        if !STARTUP {
                            i += 1;
                            continue;
                        }
                        usage.push(UsageIssue::UnexpectedLong(name.to_vec()));
                        pause!(None);
                    }
                    *flag = true;
                    if STARTUP && name == b"--no-builtin-variables" {
                        result.no_rules = true;
                    }
                    match name {
                        b"--warn-undefined-variables" if !STARTUP => {
                            pause!(Some(Argument::Warning(i + offset)));
                        }
                        b"--silent" | b"--quiet" => {
                            result.silent_origin = Some(Origin::CommandLine)
                        }
                        b"--keep-going" => result.keep_going_origin = Some(Origin::CommandLine),
                        _ => {}
                    }
                } else {
                    match name {
                        b"--debug" => result
                            .debug
                            .push(value.unwrap_or_else(|| b"basic".to_vec())),
                        b"--print-directory" | b"--no-print-directory" => {
                            if value.is_some() {
                                if !STARTUP {
                                    i += 1;
                                    continue;
                                }
                                usage.push(UsageIssue::UnexpectedLong(name.to_vec()));
                                pause!(None);
                            }
                            result.directory_choice = if name == b"--print-directory" {
                                DirectoryChoice::On
                            } else {
                                DirectoryChoice::Off
                            };
                            result.directory_origin = Some(Origin::CommandLine);
                        }
                        b"--no-silent" | b"--no-keep-going" | b"--stop" => {
                            if value.is_some() {
                                if !STARTUP {
                                    i += 1;
                                    continue;
                                }
                                usage.push(UsageIssue::UnexpectedLong(name.to_vec()));
                                pause!(None);
                            }
                            if name == b"--no-silent" {
                                result.silent = false;
                                result.silent_origin = Some(Origin::CommandLine);
                            } else {
                                result.keep_going = false;
                                result.keep_going_origin = Some(Origin::CommandLine);
                            }
                        }
                        b"--shuffle" => {
                            result.shuffle = Some(value.unwrap_or_else(|| b"random".to_vec()));
                        }
                        b"--jobserver-auth" | b"--jobserver-fds" => {
                            let value = value
                                .or_else(|| {
                                    i += 1;
                                    source_args.get(i).cloned()
                                })
                                .ok_or("jobserver authentication requires an argument")?;
                            if value.is_empty() {
                                usage.push(UsageIssue::Empty(if name == b"--jobserver-auth" {
                                    b"--jobserver-auth"
                                } else {
                                    b"--jobserver-fds"
                                }));
                                pause!(None);
                            }
                            result.jobserver_auth = Some(value);
                        }
                        b"--jobserver-style" => {
                            let value = value
                                .or_else(|| {
                                    i += 1;
                                    source_args.get(i).cloned()
                                })
                                .ok_or("jobserver style requires an argument")?;
                            if value.is_empty() {
                                usage.push(UsageIssue::Empty(b"--jobserver-style"));
                                pause!(None);
                            }
                            result.jobserver_pipe = value == b"pipe";
                            result.unknown_jobserver_style =
                                (!matches!(value.as_slice(), b"fifo" | b"pipe")).then_some(value);
                        }
                        b"--file" | b"--makefile" | b"--temp-stdin" | b"--directory"
                        | b"--include-dir" | b"--old-file" | b"--assume-old" | b"--new-file"
                        | b"--assume-new" | b"--what-if" => {
                            let value = value
                                .or_else(|| {
                                    i += 1;
                                    source_args.get(i).cloned()
                                })
                                .ok_or_else(|| {
                                    format!(
                                        "option '{}' requires an argument",
                                        String::from_utf8_lossy(name)
                                    )
                                })?;
                            if value.is_empty() {
                                let canonical: &'static [u8] = match name {
                                    b"--file" | b"--makefile" => b"-f",
                                    b"--directory" => b"-C",
                                    b"--include-dir" => b"-I",
                                    b"--old-file" | b"--assume-old" => b"-o",
                                    b"--new-file" | b"--assume-new" | b"--what-if" => b"-W",
                                    _ => b"--temp-stdin",
                                };
                                if STARTUP || canonical == b"-I" {
                                    usage.push(UsageIssue::Empty(canonical));
                                }
                                pause!(None);
                            }
                            let path = PathBuf::from(OsString::from_vec(value));
                            if matches!(name, b"--old-file" | b"--assume-old") {
                                result.old_files.push(path);
                                let index = result.old_files.len() - 1;
                                pause!(Some(Argument::OldFile(index)));
                            } else if matches!(name, b"--new-file" | b"--assume-new" | b"--what-if")
                            {
                                result.new_files.push(path);
                                let index = result.new_files.len() - 1;
                                pause!(Some(Argument::NewFile(index)));
                            } else if name == b"--include-dir" {
                                result.add_include_directory(path)?;
                                let index = result.include_directories.len() - 1;
                                pause!(Some(Argument::Include(index)));
                            } else if name == b"--directory" {
                                result.directories.push(path);
                                let index = result.directories.len() - 1;
                                pause!(Some(Argument::Directory(index)));
                            } else {
                                if name == b"--temp-stdin" {
                                    result.temp_stdin_file = Some(result.files.len());
                                } else if track_jobs
                                    && path == std::path::Path::new("-")
                                    && result.stdin_argument.is_none()
                                    && expanded_long != Some(argument_index)
                                {
                                    result.stdin_argument = Some(StdinArgument {
                                        argument: argument_index,
                                        consumed: i - argument_index + 1,
                                        prefix: 0,
                                    });
                                }
                                result.files.push(path);
                                let index = result.files.len() - 1;
                                pause!(Some(Argument::File(index)));
                            }
                        }
                        b"--eval" => {
                            let value = value
                                .or_else(|| {
                                    i += 1;
                                    source_args.get(i).cloned()
                                })
                                .ok_or("option '--eval' requires an argument")?;
                            if value.is_empty() {
                                usage.push(UsageIssue::Empty(b"-E"));
                                pause!(None);
                            }
                            if evals_seen.insert(value.clone()) {
                                result.evals.push(value);
                            }
                        }
                        b"--output-sync" => {
                            result.output_sync = Some(value.unwrap_or_else(|| b"target".to_vec()))
                        }
                        b"--sync-mutex" => {
                            let value = value
                                .or_else(|| {
                                    i += 1;
                                    source_args.get(i).cloned()
                                })
                                .ok_or("option '--sync-mutex' requires an argument")?;
                            if value.is_empty() {
                                usage.push(UsageIssue::Empty(b"--sync-mutex"));
                                pause!(None);
                            }
                            result.sync_mutex = Some(value);
                        }
                        b"--load-average" | b"--max-load" => {
                            let value = value.or_else(|| {
                                if source_args.get(i + 1).is_some_and(|v| {
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
                                if source_args
                                    .get(i + 1)
                                    .is_some_and(|v| v.iter().all(u8::is_ascii_digit))
                                {
                                    i += 1;
                                    Some(args[i].clone())
                                } else {
                                    None
                                }
                            });
                            result.jobs = Some(match job_count(value.as_deref()) {
                                Ok(jobs) => jobs,
                                Err(_) => {
                                    usage.push(UsageIssue::Jobs);
                                    pause!(None);
                                }
                            });
                            if track_jobs {
                                result.command_jobs = result.jobs;
                            }
                        }
                        _ => {
                            if let Some(expanded) = expand_unique_long_option(arg) {
                                // Retry the same argument before any operand
                                // receiver or definition can observe it.
                                expanded_long = Some(i);
                                args[i] = expanded;
                                continue;
                            }
                            if !STARTUP && !GNU_LONG_OPTIONS.contains(&name) {
                                // A live flag decode ignores getopt's invalid
                                // option result and continues with later words.
                                // Registered, unimplemented features still fail.
                                i += 1;
                                continue;
                            }
                            if STARTUP {
                                usage.push(UsageIssue::UnknownLong(arg.clone()));
                                pause!(None);
                            }
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
                        b'd' => result.debug.all = true,
                        b'n' => result.dry_run = true,
                        b'p' => result.print_database = true,
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
                        b'w' => {
                            result.directory_choice = DirectoryChoice::On;
                            result.directory_origin = Some(Origin::CommandLine);
                        }
                        b'e' => result.environment_override = true,
                        b'r' => result.no_rules = true,
                        b'R' => {
                            result.no_variables = true;
                            if STARTUP {
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
                            } else if source_args.get(i + 1).is_some_and(|v| {
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
                        option @ (b'f' | b'C' | b'j' | b'I' | b'E' | b'o' | b'W') => {
                            let numeric_operand = j + 1 < arg.len()
                                || source_args
                                    .get(i + 1)
                                    .is_some_and(|next| next.iter().all(u8::is_ascii_digit));
                            let value = if j + 1 < arg.len() {
                                arg[j + 1..].to_vec()
                            } else if option == b'j' && !numeric_operand {
                                Vec::new()
                            } else {
                                i += 1;
                                if i == source_end {
                                    // The missing operand does not consume an argv slot.
                                    i -= 1;
                                    if STARTUP {
                                        usage.push(UsageIssue::MissingShort(option));
                                    }
                                    break;
                                }
                                source_args[i].clone()
                            };
                            if value.is_empty() && option != b'j' {
                                if STARTUP || matches!(option, b'E' | b'I') {
                                    usage.push(UsageIssue::Empty(match option {
                                        b'C' => b"-C",
                                        b'E' => b"-E",
                                        b'I' => b"-I",
                                        b'f' => b"-f",
                                        b'o' => b"-o",
                                        b'W' => b"-W",
                                        _ => unreachable!("validated short operand option"),
                                    }));
                                }
                                break;
                            }
                            if matches!(option, b'o' | b'W') {
                                let path = OsString::from_vec(value).into();
                                if option == b'o' {
                                    result.old_files.push(path);
                                    let index = result.old_files.len() - 1;
                                    pause!(Some(Argument::OldFile(index)));
                                } else {
                                    result.new_files.push(path);
                                    let index = result.new_files.len() - 1;
                                    pause!(Some(Argument::NewFile(index)));
                                }
                            } else if option == b'E' {
                                if evals_seen.insert(value.clone()) {
                                    result.evals.push(value);
                                }
                            } else if option == b'I' {
                                result.add_include_directory(OsString::from_vec(value).into())?;
                                let index = result.include_directories.len() - 1;
                                pause!(Some(Argument::Include(index)));
                            } else if option == b'j' {
                                result.jobs = Some(
                                    match job_count(numeric_operand.then_some(value.as_slice())) {
                                        Ok(jobs) => jobs,
                                        Err(_) => {
                                            usage.push(UsageIssue::Jobs);
                                            break;
                                        }
                                    },
                                );
                                if track_jobs {
                                    result.command_jobs = result.jobs;
                                }
                            } else if option == b'f' {
                                if track_jobs && value == b"-" && result.stdin_argument.is_none() {
                                    result.stdin_argument = Some(StdinArgument {
                                        argument: argument_index,
                                        consumed: i - argument_index + 1,
                                        prefix: j,
                                    });
                                }
                                result.files.push(OsString::from_vec(value).into());
                                let index = result.files.len() - 1;
                                pause!(Some(Argument::File(index)));
                            } else {
                                result.directories.push(OsString::from_vec(value).into());
                                let index = result.directories.len() - 1;
                                pause!(Some(Argument::Directory(index)));
                            }
                            break;
                        }
                        option => {
                            if STARTUP {
                                usage.push(UsageIssue::UnknownShort(option));
                            }
                        }
                    }
                    j += 1;
                }
            } else if rmake_core::parser::assignment(arg).is_some() {
                result.assignments.push(arg.clone());
                result.assignment_warnings.push(result.warn_undefined);
                result
                    .assignment_context
                    .push((i + offset, result.environment_override));
                let index = result.assignments.len() - 1;
                pause!(Some(Argument::Definition(index)));
            } else if !arg.is_empty() && arg != b"-" {
                result
                    .goals
                    .push(rmake_core::filenames::without_dot_prefix(arg).to_vec());
                let index = result.goals.len() - 1;
                pause!(Some(Argument::Goal(index)));
            }
            pause!(None);
        }
        self.args = args;
        self.evals_seen = evals_seen;
        self.cursor = i;
        self.end_options = end_options;
        Ok(None)
    }
}

impl Options {
    fn reject_usage(argument: Argument) -> Result<(), String> {
        if let Argument::Usage(_, issue) = argument {
            return Err(String::from_utf8_lossy(&issue.message()).into_owned());
        }
        Ok(())
    }

    /// MAKEFLAGS uses make's backslash-separated words, not shell quoting.
    /// Only the CLI subset implemented here is accepted; unsupported inherited
    /// scheduling flags must not silently change a recursive invocation.
    #[cfg(test)]
    pub fn with_environment(
        args: impl IntoIterator<Item = OsString>,
        makeflags: Option<OsString>,
        gnumakeflags: Option<OsString>,
    ) -> Result<Self, String> {
        let missing_makeflags = makeflags.is_none();
        let mut prefix_count = 0;
        let mut inherited_options = Self::default();
        for flags in [gnumakeflags, makeflags].into_iter().flatten() {
            let mut tokens = Vec::new();
            Self::append_flags(&mut tokens, flags.as_bytes());
            let count = tokens.len();
            inherited_options = Self::parse_continued_with::<String>(
                tokens,
                inherited_options,
                prefix_count,
                &mut |_, argument| Self::reject_usage(argument),
                &mut |_| {},
            )?;
            prefix_count += count;
        }
        Self::with_parsed_inherited(args, inherited_options, prefix_count, missing_makeflags)
    }

    #[cfg(test)]
    pub(crate) fn with_parsed_inherited(
        args: impl IntoIterator<Item = OsString>,
        inherited_options: Self,
        prefix_count: usize,
        missing_makeflags: bool,
    ) -> Result<Self, String> {
        Self::with_parsed_inherited_with(
            args,
            inherited_options,
            prefix_count,
            missing_makeflags,
            &mut |_, argument| Self::reject_usage(argument),
            &mut |_| {},
        )
    }

    pub(crate) fn with_parsed_inherited_with<E: From<String> + From<&'static str>>(
        args: impl IntoIterator<Item = OsString>,
        mut inherited_options: Self,
        prefix_count: usize,
        missing_makeflags: bool,
        receiver: &mut impl FnMut(&mut Self, Argument) -> Result<(), E>,
        completed: &mut impl FnMut(&Self),
    ) -> Result<Self, E> {
        let inherited = inherited_options.jobs;
        let inherited_warning = inherited_options.warn_undefined;
        let inherited_output_sync = inherited_options.output_sync.clone();
        inherited_options.silent_origin =
            inherited_options.silent_origin.map(|_| Origin::Environment);
        inherited_options.keep_going_origin = inherited_options
            .keep_going_origin
            .map(|_| Origin::Environment);
        inherited_options.directory_origin = inherited_options
            .directory_origin
            .map(|_| Origin::Environment);
        inherited_options.command_jobs = None;
        let (mut result, _) = Self::parse_with_inspection::<true, false, E>(
            args,
            inherited_options,
            prefix_count,
            true,
            0,
            receiver,
            completed,
        )?;
        let explicit = result.command_jobs;
        result.startup_missing_makeflags_warning = missing_makeflags && inherited_warning;
        result.inherited_output_sync = inherited_output_sync;
        result.inherited_jobs = inherited.is_some() && explicit.is_none();
        Ok(result.with_command_jobs(explicit))
    }

    pub(crate) fn parse_continued_with<E: From<String> + From<&'static str>>(
        args: impl IntoIterator<Item = OsString>,
        initial: Self,
        offset: usize,
        receiver: &mut impl FnMut(&mut Self, Argument) -> Result<(), E>,
        completed: &mut impl FnMut(&Self),
    ) -> Result<Self, E> {
        Self::parse_with_inspection::<true, false, E>(
            args, initial, offset, false, 0, receiver, completed,
        )
        .map(|(options, _)| options)
    }

    fn with_command_jobs(mut self, jobs: Option<usize>) -> Self {
        self.command_jobs = jobs;
        self.reset_jobserver = jobs.is_some() && self.jobserver_auth.is_some();
        if self.reset_jobserver {
            self.jobserver_auth = None;
        }
        self
    }

    fn append_flags(combined: &mut Vec<OsString>, flags: &[u8]) {
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
        Self::append_expanded_flags(combined, &decoded);
    }

    // Expanded file flags have already consumed their make-dollar quoting.
    // GNU skips C whitespace once at the start, then separates only SP/TAB.
    pub(crate) fn append_expanded_flags(combined: &mut Vec<OsString>, mut bytes: &[u8]) {
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
        // Keep every marker until decoding. `--` can be an operand, and
        // only the first unconsumed marker ends the options for this source.
        combined.extend(tokens.into_iter().map(OsString::from_vec));
    }

    /// Decode changed file bindings after reading. Existing switches persist;
    /// later file flags can change them, while explicit CLI -j keeps priority.
    #[cfg(test)]
    pub fn file_flags(&self, flags: &[Vec<u8>]) -> Result<(Self, Option<usize>), String> {
        self.file_flags_with_positions(flags)
            .map(|(options, jobs, _)| (options, jobs))
    }
    pub fn file_flags_with_positions(
        &self,
        flags: &[Vec<u8>],
    ) -> Result<(Self, Option<usize>, Vec<usize>), String> {
        self.decode_file_flags(flags, true, Origin::Environment)
    }
    #[cfg(test)]
    pub fn assigned_flags(
        &self,
        flags: &[Vec<u8>],
        origin: Origin,
    ) -> Result<(Self, Option<usize>), String> {
        self.assigned_flags_with_positions(flags, origin)
            .map(|(options, jobs, _)| (options, jobs))
    }
    pub fn assigned_flags_with_positions(
        &self,
        flags: &[Vec<u8>],
        origin: Origin,
    ) -> Result<(Self, Option<usize>, Vec<usize>), String> {
        self.decode_file_flags(flags, false, origin)
    }
    fn append_internal_jobserver_style(&self, arguments: &mut Vec<OsString>) {
        if let Some(style) = &self.unknown_jobserver_style {
            arguments.push(OsString::from_vec(
                [b"--jobserver-style=".as_slice(), style].concat(),
            ));
        } else if self.jobserver_pipe {
            arguments.push("--jobserver-style=pipe".into());
        }
    }

    /// Select only effectful include operands. The ordinary decoder remains
    /// the allocation baseline for streams with no tilde operand.
    pub(crate) fn include_decoder(&self, value: &[u8]) -> Result<Option<Decoder>, String> {
        let stored_tilde = self
            .include_directories
            .iter()
            .any(|path| path.as_os_str().as_bytes().starts_with(b"~"));
        if !value.contains(&b'~') && !stored_tilde {
            return Ok(None);
        }
        let mut incoming = Vec::new();
        Self::append_expanded_flags(&mut incoming, value);
        let inspected = Self::parse_mode::<false>(incoming.clone())?;
        if !stored_tilde
            && !inspected
                .include_directories
                .iter()
                .any(|path| path.as_os_str().as_bytes().starts_with(b"~"))
        {
            return Ok(None);
        }
        let mut prefix = Vec::new();
        Self::append_flags(&mut prefix, &self.recursive_flags_with_includes(false));
        self.append_internal_jobserver_style(&mut prefix);
        let mut seeded = Self::parse_mode::<false>(prefix)?;
        seeded.include_directories = self.include_directories.clone();
        seeded.include_assignment_positions = self.include_assignment_positions.clone();
        seeded.evals = self.evals.clone();
        seeded.silent_origin = self.silent_origin;
        seeded.keep_going_origin = self.keep_going_origin;
        seeded.debug_mask = self.debug_mask;
        seeded.directory_choice = self.directory_choice;
        seeded.directory_origin = self.directory_origin;
        seeded.command_jobs = self.command_jobs;
        seeded.reset_jobserver = self.reset_jobserver;
        Ok(Some(Decoder::new(incoming, seeded, 1, false)))
    }

    fn decode_file_flags(
        &self,
        flags: &[Vec<u8>],
        restore_cli_jobs: bool,
        origin: Origin,
    ) -> Result<(Self, Option<usize>, Vec<usize>), String> {
        let mut arguments = Vec::new();
        Self::append_flags(&mut arguments, &self.recursive_flags_with_includes(false));
        self.append_internal_jobserver_style(&mut arguments);
        let prefix_count = arguments.len();
        let prefix_end = [prefix_count];
        let mut other_ends = Vec::new();
        if flags.len() > 1 {
            other_ends.push(prefix_count);
        }
        for (index, value) in flags.iter().enumerate() {
            if index > 0 && other_ends.last() != Some(&arguments.len()) {
                other_ends.push(arguments.len());
            }
            Self::append_expanded_flags(&mut arguments, value);
        }
        let source_ends = if other_ends.is_empty() {
            &prefix_end[..]
        } else {
            &other_ends
        };
        let flag_word_count = arguments.len() - prefix_count + 1;
        let seeded = Self {
            include_directories: self.include_directories.clone(),
            include_assignment_positions: self.include_assignment_positions.clone(),
            debug_mask: self.debug_mask,
            ..Self::default()
        };
        let mut seen: std::collections::HashSet<_> = seeded
            .include_directories
            .iter()
            .map(|path| path.as_os_str().to_owned())
            .collect();
        // Capture incoming switch selections during the merged decode. Prefix
        // reconstruction must not manufacture new source-origin selections.
        let mut warning_positions = Vec::new();
        let mut notices = Vec::new();
        let (mut parsed, incoming_options) = Self::run_decoder::<false, true, String>(
            Decoder::new(arguments, seeded, 0, false),
            source_ends,
            prefix_count,
            &mut |options, argument| {
                if let Argument::Usage(position, issue) = &argument
                    && *position >= prefix_count
                    && let Some(notice) = issue.notice()
                {
                    notices.push(rmake_core::diagnostic::FlagNoticeAt {
                        position: *position - prefix_count + 1,
                        notice,
                    });
                }
                if let Argument::Warning(position) = argument
                    && position >= prefix_count
                {
                    warning_positions.push(position - prefix_count + 1);
                }
                if let Argument::Include(index) = argument {
                    let raw = options.include_directories[index].as_os_str();
                    if seen.contains(raw) {
                        options.include_directories.pop();
                        options.include_assignment_positions.pop();
                    } else {
                        // Runtime HOME expansion still requires a contextual
                        // decoder; this only preserves literal byte semantics.
                        let stored: PathBuf = OsString::from_vec(
                            rmake_core::filenames::without_dot_prefix(raw.as_bytes()).to_vec(),
                        )
                        .into();
                        seen.insert(stored.as_os_str().to_owned());
                        options.include_directories[index] = stored;
                    }
                }
                Ok(())
            },
            &mut |_| {},
        )
        .map_err(|error| Self::incoming_error(flags, error))?;
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
        // File-age operands are invocation inputs. A later MAKEFLAGS decode
        // neither exports them nor declares additional assumed files.
        parsed.old_files = self.old_files.clone();
        parsed.new_files = self.new_files.clone();
        parsed.flag_word_count = flag_word_count;
        parsed.flag_notices = notices;
        // GNU only tracks origins for selected switches. Reconstructing the
        // spelling must not turn an inherited/file setting into a CLI setting.
        if incoming_options.silent.is_some()
            && (origin == Origin::CommandLine || self.silent_origin.is_none_or(|old| origin >= old))
        {
            parsed.silent_origin = Some(origin);
            parsed.silent = incoming_options.silent.expect("selected incoming switch");
        } else {
            parsed.silent_origin = self.silent_origin;
            parsed.silent = self.silent;
        }
        if incoming_options.keep_going.is_some()
            && (origin == Origin::CommandLine
                || self.keep_going_origin.is_none_or(|old| origin >= old))
        {
            parsed.keep_going_origin = Some(origin);
            parsed.keep_going = incoming_options
                .keep_going
                .expect("selected incoming switch");
        } else {
            parsed.keep_going_origin = self.keep_going_origin;
            parsed.keep_going = self.keep_going;
        }
        if let Some(choice) = incoming_options.directory
            && (origin == Origin::CommandLine
                || self.directory_origin.is_none_or(|old| origin >= old))
        {
            parsed.directory_choice = choice;
            parsed.directory_origin = Some(origin);
        } else {
            parsed.directory_choice = self.directory_choice;
            parsed.directory_origin = self.directory_origin;
        }
        parsed.decode_debug()?;
        parsed.command_jobs = self.command_jobs;
        if restore_cli_jobs && let Some(jobs) = self.command_jobs {
            parsed.jobs = Some(jobs);
        }
        Ok((
            parsed,
            incoming_options
                .jobs
                .filter(|_| self.command_jobs.is_none()),
            warning_positions,
        ))
    }

    /// Preserve the preliminary incoming-error precedence on the cold path.
    /// Both parses are pure: callbacks only update owned decoder bookkeeping.
    #[cold]
    fn incoming_error(flags: &[Vec<u8>], merged_error: String) -> String {
        for value in flags {
            let mut incoming = Vec::new();
            Self::append_expanded_flags(&mut incoming, value);
            if let Err(error) = Self::parse_mode::<false>(incoming) {
                return error;
            }
        }
        merged_error
    }

    #[cfg(test)]
    pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Self, String> {
        Self::parse_mode::<true>(args)
    }

    fn parse_mode<const STARTUP: bool>(
        args: impl IntoIterator<Item = OsString>,
    ) -> Result<Self, String> {
        Self::parse_into_mode::<STARTUP>(args, Self::default(), 0, false)
    }

    fn parse_into_mode<const STARTUP: bool>(
        args: impl IntoIterator<Item = OsString>,
        result: Self,
        offset: usize,
        track_jobs: bool,
    ) -> Result<Self, String> {
        Self::parse_with::<STARTUP, String>(args, result, offset, track_jobs, &mut |_, argument| {
            if STARTUP {
                Self::reject_usage(argument)
            } else {
                Ok(())
            }
        })
    }

    // The same owned decoder drives synchronous startup callbacks and runtime
    // suspension. No parser-local reference survives an argument boundary.
    fn parse_with<const STARTUP: bool, E: From<String> + From<&'static str>>(
        args: impl IntoIterator<Item = OsString>,
        result: Self,
        offset: usize,
        track_jobs: bool,
        receiver: &mut impl FnMut(&mut Self, Argument) -> Result<(), E>,
    ) -> Result<Self, E> {
        Self::parse_with_inspection::<STARTUP, false, E>(
            args,
            result,
            offset,
            track_jobs,
            0,
            receiver,
            &mut |_| {},
        )
        .map(|(options, _)| options)
    }

    fn parse_with_inspection<
        const STARTUP: bool,
        const INSPECT: bool,
        E: From<String> + From<&'static str>,
    >(
        args: impl IntoIterator<Item = OsString>,
        result: Self,
        offset: usize,
        track_jobs: bool,
        incoming_start: usize,
        receiver: &mut impl FnMut(&mut Self, Argument) -> Result<(), E>,
        completed: &mut impl FnMut(&Self),
    ) -> Result<(Self, IncomingSwitches), E> {
        Self::run_decoder::<STARTUP, INSPECT, E>(
            Decoder::new(args, result, offset, track_jobs),
            &[],
            incoming_start,
            receiver,
            completed,
        )
    }

    fn run_decoder<
        const STARTUP: bool,
        const INSPECT: bool,
        E: From<String> + From<&'static str>,
    >(
        mut decoder: Decoder,
        source_ends: &[usize],
        incoming_start: usize,
        receiver: &mut impl FnMut(&mut Self, Argument) -> Result<(), E>,
        completed: &mut impl FnMut(&Self),
    ) -> Result<(Self, IncomingSwitches), E> {
        let mut incoming = IncomingSwitches::default();
        let result = (|| -> Result<(), E> {
            loop {
                let previous = if INSPECT {
                    (
                        decoder.options.silent_origin.take(),
                        decoder.options.keep_going_origin.take(),
                        decoder.options.jobs.take(),
                        decoder.options.directory_origin.take(),
                    )
                } else {
                    (None, None, None, None)
                };
                let step = decoder
                    .advance_in_sources::<STARTUP>(source_ends)
                    .map_err(E::from)?;
                if INSPECT {
                    let options = &mut decoder.options;
                    if step
                        .as_ref()
                        .is_some_and(|step| step.position >= incoming_start)
                    {
                        if options.silent_origin.is_some() {
                            incoming.silent = Some(options.silent);
                        }
                        if options.keep_going_origin.is_some() {
                            incoming.keep_going = Some(options.keep_going);
                        }
                        if let Some(jobs) = options.jobs {
                            incoming.jobs = Some(jobs);
                        }
                        if options.directory_origin.is_some() {
                            incoming.directory = Some(options.directory_choice);
                        }
                    }
                    if options.silent_origin.is_none() {
                        options.silent_origin = previous.0;
                    }
                    if options.keep_going_origin.is_none() {
                        options.keep_going_origin = previous.1;
                    }
                    if options.jobs.is_none() {
                        options.jobs = previous.2;
                    }
                    if options.directory_origin.is_none() {
                        options.directory_origin = previous.3;
                    }
                }
                let Some(step) = step else {
                    break;
                };
                for issue in step.usage {
                    decoder.options.invalid_usage |= STARTUP;
                    receiver(&mut decoder.options, Argument::Usage(step.position, issue))?;
                }
                if let Some(argument) = step.argument {
                    receiver(&mut decoder.options, argument)?;
                }
            }
            Ok(())
        })();
        completed(&decoder.options);
        result?;
        Ok((decoder.options, incoming))
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
    pub fn decode_debug(&mut self) -> Result<(), String> {
        self.debug_mask = self.debug.decode_from(self.trace, self.debug_mask)?;
        Ok(())
    }

    pub fn makeflags(&self) -> Vec<u8> {
        self.makeflags_for_phase(false)
    }

    pub fn makeflags_for_phase(&self, remaking: bool) -> Vec<u8> {
        let mut flags = self.recursive_flags_for_phase(true, remaking);
        if !self.evals.is_empty() {
            flags.extend_from_slice(b" $(-*-eval-flags-*-)");
        }
        flags
    }

    pub fn recursive_flags(&self) -> Vec<u8> {
        self.recursive_flags_with_includes(true)
    }

    fn recursive_flags_with_includes(&self, includes: bool) -> Vec<u8> {
        self.recursive_flags_for_phase(includes, false)
    }

    pub fn recursive_flags_for_phase(&self, includes: bool, remaking: bool) -> Vec<u8> {
        let flags = if remaking {
            self.flags_for_phase(includes, true)
        } else if includes {
            self.flags()
        } else {
            self.flags_with_includes(false)
        };
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
        self.flags_with_includes(true)
    }

    fn flags_with_includes(&self, includes: bool) -> Vec<u8> {
        self.flags_for_phase(includes, false)
    }

    fn flags_for_phase(&self, includes: bool, remaking: bool) -> Vec<u8> {
        let mut flags = Vec::new();
        for (set, flag) in [
            (self.always, b'B'),
            (self.debug.all, b'd'),
            (self.environment_override, b'e'),
            (self.ignore_errors, b'i'),
            (self.keep_going, b'k'),
            (self.check_symlink_times, b'L'),
            (self.dry_run && !remaking, b'n'),
            (self.print_database, b'p'),
            (self.question && !remaking, b'q'),
            (self.no_rules, b'r'),
            (self.no_variables, b'R'),
            (self.silent, b's'),
            (!self.keep_going && self.keep_going_origin.is_some(), b'S'),
            (self.touch && !remaking, b't'),
            (self.directory_choice == DirectoryChoice::On, b'w'),
        ] {
            if set {
                flags.push(flag);
            }
        }
        if includes {
            for directory in &self.include_directories {
                flags.extend_from_slice(b" -I");
                for &byte in directory.as_os_str().as_bytes() {
                    if byte.is_ascii_whitespace() || byte == b'\\' {
                        flags.push(b'\\');
                    }
                    flags.push(byte);
                }
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
        for value in &self.debug.values {
            flags.extend_from_slice(b" --debug=");
            for &byte in value {
                if byte.is_ascii_whitespace() || byte == b'\\' {
                    flags.push(b'\\');
                }
                flags.push(byte);
            }
        }
        if self.trace {
            flags.extend_from_slice(b" --trace");
        }
        if self.directory_choice == DirectoryChoice::Off {
            flags.extend_from_slice(b" --no-print-directory");
        }
        if self.warn_undefined {
            flags.extend_from_slice(b" --warn-undefined-variables");
        }
        if !self.silent && self.silent_origin.is_some() {
            flags.extend_from_slice(b" --no-silent");
        }
        if let Some(mode) = &self.shuffle {
            flags.extend_from_slice(b" --shuffle=");
            for &byte in mode {
                if byte.is_ascii_whitespace() || byte == 0x0b || byte == b'\\' {
                    flags.push(b'\\');
                }
                flags.push(byte);
            }
        }
        flags
    }
}

fn job_count(value: Option<&[u8]>) -> Result<usize, String> {
    let Some(value) = value else {
        return Ok(usize::MAX);
    };
    std::str::from_utf8(value.trim_ascii_start())
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|value| *value > 0)
        .map(|value| value as usize)
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
    fn makefile_phase_omits_execution_modes_without_changing_the_invocation() {
        let options =
            Options::parse([OsString::from("-nqtp"), OsString::from("-Ipath$$value")]).unwrap();
        let normal = options.makeflags();
        let remake = options.makeflags_for_phase(true);
        assert!(normal.starts_with(b"npqt "));
        assert!(remake.starts_with(b"p "));
        assert!(normal.ends_with(b"-Ipath$$$$value"));
        assert!(remake.ends_with(b"-Ipath$$$$value"));
        assert!(options.dry_run && options.question && options.touch);
        assert_eq!(options.makeflags(), normal);
    }

    #[test]
    fn database_switch_is_serialized_and_retained_in_live_flag_decoding() {
        for argument in ["-prR", "--print-data-base", "--print-data-b"] {
            let options = Options::parse([OsString::from(argument)]).unwrap();
            assert!(options.print_database);
            assert!(
                options
                    .flags()
                    .split(|byte| byte.is_ascii_whitespace())
                    .next()
                    .unwrap()
                    .contains(&b'p')
            );
            let (updated, _, _) = options
                .assigned_flags_with_positions(&[b"-s".to_vec()], Origin::File)
                .unwrap();
            assert!(updated.print_database);
        }
        assert!(Options::parse([OsString::from("--print-data-base=yes")]).is_err());
    }

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
    fn live_invalid_options_continue_through_clusters_and_payload_positions() {
        let (live, _) = Options::default()
            .assigned_flags(&[b"--w -Qsk SIDE:=after".to_vec()], Origin::File)
            .unwrap();
        assert!(live.silent && live.keep_going);
        assert_eq!(live.silent_origin, Some(Origin::File));
        assert_eq!(live.keep_going_origin, Some(Origin::File));
        assert_eq!(live.assignments, [b"SIDE:=after".to_vec()]);
        assert_eq!(live.assignment_context, [(3, false)]);
        assert_eq!(live.flag_word_count, 4);
        assert_eq!(live.flags(), b"ks");
    }

    #[test]
    fn live_warning_positions_exclude_prefix_flags_and_rejected_arguments() {
        let inherited = parse(&["--warn-undefined-variables"]).unwrap();
        let (live, _, positions) = inherited
            .assigned_flags_with_positions(
                &[b"A:=before --warn-undef B:=after".to_vec()],
                Origin::File,
            )
            .unwrap();
        assert_eq!(positions, [2]);
        assert_eq!(live.assignment_context, [(1, false), (3, false)]);
        let (rejected, _, positions) = inherited
            .assigned_flags_with_positions(
                &[b"--warn-undefined-variables=bad".to_vec()],
                Origin::File,
            )
            .unwrap();
        assert!(positions.is_empty());
        assert!(rejected.warn_undefined);
    }

    #[test]
    fn live_source_terminator_and_operand_lookahead_are_source_local() {
        let options = Options::default();
        let (split, _) = options
            .assigned_flags(&[b"-- -n".to_vec(), b"-s".to_vec()], Origin::File)
            .unwrap();
        assert!(!split.dry_run);
        assert!(split.silent);
        let (missing, _) = options
            .assigned_flags(&[b"-I".to_vec(), b"-s".to_vec()], Origin::File)
            .unwrap();
        assert!(missing.include_directories.is_empty());
        assert!(missing.silent);
    }

    #[test]
    fn passive_warning_positions_preserve_explicit_cli_job_priority() {
        let initial = Options::with_environment(["-j4".into()], None, None).unwrap();
        let (passive, jobs, positions) = initial
            .file_flags_with_positions(&[b"A:=before --warn-undef -j2 B:=after".to_vec()])
            .unwrap();
        assert_eq!(positions, [2]);
        assert_eq!(passive.assignment_context, [(1, false), (4, false)]);
        assert_eq!(passive.jobs, Some(4));
        assert_eq!(jobs, None);
        assert!(passive.warn_undefined);
    }

    #[test]
    fn live_arity_errors_preserve_prior_letters_and_later_definition_positions() {
        let (live, _) = Options::default()
            .assigned_flags(
                &[b"--silent=bad --no-builtin-v=bad SIDE:=after -kI".to_vec()],
                Origin::File,
            )
            .unwrap();
        assert!(!live.silent && !live.no_variables && !live.no_rules);
        assert!(live.keep_going);
        assert_eq!(live.keep_going_origin, Some(Origin::File));
        assert_eq!(live.assignments, [b"SIDE:=after".to_vec()]);
        assert_eq!(live.assignment_context, [(3, false)]);
        assert_eq!(live.flag_word_count, 5);
        assert!(live.include_directories.is_empty());
        assert!(live.include_assignment_positions.is_empty());
    }

    #[test]
    fn live_missing_operands_and_empty_values_preserve_their_distinct_effects() {
        for raw in ["-C", "-f", "-I", "-o", "-W", "--eval", "--jobserver-auth"] {
            let mut received = Vec::new();
            let live = Options::parse_with::<false, String>(
                [raw.into()],
                Options::default(),
                0,
                false,
                &mut |_, argument| {
                    received.push(argument);
                    Ok(())
                },
            )
            .unwrap();
            assert!(received.is_empty(), "{raw}");
            assert!(live.files.is_empty() && live.directories.is_empty());
            assert!(live.evals.is_empty() && live.include_directories.is_empty());
            assert!(parse(&[raw]).is_err(), "{raw}");
            assert!(Options::with_environment([], Some(raw.into()), None).is_err());
        }
        for raw in ["--silent=", "--no-silent=bad", "--no-print-directory=bad"] {
            assert!(parse(&[raw]).is_err(), "{raw}");
            assert!(Options::with_environment([], Some(raw.into()), None).is_err());
        }
        for raw in ["--old-file=", "--new-file="] {
            let (options, _) = Options::default()
                .assigned_flags(&[raw.as_bytes().to_vec()], Origin::File)
                .unwrap();
            assert!(options.old_files.is_empty() && options.new_files.is_empty());
            assert!(options.flag_notices.is_empty(), "{raw}");
        }
        for raw in ["--eval=", "--jobs=zero"] {
            let (options, _) = Options::default()
                .assigned_flags(&[raw.as_bytes().to_vec()], Origin::File)
                .unwrap();
            assert!(options.evals.is_empty() && options.jobs.is_none());
            assert_eq!(options.flag_notices.len(), 1, "{raw}");
        }
    }

    #[test]
    fn startup_validation_remains_strict_for_unknown_options() {
        for value in ["--w", "--not-an-option", "-Qsk"] {
            assert!(parse(&[value]).is_err());
            assert!(Options::with_environment([], Some(value.into()), None).is_err());
        }
        assert!(
            Options::default()
                .assigned_flags(&[b"-p".to_vec()], Origin::File)
                .unwrap()
                .0
                .print_database
        );
    }

    #[test]
    fn long_prefix_retry_preserves_raw_operands_and_notifies_receivers_once() {
        let mut events = Vec::new();
        let options = Options::parse_with::<true, String>(
            [
                OsString::from_vec(b"--fi=left=\xff".to_vec()),
                OsString::from("TAIL=value"),
            ],
            Options::default(),
            0,
            false,
            &mut |_, event| {
                events.push(event);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(options.files[0].as_os_str().as_bytes(), b"left=\xff");
        assert_eq!(options.assignment_context, [(1, false)]);
        assert!(matches!(
            events.as_slice(),
            [Argument::File(0), Argument::Definition(0)]
        ));
    }

    #[test]
    fn long_prefixes_respect_exact_names_ambiguity_and_the_end_marker() {
        assert_eq!(parse(&["--jobs=1"]).unwrap().jobs, Some(1));
        for value in ["--w", "--jo", "--no-builtin"] {
            assert!(parse(&[value]).is_err());
        }
        let stopped = parse(&["--", "--fi"]).unwrap();
        assert!(stopped.files.is_empty());
        assert_eq!(stopped.goals, [b"--fi".to_vec()]);
        assert!(parse(&["--hel"]).unwrap().help);
        assert!(parse(&["--ver"]).unwrap().version);
    }

    #[test]
    fn assumed_file_operands_remain_initial_and_are_not_exported() {
        let options = parse(&["-odep", "--assume-new", "./new", "all"]).unwrap();
        assert_eq!(options.old_files, [PathBuf::from("dep")]);
        assert_eq!(options.new_files, [PathBuf::from("./new")]);
        assert_eq!(options.goals, [b"all".to_vec()]);
        let child =
            Options::with_environment([], Some(OsString::from_vec(options.flags())), None).unwrap();
        assert!(child.old_files.is_empty() && child.new_files.is_empty());
        let (late, _) = options.file_flags(&[b"-oother -Wlater".to_vec()]).unwrap();
        assert_eq!(late.old_files, options.old_files);
        assert_eq!(late.new_files, options.new_files);
        let raw = Options::parse([OsString::from_vec(b"-W\xff".to_vec())]).unwrap();
        assert_eq!(raw.new_files[0].as_os_str().as_bytes(), b"\xff");
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
        let literal = Options::with_environment([], Some("r -- -n".into()), None).unwrap();
        assert!(!literal.dry_run);
        assert_eq!(literal.goals, [b"-n".to_vec()]);
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
        assert_eq!(
            parse(&["--jobserver-style=invalid"])
                .unwrap()
                .unknown_jobserver_style
                .as_deref(),
            Some(b"invalid".as_slice())
        );
    }

    #[test]
    fn debug_history_survives_live_lower_origin_selection_and_serialization() {
        let mut options = parse(&["-rR", "--debug=b none", "--trace"]).unwrap();
        options.decode_debug().unwrap();
        assert_eq!(options.debug_mask, crate::debug::Mask::NONE);
        assert_eq!(options.flags(), b"rR --debug=b\\ none --trace");
        let mut basic = parse(&["--debug=b"]).unwrap();
        basic.decode_debug().unwrap();
        let selected = basic
            .assigned_flags(&[b"--debug=none".to_vec()], Origin::File)
            .unwrap()
            .0;
        assert_eq!(selected.debug_mask, crate::debug::Mask::NONE);
        assert_eq!(selected.debug.values, [b"b".to_vec(), b"none".to_vec()]);
        let repeated = selected
            .assigned_flags(&[b"--debug=b".to_vec()], Origin::Environment)
            .unwrap()
            .0;
        assert_eq!(repeated.debug_mask, crate::debug::Mask::NONE);
        let resumed = selected
            .assigned_flags(&[b"--debug=B".to_vec()], Origin::Environment)
            .unwrap()
            .0;
        assert!(resumed.debug_mask.has(crate::debug::Mask::BASIC));
    }

    #[test]
    fn directory_selectors_are_ordered_and_automatic_has_no_exported_switch() {
        for (arguments, choice, flags) in [
            (
                vec!["-w", "--no-print-directory"],
                DirectoryChoice::Off,
                b" --no-print-directory".as_slice(),
            ),
            (
                vec!["--no-print-directory", "--print-directory"],
                DirectoryChoice::On,
                b"w".as_slice(),
            ),
            (vec!["-sw"], DirectoryChoice::On, b"sw".as_slice()),
            (
                vec!["-C", "sub"],
                DirectoryChoice::Automatic,
                b"".as_slice(),
            ),
        ] {
            let options = parse(&arguments).unwrap();
            assert_eq!(options.directory_choice, choice);
            assert_eq!(options.flags(), flags);
        }
        assert!(parse(&["--print-directory=yes"]).is_err());
        let automatic = parse(&[])
            .unwrap()
            .assigned_flags(&[b"-s".to_vec()], Origin::File)
            .unwrap()
            .0;
        assert_eq!(automatic.directory_choice, DirectoryChoice::Automatic);
        assert_eq!(automatic.flags(), b"s");
    }

    #[test]
    fn directory_selection_preserves_origin_across_flag_reconstruction() {
        let explicit = parse(&["-w"]).unwrap();
        let unchanged = explicit
            .assigned_flags(&[b"--no-print-directory".to_vec()], Origin::File)
            .unwrap()
            .0;
        assert_eq!(unchanged.directory_choice, DirectoryChoice::On);
        assert_eq!(unchanged.directory_origin, Some(Origin::CommandLine));
        let inherited = Options::with_environment([], Some("w".into()), None).unwrap();
        assert_eq!(inherited.directory_origin, Some(Origin::Environment));
        let selected = inherited
            .assigned_flags(&[b"--no-print-directory".to_vec()], Origin::File)
            .unwrap()
            .0;
        assert_eq!(selected.directory_choice, DirectoryChoice::Off);
        assert_eq!(selected.directory_origin, Some(Origin::File));
        let same = selected
            .assigned_flags(&[b"w".to_vec()], Origin::File)
            .unwrap()
            .0;
        assert_eq!(same.directory_choice, DirectoryChoice::On);
        let lower = selected
            .assigned_flags(&[b"w".to_vec()], Origin::Environment)
            .unwrap()
            .0;
        assert_eq!(lower.directory_choice, DirectoryChoice::Off);
        let absent = selected
            .assigned_flags(&[b"-r".to_vec()], Origin::Override)
            .unwrap()
            .0;
        assert_eq!(absent.directory_choice, DirectoryChoice::Off);
        assert_eq!(absent.directory_origin, Some(Origin::File));
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
        assert_eq!(inherited.directory_choice, DirectoryChoice::Off);
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

#[cfg(test)]
mod owned_decoder_tests {
    use super::*;

    #[test]
    fn suspended_operands_preserve_intervening_owned_state_and_end_marker() {
        let mut decoder = Decoder::new(
            ["-sIpath", "X:=one", "--include-d=other", "--", "-Igoal"].map(OsString::from),
            Options::default(),
            1,
            false,
        );
        let step = decoder.advance::<false>().unwrap().unwrap();
        assert!(matches!(step.argument, Some(Argument::Include(0))));
        assert_eq!(step.position, 1);
        decoder.options.include_directories[0] = "changed".into();
        decoder.options.jobs = Some(7);
        assert!(matches!(
            decoder.advance::<false>().unwrap().unwrap().argument,
            Some(Argument::Definition(0))
        ));
        assert!(matches!(
            decoder.advance::<false>().unwrap().unwrap().argument,
            Some(Argument::Include(1))
        ));
        let last = decoder.advance::<false>().unwrap().unwrap();
        assert!(matches!(last.argument, Some(Argument::Goal(0))));
        assert_eq!(last.position, 5);
        assert_eq!(decoder.options.goals, [b"-Igoal".to_vec()]);
        assert_eq!(decoder.options.jobs, Some(7));
        assert_eq!(
            decoder.options.include_directories,
            [
                std::path::PathBuf::from("changed"),
                std::path::PathBuf::from("other")
            ]
        );
        assert!(decoder.advance::<false>().unwrap().is_none());
    }

    #[test]
    fn consumed_optional_and_separate_operands_resume_at_the_next_definition() {
        let mut decoder = Decoder::new(
            ["-j", "2", "-I", "path", "X=one"].map(OsString::from),
            Options::default(),
            1,
            false,
        );
        assert_eq!(decoder.advance::<false>().unwrap().unwrap().position, 2);
        assert_eq!(decoder.options.jobs, Some(2));
        assert_eq!(decoder.advance::<false>().unwrap().unwrap().position, 4);
        decoder.options.environment_override = true;
        let step = decoder.advance::<false>().unwrap().unwrap();
        assert!(matches!(step.argument, Some(Argument::Definition(0))));
        assert_eq!(step.position, 5);
        assert_eq!(decoder.options.assignment_context, [(5, true)]);
    }
}

#[cfg(test)]
mod decoder_missing_operand_tests {
    use super::*;
    #[test]
    fn a_missing_ignored_operand_never_publishes_a_cursor_past_argc() {
        let mut decoder = Decoder::new(
            ["-I~/inc", "-f"].map(OsString::from),
            Options::default(),
            1,
            false,
        );
        assert_eq!(decoder.advance::<false>().unwrap().unwrap().position, 1);
        let last = decoder.advance::<false>().unwrap().unwrap();
        assert_eq!(last.position, 2);
        assert_eq!(decoder.cursor, 2);
        assert_eq!(decoder.terminal(), 3);
        assert!(decoder.advance::<false>().unwrap().is_none());
    }
}

#[cfg(test)]
mod runtime_marker_tests {
    use super::*;
    #[test]
    fn runtime_suspension_keeps_end_markers_and_their_argument_positions() {
        let mut decoder = Options::default()
            .include_decoder(b"-I~/inc -- -fshadow Z=one")
            .unwrap()
            .unwrap();
        assert_eq!(decoder.terminal(), 5);
        assert!(matches!(
            decoder.advance::<false>().unwrap().unwrap().argument,
            Some(Argument::Include(0))
        ));
        let goal = decoder.advance::<false>().unwrap().unwrap();
        assert_eq!(goal.position, 3);
        assert!(matches!(goal.argument, Some(Argument::Goal(0))));
        let definition = decoder.advance::<false>().unwrap().unwrap();
        assert_eq!(definition.position, 4);
        assert!(matches!(definition.argument, Some(Argument::Definition(0))));
        assert_eq!(decoder.options.goals, [b"-fshadow".to_vec()]);
        assert!(decoder.advance::<false>().unwrap().is_none());
    }
    #[test]
    fn persistent_tilde_selects_owned_runtime_markers_without_affecting_plain_flags() {
        let plain = Options {
            include_directories: vec!["ordinary".into()],
            ..Options::default()
        };
        assert!(plain.include_decoder(b"-s").unwrap().is_none());
        let persistent = Options {
            include_directories: vec!["~/inc".into()],
            ..plain
        };
        let mut decoder = persistent.include_decoder(b"-- -x").unwrap().unwrap();
        assert_eq!(decoder.terminal(), 3);
        assert!(matches!(
            decoder.advance::<false>().unwrap().unwrap().argument,
            Some(Argument::Goal(0))
        ));
        assert!(decoder.advance::<false>().unwrap().is_none());
        assert_eq!(
            decoder.options.include_directories,
            vec![std::path::PathBuf::from("~/inc")]
        );
        assert!(persistent.include_decoder(b"").unwrap().is_some());
    }
    #[test]
    fn runtime_inspection_distinguishes_prefix_and_ignored_operands_from_switches() {
        let previous = Options {
            silent: true,
            silent_origin: Some(Origin::Environment),
            keep_going: true,
            keep_going_origin: Some(Origin::CommandLine),
            jobs: Some(4),
            ..Options::default()
        };
        let (ignored, jobs) = previous
            .assigned_flags(&[b"-f -sk -I -kj3 X=one".to_vec()], Origin::Override)
            .unwrap();
        assert!(ignored.silent && ignored.keep_going);
        assert_eq!(ignored.silent_origin, Some(Origin::Environment));
        assert_eq!(ignored.keep_going_origin, Some(Origin::CommandLine));
        assert_eq!((ignored.jobs, jobs), (Some(4), None));
        let (selected, jobs) = previous
            .assigned_flags(&[b"--no-silent -S -j2".to_vec()], Origin::Override)
            .unwrap();
        assert!(!selected.silent && !selected.keep_going);
        assert_eq!(selected.silent_origin, Some(Origin::Override));
        assert_eq!(selected.keep_going_origin, Some(Origin::Override));
        assert_eq!((selected.jobs, jobs), (Some(2), Some(2)));
        let pinned = Options {
            command_jobs: Some(8),
            ..previous
        };
        let (selected, jobs) = pinned
            .file_flags(&[b"--no-silent -S -j2".to_vec()])
            .unwrap();
        assert!(!selected.silent && selected.keep_going);
        assert_eq!((selected.jobs, jobs), (Some(8), None));
    }

    #[test]
    fn incoming_notice_does_not_replace_the_selected_prefix_or_following_flags() {
        use rmake_core::diagnostic::FlagNotice;
        let previous = Options {
            jobs: Some(2),
            ..Options::default()
        };
        let (updated, _) = previous
            .assigned_flags(&[b"--include-dir= -s".to_vec()], Origin::File)
            .unwrap();
        assert_eq!(updated.jobs, Some(2));
        assert!(updated.silent && updated.include_directories.is_empty());
        assert!(matches!(
            updated.flag_notices.as_slice(),
            [rmake_core::diagnostic::FlagNoticeAt {
                position: 1,
                notice: FlagNotice::EmptyOption(b"-I"),
            }]
        ));
    }
}

#[cfg(test)]
mod live_debug_replay_tests {
    use super::*;
    #[test]
    fn unrelated_runtime_flags_preserve_signal_basic_without_serializing_it() {
        let options = Options {
            debug_mask: crate::debug::Mask::BASIC,
            ..Options::default()
        };
        let (updated, _, _) = options.file_flags_with_positions(&[b"s".to_vec()]).unwrap();
        assert!(updated.debug_mask.has(crate::debug::Mask::BASIC));
        assert!(updated.silent);
        assert!(
            !updated
                .recursive_flags()
                .windows(b"--debug".len())
                .any(|bytes| bytes == b"--debug")
        );
    }
}
