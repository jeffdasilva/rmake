# Linux absolute-path byte bounds

The pinned GNU profile limits each raw `abspath` word to fewer than 4,096 bytes.
A longer word contributes no result, even if redundant separators or dot
components would normalize to a short path. The selected relative path also has
an incremental result bound: reaching 4,096 bytes while appending a component
discards the word before a later `..` can shorten it. These are byte limits,
including UTF-8 input, rather than character or final-result-only checks.

The original component-vector normalizer now returns an optional owned result.
It checks raw word length, tracks normalized length through component append/pop
and omits rejected words without extra join spaces. It borrows the base and
input components instead of copying a combined path, and allocates the final
result at its known length. Pending versus selected startup directory behavior,
pure-library context errors and full argument expansion/effects are preserved.
No filesystem access, expansion-result cache or unsafe Rust is introduced.

243 new original required cases cover 240 semantic models and three existing
output-checked target-context workloads. Coverage includes 4,094–4,097-byte
absolute words; long separator/dot inputs; UTF-8 byte counts; mixed kept/skipped
words; selected relative overflow before parent reduction; inherited GNU/MAKE,
eager/deferred CLI, reader and recipe phases; output modes; and 48 argument-file
effect controls. The relative transient-overflow fixtures use conformance
workspaces whose directory is longer than three bytes. The scaling controls
exercise all 3,000 automatic/path checks with absent, explicit and pattern scopes.

Two bounded pinned GNU observations agree with every original model. Both frozen
builds match all 243; preceding rmake fails 138 and passes 105 controls. No model
refinement, oracle disagreement or new safety exception was needed. Raw inputs,
modes, observations, files, binary identities and bounds are retained in the
[historical evidence](../tests/research/abspath-bounds.json).

The complete ordinary comparison passes 12,921 required cases / zero pending,
plus 17 unchanged separately gated approved safe-error exceptions. Formatting,
warnings-denied lint, 465 Rust and 461 tooling tests pass. Three Rust boundary
checks cover raw bytes, intermediate overflow and join spacing. Tooling checks
retain all exact inputs and observations, required file effects, context workload
bodies and allocation-profiler input/output rejection.

Debug SHA256:
`6cd61a62cedad9a8f1b185b48042be1eb5da8d16518de32bc612782cb9ccfb9f`.
Release SHA256:
`86457b2ad4d6881a178611c35c5f8bb398829a284bac21af0ab3a85e752fc1bd`.
Full frozen development/optimized rmake-driven acceptance and final-commit CI
remain merge gates; their results are recorded in the PR.

GNU Make 4.4.1 function.c/main.c path behavior was consulted from the verified
archive SHA256
`dd16fb1d67bfab79a72f5e8390735c49e3e8e70b4945a15ab1f81ddb78658fb3`.
GNU source is copyright Free Software Foundation, Inc., GPL-3.0-or-later. The Rust
normalizer, models and tooling are original; no upstream implementation/tests
were copied or translated and GNU is not an execution fallback. This closes a
bounded Linux function gap. Runtime HOME/tilde, broader CLI/output, real exec,
native loading, enabled Guile, locale/Linux profiles, integration and general
performance/cache acceptance remain in the [M3/M6 audit](m3-m6-audit.md).

## Measurement evidence

Four sequential reports contain two batches of 101 alternating samples per
engine/workload, two warmups and exact output agreement. The target-context
workloads are required fixtures, including every automatic-variable, scope and
path assertion. No other local build, test or probe campaign ran during timing.
Host activity was not controlled; no confidence intervals or performance
acceptance thresholds are claimed. Only output-equivalent inputs earn timing
comparison; the preceding incorrect boundary cases are not used for speed credit.

| Workload / batch | GNU ms | Current ms | Preceding rmake ms |
| --- | ---: | ---: | ---: |
| target-context-none / A | 11.644 | 17.851 | 18.026 |
| target-context-explicit / A | 14.987 | 24.320 | 24.756 |
| target-context-pattern / A | 14.106 | 22.406 | 23.041 |
| target-context-none / B | 11.746 | 18.359 | 18.784 |
| target-context-explicit / B | 15.334 | 25.960 | 26.895 |
| target-context-pattern / B | 14.347 | 23.138 | 23.690 |
| tiny-noop / A | 0.631 | 0.737 | 0.719 |
| large-noop / A | 11.580 | 30.648 | 30.569 |
| expansion / A | 2.912 | 5.067 | 5.038 |
| tiny-noop / B | 0.642 | 0.734 | 0.748 |
| large-noop / B | 11.740 | 31.364 | 31.176 |
| expansion / B | 3.058 | 5.368 | 5.513 |

The paired preceding/current ratios for target-context workloads range from
1.008 to 1.028 here. Core results remain mixed, and rmake remains slower than
GNU on these workloads. This does not establish general speedup or parity.
Raw reports: [contexts A](benchmarks/abspath-bounds-context-a.json),
[contexts B](benchmarks/abspath-bounds-context-b.json),
[core A](benchmarks/abspath-bounds-core-a.json),
[core B](benchmarks/abspath-bounds-core-b.json).

Repeated glibc observations use matched-length binary/helper/trace paths and
strict required fixture source, argument, environment and output checks. GNU
make drives batch A and frozen rmake drives B, with reversed old/new order.
Both batches repeat the following exact counts:

| Workload | Prior requests | Current requests | Prior requested bytes | Current requested bytes |
| --- | ---: | ---: | ---: | ---: |
| tiny-noop | 287 | 287 | 45,259 | 45,259 |
| target-context-none | 255,255 | 240,255 | 20,342,469 | 19,763,469 |
| target-context-explicit | 348,267 | 333,267 | 28,595,615 | 28,016,615 |
| target-context-pattern | 324,268 | 309,268 | 27,736,885 | 27,157,885 |

Each 3,000-target workload uses 15,000 fewer requests and 579,000 fewer requested
bytes on these profiles; the tiny control is unchanged. Requested bytes are
not peak/live memory. The observer covers glibc malloc/calloc/realloc/free after
initialization, excludes aligned allocation entry points and uses sampled stacks
as allocation hints rather than CPU timings. All sixteen reports and raw traces
are retained in the [allocation observations](benchmarks/abspath-bounds-allocations.json).
Do not sum byte deltas across different path-prefix profiles.

Run `make bench-compare BENCH_SUITE=target-contexts BENCH_REPEAT=101` or
`make profile-allocations-frozen PROFILE_WORKLOAD=target-context-none` through
GNU make or rmake. The profiler also accepts the explicit and pattern variants.
