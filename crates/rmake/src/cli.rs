// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

use std::ffi::OsString;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::PathBuf;

#[derive(Default, Debug)]
pub struct Options {
    pub files: Vec<PathBuf>,
    pub directories: Vec<PathBuf>,
    pub goals: Vec<Vec<u8>>,
    pub assignments: Vec<Vec<u8>>,
    pub dry_run: bool,
    pub silent: bool,
    pub always: bool,
    pub ignore_errors: bool,
    pub keep_going: bool,
    pub question: bool,
    pub touch: bool,
    pub environment_override: bool,
    pub no_rules: bool,
    pub no_variables: bool,
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
        let mut combined = Vec::new();
        for flags in [gnumakeflags, makeflags].into_iter().flatten() {
            let bytes = flags.as_bytes();
            let mut tokens = Vec::new();
            let mut token = Vec::new();
            let mut i = 0;
            while i < bytes.len() {
                if bytes[i] == b'\\' && i + 1 < bytes.len() {
                    i += 1;
                    token.push(bytes[i]);
                } else if bytes[i].is_ascii_whitespace() {
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
                    return Err(
                        "option-like goals in inherited make flags are not implemented".into(),
                    );
                }
                // GNU omits these options when exporting MAKEFLAGS. Reject
                // them here rather than reading a different file or directory.
                if token.starts_with(b"--file")
                    || token.starts_with(b"--makefile")
                    || token.starts_with(b"--directory")
                    || (token.starts_with(b"-")
                        && !token.starts_with(b"--")
                        && (token.contains(&b'f') || token.contains(&b'C')))
                {
                    return Err("inherited file/directory options are not implemented".into());
                }
                combined.push(OsString::from_vec(token));
            }
        }
        combined.extend(args);
        Self::parse(combined)
    }

    pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Self, String> {
        let args: Vec<Vec<u8>> = args.into_iter().map(OsString::into_vec).collect();
        let mut result = Self::default();
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
                    b"--question" => Some(&mut result.question),
                    b"--touch" => Some(&mut result.touch),
                    b"--environment-overrides" => Some(&mut result.environment_override),
                    b"--no-builtin-rules" => Some(&mut result.no_rules),
                    b"--no-builtin-variables" => {
                        result.no_rules = true;
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
                } else {
                    match name {
                        b"--no-print-directory" if value.is_none() => {
                            result.no_print_directory = true
                        }
                        b"--file" | b"--makefile" | b"--directory" => {
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
                            if name == b"--directory" {
                                result.directories.push(path);
                            } else {
                                result.files.push(path);
                            }
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
                            if value.as_deref() != Some(b"1") {
                                return Err(
                                    "parallel execution is not implemented yet; use -j1".into()
                                );
                            }
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
                        b's' => result.silent = true,
                        b'B' => result.always = true,
                        b'i' => result.ignore_errors = true,
                        b'k' => result.keep_going = true,
                        b'S' => result.keep_going = false,
                        b'q' => result.question = true,
                        b't' => result.touch = true,
                        b'e' => result.environment_override = true,
                        b'r' => result.no_rules = true,
                        b'R' => {
                            result.no_variables = true;
                            result.no_rules = true;
                        }
                        option @ (b'f' | b'C' | b'j') => {
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
                            if option == b'j' {
                                if value != b"1" {
                                    return Err(
                                        "parallel execution is not implemented yet; use -j1".into(),
                                    );
                                }
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
            } else {
                result.goals.push(arg.clone());
            }
            i += 1;
        }
        Ok(result)
    }

    pub fn flags(&self) -> Vec<u8> {
        let mut flags = Vec::new();
        for (set, flag) in [
            (self.always, b'B'),
            (self.environment_override, b'e'),
            (self.ignore_errors, b'i'),
            (self.keep_going, b'k'),
            (self.dry_run, b'n'),
            (self.question, b'q'),
            (self.no_rules, b'r'),
            (self.no_variables, b'R'),
            (self.silent, b's'),
            (self.touch, b't'),
        ] {
            if set {
                flags.push(flag);
            }
        }
        if self.no_print_directory {
            flags.extend_from_slice(b" --no-print-directory");
        }
        flags
    }
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
    fn parse(args: &[&str]) -> Result<Options, String> {
        Options::parse(args.iter().map(OsString::from))
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
    fn unsupported_parallelism_fails_explicitly() {
        for args in [&["-j2"][..], &["--jobs"], &["-j", "all"], &["--jobs=0"]] {
            assert!(parse(args).is_err());
        }
        assert!(parse(&["--jobs", "1"]).is_ok());
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
        assert!(Options::with_environment([], Some("--jobserver-auth=3,4".into()), None).is_err());
        assert!(Options::with_environment([], Some("r -- -n".into()), None).is_err());
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
}
