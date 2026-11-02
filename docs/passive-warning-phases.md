# Post-read warning activation in GNUMAKEFLAGS

Post-read payload definitions now replay warning-enable events at their decoded
argument positions. Preliminary GNUMAKEFLAGS inspection can enable warnings for
expanding the following MAKEFLAGS value; payload replay starts from the policy
before that inspection and visits each actual event. Definitions before a late
warning flag retain their earlier policy. An inherited or explicit warning flag
remains active throughout. Flag-only streams keep their existing path.

The implementation reuses the owned cursor interpreter from
[live warning phases](live-warning-phases.md). It exposes transient passive
positions from the decoder and supplies the captured entry policy, without
storing a position vector in persistent Options or decoding again during replay.
Explicit CLI job-count priority remains intact. There is no new unsafe code,
dependency, expansion cache or filesystem-observation cache.

## Independent evidence

1,728 required original Makefiles cross reader/eval/include declarations, four
assignment forms, three warning positions, initial policy on/off, four positions
for `-e`, three existing-binding priorities and ASCII/raw-byte names. The models
were authored before execution. All agree with two bounded observations from
the pinned GNU Make 4.4.1 reference. Preceding rmake has 288 warning mismatches
and 1,440 controls; selected frozen debug and release agree with every model,
including exact output, status and filesystem bytes/types/modes/links. No reached
bounds or leftover children occurred. All new cases are serial; existing parallel
and runtime acceptance gates remain required.

Immediate definitions run their RHS effects even when a stronger value survives.
Conditional definitions skip an existing binding. Lazy values run when read;
their warnings use the selected recipe location, whereas post-read eager RHS
warnings are unlocated. `-e` affects definitions at its argument position.
Explicit CLI values survive either environment origin. Recipe-local `unexport`
isolates the tested reads from a later child-environment expansion without
changing stored origins or values. Ordered `seen` file contents independently
anchor effects; the printed snapshot is also exact.

[Raw evidence](../tests/research/passive-warning-phases.json) retains every input,
file mode, environment, argument, independent model, original topology and
observation for preceding/debug/release. Nine original discovery records remain
unchanged and separate, earning zero extra conformance or exception credit.
No model was rejected in this matrix. One Rust test protects passive positions
and explicit CLI job priority; five tooling tests protect the complete matrix,
raw evidence, stronger-value effects, byte inputs and mutation-sensitive anchors.

Formatting, Clippy, 523 Rust and 551 tooling tests are the local core gates.
Full frozen rmake-driven `m5-acceptance` and exact-head development/optimized CI
remain merge gates, with final results recorded in the PR. The ordinary corpus
becomes 21,457 required / zero pending across 148 families. The same 17 approved
safe-error exceptions remain separately gated with zero exact-conformance credit.

## Boundaries

This orders policies among decoded passive definitions. The collector still
expands the MAKEFLAGS source value before applying preceding GNUMAKEFLAGS payload
definitions: complete source/effect ordering remains ordinary compatibility work.
Effects before later malformed known values, contextual runtime HOME, finite
nested getopt behavior, directory/debug/database output, startup usage, real
exec/stdin, native loading, enabled Guile, broader Linux/locale/integration
acceptance and general performance parity remain open. M3/M6 are not complete.

The reference is x86-64 Linux/glibc, C locale, disabled Guile; profile
`92525c9784f4dd471cceb12ce66a9d367e267b638e1384cdc29962b738d16de7`.
Oracle SHA256: `b8806e8eb7452ed9b7ed5e18cb7e76ff9abd488c9d62a266a398d8044fade974`.
Selected debug SHA256: `ec4055dbb77af53661f02989317f9cd286c600457f4fd91724e81b0023ae135e`.
Selected release SHA256: `1915f8537865f028690891eac5738c5cb6e25e6716cddfd36c4387b0ca342e5b`.
All implementation, fixtures and models are original. No GNU source or tests
were copied or translated, and GNU is never a runtime implementation fallback.

Elapsed controls are recorded separately below; this compatibility correction
receives no performance-win claim. General performance parity remains open.

Four raw reports retain 101 alternating samples per engine/workload and exact
output agreement, with preceding PR #150 as control. GNU drives A and selected
rmake drives B through the top-level Makefile. No unrelated local build, test
or probe campaign ran during timing. All timed controls already match before
this fix and earn no speed credit for newly corrected behavior. Host activity
is uncontrolled; CPU/RSS/I/O, confidence intervals and warning-heavy performance
are unmeasured.

| Workload | GNU / selected / preceding median ms A | B | Paired preceding/selected A / B |
| --- | ---: | ---: | ---: |
| tiny-noop | 0.619 / 0.731 / 0.736 | 0.548 / 0.621 / 0.628 | 1.012 / 1.010 |
| large-noop | 11.379 / 30.357 / 30.065 | 11.115 / 29.810 / 29.824 | 0.991 / 0.996 |
| expansion | 2.958 / 5.265 / 5.297 | 2.808 / 5.001 / 5.046 | 0.998 / 1.013 |
| 0 definitions / 100 updates | 0.857 / 2.869 / 3.112 | 0.851 / 2.892 / 2.808 | 1.020 / 0.973 |
| 64 definitions / 100 updates | 0.927 / 2.769 / 2.938 | 1.075 / 3.321 / 3.160 | 1.026 / 0.937 |
| 512 definitions / 100 updates | 1.233 / 3.523 / 3.511 | 1.297 / 2.799 / 2.882 | 1.072 / 1.047 |

Ratios above one favor selected rmake. Core differences are small; runtime
results are mixed, including a roughly 5% higher 64-definition median in B
after a lower median in A. The full observations are retained without selecting
only favorable samples. GNU remains faster on these workloads.

Reports: [core A](benchmarks/passive-warning-core-a.json),
[core B](benchmarks/passive-warning-core-b.json),
[runtime A](benchmarks/passive-warning-runtime-a.json),
[runtime B](benchmarks/passive-warning-runtime-b.json).
