# Command-line binding replay

GNU Make 4.4.1 captures command-line variable bindings at startup and reapplies
their encoded values after reading the Makefiles. This is separate from initial
assignment evaluation and from transport to recursive make children.

rmake records unique expanded command-line names in first-declaration order.
After startup assignments it serializes the winning bindings in reverse order,
including environment bindings retained by `?=`. Simple values use `:=`; recursive
values use `=`. Dollars double and blanks/backslashes are quoted. The automatic,
simple `-*-command-variables-*-` binding holds the snapshot, and the default
recursive `MAKEOVERRIDES` binding references it. Stronger existing definitions
retain ordinary precedence. Snapshot construction respects the expansion limit.

The nonempty startup `MAKEOVERRIDES` binding activates post-read replay. Clearing
it in a Makefile suppresses replay; changing it can supply different assignments.
A binding added only in a Makefile cannot activate a previously inactive replay.
The replay runs after all input files and before default-goal selection.

The encoded text is expanded once, then split with backslash unquoting. Leading
C whitespace is skipped; interior separators are space and tab. Quotes are
literal bytes. Nonassignment words are ignored rather than becoming goals.
Assignments use environment origin, or environment-override origin under `-e`,
and existing assignment precedence and binding export policy. This restores an
undefined command-line binding, but does not generally replace a file binding.
New valid environment-origin names become eligible for export.

The outer replay expansion has invocation-level diagnostics. Nested name/value
expansion and validation keep their definition locations. Output and filesystem
effects use the normal expansion interfaces, without an extra expansion pass.

Thirty original fixtures and five promoted cases cover metadata, snapshot order,
conditional imports, undefine, precedence, export lifetime, quoting, diagnostic
locations and phase ordering. A separate 480-program comparison crosses five
assignment operators, four values, imported bindings, `-e` and six Makefile
mutations; all stream/status comparisons matched the pinned oracle.

## Remaining scope

This implements binding replay, not general `MAKEFLAGS` compatibility. The visible
`MAKEFLAGS` template, option mutation, inherited flag parsing, recursive command
transport, startup hooks and shell assignments still need work. Existing guards
and pending fixtures retain those boundaries. This phase must remain distinct
from the startup numeric `MAKELEVEL` snapshot.

GNU Make 4.4.1 `src/main.c` (`main`, command-variable snapshot construction,
post-read `decode_env_switches`, `handle_non_switch_argument`, `quote_for_env`)
and `src/variable.c` (`define_variable_in_set`, `set_special_var`) were consulted
for phases, encoding and precedence. The Rust implementation and fixtures are
independently authored; no upstream source text or tests were copied.
