// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Pure .ONESHELL preparation derived from original GNU Make 4.4.1 probes.
//! Initial probes preceded source inspection; later argv/tail findings use
//! verified job.c references with an independently authored state model.

use crate::process::{Invocation, prepare as prepare_ordinary};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawScript {
    pub bytes: Vec<u8>,
    /// Raw modifier-only recipes are empty, unlike expansions yielding a LF.
    pub has_commands: bool,
    /// Only first-line '+' and direct MAKE references force the entire script.
    pub recursive: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Prefixes {
    pub quiet: bool,
    pub ignore_errors: bool,
    pub force: bool,
}

/// Inputs are reader/parser logical recipe bodies with their initial recipe
/// prefix already removed. Preserve empty entries and continuation newlines.
/// The caller retains the first recipe's source location and parse-time prefix.
pub fn collect<'a>(lines: impl IntoIterator<Item = &'a [u8]>) -> RawScript {
    let mut bytes = Vec::new();
    for (index, line) in lines.into_iter().enumerate() {
        if index != 0 {
            bytes.push(b'\n');
        }
        bytes.extend_from_slice(line);
    }
    let has_commands = bytes
        .iter()
        .any(|byte| !matches!(byte, b' ' | b'\t' | b'\n' | b'@' | b'-' | b'+'));
    let recursive = prefixes(&bytes).1.force
        || bytes
            .windows(7)
            .any(|part| part == b"$(MAKE)" || part == b"${MAKE}");
    RawScript {
        bytes,
        has_commands,
        recursive,
    }
}

/// Remove only first-line make modifiers. An LF always ends this scan, even if
/// the first logical recipe line is empty. Flags introduced by expansion count.
pub fn prefixes(mut input: &[u8]) -> (&[u8], Prefixes) {
    let mut flags = Prefixes::default();
    loop {
        let offset = input
            .iter()
            .position(|byte| !matches!(byte, b' ' | b'\t'))
            .unwrap_or(input.len());
        input = &input[offset..];
        match input.first() {
            Some(b'@') => flags.quiet = true,
            Some(b'-') => flags.ignore_errors = true,
            Some(b'+') => flags.force = true,
            _ => return (input, flags),
        }
        input = &input[1..];
    }
}

/// Transform an expanded body after `prefixes` has removed first-line make
/// modifiers. The prefix belongs to the recipe when parsed, not the final value
/// of .RECIPEPREFIX. Call before echo, including dry-run; do not call twice.
pub fn transform(command: &[u8], shell: &[u8], recipe_prefix: u8) -> Vec<u8> {
    internal_prefixes(command, recipe_prefix, bourne_compatible(shell))
}

/// An execution plan for the already-transformed script. The residual offset
/// describes GNU's fast-prefix replay boundary, not another make expansion.
/// The caller requests this only for actual execution, after unforced modes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedScript {
    pub invocation: Invocation,
    pub next: Option<usize>,
}

#[cfg(test)]
fn prepare(command: &[u8], shell: &[u8], flags: &[u8], ifs: Option<&[u8]>) -> Invocation {
    prepare_script(command, shell, flags, ifs).invocation
}

pub fn prepare_script(
    command: &[u8],
    shell: &[u8],
    flags: &[u8],
    ifs: Option<&[u8]>,
) -> PreparedScript {
    let next = residual_offset(command, shell, flags, ifs);
    if next.is_none()
        && let direct @ Invocation::Direct { .. } = prepare_ordinary(command, shell, flags, ifs)
    {
        return PreparedScript {
            invocation: direct,
            next,
        };
    }
    let mut args = Vec::new();
    if !flags.iter().all(|byte| matches!(byte, b' ' | b'\t')) {
        let (program, words) = match prepare_ordinary(flags, b"/bin/sh", b"-c", None) {
            Invocation::Direct { program, args } => (program, args),
            Invocation::Shell { program, mut args } => {
                // A slow flag parse re-enters GNU's .ONESHELL Bourne-body
                // preparation. Make modifiers on that body are consumed too.
                // This affects argv bytes even though no interpreter runs here.
                if let Some(body) = args.last_mut() {
                    *body = transform(prefixes(body).0, b"/bin/sh", b'\t');
                }
                (program, args)
            }
        };
        args.push(program);
        args.extend(words);
    }
    args.push(command.to_vec());
    PreparedScript {
        invocation: Invocation::Shell {
            program: shell.to_vec(),
            args,
        },
        next,
    }
}

fn residual_offset(
    command: &[u8],
    shell: &[u8],
    flags: &[u8],
    ifs: Option<&[u8]>,
) -> Option<usize> {
    let mut quoted = false;
    let mut at = 0;
    while let Some(&byte) = command.get(at) {
        if byte == b'\\' && command.get(at + 1) == Some(&b'\n') {
            at += 2;
            continue;
        }
        if byte == b'\n' {
            if !quoted {
                return None;
            }
            let mut prefix = command[..at].to_vec();
            prefix.push(b'\'');
            return matches!(
                prepare_ordinary(&prefix, shell, flags, ifs),
                Invocation::Direct { .. }
            )
            .then_some(at + 1);
        }
        if byte == b'\'' {
            quoted = !quoted;
        } else if !quoted && byte == b'\\' && at + 1 < command.len() {
            at += 1;
        }
        at += 1;
    }
    None
}

fn bourne_compatible(shell: &[u8]) -> bool {
    let basename = shell.rsplit(|byte| *byte == b'/').next().unwrap_or(shell);
    matches!(
        basename,
        b"sh" | b"bash" | b"dash" | b"ash" | b"ksh" | b"rksh" | b"zsh"
    )
}

/// GNU's custom-shell .ONESHELL argument path can free an interpreter-owned
/// argument after a slow flag parse. Classify only the hazardous lexical forms;
/// balanced plain quotes, escaped syntax and ordinary Bourne shells still work.
pub fn unsafe_configuration(shell: &[u8], flags: &[u8]) -> bool {
    if bourne_compatible(shell) {
        return false;
    }
    // A literal configuration LF with a leading double-quoted flag reaches a
    // distinct recorded allocator fault. Other LF configurations can simply
    // fail exec normally and must retain those ordinary diagnostics.
    if shell.contains(&b'\n') && flags.trim_ascii_start().starts_with(b"\"") {
        return true;
    }
    let mut quote = None;
    let mut at = 0;
    while let Some(&byte) = flags.get(at) {
        at += 1;
        if quote == Some(b'\'') {
            if byte == b'\'' {
                quote = None;
            }
            continue;
        }
        if byte == b'\\' {
            if let Some(&next) = flags.get(at)
                && (quote.is_none() || matches!(next, b'"' | b'\\' | b'$' | b'`' | b'\n'))
            {
                at += 1;
            }
            continue;
        }
        if byte == b'"' {
            quote = if quote.is_some() { None } else { Some(byte) };
            continue;
        }
        if quote == Some(b'"') {
            if matches!(byte, b'$' | b'`') {
                return true;
            }
        } else if byte == b'\'' {
            quote = Some(byte);
        } else if matches!(
            byte,
            b'\n'
                | b'#'
                | b';'
                | b'*'
                | b'?'
                | b'['
                | b']'
                | b'&'
                | b'|'
                | b'<'
                | b'>'
                | b'('
                | b')'
                | b'{'
                | b'}'
                | b'$'
                | b'`'
                | b'^'
                | b'~'
                | b'!'
        ) {
            return true;
        }
    }
    quote.is_some()
}

fn internal_prefixes(command: &[u8], recipe_prefix: u8, bourne: bool) -> Vec<u8> {
    let mut output = Vec::with_capacity(command.len());
    let mut index = 0;
    let mut backslashes = 0;
    while let Some(&byte) = command.get(index) {
        output.push(byte);
        index += 1;
        if byte == b'\n' && backslashes % 2 == 0 {
            if command.get(index) == Some(&recipe_prefix) {
                index += 1;
            }
            if bourne {
                while command
                    .get(index)
                    .is_some_and(|byte| matches!(byte, b' ' | b'\t' | b'@' | b'-' | b'+'))
                {
                    index += 1;
                }
            }
        }
        backslashes = if byte == b'\\' { backslashes + 1 } else { 0 };
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safety_classifier_preserves_quotes_escapes_bourne_and_normal_exec_errors() {
        for flags in [
            b"-c $NAME".as_slice(),
            b"-c \"open",
            b"one;two",
            b"one\ntwo",
        ] {
            assert!(unsafe_configuration(b"./capture", flags));
            assert!(!unsafe_configuration(b"/bin/sh", flags));
        }
        for flags in [
            b"-c".as_slice(),
            b"-c 'two words'",
            b"-c \"two words\"",
            b"-c '$NAME'",
            b"-c \\$NAME",
        ] {
            assert!(!unsafe_configuration(b"./capture", flags));
            assert!(!unsafe_configuration(b"./capture A\nB", flags));
        }
        assert!(unsafe_configuration(b"./capture A\nB", b"\"-c\""));
        assert!(!unsafe_configuration(b"./capture", b"\"-c\""));
    }

    #[test]
    fn slow_quoted_flags_consume_make_modifiers_at_the_reentered_body() {
        assert_eq!(
            prepare(b"printf body", b"./capture", b"-c \"extra words\"", None),
            Invocation::Shell {
                program: b"./capture".to_vec(),
                args: vec![
                    b"/bin/sh".to_vec(),
                    b"-c".to_vec(),
                    b"c \"extra words\"".to_vec(),
                    b"printf body".to_vec()
                ]
            }
        );
    }

    struct Prepared {
        echo: Vec<u8>,
        invocation: Invocation,
    }

    fn shell(command: &[u8], executable: &[u8], prefix: u8) -> Prepared {
        let echo = transform(command, executable, prefix);
        let invocation = prepare(&echo, executable, b"-c", None);
        Prepared { echo, invocation }
    }

    #[test]
    fn empty_shell_keeps_internal_modifiers_and_remains_the_executable() {
        let command = b"first\n\t@second\n-+third";
        let transformed = transform(command, b"", b'\t');
        assert_eq!(transformed, b"first\n@second\n-+third");
        for flags in [b"".as_slice(), b"-c", b"./wrapper"] {
            let mut args = Vec::new();
            if !flags.is_empty() {
                args.push(flags.to_vec());
            }
            args.push(transformed.clone());
            assert_eq!(
                prepare(&transformed, b"", flags, None),
                Invocation::Shell {
                    program: vec![],
                    args
                }
            );
        }
    }

    #[test]
    fn joins_recipe_lines_without_inventing_terminal_newline() {
        let script = collect([b"@first".as_slice(), b"@second"]);
        assert_eq!(script.bytes, b"@first\n@second");
        assert!(script.has_commands);
        assert!(!script.recursive);
    }

    #[test]
    fn keeps_leading_trailing_and_middle_empty_recipe_lines() {
        let script = collect([b"".as_slice(), b"first", b"", b"second", b""]);
        assert_eq!(script.bytes, b"\nfirst\n\nsecond\n");
    }

    #[test]
    fn raw_empty_and_expansion_to_empty_are_distinct() {
        assert!(!collect([b"@".as_slice(), b" \t+-@ "]).has_commands);
        assert!(collect([b"$(EMPTY)".as_slice(), b"$(EMPTY)"]).has_commands);
        assert_eq!(shell(b"\n", b"./other", b'\t').echo, b"\n");
    }

    #[test]
    fn only_initial_plus_marks_raw_script_recursive() {
        assert!(collect([b" @ +first".as_slice(), b"second"]).recursive);
        assert!(!collect([b"first".as_slice(), b"+second"]).recursive);
        assert!(!collect([b"".as_slice(), b"+second"]).recursive);
    }

    #[test]
    fn direct_make_reference_anywhere_marks_recursive_including_escaped_dollar() {
        for body in [b"$(MAKE)".as_slice(), b"${MAKE}", b"$$(MAKE)"] {
            assert!(collect([b"first".as_slice(), body]).recursive);
        }
        assert!(!collect([b"$(M)".as_slice()]).recursive);
    }

    #[test]
    fn first_prefix_scan_stops_at_newline() {
        let (body, flags) = prefixes(b" \t@ - + first\n@second");
        assert_eq!(body, b"first\n@second");
        assert!(flags.quiet && flags.ignore_errors && flags.force);
        assert_eq!(
            prefixes(b" \n@second"),
            (b"\n@second".as_slice(), Prefixes::default())
        );
    }

    #[test]
    fn simple_single_command_retains_direct_fast_path() {
        assert_eq!(
            shell(b"missing-command", b"/bin/sh", b'\t').invocation,
            Invocation::Direct {
                program: b"missing-command".to_vec(),
                args: Vec::new()
            }
        );
        assert!(matches!(
            shell(b"missing-command\n", b"/bin/sh", b'\t').invocation,
            Invocation::Shell { .. }
        ));
    }

    #[test]
    fn slow_path_does_not_tokenize_shell_name() {
        for executable in [b"./sh -extra".as_slice(), b"'./sh'", b"./sh "] {
            let prepared = shell(b"first\n@second", executable, b'\t');
            assert_eq!(prepared.echo, b"first\n@second");
            assert_eq!(
                prepared.invocation,
                Invocation::Shell {
                    program: executable.to_vec(),
                    args: vec![b"-c".to_vec(), b"first\n@second".to_vec()],
                }
            );
        }
    }

    #[test]
    fn recognized_shells_strip_internal_modifiers_and_indentation() {
        for executable in [
            b"sh".as_slice(),
            b"./bash",
            b"/bin/dash",
            b"ash",
            b"ksh",
            b"rksh",
            b"/a/../zsh",
        ] {
            assert_eq!(
                shell(b"first\n \t @ - +second", executable, b'\t').echo,
                b"first\nsecond"
            );
        }
    }

    #[test]
    fn other_shell_names_preserve_internal_modifiers() {
        for executable in [
            b"mksh".as_slice(),
            b"pdksh",
            b"yash",
            b"fish",
            b"csh",
            b"tcsh",
            b"busybox",
            b"SH",
            b"bash.exe",
            b"mysh",
        ] {
            assert_eq!(
                shell(b"first\n @second\n-third\n+fourth", executable, b'\t').echo,
                b"first\n @second\n-third\n+fourth"
            );
        }
    }

    #[test]
    fn custom_shell_still_loses_one_additional_initial_recipe_prefix() {
        assert_eq!(
            shell(b"first\n\t@second\n\t\t@third", b"./other", b'\t').echo,
            b"first\n@second\n\t@third"
        );
        assert_eq!(
            shell(b"first\n>@second\n>>@third", b"./other", b'>').echo,
            b"first\n@second\n>@third"
        );
        assert_eq!(
            shell(b"first\n >@second", b"./other", b'>').echo,
            b"first\n >@second"
        );
    }

    #[test]
    fn first_line_never_loses_an_extra_recipe_prefix() {
        assert_eq!(
            shell(b">first\n>@second", b"./other", b'>').echo,
            b">first\n@second"
        );
    }

    #[test]
    fn odd_backslash_run_preserves_next_line_and_even_run_does_not() {
        for count in 1..=4 {
            let mut command = b"first ".to_vec();
            command.extend(std::iter::repeat_n(b'\\', count));
            command.extend_from_slice(b"\n\t@second\n@third");
            let mut expected = b"first ".to_vec();
            expected.extend(std::iter::repeat_n(b'\\', count));
            expected.extend_from_slice(if count % 2 == 1 {
                b"\n\t@second"
            } else {
                b"\nsecond"
            });
            expected.extend_from_slice(b"\nthird");
            assert_eq!(shell(&command, b"./sh", b'\t').echo, expected);
        }
    }

    #[test]
    fn continuation_keeps_additional_prefix_for_custom_shell_too() {
        assert_eq!(
            shell(b"first \\\n>@second\n>@third", b"./other", b'>').echo,
            b"first \\\n>@second\n@third"
        );
    }

    #[test]
    fn internal_filtering_is_lexical_even_inside_quotes_and_here_documents() {
        assert_eq!(
            shell(b"first 'a\n@b'", b"./sh", b'\t').echo,
            b"first 'a\nb'"
        );
        assert_eq!(
            shell(b"cat <<EOF\n   spaced\n@marked\nEOF", b"./sh", b'\t').echo,
            b"cat <<EOF\nspaced\nmarked\nEOF"
        );
    }

    #[test]
    fn flag_words_support_single_quotes_and_backslash_escapes() {
        let prepared = prepare(b"body", b"./other", b"one 'two words' a\\ b ''", None);
        assert_eq!(
            prepared,
            Invocation::Shell {
                program: b"./other".to_vec(),
                args: vec![
                    b"one".to_vec(),
                    b"two words".to_vec(),
                    b"a b".to_vec(),
                    b"".to_vec(),
                    b"body".to_vec()
                ],
            }
        );
    }

    #[test]
    fn empty_flags_are_not_replaced_by_c() {
        for flags in [b"".as_slice(), b" \t"] {
            assert_eq!(
                prepare(b"body", b"./other", flags, None),
                Invocation::Shell {
                    program: b"./other".to_vec(),
                    args: vec![b"body".to_vec()]
                }
            );
        }
    }

    #[test]
    fn complex_flags_keep_the_nested_interpreter_argv_as_data() {
        for flags in [
            b"one \"two words\"".as_slice(),
            b"NAME=value",
            b"command argument",
        ] {
            assert_eq!(
                prepare(b"body", b"./other", flags, None),
                Invocation::Shell {
                    program: b"./other".to_vec(),
                    args: vec![
                        b"/bin/sh".to_vec(),
                        b"-c".to_vec(),
                        flags.to_vec(),
                        b"body".to_vec()
                    ],
                }
            );
        }
    }

    #[test]
    fn quoted_newlines_retain_the_fast_prefix_residual_boundary() {
        let command = b"printf '%s\\n' 'first\nlast'";
        let result = prepare_script(command, b"/bin/sh", b"-c", None);
        assert_eq!(
            result.next,
            command.iter().position(|b| *b == b'\n').map(|n| n + 1)
        );
        assert_eq!(
            result.invocation,
            Invocation::Shell {
                program: b"/bin/sh".to_vec(),
                args: vec![b"-c".to_vec(), command.to_vec()]
            }
        );
        for (shell, flags) in [
            (b"/bin/bash".as_slice(), b"-c".as_slice()),
            (b"/bin/sh", b"-ce"),
        ] {
            assert_eq!(prepare_script(command, shell, flags, None).next, None);
        }
    }

    #[test]
    fn a_later_metacharacter_does_not_erase_an_earlier_residual_boundary() {
        assert!(
            prepare_script(b"printf 'first\nlast'; true", b"/bin/sh", b"-c", None)
                .next
                .is_some()
        );
        assert_eq!(
            prepare_script(b"printf \"first\nlast\"", b"/bin/sh", b"-c", None).next,
            None
        );
    }

    #[test]
    fn escaped_newlines_do_not_create_a_residual_pass() {
        assert_eq!(
            prepare_script(b"printf 'first\\\nlast'", b"/bin/sh", b"-c", None).next,
            None
        );
        assert_eq!(
            prepare_script(b"printf first\\\nlast", b"/bin/sh", b"-c", None).next,
            None
        );
    }

    #[test]
    fn non_utf8_commands_and_shell_names_are_preserved() {
        let prepared = shell(b"first\xff\n@second\xfe", b"./other\xff", b'\t');
        assert_eq!(prepared.echo, b"first\xff\n@second\xfe");
        assert_eq!(
            prepared.invocation,
            Invocation::Shell {
                program: b"./other\xff".to_vec(),
                args: vec![b"-c".to_vec(), b"first\xff\n@second\xfe".to_vec()],
            }
        );
    }
}
