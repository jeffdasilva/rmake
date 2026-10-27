# Shell configuration boundaries

GNU Make 4.4.1 treats expanded `SHELL` text differently from expanded
`.SHELLFLAGS` when preparing ordinary recipes. Original executable probes and
argv-reporting scripts establish the behavior below. The initial literal-shell
slice used executable observations only. The later empty-shell assignment
boundary is source-informed, as recorded under reference provenance below.

## Protecting bytes from SHELL

Before ordinary configuration parsing, protect these bytes originating in
`SHELL`: `# ; " * ? [ ] & | < > ( ) { } $` plus backtick, `^`, `~` and `!`.
The subsequent configuration parser retains their literal values. For example,
`SHELL = ./capture $NAME` passes literal `$NAME` to that executable; it must not
ask an outer shell to expand the name. Literal metacharacters in the executable's
filename receive the same treatment.

Protection occurs before single-quote processing. Consequently a dollar inside
single quotes retains an inserted backslash, while double quotes originating in
`SHELL` are literal bytes rather than grouping syntax. Existing single quotes,
backslashes, spaces and tabs still affect tokenization. An existing backslash can
interact with the inserted protection and require shell evaluation. A general
literal whitespace split would therefore be incorrect.

`.SHELLFLAGS` remains unprotected at this boundary. Its supported quoting,
expansion and redirection behavior still uses the configuration parser and shell
fallback. Assignment prefixes and builtin command names also retain their
existing fallback decisions. The recipe body remains one argument during
configuration construction, so its syntax is evaluated only by the eventual
selected interpreter.

## Launching and other modes

The prepared literal executable path goes through the normal launcher. Missing,
non-executable and directory paths retain their respective diagnostics and GNU's
recipe error status. Executable text without a shebang retains the existing
`ENOEXEC` fallback with its original path and argument vector.

`.ONESHELL` retains a separate rule: the entire expanded `SHELL` value is its
executable path. It does not use ordinary shell-name tokenization. Unforced
dry-run/query/touch modes do not launch a shell; forced recipes exercise the
normal launch boundary. Permanent fixtures cover those distinctions and mixed
protected shell bytes with interpreted flags.

## Empty and undefined shell values

The default `/bin/sh` is a variable binding established at startup. An explicit
empty assignment, a recursive value that expands to empty, or `undefine SHELL`
does not restore that default when preparing a recipe.

For ordinary recipes, an empty shell contributes no words to the configuration.
The first parsed `.SHELLFLAGS` word becomes the executable: the usual `-c` thus
produces a failed attempt to execute a program named `-c`. Flags can instead name
an actual executable and arguments, which receive the complete recipe as the
last argument.

If both shell and flags contribute no words, the complete recipe is generally
the executable name, with no additional arguments. Whitespace and punctuation
inside that name stay literal. Exact recognized builtin names use the shell path; an
exact `:` retains GNU's no-child optimization and consequently skips export
expansion. Trailing whitespace is significant here: `: ` is a different filename
and does not receive that optimization.

An equals sign in that complete recipe instead requires a shell invocation,
including for invalid assignment names. The shell receives one protected word,
with assignment syntax still recognizable: `A=B C=D` is one assignment with a
literal space in its value. Metacharacters in that value must not execute other
commands or redirect files. Invalid assignment names attempt that literal word
as a program, retaining shell-generated missing-command diagnostics. This path
does prepare the child's environment, unlike the optimized colon.

In `.ONESHELL`, an empty shell remains an empty executable name, and the Linux
PATH search reports permission denied when it encounters directory entries.
Its internal recipe prefixes remain intact because an empty name does not
identify a Bourne-compatible shell. Dry-run output preserves that transformed
script even though no launch occurs. Execution modifiers, ignored failures,
keep-going and touch/query modes retain their usual control-flow decisions.

## Follow-up coverage and open boundaries

[Shell argv and residual execution](shell-argv-replay.md) closes the two literal
configuration-LF cases, complex quoted one-shell flags and quoted-LF residual
execution. Ordinary expanded multiline recipes also retain their successive
command boundaries, including empty configuration and assignment cases. The
whole-word behavior above does not imply that a complete expanded multiline
recipe is always one executable or assignment.

There are no pending exemptions in the two curated shell families. Broader
language and interface completeness remains open; curated coverage alone is
not an exhaustive compatibility proof.

Exploratory complex-flag inputs also exposed two GNU allocator aborts, and one
background-process observation was unsuitable as a stable comparison. Their
original inputs and outcomes are retained as research evidence; they are not
counted as compatibility passes or imitated by deliberately crashing rmake.
The two minimal abort reproductions were rechecked and are retained in
[the oracle research record](../tests/research/gnu-441-oneshell-flags-aborts.json),
including exact input/output bytes and reference identities.

## Reference provenance

Empty-shell fixtures and the first implementation preceded source inspection.
Original executable probes then exposed the assignment boundary, and a reference
review confirmed its cause before the final correction. GNU Make 4.4.1's
[official release archive](https://ftp.gnu.org/gnu/make/make-4.4.1.tar.gz) was
consulted on 2026-10-18; archive SHA256:
`dd16fb1d67bfab79a72f5e8390735c49e3e8e70b4945a15ab1f81ddb78658fb3`.
The inspected `src/job.c` sections were `construct_command_argv_internal`'s
lexical equals-sign handling and slow-path configuration construction:
lines 2990–3068, 3262–3295, 3463–3546 and 3590–3608, plus its GPL notice.
File SHA256:
`c1c32d4bd7fd56d898e952ed094a488edf2d4333a02058a3302d28f5e3f8ca09`.

The upstream file is copyright 1988–2023 Free Software Foundation, Inc., under
GPL-3.0-or-later. The Rust implementation and fixtures are original; no upstream
implementation or test text was copied or translated. Reference files remain
outside this repository. Executable comparisons, including file effects and
diagnostics, validate the resulting behavior independently of that explanation.
