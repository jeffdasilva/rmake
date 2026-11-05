// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! CLI source ownership and restart argument construction contain no replay.
use crate::cli::StdinArgument;
use crate::output::Controller;
use std::ffi::OsString;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::Path;

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
    span: StdinArgument,
    path: &Path,
) -> Vec<OsString> {
    let mut result = Vec::with_capacity(original.len() + 1);
    result.extend_from_slice(&original[..span.argument]);
    if span.prefix > 1 {
        result.push(OsString::from_vec(
            original[span.argument].as_bytes()[..span.prefix].to_vec(),
        ));
    }
    let mut replacement = b"--temp-stdin=".to_vec();
    replacement.extend_from_slice(path.as_os_str().as_bytes());
    result.push(OsString::from_vec(replacement));
    result.extend_from_slice(&original[span.argument + span.consumed..]);
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
                StdinArgument {
                    argument: index,
                    consumed,
                    prefix,
                },
                Path::new("raw-$name"),
            );
            assert_eq!(
                rewritten,
                expected.into_iter().map(OsString::from).collect::<Vec<_>>()
            );
        }
    }
}
