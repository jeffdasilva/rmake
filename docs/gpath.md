# GPATH permanent filenames

A file found through selective `vpath` or global `VPATH` keeps its searched name
when the selected directory is also in `GPATH`. This changes rule selection,
secondary expansion, scope activation, automatic variables and rebuild placement.
The engine prepares that identity before those operations. It consumes the same
metadata observation afterward, preserving freshness decisions made before
expansion effects. An existing local file wins; GPATH itself does not search.

The global policy is expanded once after each completed Makefile read. Target
bindings and later recipe-time `eval` changes do not reconfigure it. Makefile
restarts clear aliases and observations and read the policy again. Makefile
records, including failed optional includes and startup preloads, bypass VPATH
and GPATH search. Their prerequisites still follow the search policy; generated
includes rebuild locally and trigger rereading. Required/optional include,
implicit include-recipe, ordinary VPATH and restart-policy fixtures cover this
boundary. Directory
matching uses lexical bytes, including relative components and exactly one
removed trailing slash, rather than canonical filesystem paths. GNU 4.4.1 also
has a repeated-trailing-separator search quirk that can replace the first
basename byte; the candidate constructs and checks the same path. Original
controls exercise both missing candidates and an existing alternate filename.

## File records and parallel jobs

File-record aliases are separate from tentative VPATH names. A subsequent
[declared-record slice](vpath-file-records.md) also permits ordinary VPATH searches
to merge into existing filename records. GPATH additionally keeps new identities.
Renaming moves a
rule's dependencies, recipe metadata, explicit filename role and variable scope.
Merging into an existing record retains its recipe and bindings, appends source
prerequisites and fills missing bindings. Recipe conflicts produce GNU's three
warnings; incompatible single/double-colon records fail after those warnings.
Static stems, grouped peers, archive container/member identities and special
flags have dedicated fixtures. Canonical names reach duplicate prerequisite
projection, cycle detection, goals and automatic values.

An already visited destination can be evaluated again when a newly visited
command-line goal merges into it. Prerequisite/peer merges occur before the
already-considered check and reuse completed destinations, as clarified by the
[declared-record follow-up](vpath-file-records.md). GNU can even start another recipe while the
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

The slice adds 118 independently authored required fixtures, under
`resolution.gpath`; the preceding main release (`67eae12`) fails 96 and passes
22. Coverage includes searched/no-search controls, lexical paths, policy effects,
CLI modes, explicit/static/implicit/deferred rules, scopes, recipe merges,
archives, grouped outputs, aliases, cycles, diagnostics and parallel execution.
No GNU implementation or tests were copied.

Four bounded parallel cases use file barriers to force overlapping dispatch,
independent continuations, ignored failures and failure cleanup. Each passed
12 repeated oracle self-comparisons and 12 repeated candidate comparisons locally.
They also run in `make test-graph-progress`, in CI, with `PROGRESS_REPEAT=40`
by default. The eight-step `gpath-rebuild-location` stateful scenario verifies
exact bytes and timestamps across rebuilds, query modes, removal, local precedence
and clearing GPATH. The corpus has 3,116 cases: 3,104 required and 12 pre-existing
pending mismatches. Stateful coverage is 52 scenarios / 148 invocations.

These counts describe current tests, not the full GNU specification. M4 remains
open for broader file-record and directory-cache interactions, alias effects on
other language metadata, descendant scope snapshots after late merges, more
Makefile-remake/intermediate-lifecycle combinations, and the archive format/fault
matrix. The
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
| 1,000 explicit searched files | 6.157 | 4.906 | 3.242 |
| 1,000 original/canonical alias pairs | 6.921 | 6.926 | 3.540 |
| 400 implicit searched files | 3.626 | 3.398 | 2.430 |
| Startup | 0.723 | 0.729 | 0.653 |
| 10,000 ordinary no-op targets | 30.939 | 31.662 | 12.007 |
| 5,000 expansion assignments | 4.832 | 4.795 | 2.911 |
| Parallel branches, depth 1 | 89.424 | 89.412 | 88.461 |
| Parallel branches, depth 4 | 90.469 | 89.825 | 88.276 |

Core and GPATH runs use 101 samples per engine; parallel controls use 31. The
new GPATH behavior costs up to about 26% on these selected workloads; ordinary
core and parallel differences are small. GNU is faster in this run. These
measurements establish neither a general performance improvement nor equivalence.
They include startup, two warmups per engine and warm OS caches, with alternating
engine order and retained raw samples and executable hashes. There is no CPU
isolation, confidence interval, CPU/RSS/I/O profile or performance acceptance gate.
Ignored reports: `artifacts/bench/gpath-101.json`, `gpath-core-101.json` and
`gpath-parallel-31.json` in that directory.

The measured current release SHA256 is
`2d8ee0fc457659730c23c3bbda7e47615923bb595fec76d32db0767730f241f1`;
the preceding release SHA256 is
`0ca32f8eb8264297145ea2e0204b0f67a1de8fd2c01c4256eed0551529995160`.

The table above records the introduction build. A subsequent
[filename storage cleanup](gpath-name-storage.md) removes duplicate permanent-name
entries and adds a mixed permanent/tentative-path benchmark control.
