# Stdout write, flush, and close errors

The fourteen original Linux cases run with a capped inherited regular
file. They compare raw streams, status, complete filesystem state, and
process containment with two GNU Make 4.4.1 executions.

The cases distinguish parent output from a child-only partial write.
Output synchronization can fail during a replay write or its subsequent
flush. Ordinary exit observes the stdout stream error and returns status
1. A failing close reports the errno as well. The help and version cases
use a zero-byte cap, so branding does not enter the comparison.

The original C extension checks that both its own write and a Makefile
info message set the shared stdout error indicator. Its exit callback
sees the live stream before close, and its ELF destructor sees the
descriptor closed. Separate stderr cases preserve normal exit status.
