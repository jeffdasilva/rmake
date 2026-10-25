# Lazy grouped and double-colon rule records

Intermediate inspection determines whether an existing parent needs rebuilding;
it does not run intermediate recipes. Required middles materialize after the
parent's prerequisite inspection phases, including grouped peer phases. This
preserves effect order even when the parent is missing or `-B` is active.

For a grouped intermediate, only the triggering file's prerequisites expand
during inspection. Peer expansion, peer-only inputs and command selection happen
when materialization is needed. A newer peer-only input or an error in an unused
peer clause does not force an otherwise current parent to rebuild.

Double-colon inspection initially uses the head file record, including its
secondary expansion. A newer input in a later branch does not itself trigger
materialization. When the head requires rebuilding, normal traversal visits the
branches separately using the original timestamp. The expanded head is retained
without replacing sibling records or replaying its effects. Inspection clones
only that selected record. Query mode stops at the first outdated branch unless
keep-going requests further traversal.

## Freshness, cycles and file lifetimes

The need to rebuild a parent is separate from the intermediate's final timestamp.
A later double-colon branch can deliberately restore an older date; that does
not undo the decision that the parent needs rebuilding. A sparse per-frame edge
set retains this evidence across waits. GNU's serial path includes that change
in `$?`; parallel completion uses the final timestamp for that automatic list.
Both forms have strict fixtures, including duplicate normal/order-only roles.
The completed double-colon outcome observes the actual final file timestamp.

Inspection diagnoses and removes circular edges from the inspected record.
Their removal survives later materialization and preserves other branches.

Global `.SECONDARY` marks records present at the dependency snapshot, not every
filename later produced by a deferred clause. A required late leaf still raises
a missing-rule error. The policy keeps this inventory only when global secondary
is active, and carries its identity through new GPATH renames. This stores parsed
file-role facts; filesystem freshness, effects and expansion results are not
cached across invocations.

Rule definitions are frozen after initial reading and policy validation, before
VPATH/GPATH setup expansion. Variable assignments remain allowed. A fresh database
on Makefile restart restores the reader phase. Eval syntax/phase errors without
a caller retain their enclosing binding's diagnostic location, including a
program-only location for environment values.

## Validation and remaining M4 work

The slice adds 143 original required fixtures under `rules.lazy-records`.
The preceding `854d697` release fails 101 and passes 42. Coverage includes grouped,
ordinary/grouped double-colon and duplicate records, lazy/required expansion,
peer-only inputs, late errors, query/keep-going/dry/touch/always/parallel modes,
scopes, cycles, timestamp restoration, `$?`, and early/late filename records.

Two original stateful scenarios add nineteen invocations. The grouped scenario
covers peer-only changes, no-op/query runs, materialization, removal and missing
late leaves. The double-colon scenario checks head-only inspection, ignored later
inputs, query stopping, existing and missing middle timestamps, restored dates,
removal and recovery. Both independently anchor exact bytes and timestamps.

Four parallel freshness cases passed 24 repeated oracle self-comparisons and
24 candidate comparisons locally. They join `make test-graph-progress` and CI,
with 40 repetitions by default. The corpus contains 3,442 cases: 3,430 required
candidate matches and 12 existing pending mismatches. Stateful coverage is
56 scenarios / 186 invocations. The implementation and fixtures are original;
no upstream code or tests were copied.

This closes the deferred grouped/double-colon inspection gap recorded by the
[earlier intermediate-policy slice](intermediate-policy.md). M4 remains open for
wider file-record aliases and metadata, directory-cache interactions, descendant
scope refresh after late merges, further lifecycle/observation combinations and
the archive format/fault matrix. Required runtime and compatibility profiles
still have open gates. Passing these cases is not an exhaustive compatibility
audit. Persistent graph caching remains deferred until its validation and
invalidation design is proved.

## Measurements

`make bench-compare BENCH_SUITE=lazy-records BENCH_REPEAT=101` prepares 1,000
physical current outputs and absent grouped peers/middles outside timing. Empty
recipes launch no children. The same streams/status converge on the previous
build even where it performs too much grouped work or omits required head-record
inspection. These timings do not establish that the baseline was compatible.

A local 101-sample paired comparison against `854d697` measured:

| Workload | Current rmake (ms) | Previous rmake (ms) | GNU 4.4.1 (ms) |
| --- | ---: | ---: | ---: |
| 1,000 grouped middles | 12.248 | 17.556 | 5.788 |
| 1,000 double-colon middles | 12.644 | 10.328 | 6.061 |
| Startup | 0.747 | 0.713 | 0.668 |
| 10,000 ordinary no-op targets | 31.082 | 30.969 | 11.869 |
| 5,000 expansion assignments | 4.817 | 4.946 | 2.941 |

Grouped inspection uses about 30% less elapsed time. Double-colon inspection
costs about 22% more while performing head expansion and prerequisite work the
old implementation omitted. Core differences are small in this run. Current
release SHA256:
`fb8de3ae655bf2250f031f66ffd9703a469d6dcd048bf4d814a0716c28e41b09`. Previous release SHA256:
`73169175d4ea74fa042bc316ff06a604a98192b4596440b8214e796fbf7e6ef5`.


Reports with source hashes, executable identities and all samples are ignored
`artifacts/bench/lazy-records-101.json` and `artifacts/bench/lazy-records-core-101.json`.
They include startup, two warmups per engine, warm OS caches and alternating
engine order with exact output/status checks. There is no CPU isolation,
confidence interval, resource profile or performance acceptance threshold.
GNU remains faster in these workloads; these mixed results do not establish a
general speed advantage.

A subsequent [declared VPATH record slice](vpath-file-records.md) covers missing
searched records, ordinary path merges and goal/prerequisite merge phases.
The wider M4 and profile gates remain open.
