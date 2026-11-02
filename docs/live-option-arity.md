# Live option arity and skipped effects

Live MAKEFLAGS decoding now skips implemented no-argument switches carrying a
forbidden `=value`, and implemented required-argument switches without an operand
at the end of the input. Rejected options do not mutate switch state, origins,
include-directory metadata or operand receivers. Valid earlier short-cluster
letters and later definitions retain their original order and positions.
Startup command-line and inherited-environment parsing remain strict. Explicit
empty operands, invalid numeric values and registered unimplemented features
retain their separate validation; this is not complete malformed-option support.

770 original required Makefiles comprise 700 active forms and 70 inactive reader
controls. Twenty-two no-argument spellings each cover empty and literal attached
values. Fourteen long and six short required-argument spellings cover missing
operands; six mixed clusters check that an earlier valid silent switch survives.
Every active form has pure and assignment-payload variants across reader, eval,
include, recipe and secondary expansion. Exact MFLAGS, assignment value/origin/
flavor, retained default CC and ordered file witnesses distinguish skipped
effects from a successful continuation that applied the rejected switch.

All independent models agree with two bounded GNU Make 4.4.1 observations. Both
frozen builds match exact streams, status and filesystem bytes/types/modes/link
relationships without reached bounds or leftover children. Preceding rmake has
700 mismatches and 70 exact inactive controls. No model was rejected or changed
to fit observed output. Research identifiers containing short-option capitals
are lowercased only for valid fixture IDs; input and output bytes are unchanged.
[Raw evidence](../tests/research/live-option-arity.json) retains the original
research identifiers, all inputs/modes, expectations, identities and observations.

Two Rust tests protect decoder phases, prefix retry, prior cluster letters,
assignment positions, missing-operand receiver suppression and separate empty/
numeric validation. Five tooling tests protect the complete inventory, raw
observations, all phases, inactive controls, origins and ordered file effects.
The standard top-level Makefile gates discover every fixture. Full frozen
rmake-driven `m5-acceptance` and exact-head development/optimized CI gate merging;
the final results belong to the PR. Formatting, Clippy, 520 Rust and 534 tooling
tests pass before that combined acceptance run.

The ordinary corpus becomes 18,506 required / zero pending, with the same 17
separately gated approved safe-error exceptions. Full M3/M6 remains open:
startup usage and ambiguity, broader malformed arguments, nested/runtime HOME
decoding, directory/debug/database output, real exec/stdin lifecycle, native
loading, enabled Guile, Linux/locale/integration acceptance and general
performance parity still require implementation and evidence. Persistent caching
retains its separate safety and correctness design.

The qualified profile is x86-64 Linux/glibc, C locale and disabled Guile.
Oracle profile: `92525c9784f4dd471cceb12ce66a9d367e267b638e1384cdc29962b738d16de7`.
Oracle SHA256: `b8806e8eb7452ed9b7ed5e18cb7e76ff9abd488c9d62a266a398d8044fade974`.
Verified source archive: `dd16fb1d67bfab79a72f5e8390735c49e3e8e70b4945a15ab1f81ddb78658fb3`.
All Rust, fixtures, models and observers are original. No GNU implementation or
tests were copied or translated; GNU is never a runtime implementation fallback.

Frozen debug SHA256: `358f65eb9711ba154ec4d5d90260aa7cf2593252a65b12b790883f859edce65d`.
Frozen release SHA256: `3ab6131d069f871016629667136d9cbabcff0ae7ba939d05ce4049f2aa8d2ca5`.
Final binaries are byte-identical to those used for the raw observations.

## Measurements

Four reports retain two sequential batches of 101 alternating samples per engine
and workload, two warmups and exact output agreement. GNU drives batch A; frozen
rmake drives batch B through the top-level Makefile. The preceding rmake control
was built in this same worktree. Every timed input already matches on that build;
newly accepted arity cases receive no timing credit. No unrelated local build,
test or probe campaign runs during timing. Host activity remains uncontrolled;
there are no CPU/RSS/I/O measurements, confidence intervals or performance gates.

| Workload | GNU / candidate / preceding median ms A | GNU / candidate / preceding median ms B | Paired preceding/candidate A / B |
|---|---:|---:|---:|
| tiny-noop | 0.713 / 0.867 / 0.878 | 0.560 / 0.647 / 0.640 | 1.011 / 0.982 |
| large-noop | 11.292 / 30.701 / 29.927 | 11.206 / 30.362 / 29.653 | 0.979 / 0.977 |
| expansion | 2.909 / 5.051 / 5.023 | 2.848 / 4.984 / 4.951 | 0.988 / 0.995 |
| runtime-includes-0 | 0.679 / 0.803 / 0.792 | 0.599 / 0.714 / 0.715 | 0.993 / 0.994 |
| runtime-includes-1000 | 2.934 / 17.355 / 17.023 | 2.868 / 16.115 / 16.016 | 1.006 / 0.984 |

Ratios above one favor the candidate. Results remain mixed; large-noop is about
2% slower in both batches. These observations do not establish general speedup
or GNU performance parity. Raw reports: [core A](benchmarks/live-arity-core-a.json),
[core B](benchmarks/live-arity-core-b.json),
[runtime A](benchmarks/live-arity-runtime-includes-a.json),
[runtime B](benchmarks/live-arity-runtime-includes-b.json).
