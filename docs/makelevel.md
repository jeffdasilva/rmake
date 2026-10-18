# Invocation depth and MAKELEVEL

GNU Make 4.4.1 keeps invocation depth separately from the visible `MAKELEVEL`
variable. This slice follows original executable probes on the pinned Linux
x86-64 profile; no GNU implementation or upstream tests were consulted for it.
The public [recursive variable documentation](https://www.gnu.org/software/make/manual/html_node/Variables_002fRecursion.html)
describes the role of the variable. The conversion and timing details below are
observed compatibility behavior, not a recommended numeric parsing convention.

## Startup snapshot

Environment import and command-line assignments precede the depth snapshot.
Read the winning binding's stored bytes at that point. A simple command-line
assignment has already expanded its value, while a recursive assignment has not.
For example, `MAKELEVEL:=$(intcmp 3,3)` gives depth three; the recursive `=` form
has depth zero even though a later ordinary variable reference expands to three.

The observed conversion is:

- Missing or empty values produce zero. A literal first byte `-` also produces
  zero, before whitespace processing.
- Otherwise skip C ASCII whitespace, including vertical tab, accept one sign,
  and consume decimal digits. Trailing non-digits are ignored.
- The magnitude saturates at the unsigned 64-bit maximum on overflow. A negative
  in-range magnitude is negated modulo 2^64; overflow retains the maximum for
  either sign. Invocation depth is the low 32 bits of that result.

Thus `-7` and ` -7` are distinct: the first is zero, the second 4294967289.
`7tail` is seven, 4294967296 is zero, and 18446744073709551616 becomes
4294967295. Child depth uses wrapping 32-bit addition, so its successor is zero.

After the snapshot, normalization installs canonical decimal `MAKELEVEL` as a
simple environment-origin binding, using environment-override origin under `-e`.
Normal precedence retains stronger command-line bindings. With no incoming value,
the synthesized binding is `0`. This stage precedes Makefile evaluation; later
file assignments, overrides, undefines and exports cannot change the snapshot.

## Child environments and diagnostics

The ordinary export pass still runs before setting a child's numeric `MAKELEVEL`.
An incoming environment binding retains export membership, even if a file later
changes its value. An explicit export or export-all can likewise select it, so a
stored expansion error may fail before the numeric child override occurs.
Synthesizing the startup binding alone does not add inherited export membership.

`unexport MAKELEVEL` suppresses that ordinary expansion. The child still receives
the special numeric depth increment. Recomputing depth from the current variable
would change behavior and can introduce forbidden expansion effects.

Runtime diagnostics and status messages use `make[N]` when invocation depth is
nonzero, preserving the actual invocation name. File/line diagnostics retain
their source location instead. Command-line immediate expansion errors occur
before depth initialization and use the plain invocation name. The executable
path stored in `MAKE` and `MAKE_COMMAND` does not acquire a diagnostic suffix.

## Remaining scope

GNU also reapplies command-line bindings after parsing Makefiles. Existing rmake
does not implement that general phase. Original pending cases preserve its
effects on both `MAKELEVEL` and ordinary variables, including `override undefine`
and a conditional command-line assignment that loses to an imported recursive
value. Parse-time normalization and recipe-time replay must remain distinct.
Another pending generic assignment case preserves a leading-vertical-tab RHS
lexing difference. That is separate from the startup numeric converter, which
handles vertical tab correctly in raw imported values.

This slice does not complete recursive make option propagation, directory
messages, jobserver cooperation or all startup diagnostics. It establishes the
depth snapshot, ordinary export timing and the tested runtime message prefixes
without claiming full recursive-build compatibility.
