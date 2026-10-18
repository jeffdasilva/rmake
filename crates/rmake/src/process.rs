// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Prepare Linux recipe invocations without executing them.
//!
//! The direct-execution boundaries below were established with original black-box
//! probes of GNU Make 4.4.1. No GNU implementation source was consulted. In
//! particular, GNU's list of commands requiring a shell differs from the list of
//! commands a particular shell implements as built-ins.

/// Arguments exclude argv[0]. All values retain their original Unix bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Invocation {
    Direct {
        program: Vec<u8>,
        args: Vec<Vec<u8>>,
    },
    Shell {
        program: Vec<u8>,
        args: Vec<Vec<u8>>,
    },
}

/// Select GNU Make's simple-command fast path, or preserve shell evaluation.
///
/// `command`, `shell`, `flags`, and `ifs` have already undergone make expansion.
/// This function performs no expansion, filesystem access, or process execution.
/// An ENOEXEC fallback belongs to the launcher for either invocation kind: run
/// `/bin/sh` with the attempted program and its prepared arguments, without
/// adding `.SHELLFLAGS` again.
pub fn prepare(command: &[u8], shell: &[u8], flags: &[u8], ifs: Option<&[u8]>) -> Invocation {
    let shell = if shell.is_empty() { b"/bin/sh" } else { shell };
    if shell == b"/bin/sh"
        && matches!(flags, b"-c" | b"-ec")
        && ifs.is_none_or(|value| {
            value
                .iter()
                .all(|byte| matches!(byte, b' ' | b'\t' | b'\n'))
        })
        && let Some(mut words) = split_words(command, Mode::Recipe)
        && !words.is_empty()
        && !requires_shell(&words[0])
    {
        let program = words.remove(0);
        return Invocation::Direct {
            program,
            args: words,
        };
    }
    shell_invocation(command, shell, flags)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Recipe,
    Configuration,
}

/// Return None as soon as shell interpretation is necessary. Quote state is
/// lexical: an escaped or single-quoted '=' in argv[0] is not an assignment.
fn split_words(input: &[u8], mode: Mode) -> Option<Vec<Vec<u8>>> {
    let mut words = Vec::new();
    let mut word = Vec::new();
    let mut started = false;
    let mut quote = None;
    let mut index = 0;
    while let Some(&byte) = input.get(index) {
        index += 1;
        if quote == Some(b'\'') {
            if byte == b'\'' {
                quote = None;
            } else {
                word.push(byte);
            }
            continue;
        }
        if quote == Some(b'"') {
            match byte {
                b'"' => quote = None,
                b'$' | b'`' => return None,
                b'\\' => match input.get(index) {
                    Some(b'\n') => index += 1,
                    Some(next @ (b'$' | b'`' | b'"' | b'\\')) => {
                        word.push(*next);
                        index += 1;
                    }
                    _ => word.push(byte),
                },
                _ => word.push(byte),
            }
            continue;
        }
        match byte {
            b' ' | b'\t' => {
                if started {
                    words.push(std::mem::take(&mut word));
                    started = false;
                }
            }
            b'\'' => {
                started = true;
                quote = Some(byte);
            }
            b'"' if mode == Mode::Configuration => {
                started = true;
                quote = Some(byte);
            }
            b'\\' => match input.get(index) {
                Some(b'\n') => index += 1,
                Some(next) => {
                    word.push(*next);
                    started = true;
                    index += 1;
                }
                // GNU drops a final unpaired backslash in this fast path.
                None => {}
            },
            b'=' if words.is_empty() => return None,
            byte if is_metacharacter(byte) => return None,
            _ => {
                word.push(byte);
                started = true;
            }
        }
    }
    if quote.is_some() {
        return None;
    }
    if started {
        words.push(word);
    }
    Some(words)
}

fn is_metacharacter(byte: u8) -> bool {
    matches!(
        byte,
        b'\n'
            | b'#'
            | b';'
            | b'"'
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
    )
}

fn requires_shell(program: &[u8]) -> bool {
    matches!(
        program,
        b":" | b"."
            | b"alias"
            | b"bg"
            | b"break"
            | b"cd"
            | b"command"
            | b"continue"
            | b"eval"
            | b"exec"
            | b"exit"
            | b"export"
            | b"fc"
            | b"fg"
            | b"getopts"
            | b"hash"
            | b"jobs"
            | b"logout"
            | b"read"
            | b"readonly"
            | b"return"
            | b"set"
            | b"shift"
            | b"test"
            | b"times"
            | b"trap"
            | b"type"
            | b"ulimit"
            | b"umask"
            | b"unalias"
            | b"unset"
            | b"wait"
            | b"if"
            | b"case"
            | b"for"
            | b"while"
    )
}

fn shell_invocation(command: &[u8], shell: &[u8], flags: &[u8]) -> Invocation {
    let mut configuration = Vec::with_capacity(shell.len() + flags.len() + 1);
    // GNU protects these SHELL bytes before reparsing the configuration. This
    // happens even inside single quotes: their contents retain the inserted
    // backslash. Existing quotes/backslashes still participate in parsing, so
    // this is neither a shell evaluation of SHELL nor a literal argv split.
    for &byte in shell {
        if byte != b'\n' && is_metacharacter(byte) {
            configuration.push(b'\\');
        }
        configuration.push(byte);
    }
    configuration.push(b' ');
    configuration.extend_from_slice(flags);
    if let Some(mut words) = split_words(&configuration, Mode::Configuration)
        && !words.is_empty()
        && !requires_shell(&words[0])
    {
        let program = words.remove(0);
        words.push(command.to_vec());
        return Invocation::Shell {
            program,
            args: words,
        };
    }

    // Flags retain shell syntax; assignments, builtins, and existing backslash
    // combinations can also require evaluation after the SHELL protection.
    // Preserve that syntax while keeping the recipe one literal argument.
    configuration.push(b' ');
    single_quote(command, &mut configuration);
    Invocation::Shell {
        program: b"/bin/sh".to_vec(),
        args: vec![b"-c".to_vec(), configuration],
    }
}

fn single_quote(input: &[u8], output: &mut Vec<u8>) {
    output.push(b'\'');
    for &byte in input {
        if byte == b'\'' {
            output.extend_from_slice(b"'\\''");
        } else {
            output.push(byte);
        }
    }
    output.push(b'\'');
}

#[cfg(test)]
mod tests {
    use super::*;

    fn direct(program: &[u8], args: &[&[u8]]) -> Invocation {
        Invocation::Direct {
            program: program.to_vec(),
            args: args.iter().map(|arg| arg.to_vec()).collect(),
        }
    }

    fn shell(command: &[u8]) -> Invocation {
        Invocation::Shell {
            program: b"/bin/sh".to_vec(),
            args: vec![b"-c".to_vec(), command.to_vec()],
        }
    }

    fn default_prepare(command: &[u8]) -> Invocation {
        prepare(command, b"/bin/sh", b"-c", None)
    }

    #[test]
    fn direct_command_has_no_shell_argument() {
        assert_eq!(default_prepare(b"tool a b"), direct(b"tool", &[b"a", b"b"]));
    }

    #[test]
    fn missing_program_is_still_direct() {
        assert_eq!(
            default_prepare(b"rmake-no-such-executable"),
            direct(b"rmake-no-such-executable", &[])
        );
    }

    #[test]
    fn whitespace_delimits_without_empty_arguments() {
        assert_eq!(
            default_prepare(b" \ttool  a\t b \t"),
            direct(b"tool", &[b"a", b"b"])
        );
    }

    #[test]
    fn other_ascii_whitespace_is_literal() {
        assert_eq!(
            default_prepare(b"tool a\rb\x0bc\x0cd"),
            direct(b"tool", &[b"a\rb\x0bc\x0cd"])
        );
    }

    #[test]
    fn single_quotes_group_and_preserve_empty_arguments() {
        assert_eq!(
            default_prepare(b"tool '' 'two words' a'b c'd"),
            direct(b"tool", &[b"", b"two words", b"ab cd"])
        );
    }

    #[test]
    fn empty_quoted_program_is_direct() {
        assert_eq!(default_prepare(b"'' arg"), direct(b"", &[b"arg"]));
    }

    #[test]
    fn single_quotes_preserve_shell_syntax() {
        assert_eq!(
            default_prepare(b"tool '$x `cmd` \\ * # ! \"'"),
            direct(b"tool", &[b"$x `cmd` \\ * # ! \""])
        );
    }

    #[test]
    fn single_quotes_preserve_backslash_newline() {
        assert_eq!(
            default_prepare(b"tool 'a\\\nb'"),
            direct(b"tool", &[b"a\\\nb"])
        );
    }

    #[test]
    fn outside_backslashes_protect_metacharacters() {
        assert_eq!(
            default_prepare(br"tool a\ b \$x \# \* \\"),
            direct(b"tool", &[b"a b", b"$x", b"#", b"*", b"\\"])
        );
    }

    #[test]
    fn backslash_newline_is_removed_outside_quotes() {
        assert_eq!(
            default_prepare(b"tool a\\\nb a\\\n b"),
            direct(b"tool", &[b"ab", b"a", b"b"])
        );
    }

    #[test]
    fn final_backslash_is_dropped() {
        assert_eq!(default_prepare(b"tool a\\"), direct(b"tool", &[b"a"]));
    }

    #[test]
    fn double_quotes_always_require_shell_in_recipe() {
        for command in [
            b"\"tool\"".as_slice(),
            b"tool \"abc\"",
            b"tool a\"b\"c",
            b"tool \"\"",
        ] {
            assert_eq!(default_prepare(command), shell(command));
        }
    }

    #[test]
    fn unterminated_quotes_preserve_command_for_shell_diagnostic() {
        for command in [b"tool 'unterminated".as_slice(), b"tool \"unterminated"] {
            assert_eq!(default_prepare(command), shell(command));
        }
    }

    #[test]
    fn all_observed_metacharacters_require_shell() {
        for byte in b"\n#;\"*?[]&|<>(){}$`^~!" {
            let command = [b"tool a".as_slice(), &[*byte], b"b"].concat();
            assert_eq!(default_prepare(&command), shell(&command), "byte {byte}");
        }
    }

    #[test]
    fn ordinary_punctuation_is_not_shell_syntax() {
        assert_eq!(
            default_prepare(b"tool %,/:+=@_-"),
            direct(b"tool", &[b"%,/:+=@_-"])
        );
    }

    #[test]
    fn unquoted_equals_in_first_word_requires_shell() {
        for command in [b"A=b tool".as_slice(), b"1a=b tool", b"=b", b"a''=b"] {
            assert_eq!(default_prepare(command), shell(command));
        }
    }

    #[test]
    fn quoted_equals_and_argument_equals_are_literal() {
        assert_eq!(default_prepare(b"'a=b'"), direct(b"a=b", &[]));
        assert_eq!(default_prepare(b"a\\=b"), direct(b"a=b", &[]));
        assert_eq!(default_prepare(b"tool a=b"), direct(b"tool", &[b"a=b"]));
    }

    #[test]
    fn shell_commands_are_classified_after_unquoting() {
        for command in [b"cd /".as_slice(), b"'cd' /", b"c\\d /"] {
            assert_eq!(default_prepare(command), shell(command));
        }
    }

    #[test]
    fn observed_gnu_shell_commands_are_not_executed_directly() {
        for command in b": . alias bg break cd command continue eval exec exit export fc fg getopts hash jobs logout read readonly return set shift test times trap type ulimit umask unalias unset wait if case for while".split(|byte| *byte == b' ') {
            assert_eq!(default_prepare(command), shell(command));
        }
    }

    #[test]
    fn common_shell_builtins_still_use_gnu_direct_path() {
        for command in b"echo printf pwd true false kill local until then else elif fi esac do done function select time".split(|byte| *byte == b' ') {
            assert_eq!(default_prepare(command), direct(command, &[]));
        }
    }

    #[test]
    fn absolute_builtin_name_is_an_external_program() {
        assert_eq!(
            default_prepare(b"/bin/test a"),
            direct(b"/bin/test", &[b"a"])
        );
    }

    #[test]
    fn exact_ec_flags_allow_direct_execution() {
        assert_eq!(
            prepare(b"tool", b"/bin/sh", b"-ec", None),
            direct(b"tool", &[])
        );
    }

    #[test]
    fn other_flags_remain_shell_arguments() {
        for (flags, args) in [
            (b"-ce".as_slice(), vec![b"-ce".to_vec(), b"tool".to_vec()]),
            (
                b"-x -c",
                vec![b"-x".to_vec(), b"-c".to_vec(), b"tool".to_vec()],
            ),
            (b"", vec![b"tool".to_vec()]),
        ] {
            assert_eq!(
                prepare(b"tool", b"/bin/sh", flags, None),
                Invocation::Shell {
                    program: b"/bin/sh".to_vec(),
                    args
                }
            );
        }
    }

    #[test]
    fn ifs_accepts_any_subset_and_order_of_standard_whitespace() {
        for ifs in [b"".as_slice(), b" \t\n", b"\n\t ", b"  "] {
            assert_eq!(
                prepare(b"tool a", b"/bin/sh", b"-c", Some(ifs)),
                direct(b"tool", &[b"a"])
            );
        }
    }

    #[test]
    fn custom_ifs_requires_shell() {
        for ifs in [b":".as_slice(), b" \t\nx", b"\r"] {
            assert_eq!(
                prepare(b"tool a", b"/bin/sh", b"-c", Some(ifs)),
                shell(b"tool a")
            );
        }
    }

    #[test]
    fn custom_shell_receives_exact_command() {
        assert_eq!(
            prepare(b"tool 'a b'", b"/bin/bash", b"-c", None),
            Invocation::Shell {
                program: b"/bin/bash".to_vec(),
                args: vec![b"-c".to_vec(), b"tool 'a b'".to_vec()]
            }
        );
    }

    #[test]
    fn empty_shell_uses_default() {
        assert_eq!(prepare(b"tool", b"", b"-c", None), direct(b"tool", &[]));
    }

    #[test]
    fn shell_configuration_supports_quoted_and_escaped_words() {
        assert_eq!(
            prepare(
                b"body",
                b"'./wrapper' --option",
                b"one \"two words\" 'three words' a\\ b \"\" ''",
                None
            ),
            Invocation::Shell {
                program: b"./wrapper".to_vec(),
                args: vec![
                    b"--option".to_vec(),
                    b"one".to_vec(),
                    b"two words".to_vec(),
                    b"three words".to_vec(),
                    b"a b".to_vec(),
                    b"".to_vec(),
                    b"".to_vec(),
                    b"body".to_vec()
                ]
            }
        );
    }

    #[test]
    fn shell_metacharacters_are_literal_before_configuration_parsing() {
        for byte in b"#;\"*?[]&|<>(){}$`^~!" {
            let program = [b"./capture".as_slice(), &[*byte], b"tail"].concat();
            let argument = [b"left".as_slice(), &[*byte], b"right"].concat();
            let shell_text = [&program[..], b" ", &argument].concat();
            assert_eq!(
                prepare(b"body", &shell_text, b"-c", None),
                Invocation::Shell {
                    program,
                    args: vec![argument, b"-c".to_vec(), b"body".to_vec()],
                },
                "byte {byte}",
            );
        }
    }

    #[test]
    fn shell_double_quotes_are_literal_but_single_quotes_still_group() {
        assert_eq!(
            prepare(
                b"body",
                b"./capture \"two words\" 'three words'",
                b"-c",
                None
            ),
            Invocation::Shell {
                program: b"./capture".to_vec(),
                args: vec![
                    b"\"two".to_vec(),
                    b"words\"".to_vec(),
                    b"three words".to_vec(),
                    b"-c".to_vec(),
                    b"body".to_vec()
                ],
            },
        );
        assert_eq!(
            prepare(
                b"body",
                b"./capture '$NAME' 'a;b' '\"quoted\"'",
                b"-c",
                None
            ),
            Invocation::Shell {
                program: b"./capture".to_vec(),
                args: vec![
                    b"\\$NAME".to_vec(),
                    b"a\\;b".to_vec(),
                    b"\\\"quoted\\\"".to_vec(),
                    b"-c".to_vec(),
                    b"body".to_vec()
                ],
            },
        );
    }

    #[test]
    fn protected_shell_and_interpreted_flags_keep_separate_boundaries() {
        assert_eq!(
            prepare(b"body 'quoted'", b"./capture $NAME", b"-c \"$FLAG\"", None),
            Invocation::Shell {
                program: b"/bin/sh".to_vec(),
                args: vec![
                    b"-c".to_vec(),
                    b"./capture \\$NAME -c \"$FLAG\" 'body '\\''quoted'\\'''".to_vec()
                ],
            },
        );
    }

    #[test]
    fn original_backslashes_can_reactivate_protected_shell_syntax() {
        assert_eq!(
            prepare(b"body", b"./capture \\$NAME", b"-c", None),
            Invocation::Shell {
                program: b"/bin/sh".to_vec(),
                args: vec![b"-c".to_vec(), b"./capture \\\\$NAME -c 'body'".to_vec()],
            },
        );
        assert_eq!(
            prepare(b"body", b"./capture\\ space", b"-c", None),
            Invocation::Shell {
                program: b"./capture space".to_vec(),
                args: vec![b"-c".to_vec(), b"body".to_vec()],
            },
        );
    }

    #[test]
    fn double_quoted_configuration_backslash_rules() {
        assert_eq!(
            prepare(b"body", b"./wrapper", b"\"a\\qb\\$c\\\"d\\\\e\"", None),
            Invocation::Shell {
                program: b"./wrapper".to_vec(),
                args: vec![b"a\\qb$c\"d\\e".to_vec(), b"body".to_vec()]
            }
        );
    }

    #[test]
    fn shell_syntax_in_configuration_uses_outer_shell() {
        assert_eq!(
            prepare(b"body 'quoted'", b"./wrapper", b"one;two", None),
            Invocation::Shell {
                program: b"/bin/sh".to_vec(),
                args: vec![
                    b"-c".to_vec(),
                    b"./wrapper one;two 'body '\\''quoted'\\'''".to_vec()
                ]
            }
        );
    }

    #[test]
    fn malformed_configuration_is_left_for_shell_diagnostic() {
        assert_eq!(
            prepare(b"body", b"./wrapper", b"'unterminated", None),
            Invocation::Shell {
                program: b"/bin/sh".to_vec(),
                args: vec![b"-c".to_vec(), b"./wrapper 'unterminated 'body'".to_vec()]
            }
        );
    }

    #[test]
    fn recipe_bytes_are_not_interpreted_in_configuration() {
        assert_eq!(
            prepare(b"$(touch file); 'quoted'\nnext", b"./wrapper", b"-c", None),
            Invocation::Shell {
                program: b"./wrapper".to_vec(),
                args: vec![b"-c".to_vec(), b"$(touch file); 'quoted'\nnext".to_vec()]
            }
        );
    }

    #[test]
    fn non_utf8_bytes_are_preserved() {
        assert_eq!(
            default_prepare(b"tool\xff '\xfe'"),
            direct(b"tool\xff", &[b"\xfe"])
        );
        assert_eq!(
            prepare(b"body\xff", b"./wrapper\xfe", b"-c", None),
            Invocation::Shell {
                program: b"./wrapper\xfe".to_vec(),
                args: vec![b"-c".to_vec(), b"body\xff".to_vec()]
            }
        );
    }

    #[test]
    fn embedded_nul_is_not_silently_truncated() {
        assert_eq!(default_prepare(b"tool a\0b"), direct(b"tool", &[b"a\0b"]));
    }

    #[test]
    fn empty_command_stays_an_empty_shell_command() {
        assert_eq!(default_prepare(b""), shell(b""));
    }
}
