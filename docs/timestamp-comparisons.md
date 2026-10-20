# Low-resolution target timestamps

`.LOW_RESOLUTION_TIME: target` compares the target against prerequisites using
the final nanosecond of its existing timestamp's second. A prerequisite one
nanosecond newer within that second does not force a rebuild; a prerequisite at
the next second does. A nonzero fractional target timestamp emits GNU's warning
when the target is visited. Warnings precede secondary expansion and appear per
visited double-colon branch, including up-to-date branches.

The comparison timestamp is separate from actual filesystem metadata. Parent
targets still see a prerequisite's actual timestamp. No-op builds preserve the
original mtime; error cleanup compares changed outputs with their actual prior
times. The special target uses literal file names: an empty list does not apply
to every file, and a pattern name does not automatically mark inferred files.
Grouped rules retain GNU's raw peer timestamp comparisons, including the
triggering target's appearance in explicit peer lists.

Thirty-three original required differential fixtures cover whole/fractional
seconds, the final nanosecond, the next second, missing/phony/no-command files,
literal pattern flags, order-only prerequisites, modes, implicit rules, grouped
rules, double-colon warnings, secondary warning/error order and cleanup. Four
stateful scenarios add ten invocations with controlled nanosecond timestamps.
They assert unchanged metadata, exact rebuilt timestamps and preservation of
unchanged fractional outputs after errors. All run through the normal Makefile
and GitHub CI gates.

General clock-skew/out-of-range diagnostics and archive-member timestamp rules
remain unfinished. This slice does not make those broader compatibility claims.
The implementation and Makefiles are original work, with the pinned GNU Make
4.4.1 manual and `remake.c` consulted as behavioral references. No upstream code
or tests were copied. No persistent metadata cache is introduced.
