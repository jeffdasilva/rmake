# Ordinary shell configuration boundaries

GNU Make 4.4.1 treats expanded `SHELL` text differently from expanded
`.SHELLFLAGS` when preparing ordinary recipes. Original executable probes and
argv-reporting scripts establish the behavior below. No GNU implementation
source or upstream tests were consulted for this slice.

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

## Open boundaries

Literal LF within configuration has additional GNU parsing rules, and empty
`SHELL` differs between ordinary and one-shell execution. Four original pending
fixtures preserve those observed mismatches. Complex one-shell flag handling
also remains incomplete. These are open compatibility requirements, not scope
exceptions.

Exploratory complex-flag inputs also exposed two GNU allocator aborts, and one
background-process observation was unsuitable as a stable comparison. Their
original inputs and outcomes are retained as research evidence; they are not
counted as compatibility passes or imitated by deliberately crashing rmake.
The two minimal abort reproductions were rechecked and are retained in
[the oracle research record](../tests/research/gnu-441-oneshell-flags-aborts.json),
including exact input/output bytes and reference identities.
