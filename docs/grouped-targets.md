# Grouped output rules

Explicit `&:` rules and implicit multi-pattern rules now share completion across
outputs. The triggering target supplies automatic variables and target context.
The scheduler visits its prerequisites and the peer prerequisites in GNU's
observable order, expanding secondary prerequisites as each phase is reached.
Peer-only prerequisites affect readiness and staleness without entering the
triggering target's automatic lists. `.WAIT` and named `.NOTPARALLEL` retain their
local prerequisite boundaries.

A running recipe reserves its peers until the full command continuation finishes.
Final status and fresh output timestamps are then published together. Failure
cleanup checks each member's previous timestamp, phony/precious flags and actual
file type. Ordinary touch mode updates requested files individually. Recipe
selection, including dry runs and recipes that expand to no commands, still
shares status where GNU does. Implicit peer names use the full selected stem,
including directories, and missing-peer warnings depend on an actual advance in
the triggering output's timestamp. Explicit recipe overrides retain GNU 4.4.1's
observed peer state, even where the manual's wording suggests a simpler model.

Eighty-three original required fixtures exercise outputs, missing recipes, empty
commands, three-member groups, peer prerequisites and ordering, phony outputs,
secondary expansion, contexts, overrides, implicit replacement priority, duplicate names, cycles, errors,
precious flags, modes, restarts, custom recipe prefixes, eval, directory stems,
terminal implicit rules and parallel continuations. Marker handshakes prove
parallel progress; guarded assertions check prerequisite ordering. The existing
shared-intermediate probe is now required. Eight new stateful scenarios compare
17 invocations, including one-nanosecond staleness boundaries, deleted peers,
per-invocation completion, touch behavior and old-timestamp error cleanup.

All these gates run through the existing Makefile and GitHub CI. Assertions use
the pinned GNU Make 4.4.1 oracle with exact status, byte streams and filesystem
observations; stateful tests also compare timestamp relations. No output
normalization or GNU execution backend is used. No persistent cache is added.

Independent overlapping `&::` declarations now have a separate completion
identity shared by the per-target branches of each declaration. A running
declaration reserves that identity; after prerequisites finish, suspended peers
check ownership again before selecting commands. Completion of one group does
not finish the other groups of a shared target. Initial double-colon timestamps
remain available to later branches even after an earlier group creates a peer.
Secondary expansion is retained in the appropriate declaration rather than
replacing the file's whole branch list. Ordinary touch mode still visits each
requested branch separately. Cycles preserve the different serial and parallel
GNU outcomes. Named `.NOTPARALLEL` follows GNU 4.4.1's behavior on these branches;
use explicit `.WAIT` barriers to order grouped double-colon prerequisites.

Forty additional required fixtures cover overlapping/repeated declarations,
three groups, command-line goals, shared contexts, prerequisite and expansion
order, mixed ordinary double-colon declarations, cached missing timestamps,
errors, precious flags, duplicates, modes, cycles, restarts and parallel
continuations. Three additional stateful scenarios compare six invocations.
The combined corpus has 2,368 cases, 2,356 required and 12 pending; the stateful
gate covers 40 scenarios / 96 invocations.

Archives, implicit secondary expansion, signal handling, load limits and output
synchronization remain outside completed compatibility. Required fixture
counts are a regression contract, not proof of full GNU behavior.

The implementation and fixtures are original work. The pinned GPL GNU Make
4.4.1 `read.c`, `remake.c`, `implicit.c`, `rule.c` and manual were consulted as references;
no upstream code or test files were copied.

## Repeated names inside a declaration

Repeated names in `::` and `&::` declarations create distinct per-file branches,
with independent prerequisite lists and secondary-expansion state. A grouped
declaration still shares one completion identity. Each peer cursor carries its
occurrence ordinal, so expanding or dropping a dependency in one branch does
not overwrite another branch of the same file. Ordinary double-colon commands
run for each eligible occurrence; grouped commands retain their shared
completion behavior. Single-colon duplicate prerequisites and warnings remain
as observed in GNU.

Thirty-five additional required fixtures check these distinctions through
automatic variables, repeated prerequisites, order-only inputs, secondary
expansion, overlapping groups, cycles, errors, modes and parallel continuations.
The same corpus checks that colons in references surviving immediate expansion
are visible to GNU's static-pattern separator scan, including its diagnostics.
Backslash parity quotes that separator; references evaluated immediately retain
their existing expansion behavior. The pinned `read.c` was consulted as a
behavioral reference; no upstream code or fixtures were copied.

The current corpus has 2,436 cases: 2,424 required and 12 pending. These counts
remain a regression contract rather than a complete compatibility inventory.
