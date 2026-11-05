// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! CLI source ownership and restart argument construction contain no replay.
use crate::cli::FileArgument;
use crate::output::Controller;
use std::ffi::OsString;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};

pub(crate) fn initialize(output: &Controller, program: &[u8]) {
    use rmake_linux::temporary::Warning;
    for warning in rmake_linux::temporary::initialize().warnings() {
        match warning {
            Warning::Environment { name, value, error } => {
                let reason = error.as_ref().map(crate::engine::os_error);
                output.write_parts(
                    true,
                    &[
                        program,
                        b": ",
                        name,
                        b" value ",
                        value.as_bytes(),
                        b": ",
                        reason
                            .as_deref()
                            .map_or(b"not a directory".as_slice(), str::as_bytes),
                        b"\n",
                    ],
                );
            }
            Warning::Default => output.write_parts(
                true,
                &[program, b": using default temporary directory '/tmp'\n"],
            ),
        }
    }
}

pub(crate) fn unlink(
    source: Option<&rmake_linux::stdin_source::Source>,
    output: &Controller,
    program: &[u8],
) {
    if let Some(source) = source
        && let Err(error) = source.unlink()
    {
        output.write_parts(
            true,
            &[
                program,
                b": unlink (temporary file): ",
                source.path().as_os_str().as_bytes(),
                b": ",
                crate::engine::os_error(&error).as_bytes(),
                b"\n",
            ],
        );
    }
}

pub(crate) fn restart_arguments(
    original: &[OsString],
    named: &[(FileArgument, PathBuf)],
    stdin: Option<(FileArgument, &Path)>,
) -> Vec<OsString> {
    let mut files: Vec<_> = named
        .iter()
        .map(|(span, path)| (*span, path.as_path(), false))
        .chain(stdin.map(|(span, path)| (span, path, true)))
        .collect();
    files.sort_unstable_by_key(|(span, _, _)| span.argument);

    let mut result = Vec::with_capacity(original.len() + 1);
    let mut next = 0;
    for (span, path, stdin) in files {
        result.extend_from_slice(&original[next..span.argument]);
        let option = original[span.argument].as_bytes();
        let prefix = if stdin {
            if span.prefix > 1 {
                result.push(OsString::from_vec(option[..span.prefix].to_vec()));
            }
            b"--temp-stdin=".as_slice()
        } else if span.prefix == 0 {
            b"-f"
        } else if span.consumed == 1 {
            &option[..=span.prefix]
        } else {
            result.push(original[span.argument].clone());
            b""
        };
        let replacement = [prefix, path.as_os_str().as_bytes()].concat();
        result.push(OsString::from_vec(replacement));
        next = span.argument + span.consumed;
    }
    result.extend_from_slice(&original[next..]);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argument_spans_preserve_clusters_raw_paths_and_surrounding_inputs() {
        for (arguments, index, consumed, prefix, expected) in [
            (
                vec!["A=one", "-f-", "goal"],
                1,
                1,
                1,
                vec!["A=one", "--temp-stdin=raw-$name", "goal"],
            ),
            (
                vec!["-ksf", "-", "goal"],
                0,
                2,
                3,
                vec!["-ks", "--temp-stdin=raw-$name", "goal"],
            ),
            (
                vec!["--file=-", "-fother"],
                0,
                1,
                0,
                vec!["--temp-stdin=raw-$name", "-fother"],
            ),
        ] {
            let arguments: Vec<_> = arguments.into_iter().map(OsString::from).collect();
            let rewritten = restart_arguments(
                &arguments,
                &[],
                Some((
                    FileArgument {
                        argument: index,
                        consumed,
                        prefix,
                    },
                    Path::new("raw-$name"),
                )),
            );
            assert_eq!(
                rewritten,
                expected.into_iter().map(OsString::from).collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn named_operands_and_stdin_share_original_argv_coordinates() {
        let input = [
            "--makefile",
            "~/first",
            "-sf-",
            "A=x",
            "-kf",
            "~/last",
            "goal",
        ];
        let original: Vec<_> = input.into_iter().map(OsString::from).collect();
        let first = FileArgument {
            argument: 0,
            consumed: 2,
            prefix: 0,
        };
        let stdin = FileArgument {
            argument: 2,
            consumed: 1,
            prefix: 2,
        };
        let last = FileArgument {
            argument: 4,
            consumed: 2,
            prefix: 2,
        };
        let selected = [
            (first, "retained/first".into()),
            (last, "retained/last".into()),
        ];
        let expected = [
            "-fretained/first",
            "-s",
            "--temp-stdin=tmp",
            "A=x",
            "-kf",
            "retained/last",
            "goal",
        ];
        assert_eq!(
            restart_arguments(&original, &selected, Some((stdin, Path::new("tmp")))),
            expected.into_iter().map(OsString::from).collect::<Vec<_>>()
        );
    }
}
