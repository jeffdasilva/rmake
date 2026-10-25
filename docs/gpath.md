# GPATH permanent filenames

A file found through selective `vpath` or global `VPATH` keeps its searched name
when the selected directory is also in `GPATH`. This changes rule selection,
secondary expansion, scope activation, automatic variables and rebuild placement.
The engine prepares that identity before those operations. It consumes the same
metadata observation afterward, preserving freshness decisions made before
expansion effects. An existing local file wins; GPATH itself does not search.

The global policy is expanded once after each completed Makefile read. Target
bindings and later recipe-time `eval` changes do not reconfigure it. Makefile
restarts clear aliases and observations and read the policy again. Directory
matching uses lexical bytes, including relative components and exactly one
removed trailing slash, rather than canonical filesystem paths. GNU 4.4.1 also
has a repeated-trailing-separator search quirk that can replace the first
basename byte; the candidate constructs and checks the same path. Original
controls exercise both missing candidates and an existing alternate filename.

## File records and parallel jobs

Permanent aliases are separate from tentative VPATH names. Renaming moves a
rule's dependencies, recipe metadata, explicit filename role and variable scope.
Merging into an existing record retains its recipe and bindings, appends source
prerequisites and fills missing bindings. Recipe conflicts produce GNU's three
warnings; incompatible single/double-colon records fail after those warnings.
Static stems, grouped peers, archive container/member identities and special
flags have dedicated fixtures. Canonical names reach duplicate prerequisite
projection, cycle detection, goals and automatic values.

An already visited destination can be evaluated again when a newly visited
original name merges into it. GNU can even start another recipe while the
first recipe for that resulting name is running. Each worker now has a private
completion identity and continuation. A primary table serves ordinary jobs; a
separate table retains overlapping workers only when needed. Slot accounting,
stdin ownership, jobserver tokens, failure cleanup and shutdown include every
worker. A late scope merge refreshes direct target bindings without repeating
pattern activation effects or changing already expanded command strings.

All observations are invocation-local. There is no persistent dependency graph,
source or archive-member cache in this change. Filesystem invalidation boundaries
continue to govern failed-search observations; alias identity is permanent for
the current Makefile read, as distinct from a reusable freshness observation.

## Validation

The slice adds 111 independently authored required fixtures, under
`resolution.gpath`; the preceding main release (`67eae12`) fails 91 and passes
20. Coverage includes searched/no-search controls, lexical paths, policy effects,
CLI modes, explicit/static/implicit/deferred rules, scopes, recipe merges,
archives, grouped outputs, aliases, cycles, diagnostics and parallel execution.
No GNU implementation or tests were copied.

Four bounded parallel cases use file barriers to force overlapping dispatch,
independent continuations, ignored failures and failure cleanup. Each passed
12 repeated oracle self-comparisons and 12 repeated candidate comparisons locally.
They also run in `make test-graph-progress`, in CI, with `PROGRESS_REPEAT=40`
by default. The eight-step `gpath-rebuild-location` stateful scenario verifies
exact bytes and timestamps across rebuilds, query modes, removal, local precedence
and clearing GPATH. The corpus has 3,109 cases: 3,097 required and 12 pre-existing
pending mismatches. Stateful coverage is 52 scenarios / 148 invocations.

These counts describe current tests, not the full GNU specification. M4 remains
open for broader file-record and directory-cache interactions, alias effects on
other language metadata, descendant scope snapshots after late merges, GPATH
Makefile-remake/restart interactions, and the archive format/fault matrix. The
remaining runtime and optional compatibility profiles also need work. Persistent
graph caching remains deferred until its validation and invalidation proof exists.

## Measurements

`make bench-compare BENCH_SUITE=gpath BENCH_REPEAT=101` measures three original
no-child workloads over physical files prepared outside timing, with fixed equal
historical input/output timestamps. It checks exact output and successful status
for every timed run. Optional `BENCH_BASELINE=/absolute/path/to/previous/rmake`
compares the previous release. These no-op outputs converge even though that
release lacks permanent-name behavior; its timing is not a compatibility result.

A local alternating paired run against `67eae12` measured these medians:

| Workload | Current rmake (ms) | Previous rmake (ms) | GNU 4.4.1 (ms) |
| --- | ---: | ---: | ---: |
| 1,000 explicit searched files | 5.634 | 4.605 | 2.997 |
| 1,000 original/canonical alias pairs | 6.174 | 6.106 | 3.313 |
| 400 implicit searched files | 3.297 | 3.001 | 2.236 |
| Startup | 0.720 | 0.717 | 0.641 |
| 10,000 ordinary no-op targets | 29.980 | 29.897 | 11.924 |
| 5,000 expansion assignments | 4.768 | 4.887 | 2.958 |
| Parallel branches, depth 1 | 89.491 | 89.019 | 87.911 |
| Parallel branches, depth 4 | 88.015 | 87.681 | 86.108 |

Core and GPATH runs use 101 samples per engine; parallel controls use 31. The
new GPATH behavior costs up to about 22% on these selected workloads; ordinary
core and parallel differences are small. GNU is faster in this run. These
measurements establish neither a general performance improvement nor equivalence.
They include startup, two warmups per engine and warm OS caches, with alternating
engine order and retained raw samples and executable hashes. There is no CPU
isolation, confidence interval, CPU/RSS/I/O profile or performance acceptance gate.
Ignored reports: `artifacts/bench/gpath-101.json`, `gpath-core-101.json` and
`gpath-parallel-31.json` in that directory.
