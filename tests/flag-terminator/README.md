# End of options in inherited flags

The startup cases cover option-looking literal goals and command-line variable
names from `MAKEFLAGS` and `GNUMAKEFLAGS`, separate argv sources, raw bytes and
a second literal `--`. The reader cases distinguish live and passive sources
from startup: non-assignments no longer add goals, while definitions and actual
options still take effect. An unconsumed `--` affects only its own source; an
option that takes an operand can consume it as an ordinary value. The project
also tests physical include lookup, contextual HOME lookup, one recursive
submake, and one actual image restart.

`make test-flag-terminator` compares both verified GNU runs and rmake under the
same path, with exact streams, status and all owned filesystem effects. Models
also check requested goals, binding values, command execution or dry-run, and
selected Makefile effects. Use `FLAG_TERMINATOR_CASE=inherited-short-goal` to
replay one case.

GNU Make 4.4.1 `src/main.c` (`decode_env_switches`, `decode_switches` and
`handle_non_switch_argument`) was inspected to understand startup origins and
separate getopt cursors. The implementation and fixtures are original.
