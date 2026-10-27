# Export evaluation order

GNU Make 4.4.1 expands exported variables by environment-table traversal, rather
than alphabetical or declaration order. Original three-variable examples observe
`B, A, C` and `Z, Y, X`. Warnings, errors, file writes, shell capture and later
`eval` changes make this order part of observable execution.

The core now retains an independent owned name index beside its binding lookup.
Lookup and ordinary definition iteration keep their existing behavior. Global,
file and pattern scopes have distinct table sizes and histories. Environment
construction traverses specific scopes first, preserves non-exported scoped
names as shadowing entries, and admits only eligible global exports. Inherited
private bindings are omitted according to the existing scope rules. File-local
automatic definitions are replayed onto the corresponding explicit file table
for ordering without flattening or changing variable lookup precedence.

The index models length-aware byte fingerprints, collision chains, tombstones,
growth and same-size rebuilding. It shares immutable name storage with bindings.
Definitions retain cheap insertion/deletion events; slot realization waits until
environment construction needs it. Later mutations update a realized index, and
scope copies preserve independent histories. Empty global/file/pattern binding
maps share immutable storage and use copy on write. No value expansion or effect
is cached or skipped by these changes.

Recipe and shell-function environments use the same ordering model. Their
selected-name table is fixed before expansion, while each subsequent binding
uses current value, origin, definition context and export eligibility. Thus an
earlier export can change or unexport a later one through `eval`. A runtime
export directive selects a visible target/pattern binding when present, without
changing an unrelated global binding's policy.

## Required evidence

Two historical pending export-order fixtures are now required. Add **263 original
required Makefiles** covering declaration reversal, byte-length boundaries,
colliding/wrapped buckets, growth thresholds, deletion/reinsertion, exported and
unexported names, file/pattern/ancestor/private scopes, export-all, nested shell
capture, live eval, fatal expansion, command variables, inherited values and
output-sync modes. Twenty-four of these compare enabled and disabled built-in
rule/variable profiles. Each new fixture was compared with two identical-path
GNU runs and the candidate before its exact stream/status expectations were
saved. Filesystem observations remain exact.

Fixture children inspect only authored variable names, in a fixed key order,
and print the complete side-effect journal bytes. That checks presence, values
and effect sequencing without making unrelated process-environment contents a
test input. The native environment-vector byte order is a separate remaining
M6 surface; this change does not replace Rust's process-environment serializer.

The diagnostic matrix adds twelve independently authored global/file/pattern
info/warning sequence anchors. It now has **218 cells in 15 families** and rejects
reordered output even when two engines agree. Four new Rust tests protect
observed orders, delayed realization across growth/deletion/snapshots, scoped
policy mutation and reservation lifetime. Tooling protects the original and
growth/live-eval reproducers from becoming pending exemptions.

Current inventory: **5,689 curated cases / 5,683 required / six pending**,
**421 Rust tests / 326 tooling tests**. M3 remains open for other language/effect
gaps; this is not full GNU environment or language compatibility. Both development
and optimized combined gates remain mandatory. The CI time ceiling is sixty
minutes to provide headroom above the preceding full PR's roughly forty-minute
run; no checks were removed.

## Metadata boundary

The runtime reserves the known GNU startup key occupancy used by this profile,
including the exported `MFLAGS` slot, separately from unfinished metadata values.
Reservations do not define variables, advertise features or supply an invented
child value. Removing a reservation or subsequently defining a real binding
preserves its ordering lifetime. Complete `MFLAGS`, `.VARIABLES`, `.FEATURES`,
other special-variable metadata and their full startup/replay interfaces remain
M6 work. Absent `GNUMAKEFLAGS` now stays absent during post-read replay rather
than gaining a synthetic binding.

## Performance

An initial eager index increased the selected expansion median about 15% against
preceding main. Delayed realization, shared name keys, single-search insertion
and shared empty maps removed that regression. A final quiescent Linux run used
frozen optimized binaries, two warmups and 101 alternating samples per engine,
checking exact output and successful status on every invocation:

| Workload | GNU median ms | Current rmake ms | Preceding main ms |
| --- | ---: | ---: | ---: |
| Tiny no-op | 0.662 | 0.770 | 0.770 |
| 10,000-rule no-op | 11.380 | 30.689 | 30.540 |
| 5,000 expansions | 2.823 | 4.669 | 4.659 |
| Parallel branches, depth 1 | 88.094 | 88.979 | 88.843 |
| Parallel branches, depth 4 | 88.217 | 89.546 | 89.096 |

The selected medians remain close to preceding main. There is no CPU isolation,
confidence interval or general speed claim. GNU remains faster on these inputs.
Raw samples, source hashes, arguments and executable identities are retained in
ignored local `artifacts/bench/export-order-final-core.json` and
`artifacts/bench/export-order-final-parallel.json`. Reproduce through the top-level
Makefile, then repeat with `BENCH_SUITE=parallel-branches`:

```sh
make bench-compare BENCH_SUITE=core BENCH_REPEAT=101 \
    BENCH_CANDIDATE=/path/to/current BENCH_BASELINE=/path/to/previous
```
Persistent source/graph caches remain behind M8's separate validation gates.

## Provenance

Consulted `src/variable.c`, `src/hash.c`, `src/hash.h`, `src/read.c` and
`src/main.c` in the verified GNU Make 4.4.1 archive, SHA-256
`dd16fb1d67bfab79a72f5e8390735c49e3e8e70b4945a15ab1f81ddb78658fb3`.
The Rust data model, indexed lane permutation, lazy history, fixtures, children
and matrix anchors were independently authored. No GNU implementation or
upstream test code was copied or translated.
