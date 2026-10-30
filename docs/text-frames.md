# Pure text expansion frames

Recursive variables can now contain literal text, escaped dollars and multiple
literal references without spending a Rust call frame for every variable.
Eligible graphs use heap frames and a flat arena of output pieces. Shared child
bindings are visited for every reference, so missing-variable warnings repeat in
the same order. Reference lookup retains lexical/call frames, target scopes,
private inheritance and exact name/layer cycle identity. Cycles report the real
definition or nearest located parent; warnings retain the first binding/caller
context. Simple rebinding bypasses an older recursive active key.

The temporary syntax plan borrows immutable binding text and keeps instructions
in one buffer. It accepts literal ASCII alphanumeric/underscore names, brace,
parenthesis and short references, ordinary text and `$$`. It rejects computed
names, substitution references, function/effect bodies, dynamic metadata and
outer append layers before any diagnostic receiver runs. These use the full
evaluator. A rejected complete plan emits no partial warnings or effects.

A trial walk records output pieces and missing-reference events without copying
every intermediate string. Raw input NUL truncation and recursive return NUL
truncation remain separate. Simple values retain literal dollars and raw NULs
until their containing recursive frame returns; later warnings in that frame
still run, while caller suffix text survives. Raw intermediate output lengths
still receive the existing 16 MiB check before return truncation. Compiler and
trial metadata have bounded budgets, and budget rejection drops temporary state
before falling back. No Rust guard is raised, no semantic result is cached and
no unsafe Rust or GNU runtime fallback is introduced.

After the trial, missing warnings are emitted in order under the live receiver
policy. Cancellation prevents subsequent warnings, value output and a prepared
cycle error. Immutable store borrows and Rust field borrowing prevent mutation
of the bindings used by the plan. This path contains no interpreter effects.
The successful result is flattened iteratively once; the flat arena also avoids
recursive tree destruction.

479 new required original controls cover text before/after references, branching
and repeated shared children, missing/literal/simple/cyclic leaves, depths up to
256, mixed reference spelling, short references, escaped dollars, exact warning
order, numeric lexical/call frames, inherited/private target scopes, command-line
cycle locations and effectful/append/computed fallbacks. Two bounded pinned GNU
observations agree with each independent model. Frozen debug and release builds
match all 479; preceding rmake fails 182 and passes 297. No model refinement or
new crash/hang exception was needed. The earlier model named
`text-cross-fallback-computed` contains a nested literal alias; separate
`text-cross-computed-name` inputs exercise actual computed names.

[Raw evidence](../tests/research/text-frames.json) preserves exact original files,
modes, arguments, environments, output bytes, statuses, resulting files, bounds
and executable identities. These records are historical evidence, not separate
conformance credit. The ordinary corpus is 13,753 required / zero pending, plus
17 unchanged, separately gated approved safe-error exceptions.

Eight additional Rust tests check a 5,000-frame text graph and cycle, library NUL
boundaries, live warning policy, cancellation, compiler/trial fallback without
duplicate warnings, output-limit ordering and simple rebinding. The library
stress tests earn no additional GNU conformance credit. Formatting, lint, 476
Rust and 473 tooling tests pass; the preceding 13,274-case corpus also passes.
The complete frozen rmake-driven acceptance flow and final-commit GitHub CI
remain merge gates; their final outcomes are recorded in the PR.

Debug SHA256:
`58e9fbc4ca8c48d45def29612d1fd501e3d450cb2f71817fba6222ea4157c804`.
Release SHA256:
`509910d4925f0cd77116a98c89d9cc3d3df9799d71998bb18c1989b311e525a4`.

GNU Make 4.4.1 expand.c variable expansion, variable.c diagnostics and the pinned
public semantics were consulted from verified archive SHA256
`dd16fb1d67bfab79a72f5e8390735c49e3e8e70b4945a15ab1f81ddb78658fb3`.
GNU source is copyright Free Software Foundation, Inc., GPL-3.0-or-later. Rust,
models and tooling are original; no upstream implementation/tests were copied
or translated.

This is a bounded M3/M6 slice. Effectful/general function/eval/include
continuations, arbitrary nonliteral names, append-depth/output resource
acceptance and broad input/profile behavior remain open. The unchanged internal
resource guards do not qualify as approved oracle safety exceptions. Native
loading, enabled Guile, real exec, runtime HOME/tilde, broader CLI/output and
Linux/locale/integration acceptance, persistent caching and general performance
parity remain in the [M3/M6 audit](m3-m6-audit.md).

## Measurements

The output-checked text workloads perform 1,000 eager reads through depths 0,
32 and 96 and observe every complete value. All pass on GNU, preceding rmake and
the candidate. Incorrect preceding deep cases earn no timing credit. The first
owned-plan prototype matched its models but added avoidable source copies and
small allocations; it was replaced by borrowed text and a shared instruction
buffer before final measurements. The final path retains overhead on some
shallower graphs; reduced Rust stack use is not a universal speedup.

Final repeated paired observations are recorded below. Host activity is not
controlled, and no confidence intervals or performance thresholds are claimed.
No unrelated local build, test or probe campaign runs during timing.

Run `make bench-compare BENCH_SUITE=text-frames BENCH_REPEAT=101` through GNU make
or rmake. Existing `core` and `literal-chains` suites retain their own controls.

Four sequential reports contain two batches of 101 alternating samples per engine/workload, with two warmups and exact byte/status agreement. GNU make drives batch A; the frozen candidate drives batch B.

| Workload / batch | GNU ms | Current ms | Preceding rmake ms |
| --- | ---: | ---: | ---: |
| text-frames-0 / A | 1.102 | 1.698 | 1.687 |
| text-frames-32 / A | 4.133 | 12.837 | 10.391 |
| text-frames-96 / A | 9.613 | 29.433 | 34.476 |
| text-frames-0 / B | 1.049 | 1.659 | 1.649 |
| text-frames-32 / B | 4.127 | 13.241 | 10.559 |
| text-frames-96 / B | 9.719 | 29.886 | 35.137 |
| tiny-noop / A | 0.648 | 0.777 | 0.768 |
| large-noop / A | 12.286 | 32.579 | 33.154 |
| expansion / A | 3.690 | 6.227 | 6.149 |
| tiny-noop / B | 0.625 | 0.745 | 0.749 |
| large-noop / B | 11.883 | 32.093 | 32.242 |
| expansion / B | 3.067 | 5.402 | 5.379 |

The paired preceding/current ratios at 96 levels are 1.175 and 1.172. At 32 levels they are 0.809 and 0.793, a reproducible regression retained for future optimization. Zero-depth/core observations remain mixed, and GNU is faster on these workloads. This does not establish general performance parity. No live/peak-memory or allocation-count claim is made.

Raw reports: [text A](benchmarks/text-frames-text-a.json), [text B](benchmarks/text-frames-text-b.json), [core A](benchmarks/text-frames-core-a.json), [core B](benchmarks/text-frames-core-b.json).
