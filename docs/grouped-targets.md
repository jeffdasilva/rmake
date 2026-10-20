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

Independent overlapping `&::` declarations are not implemented in this slice.
Archives, implicit secondary expansion, signal handling, load limits and output
synchronization also remain outside completed compatibility. Required fixture
counts are a regression contract, not proof of full GNU behavior.

The implementation and fixtures are original work. The pinned GPL GNU Make
4.4.1 `read.c`, `remake.c`, `implicit.c`, `rule.c` and manual were consulted as references;
no upstream code or test files were copied.
