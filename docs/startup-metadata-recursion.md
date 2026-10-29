# Recursion and diagnostic identity during startup include metadata

Startup now selects the numeric recursion level after all CLI bindings and before
include metadata effects. CLI effects still run at the initial level. Public
MAKELEVEL bytes remain unchanged until ordinary automatic-variable setup, with
normal binding priority preserved. Metadata shell captures use the selected
level, including unsigned wrap for their child environment.

Metadata warnings and unlocated errors use the selected program level. Located
errors retain the real definition filename and line instead of losing that
source during constructor unwinding. A diagnostic program identity is owned by
the error; a real source location takes precedence over it.

Final assignment priority and scope preservation now reuse one current binding
observation after RHS effects and environment promotion. This removes a duplicate
ordered-map search without retaining a value across evaluation or adding a cache.

55 original required cases cover raw numeric profiles, environment versus CLI
selection, output modes, early CLI versus metadata shell/warning boundaries,
constructor errors, real-file definitions and Makefile restarts. The preceding
build fails 35 cases and passes 20 controls. Both current builds match independent
models and two pinned GNU observations for all 55 cases and the preceding 155
include/RHS controls. The corpus becomes 10,920 required ordinary cases, zero
pending, with 17 unchanged separately gated safe-error exceptions.

Two initial predictions incorrectly treated command assignment leading whitespace
like process-environment bytes. GNU strips assignment-value whitespace before
numeric selection, so the single argv token `MAKELEVEL= -1` selects zero while environment
`MAKELEVEL=' -1'` selects the unsigned maximum. Source review and two agreeing GNU
observations refined those models; original expected bytes and observations are
retained without passing credit. Expectations were not rewritten to fit rmake.

This does not move include construction before defaults or complete its eval
context. Eleven of the earlier nineteen startup-metadata research cells remain
finite ordinary mismatches. Full startup/output lifecycles, real exec, remaining
CLI/native/Guile/locale and profile/integration work remain in the
[M3/M6 audit](m3-m6-audit.md). General performance parity remains open.

GNU Make 4.4.1 main.c level-selection ordering and read.c include construction
were consulted from the verified archive, SHA256
`dd16fb1d67bfab79a72f5e8390735c49e3e8e70b4945a15ab1f81ddb78658fb3`.
GNU source is copyright Free Software Foundation, Inc., GPL-3.0-or-later. Rust and
fixtures are original; no upstream implementation or tests were copied or
translated. No unsafe Rust or GNU execution fallback is added.

Formatting, warnings-denied lint, 460 Rust and 429 tooling tests pass. The
preceding 10,865-case development comparison passes, and all 55 final fixtures
match two GNU observations under each frozen build. Full frozen rmake-driven
development/optimized acceptance and CI for this commit remain merge gates; the
PR records their final results.

Debug SHA256:
`aa253edfecd1540d96edaecc6a2f1a27b98fdbf2fd8f4c83aac4589de4ac1cfd`.
Release SHA256:
`eb8be335402c0408697d188756acbbf2d19035966bca98ec0185187405c357e4`.

[Historical observations](../tests/research/startup-metadata-level.json) retain
raw bytes, file snapshots, repeated models and the incomplete CLI-whitespace
predictions. They earn no separate conformance or safety-exception credit.

Six sequential timing reports contain 101 alternating samples per engine, with
exact output checks and no independent local build, test or probe campaign during
timing. Host activity was not controlled. GNU and rmake drove the Makefile flow.
Several assignment-heavy medians are roughly 3–6 percent lower than preceding
rmake in this run; tiny and large no-op results remain mixed. GNU remains faster
on several workloads, with no general speedup or parity claim. The current
binding observation still occurs after RHS effects and environment promotion; no
filesystem or evaluation observation is cached.

| Workload | GNU ms | Current ms | Preceding rmake ms |
| --- | ---: | ---: | ---: |

| tiny-noop | 0.710 | 0.872 | 0.866 |
| large-noop | 11.165 | 29.673 | 29.251 |
| expansion | 2.796 | 4.946 | 5.164 |
| startup-arguments-0 | 0.539 | 0.638 | 0.642 |
| startup-arguments-1000 | 2.108 | 1.961 | 2.034 |
| startup-arguments-5000 | 25.776 | 8.135 | 8.646 |
| eval-arguments-0 | 0.687 | 0.790 | 0.803 |
| eval-arguments-1000 | 2.935 | 1.779 | 1.876 |
| startup-effects-0 | 0.640 | 0.739 | 0.746 |
| startup-effects-1000 | 2.525 | 2.994 | 3.088 |
| initial-flags-0 | 0.652 | 0.771 | 0.780 |
| initial-flags-1000 | 2.584 | 2.857 | 2.976 |
| runtime-includes-0 | 0.621 | 0.746 | 0.743 |
| runtime-includes-1000 | 2.739 | 10.214 | 10.539 |

Raw reports: [core](benchmarks/startup-metadata-level-core.json), [startup-arguments](benchmarks/startup-metadata-level-startup-arguments.json), [eval-arguments](benchmarks/startup-metadata-level-eval-arguments.json), [startup-effects](benchmarks/startup-metadata-level-startup-effects.json), [initial-flags](benchmarks/startup-metadata-level-initial-flags.json), [runtime-includes](benchmarks/startup-metadata-level-runtime-includes.json).

Reproduce with `make bench-compare BENCH_SUITE=runtime-includes BENCH_CANDIDATE=/path/to/current BENCH_BASELINE=/path/to/preceding BENCH_REPEAT=101`.
