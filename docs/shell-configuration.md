# Choose a shell for recipes

On Linux, set `SHELL` in a Makefile or on the command line to choose the recipe
interpreter. Without that setting, rmake uses `/bin/sh`. Your `SHELL`
environment variable describes your interactive shell. Set `.SHELLFLAGS` to
pass options to the recipe interpreter. The default is `-c`, or `-ec` when `.POSIX` is active.
Keep `-c` when the shell expects the recipe as a command string. See the
[GNU Make reference for choosing a shell](https://www.gnu.org/software/make/manual/html_node/Choosing-the-Shell.html).

## Run recipes with Bash

This example requires Bash at `/bin/bash`. In an empty directory, save it as
`Makefile`. Start each recipe line with a tab.

```makefile
SHELL := /bin/bash
.SHELLFLAGS := -eu -o pipefail -c
BUILD_SYSTEM := $(shell uname -s)

.PHONY: all check
all:
	@printf 'Building on %s\n' '$(BUILD_SYSTEM)'

check:
	@false | true; printf 'Pipeline succeeded\n'
```

1. Run `rmake`. The `all` recipe prints `Building on Linux`.
2. Run `rmake check`. Bash treats the pipeline as failed because `false`
   failed. The command stops and rmake reports a recipe error.

`$(shell ...)` runs when the expression expands. The sample uses `:=` to run
`uname` once each time rmake reads the Makefile. The `all` recipe runs later,
when rmake builds that target. See the
[GNU Make shell function reference](https://www.gnu.org/software/make/manual/html_node/Shell-Function.html).

## Reproduce a shell difference

From the rmake checkout, run `make bootstrap` and then
`make test-diff test-shell-capture-lifetime`. For a small Makefile, compare both
engines in a clean directory. Keep the exact shell assignments, recipe
prefixes, flags, exit status, and diagnostic output. If the failure needs a
second command or a recursive target, retain it in the reproduction. See
[shell argument fixtures](shell-argv-replay.md) and the
[overall testing guide](testing.md).

## Quoting in `SHELL` and `.SHELLFLAGS`

When preparing an ordinary recipe command, the engine protects shell
metacharacters that come from the expanded `SHELL` value. These include
`# ; " * ? [ ] & | < > ( ) { } $`, backtick, `^`, `~`, and `!`. For
example, `SHELL = ./capture $$NAME` passes the literal argument `$NAME` to
`./capture`. The same protection applies to characters in the executable name.

Protection runs before single-quote processing. A dollar inside single
quotes retains the backslash added during protection. Double quotes from
`SHELL` are literal. Single quotes, backslashes, spaces, and tabs still affect
where arguments split. Some combinations of backslashes and metacharacters
require full shell evaluation.

The engine interprets quoting, expansions, and redirections from
`.SHELLFLAGS` during command construction. Assignments and shell built-ins can
also require a shell. The recipe text stays together as one argument to the
selected interpreter.

## Recipe execution modes

The prepared literal executable path goes through the normal launcher. Missing,
non-executable and directory paths retain their respective diagnostics and GNU's
recipe error status. Executable text without a shebang retains the existing
`ENOEXEC` fallback with its original path and argument vector.

`.ONESHELL` retains a separate rule: the entire expanded `SHELL` value is its
executable path. It does not use ordinary shell-name tokenization. Unforced
dry-run/query/touch modes do not launch a shell; forced recipes exercise the
normal launch boundary. Permanent fixtures cover those distinctions and mixed
protected shell bytes with interpreted flags.

## Warnings during command construction

Recipe bodies expand under the active warning policy. The later shell
configuration starts with undefined-variable warnings disabled and expands
`SHELL`, `.SHELLFLAGS`, then `IFS` in that order. Explicit warnings and errors
still appear at the recipe location. A live `MAKEFLAGS` assignment can enable
undefined-variable warnings within configuration. After configuration, the
next command or target uses the saved policy.

A silent or ignore-errors option enabled by configuration applies to the
current command. Dry-run and trace output retain their ordinary precedence.
Local recipe modifiers and the corresponding special targets still apply.

## Shell functions in one-shell mode

A Bourne shell function call preserves prefix bytes in direct arguments. If
command construction falls back to the shell, it removes blanks and modifiers
at the start of each logical line. This scan follows backslash parity across
newlines without interpreting shell quotes. Custom shells skip the initial
spaces and tabs and retain the remaining command bytes. The focused fixtures
cover both paths, quoting, continued lines, and ordinary recipes.

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

## Verified cases and open behavior

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

## Source references

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

## Continuations that produce no command

After expanding the recipe and shell configuration, the direct lexer can consume
a backslash-newline and its indentation without producing an argument. This
command slot contributes no query result, echo, or process. Ordinary recipes
can still run later slots. A quoted empty argument and a shell command such as
`:` still count as commands, as does continuation-only input routed through a
nondefault shell or flags. Touch mode retains its target update.

## Linux executable search

The child environment determines the executable search path. The launcher
selects the first executable entry that is a non-directory, then attempts the
launch. If that executable names an interpreter that is missing, the launch
fails there. An executable text file with no recognized format instead runs
through `/bin/sh` with its selected path and original arguments. When no
executable qualifies, the last access error other than `ENOENT` determines
the diagnostic. A quoted empty program name has no current-directory match.

When a recipe child exits with status 127, GNU Make checks the executable path
used for that command. A failed stat, a directory, missing owner execute
permission, or an empty executable produces an additional diagnostic before
the recipe error. Serial and parallel recipes retain the path selected at
launch, including paths resolved through the child's search path.
In an ordinary serial recipe, a later missing, inaccessible or
non-directory executable lookup also checks the previously launched path.
Without synchronization, its diagnostic follows the lookup failure and
precedes the recipe error. With recursive output synchronization, it appears
before the pending recipe output. The lookup failure and recipe error keep
their order within that output. Ignoring the recipe error preserves both
executable diagnostics. A launch rejected by the Linux argument or filename
limit uses the same recheck and output order. A local symlink loop also triggers
the recheck in ordinary serial output.
With `-n` and unsynchronized serial output, an unignored missing executable on
a forced direct command triggers the same recheck before the recipe error.

An ordinary parallel recipe continuation keeps the latest successfully
launched path through skipped commands and ignored lookup failures.
With target or recursive output synchronization, the recheck appears before
the recipe buffer. With line synchronization, completed lines have already
been printed when the recheck appears, and the current line's output follows
it.

Shell capture opens its pipe before executable selection. A failed lookup or
launch retains the pipe ends for the interpreter's lifetime. A pipe allocation
failure reports the active source location and preserves the previous
`.SHELLSTATUS`. Under descriptor pressure, the POSIX spawn backend can launch
with the capture pipe already in place, avoiding an additional fork-status
socket. With a startup limit of 12 or 13 descriptors on supported glibc
versions, rmake leaves two descriptors available for shell capture and uses
the existing bounded interruptible wait. The recursive jobserver budget is
independent of this choice. At higher limits, rmake reserves a signal wake
pipe when three private descriptor slots remain available for an open
makefile and its initial capture. This capacity check includes inherited
descriptors and finishes before the signal handlers can use the pipe.

After the first failed capture with two private endpoints, the existing wake
pipe accounts for one retained pair. Its descriptors remain live for signal
notification. Later failed pairs remain open for the interpreter's lifetime.
When a capture occupies a standard descriptor, that slot retains its usual
behavior.

A shell function or shell assignment also skips process creation when the
default command lexer consumes every byte as a continuation, an unquoted
final backslash, or surrounding blanks. Shell configuration still expands
first. Child exports are untouched, and the previous `.SHELLSTATUS` remains
visible. Quoted arguments and nondefault shell configurations retain their
normal launch behavior.

A bare newline from expansion reaches shell command construction. The
default fast path treats it as a literal program name. In `.ONESHELL`
mode a bare newline instead reaches the shell as script text. Both
expand the shell configuration and child exports before the launch.

An ordinary shell capture that uses the slower shell constructor trims
its original leading spaces and tabs. It then folds any unescaped newlines.
If folding consumes the entire command, the interpreter launches the
configured shell with its flag arguments alone. Explicit empty flag words
and command bytes remaining after the first newline are still passed.

Shell capture expands the command argument before expanding `SHELL`,
`.SHELLFLAGS`, and `IFS`, in that order. Configuration expansion suppresses
undefined-variable warnings and restores the caller's policy afterward.
It reads the raw `.SHELLFLAGS` from the active expansion scope and rescans
that value. A local binding from `let` or `foreach` can supply flags to a
nested capture, including when that capture runs while configuring a recipe.
A command that is empty or contains only spaces and tabs returns empty
after configuration. It preserves `.SHELLSTATUS` and skips child exports.

Shell environment preparation uses the original environment value when it
encounters a recursive export of a variable whose named `call` body is
still active. An exported alias can encounter the same fallback through either
a direct reference or a named call. Call arguments still expand first.
