# Capture terminal output after process completion

The inherited-PTY collector could receive an empty readiness result, then see
that the child had exited and stop reading. Bytes written between those two
checks remained unread. Because GNU and rmake share the same PTY for an exact
comparison, GNU's tail could then appear at the start of the candidate capture.
A separate stderr pipe could also lose its final bytes when closed.

After waiting for completion and performing the existing group cleanup, drain
each stream nonblockingly before closing a pipe or reusing the PTY. A retained
slave descriptor means PTY EOF cannot be used as the completion condition. Keep
all captured bytes in the observation that produced them. Expected streams,
terminal spellings, CRLF bytes, status comparisons and deadlines are unchanged.

The [original failure record](evidence/terminal-capture-race.json) preserves the
old runner identity and raw observations: GNU had only reader messages; the
candidate began with the omitted GNU helper tail before its own reader/helper
output. This is a collector defect, not passing engine conformance evidence.

Two deterministic regressions force completion immediately after an empty
readiness result and reuse the same PTY for consecutive observations. They cover
separate stderr and merged output, fail with the original collector, and pass
with the drain. They use an original small byte producer to test transport
without requiring an installed GNU reference in the tooling unit suite.

With the fix, the affected cell passed 100 repetitions each for GNU self-test,
the preceding rmake and the current rmake; the complete 192-cell terminal matrix
also passed. Full development/optimized acceptance and exact-head CI are rerun.
No timeout, expected stream or product implementation was changed to conceal the
failure. The top-level `test-tools` and `test-runtime-effects` flows include the
regressions and the original terminal matrix.
