# Failed implicit search invalidation

After a failed search, a later recipe can create its missing prerequisite. GNU
Make 4.4.1 expires directory observations when a command completes or a file
function opens a file for writing. Failed-search observations must expire at the
same boundary; retaining them could incorrectly reject a later valid rule.

The Linux filesystem runtime now exposes the generation of its existing
invalidation boundary. Completed children, including failures, and opened file
writes advance this generation. The engine checks it before reading or inserting
a failed-search observation and clears stale entries. This centralizes the
policy instead of adding invalidation calls to every shell/recipe hook.

Reads, realpath queries, skipped dry-run commands and GNU's internal touch mode and optimized colon commands
retain their previous observations. A no-change command still expires them.
Candidate cancellations already made inside one search remain in force, even if
a later word changes the generation; future searches use the new generation.
Makefile restarts continue to clear the engine's search state.

Fourteen original required fixtures cover these mutations and controls, including
file writes with no child command, failed commands, writes inside candidate words
and search-local cancellations. The dry-run control also exposed and fixes an
unrelated diagnostic: `-kn` suppresses the blocked-goal summary, as GNU does.
The corpus now has 2,998 cases: 2,986 required matches and 12 existing pending
mismatches. Stateful coverage remains 51 scenarios / 140 invocations. No upstream
code or tests were copied. GPATH/permanent renaming and the broader M4 ledger
remain open.

## Measurements

A local 101-sample alternating paired comparison against the preceding implicit
search build (`2714f67`, merged as `3fcf50e`) measured:

| Workload | Current rmake (ms) | Previous rmake (ms) | GNU 4.4.1 (ms) |
| --- | ---: | ---: | ---: |
| 64 rejected rules | 2.975 | 2.998 | 1.218 |
| 256 rejected rules | 14.009 | 13.859 | 3.559 |
| Startup | 0.686 | 0.690 | 0.635 |
| 10,000 no-op targets | 28.440 | 28.371 | 11.322 |
| Expansion | 4.683 | 4.576 | 2.808 |

The generation check preserves the prior rejection improvement with small local
timing differences; these are not claimed as new performance gains. GNU remains
faster. Samples include startup, two warmups per engine and warm OS caches, with
identical paths/arguments/environment and exact output checks. Runs were
sequential outside functional validation, without CPU isolation, confidence
intervals or CPU/RSS/I/O profiling. Raw samples and identities remain in ignored
`artifacts/bench/implicit-invalidation-101.json` and
`artifacts/bench/implicit-invalidation-core-101.json`. There is no persistent
cache or broad performance-parity claim.

Subsequent [GPATH coverage](gpath.md) adds permanent filename renaming and
selected alias/parallel interactions. Earlier corpus counts and open-work lists
above describe this document's original slice; M4 still has broader open gates.
