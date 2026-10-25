# Archive-first VPATH resolution

Archive-member goals search for the archive filename, then inspect the selected
archive for the requested member. Selective patterns match `lib.a`, rather than
`lib.a(one.o)` or `one.o`. An existing local archive stops directory search, even
when it is empty, invalid, lacks the member or has a zero member date. Otherwise,
the first existing archive in selective VPATH/general VPATH order wins; a missing
member does not permit a later archive to replace it.

Resolved member names reach prerequisite automatic variables and direct-goal
messages. Stale recipes retain the original local target name. Goal summaries
also follow GNU's merging behavior when resolution reaches a name already entered
by a declaration or command-line goal. Ordinary-file controls cover that shared
message behavior.

GNU's implicit viability probe uses a different path: it checks the complete
literal prerequisite spelling in VPATH, without performing the archive's own
search. An archive available through VPATH can therefore work as an explicit
prerequisite while failing the corresponding implicit prerequisite probe. The
engine keeps these operations separate.

The runtime also preserves GNU's archive observation lifetime. Reading a VPATH
member leaves the original archive locally absent for a later explicit archive
prerequisite visit. Completion of that searched member does not add a header
validation that GNU does not perform on a newly created local copy. Phony members
skip date validation. These rare observations are stored only for the current
invocation and cleared on Makefile restart; archive contents/member indexes are
not cached. A later invocation checks the local archive afresh, including malformed
numeric fields. Existing local-archive completion checks remain in place.

The slice adds 64 original required fixtures; 42 fail on the preceding build.
They cover archive-first stopping rules, selective/global priority, resolved
names, aliases, local rebuilds, malformed metadata, phony targets, scopes,
secondary expansion, explicit versus implicit search, modes and parallel jobs.
Two new stateful scenarios add eleven invocations covering search priority,
local rebuilds, query mode, corruption detection and return to VPATH after local
removal. Corpus: 2,804 cases, with 2,792 required matches and 12 existing pending
mismatches. Stateful coverage: 50 scenarios / 133 invocations. Full development
gates, optimized conformance and targeted GNU/GNU plus optimized GNU/rmake
stateful checks pass. GNU 4.4.1 source was consulted for behavior; no upstream
implementation or test material was copied.

A 101-sample paired core check against `137372f` measured startup at 0.801 ms
(previous 0.815 ms), 10,000 no-op targets at 28.195 ms (27.974 ms), and expansion
at 4.698 ms (4.629 ms). A 31-sample archive run measured dense 64/256 members at
1.178/4.983 ms (previous 1.154/5.128 ms), and 4 KiB/64 KiB payloads at
1.553/1.607 ms (1.552/1.571 ms). The small differences do not establish a
performance change. GNU remains faster except on the dense 256-member workload.
Runs alternated engine order, checked exact outputs, and included startup with
warm OS caches; no CPU isolation, resource profile or confidence intervals were
used. Raw samples are ignored artifacts `artifacts/bench/archive-vpath-core-101.json`
and `artifacts/bench/archive-vpath-archives-31.json`.

This is a slice of M4, not its exit gate. GPATH/permanent renaming, broader file
alias and directory-cache behavior, implicit secondary expansion, per-clause
static stems and the remaining archive format/fault matrix are still open.
Persistent dependency-graph caching remains deferred until its invalidation
contract can be validated.

Subsequent [GPATH coverage](gpath.md) adds permanent filename renaming and
selected alias/parallel interactions. Earlier corpus counts and open-work lists
above describe this document's original slice; M4 still has broader open gates.
