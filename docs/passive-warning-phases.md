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

1,872 required original Makefiles include a 1,728-case matrix crossing reader/eval/include declarations, four
assignment forms, three warning positions, initial policy on/off, four positions
for `-e`, three existing-binding priorities and ASCII/raw-byte names. The models
were authored before execution. All agree with two bounded observations from
the pinned GNU Make 4.4.1 reference. Preceding rmake has 288 warning mismatches
and 1,584 controls; selected frozen debug and release agree with every model,
including exact output, status and filesystem bytes/types/modes/links. No reached
bounds or leftover children occurred. All new cases are serial; existing parallel
and runtime acceptance gates remain required.

The additional 144 source-expansion models enable warnings through a nested
MAKEFLAGS assignment while GNUMAKEFLAGS expands, before its payload is decoded.
These were authored before executing the control. The first prototype captured
policy too early and regressed 48 of these cases; preceding rmake passes all 144.
The selected implementation captures policy after source expansion, and both
frozen builds pass all 1,872 models. The interrupted first acceptance run earns
no pass credit. Its source commit, complete earlier proof and all new regression
observations remain recorded rather than discarded.

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
and explicit CLI job priority; seven tooling tests protect the complete matrix,
raw evidence, stronger-value effects, byte inputs and mutation-sensitive anchors.

Formatting, Clippy, 523 Rust and 553 tooling tests are the local core gates.
Full frozen rmake-driven `m5-acceptance` and exact-head development/optimized CI
remain merge gates, with final results recorded in the PR. The ordinary corpus
becomes 21,601 required / zero pending across 148 families. The same 17 approved
safe-error exceptions remain separately gated with zero exact-conformance credit.

## Boundaries

This stage orders policies among decoded passive definitions. Its collector still
expands the MAKEFLAGS source value before applying preceding GNUMAKEFLAGS payload
definitions: complete source/effect ordering remains ordinary compatibility work.
Effects before later malformed known values, contextual runtime HOME, finite
nested getopt behavior, directory/debug/database output, startup usage, real
exec/stdin, native loading, enabled Guile, broader Linux/locale/integration
acceptance and general performance parity remain open. M3/M6 are not complete.

The subsequent [sequential source follow-up](passive-source-order.md) closes the
collector ordering gap. The observations above remain specific to this earlier
stage; within-source malformed values, contextual HOME and broader acceptance
still need their own qualification.

The reference is x86-64 Linux/glibc, C locale, disabled Guile; profile
`92525c9784f4dd471cceb12ce66a9d367e267b638e1384cdc29962b738d16de7`.
Oracle SHA256: `b8806e8eb7452ed9b7ed5e18cb7e76ff9abd488c9d62a266a398d8044fade974`.
Selected debug SHA256: `6f02b0520caf8b9f5b634dbdf19dc92044da9ce8179af0d7599610a61c911f2f`.
Selected release SHA256: `87fdddd768dcd7526ce8672956239e6ecea916ce28fab7bec6567110794abea4`.
All implementation, fixtures and models are original. No GNU source or tests
were copied or translated, and GNU is never a runtime implementation fallback.

Elapsed controls are recorded separately below; this compatibility correction
receives no performance-win claim. General performance parity remains open.

Four selected reports retain 101 alternating samples per engine/workload and exact
output agreement, with preceding PR #150 as control. GNU drives A and selected
rmake drives B through the top-level Makefile. No unrelated local build, test
or probe campaign ran during timing. All timed controls already match before
this fix and earn no speed credit for newly corrected behavior. Host activity
is uncontrolled; CPU/RSS/I/O, confidence intervals and warning-heavy performance
are unmeasured.

| Workload | GNU / selected / preceding median ms A | B | Paired preceding/selected A / B |
| --- | ---: | ---: | ---: |
| tiny-noop | 0.619 / 0.735 / 0.735 | 0.548 / 0.616 / 0.639 | 1.006 / 1.032 |
| large-noop | 11.186 / 30.215 / 29.691 | 11.308 / 30.324 / 29.762 | 0.988 / 0.985 |
| expansion | 2.847 / 5.055 / 5.170 | 2.964 / 5.185 / 5.280 | 1.023 / 1.019 |
| 0 definitions / 100 updates | 0.868 / 2.486 / 2.413 | 0.842 / 2.540 / 2.532 | 0.942 / 0.991 |
| 64 definitions / 100 updates | 0.927 / 2.494 / 2.512 | 0.942 / 3.202 / 3.303 | 1.012 / 1.045 |
| 512 definitions / 100 updates | 1.257 / 3.322 / 3.174 | 1.283 / 3.358 / 3.399 | 0.965 / 1.003 |

Ratios above one favor selected rmake. Large-noop medians are roughly 2% higher
in both batches; expansion medians are about 2% lower. Runtime results are mixed,
including a roughly 5% higher 512-definition median in A and a lower median in B.
The full observations are retained without selecting only favorable samples.
GNU remains faster on these workloads. No general improvement is established.

Reports: [core A](benchmarks/passive-warning-core-a.json),
[core B](benchmarks/passive-warning-core-b.json),
[runtime A](benchmarks/passive-warning-runtime-a.json),
[runtime B](benchmarks/passive-warning-runtime-b.json).

The unselected entry-capture prototype's earlier reports remain separate:
[core A](benchmarks/passive-warning-prototype-core-a.json),
[core B](benchmarks/passive-warning-prototype-core-b.json),
[runtime A](benchmarks/passive-warning-prototype-runtime-a.json),
[runtime B](benchmarks/passive-warning-prototype-runtime-b.json).
Its [source identity and archive manifest](../tests/research/passive-warning-entry-prototype.json)
and [complete compressed original proof](../tests/research/passive-warning-entry-prototype.json.gz)
have checked compressed/uncompressed hashes. None earns selected-build credit.
