# Command-line assignment effects and default phases

Command-line variable names and eager right-hand sides now use the full database
expander. Nested `eval`, includes, exports, lexical frames, file I/O and shell
requests share the ordinary evaluator and owned effect boundaries. Assignment
arguments remain single definition tokens: literal comments, semicolons and
newlines are not reinterpreted as Makefile statements unless an explicit `eval`
asks the reader to parse them. Names expand once; lazy assignments retain their
normal timing and conditional assignments skip unused effects.

Early CLI includes search the current directory. GNU has not constructed its
`-I` search path at this phase. Real included-file errors retain their filename
and physical line through constructor unwinding; anonymous errors retain the
program prefix. Already emitted output survives buffered startup failure under
all four output synchronization selections.

## Default installation

`SHELL`, the pinned default-variable table and `.DEFAULT_GOAL` are installed after
CLI assignment effects and before the `-E` buffer pass. Early reads therefore
observe missing defaults. Explicit command definitions retain priority; a
file-priority `.DEFAULT_GOAL` created by early eval is reset by initialization.
Early `undefine .DEFAULT_GOAL` is valid before live metadata activation. This
closes the recorded finite CLI eval control without extending safe-error policy.

An imported `SHELL` is available during early CLI expansion, with its original
no-export policy. After CLI effects, an imported or empty selected shell becomes
`/bin/sh` at file origin, preserving flavor, source identity and export metadata.
An explicit nonempty command shell survives. Without a selected early shell,
GNU attempts the configuration's `-c` word as a program and returns shell status
127; the candidate preserves that observable result. Internal shell selection
is silent under undefined-variable warnings; actual missing-variable references
and later GNUMAKEFLAGS decoding retain their own warning phases.

This is a bounded phase correction, not complete startup reconstruction. Suffix
metadata, automatic/default metadata, raw option order, inherited flag expansion,
CLI directory effects and true process replacement retain separate audit work.

## Evidence

186 independently authored required cases cover seven assignment forms and four
built-in profiles, 64 priority/undefine combinations, default visibility, shell
selection, scoped/nested effects, include search, constructor diagnostics, raw
bytes, exports, recursion and stable restart values. Exact status/stream models
and intended file effects were checked against two pinned GNU observations and
the candidate. The earlier 9,992-case ordinary corpus also passed after the code
changes. The corpus is now 10,178 ordinary required cases, zero pending entries,
and 17 separately gated safe-error exceptions. Complete development/optimized
acceptance and exact-head CI remain merge gates.

The historical warning and inactive-metadata probe records are retained. Their
current required controls are `cli-assignment-warning-cc`,
`cli-assignment-warning-goal` and `cli-assignment-metadata-inactive`; historical
prototype mismatches are not current acceptance results.

## Ordinary restart gap

[`cli-assignment-restart-effect.json`](../tests/research/cli-assignment-restart-effect.json)
records an independently modeled immediate CLI eval which appends to a file.
Both GNU runs append once at initial startup and again after Makefile re-exec.
The current in-process restart restores a snapshot taken after CLI assignments,
so it retains their values without repeating their I/O. This is a finite,
ordinary compatibility mismatch, with zero passing or safety-exception credit.
Startup replay must reconstruct pre-assignment state and its ordered phases;
re-expanding effects against the final variable snapshot would corrupt appends,
conditionals and earlier lookups. Full self-exec lifecycle remains open.

The fixed GNU 4.4.1 source was consulted for startup order, shell initialization,
include-path construction and error identities. No GNU implementation or
upstream tests were copied or translated. No unsafe code, semantic cache or GNU
runtime fallback is added.

## Frozen candidate and performance observations

Formatting, warnings-denied lint, 457 Rust tests and 392 tooling tests passed.
All 186 final controls match independent models and two GNU observations under
both frozen candidates. The debug SHA256 is
`f9c417bdb23861b04c9df6ec7b2f1f975e3600b1218ec49043381548e02977f4`;
the release SHA256 is
`233078d1c9686204b52b1050a2c3b96306d1aa31f6ec48c907f9e2cd0f5cc1f5`.
Full development/optimized acceptance and exact-head CI remain merge gates.

Each report contains 101 alternating samples per implementation, validates exact
output, and records candidate, preceding-release and oracle identities. No build
or test campaign ran concurrently; other host activity was not controlled.
These timings do not establish a statistical change or general performance
parity. Common workloads remain mixed; large no-op and expansion medians are
about 2 percent higher than preceding rmake, and the 1,000-buffer eval median is
about 4 percent higher. This is a compatibility correction with no speedup claim.

| Workload | GNU ms | Current ms | Preceding rmake ms |
| --- | ---: | ---: | ---: |

| tiny-noop | 0.704 | 0.838 | 0.850 |
| large-noop | 12.090 | 32.587 | 32.060 |
| expansion | 2.832 | 5.095 | 5.007 |
| startup-arguments-0 | 0.659 | 0.812 | 0.803 |
| startup-arguments-1000 | 2.183 | 2.021 | 2.011 |
| startup-arguments-5000 | 25.971 | 8.434 | 8.447 |
| parallel-branches-depth-1 | 87.309 | 89.212 | 89.542 |
| parallel-branches-depth-4 | 87.549 | 89.969 | 89.908 |
| eval-arguments-0 | 0.711 | 0.850 | 0.840 |
| eval-arguments-1000 | 3.156 | 2.094 | 2.014 |

Raw reports: [core](benchmarks/cli-assignment-core.json), [startup arguments](benchmarks/cli-assignment-startup-arguments.json), [parallel branches](benchmarks/cli-assignment-parallel-branches.json), [eval arguments](benchmarks/cli-assignment-eval-arguments.json).
