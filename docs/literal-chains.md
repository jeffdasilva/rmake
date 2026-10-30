# Literal variable-reference chains

A long finite chain such as `A = $(B)`, `B = ${C}`, `C := end` previously
consumed a Rust expansion frame for every link and reached rmake's recursion
guard while GNU still succeeded. Deep cycles also lost GNU's recursive-variable
diagnostic to that guard. Literal chains now use an iterative heap walk.

The walk first determines whether the entire chain is free of interpreter
effects. It accepts whole literal references with ASCII alphanumeric/underscore
names, including a single-character `$x` reference. It follows current lexical
and store-layer lookups, retaining exact name/layer cycle identity. Borrowed keys
in a temporary membership set avoid quadratic cycle scans and per-name copies.
Simple values retain literal dollars. Both recursive input and alias-return NUL
boundaries are preserved before caller suffix output.

Dynamic metadata, outer append layers, computed names and any function/effect
body use the full evaluator. An unsuccessful eligibility walk changes no binding,
context or active stack. The complete pure walk returns owned values or missing/
error descriptors before warning output. Missing-child warnings retain the first
binding/caller context; cycle errors retain the real definition or nearest
located parent. Cancellation is checked during the walk. No Rust recursion guard
is raised, no semantic result is cached and no unsafe Rust is added.

353 new required original controls cover 256 depth models, 94 cross-family models
and three output-checked scaling workloads. The depth matrix covers chains up to
512 references, simple/recursive/missing/cyclic leaves, reader/recipe phases and
output modes. Cross-checks cover brace/parenthesis/short references, simple dollar
values, warnings, CLI cycle definitions, numeric lexical/call bindings, private
and inherited target scopes, append layers, active-parent cycles and one-time
file/eval fallback effects. Two bounded pinned GNU observations agree with every
final model. Both frozen builds match all 353; preceding rmake fails 166 and passes
187 controls. The original 136 deep-chain research failures are all closed.

[Raw evidence](../tests/research/literal-chains.json) retains exact inputs, modes,
bytes/status/files, bounds and binary identities. Twenty-four initial warning
models omitted an unrelated late undefined GNUMAKEFLAGS warning; revised inputs
import an empty GNUMAKEFLAGS binding while keeping the intended warning unchanged.
Four initial numeric models incorrectly expected a call to mask a surrounding
let binding. Their original failed models/observations earn no credit. Corrected
models were independently qualified again. The original 256 depth models needed
no refinement, and no new oracle crash/hang exception is used.

The ordinary corpus is now 13,274 required / zero pending, plus 17 unchanged
separately gated approved safe-error exceptions. Formatting, warnings-denied
lint, 468 Rust and 466 tooling tests pass. Three Rust tests cover a 5,000-link
heap walk and cycle, library NUL/literal-dollar boundaries, and warning context
restoration within one expansion. This library stress test does not earn extra
GNU conformance credit. The preceding 12,921-case corpus also passes.

Debug SHA256:
`9a76475f5ae1ad5f39a173065713a787a0a5a5b4a2bef4c295171751996e1312`.
Release SHA256:
`a0df269aba0915779ec55cf68e8dc60a4e88ffa97a8f83577f80ce68e686bd5c`.
Full frozen rmake-driven development/optimized acceptance and final-commit CI
remain merge gates; their results are recorded in the PR.

GNU Make 4.4.1 expand.c variable expansion, variable.c diagnostics and the pinned
public semantics were consulted from verified archive SHA256
`dd16fb1d67bfab79a72f5e8390735c49e3e8e70b4945a15ab1f81ddb78658fb3`.
GNU source is copyright Free Software Foundation, Inc., GPL-3.0-or-later. Rust,
models and tooling are original; no upstream implementation/tests copied or
translated, and no GNU execution fallback introduced.

This is a bounded M3/M6 slice. General function/eval/include continuations,
nonliteral deep references, append-depth/output budgets and broader resource
acceptance remain open. The internal guards are not the seventeen approved
recorded oracle exceptions. Runtime HOME/tilde, broader CLI/output, real exec,
native loading, enabled Guile, Linux/locale/integration and persistent-cache/
general-performance acceptance remain in the [M3/M6 audit](m3-m6-audit.md).

## Measurements

Four sequential reports contain two batches of 101 alternating samples per
engine/workload, two warmups and exact output agreement. The chain workloads
perform 1,000 eager reads through depths 0, 32 and 96, observing every value.
All three workloads pass as required fixtures on GNU, preceding rmake and the
candidate. Previously incorrect deeper cases earn no timing credit. No other
local build, test or probe campaign ran during timing. Host activity was not
controlled; there are no confidence intervals or performance thresholds.

| Workload / batch | GNU ms | Current ms | Preceding rmake ms |
| --- | ---: | ---: | ---: |
| literal-chains-0 / A | 1.146 | 1.752 | 1.724 |
| literal-chains-32 / A | 3.151 | 4.130 | 6.419 |
| literal-chains-96 / A | 7.924 | 8.123 | 21.917 |
| literal-chains-0 / B | 1.037 | 1.507 | 1.486 |
| literal-chains-32 / B | 3.171 | 4.205 | 6.411 |
| literal-chains-96 / B | 8.013 | 8.232 | 22.632 |
| tiny-noop / A | 0.625 | 0.748 | 0.693 |
| large-noop / A | 11.309 | 30.748 | 30.722 |
| expansion / A | 2.847 | 4.989 | 5.111 |
| tiny-noop / B | 0.556 | 0.657 | 0.647 |
| large-noop / B | 11.245 | 29.921 | 30.042 |
| expansion / B | 2.902 | 4.977 | 5.099 |

The paired preceding/current ratio is 2.708 and 2.744 for 96 references, and
1.554 and 1.535 for 32. The zero-depth and core observations retain slower/mixed
results, and GNU remains faster on several workloads. This does not establish
general performance parity. Raw reports: [chains A](benchmarks/literal-chains-alias-a.json),
[chains B](benchmarks/literal-chains-alias-b.json),
[core A](benchmarks/literal-chains-core-a.json),
[core B](benchmarks/literal-chains-core-b.json).

Sixteen glibc profiles use matched-length old/new binary/helper/trace paths and
strict required-fixture source, arguments, environment and output checks. GNU
make drives batch A and frozen rmake drives B, with reversed old/new order. Both
batches repeat these exact request counts:

| Workload | Prior requests | Current requests | Prior requested bytes | Current requested bytes |
| --- | ---: | ---: | ---: | ---: |
| tiny-noop | 287 | 287 | 45,259 | 45,259 |
| literal-chains-0 | 10,471 | 10,471 | 459,133 | 459,133 |
| literal-chains-32 | 174,665 | 16,665 | 3,900,428 | 4,640,428 |
| literal-chains-96 | 497,061 | 18,061 | 13,706,828 | 8,894,828 |

The 96-reference workload uses 479,000 fewer requests and 4,812,000 fewer requested
bytes. At 32 references requests decrease while requested bytes increase by
740,000; the temporary membership index has a different allocation tradeoff.
The controls are unchanged. Requested bytes are not live/peak memory. The observer
covers glibc malloc/calloc/realloc/free after initialization, excludes aligned
entry points and treats sampled stacks as allocation hints rather than CPU time.
All reports and raw traces are retained in the [allocation observations](benchmarks/literal-chains-allocations.json).
Do not sum byte deltas across different path-prefix profiles.

Run `make bench-compare BENCH_SUITE=literal-chains BENCH_REPEAT=101` or
`make profile-allocations-frozen PROFILE_WORKLOAD=literal-chains-96` through
GNU make or rmake. The profiler also supports zero and 32 references.
