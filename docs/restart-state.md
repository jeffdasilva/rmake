# Imported restart counters and export policy

The GNU process restart number is now separate from the number of rereads performed
by this rmake process. Imported numeric state controls `-B` during Makefile
remaking and the appropriate first-start jobserver decisions. Actual rereads
control in-process reconstruction, initial mutex handling and the development
iteration budget. The GNU number wraps as an unsigned 32-bit value; an imported
number above 128 does not itself consume the reread budget. CLI/file assignments
to the visible variable do not change the counter selected from the original
process environment.

The public imported MAKE_RESTARTS value retains its raw whitespace, signs and
suffixes, apart from one leading directory-trace marker. Numeric conversion uses
the pinned Linux unsigned conversion already used for MAKELEVEL, with MAKELEVEL's
separate first-byte-minus guard preserved. A new restart installs the new decimal
count at environment origin.

Imported and reconstructed MAKE_RESTARTS bindings have a no-export policy. This
is binding metadata: explicit export can enable it; export-all does not override
it; undefine destroys it; a fresh command-line definition without an imported
binding can export normally. Both recipe and shell-function environments retain
this policy, including recursive child startup.

The prepared startup Plan now transfers its owned original argv/environment into
the next engine. It consumes the plan rather than cloning the entire original
input and discarding the first copy. One owned argv copy still supports decoding
and restart reconstruction. Small program/invocation identities are retained
across the consuming restart call. Effects are neither cached nor skipped.

## Evidence and limits

214 original required controls cover 21 numeric input profiles across direct,
effect-prepared and snapshot reconstruction, with serial/pipe/synchronized modes;
export policy lifetime, shell/recipe/recursive environments; and visible-variable
reassignment independent of the process counter. A fixed-nanosecond file producer
makes unsigned wrap with `-B` reproducible across two actual remakes.

Eight additional required controls establish that very early MAKEFLAGS bindings
are inactive special variables. An initial audit assumed their setters would
immediately decode flags. Two GNU observations contradicted that model; source
review confirmed the `v->special` guard and later activation. The implementation
already matches this boundary, so no early reset receiver was added. These
controls protect that behavior rather than claiming a new decoder feature.

All 222 controls have independent exact models and intended file-effect checks,
compared against two pinned GNU observations and the candidate. The earlier
10,488 required corpus passed during development. The corpus becomes 10,710
ordinary required cases, zero pending, with 17 unchanged separate safe-error
exceptions. Negative tooling controls protect wraparound, raw control bytes,
policy lifetime, activation and research classification.

Two finite directory-marker probes are retained as
[ordinary research](../tests/research/imported-restart-directory-markers.json).
GNU emits a final leaving-directory message for an imported traced marker even
with `--no-print-directory`. The counter/value behavior is corrected; complete
directory reporting remains M6 work. Initial incomplete models, two raw GNU runs,
before/after candidate observations, inputs and binary identities are retained.
These are not passing fixtures, pending curated cases or safe-error exceptions.

The initial snapshot-warning models also exposed an unrelated undefined A read;
the refined inputs define A explicitly while retaining the two intended missing
MAKEFLAGS warnings and the snapshot path. Earlier observations were retained
before refinement. No model was automatically rewritten to fit candidate output.

This is not full M3/M6 acceptance. Actual self-exec, complete special activation
and malformed options, directory/debug/database output, old/new-file assumptions,
shuffle, native/Guile, locales and broader profile/integration gates remain in
[the audit](m3-m6-audit.md). The 128-reread development limit remains a separate
experimental resource limit, not an approved exception for finite larger builds.

GNU Make 4.4.1 `src/main.c` environment import/re-exec/remake phases,
`src/misc.c` unsigned conversion and `src/variable.c` special-binding guard were
consulted from the verified source archive, SHA256
`dd16fb1d67bfab79a72f5e8390735c49e3e8e70b4945a15ab1f81ddb78658fb3`.
GNU source is copyright Free Software Foundation, Inc., GPL-3.0-or-later. Rust
implementation, fixtures and tooling are original; no upstream implementation or
tests were copied or translated. No unsafe Rust or GNU execution fallback is added.

## Frozen validation and performance observations

Formatting, warnings-denied lint, 457 Rust and 418 tooling tests passed. All 463
combined inherited/ordered/restart controls match two GNU runs and independent
models under both frozen builds. The 128-cell exit gate also passed under both.
Debug SHA256:
`c11e29abf2da4d3770901af93b09268fcb7733b733b8dc29bac9c5b22a4bf67e`.
Release SHA256:
`58cd282afd36baeb8ecfa52d7157a9166ad85cb5883a60d60a3a766e750cb526`.
Full final rmake-driven development/optimized acceptance and exact-head CI remain
merge gates.

Six reports contain 101 alternating, exact-output-checked samples per engine.
No build or test campaign ran concurrently; host activity was not controlled.
Timings remain mixed: large no-op, eager CLI effects and inherited effects have
medians roughly 2–3 percent higher than preceding rmake in this run. Some other
medians are lower or close. No statistical speedup or general parity is claimed.
GNU remains faster on several common workloads.

| Workload | GNU ms | Current ms | Preceding rmake ms |
| --- | ---: | ---: | ---: |
| tiny-noop | 0.717 | 0.867 | 0.871 |
| large-noop | 11.548 | 30.353 | 29.700 |
| expansion | 2.876 | 5.050 | 5.075 |
| startup-arguments-0 | 0.649 | 0.762 | 0.760 |
| startup-arguments-1000 | 2.201 | 2.100 | 2.074 |
| startup-arguments-5000 | 25.907 | 8.411 | 8.373 |
| parallel-branches-depth-1 | 87.972 | 90.552 | 90.475 |
| parallel-branches-depth-4 | 88.161 | 90.521 | 90.891 |
| eval-arguments-0 | 0.716 | 0.886 | 0.891 |
| eval-arguments-1000 | 3.057 | 1.950 | 1.945 |
| startup-effects-0 | 0.644 | 0.766 | 0.765 |
| startup-effects-1000 | 2.563 | 3.101 | 3.040 |
| initial-flags-0 | 0.587 | 0.685 | 0.695 |
| initial-flags-1000 | 2.537 | 2.902 | 2.828 |

Raw timing reports: [core](benchmarks/restart-state-core.json), [startup](benchmarks/restart-state-startup.json), [parallel](benchmarks/restart-state-parallel.json), [eval](benchmarks/restart-state-eval.json), [effects](benchmarks/restart-state-effects.json), [inherited](benchmarks/restart-state-inherited.json).

The profiler now accepts the required, recipe-free CLI/inherited-effect workloads,
checks their source/argv/environment against the required fixture and validates
all expected output bytes. Two executions per engine/workload recorded identical
counters, using equal-length trace/helper paths:

| glibc request metric | Preceding | Current |
| --- | ---: | ---: |
| effects: calls | 29,771 | 28,747 |
| effects: requested_bytes | 2,111,797 | 2,050,273 |
| inherited: calls | 28,850 | 28,824 |
| inherited: requested_bytes | 2,342,666 | 2,307,072 |

The CLI-effect observation has 1,024 fewer allocation requests (about 3.4 percent)
and 61,524 fewer requested bytes (about 2.9 percent). The inherited-effect
observation has 26 fewer requests and 35,594 fewer requested bytes. This profiler
observes glibc malloc/calloc/realloc/free after initialization, excludes aligned
entry points, and measures requested bytes rather than retained/peak memory.
Sampled stacks are hints, not CPU measurements. These counts do not establish a
wall-clock speedup. Raw allocation observations: [effects/priorxx/0](benchmarks/restart-state-alloc-effects-priorxx-0.json), [effects/priorxx/1](benchmarks/restart-state-alloc-effects-priorxx-1.json), [effects/current/0](benchmarks/restart-state-alloc-effects-current-0.json), [effects/current/1](benchmarks/restart-state-alloc-effects-current-1.json), [inherited/priorxx/0](benchmarks/restart-state-alloc-inherited-priorxx-0.json), [inherited/priorxx/1](benchmarks/restart-state-alloc-inherited-priorxx-1.json), [inherited/current/0](benchmarks/restart-state-alloc-inherited-current-0.json), [inherited/current/1](benchmarks/restart-state-alloc-inherited-current-1.json).

The frozen profiling flow was exercised with GNU and rmake. Reproduce through
`make profile-allocations-frozen PROFILE_CANDIDATE=/path/to/binary
PROFILE_WORKLOAD=startup-effects-1000 PROFILE_OUTPUT=/path/to/report` on one line.
The existing `profile-allocations` target still builds its isolated profile first.
No fixed performance acceptance threshold follows from these observations.
