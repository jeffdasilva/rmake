# Recipe-free eval allocation profiles

The allocation observer now accepts original nested-eval controls at depths 0,
32 and 96. Each performs 100 independently observed reads, then uses an empty
phony rule. The expected output includes all 100 `LEAF` lines and GNU's empty-rule
status message. This keeps a child shell from opening and overwriting the
observer trace. The existing exact source/argument/environment/output checks
apply before a profile is accepted.

Use the existing top-level flow:

```
make profile-allocations-frozen PROFILE_CANDIDATE=/absolute/path/rmake PROFILE_WORKLOAD=profile-nested-evals-96
```

Three new original required Makefiles bring the ordinary corpus to 15,396
required / zero pending, with 17 unchanged separately gated safe-error exceptions.
Two pinned GNU Make 4.4.1 observations agree with each independent model; both
frozen product builds match all three. The runtime source is unchanged from
PR #142 (head `398ea18743d969bea0837532dc4b421eeba97686`). The rebuilt product
release matches the unchanged-parent build control byte for byte. Formatting,
lint and 509 Rust tests pass. Three new tooling tests bring tooling coverage to
510; they protect frozen observations, every-read models with negative controls
and profiling evidence. Full exact-head CI remains the merge gate.

[Raw evidence](../tests/research/eval-allocation-profiles.json) keeps the product
qualification separate from rejected performance experiments. Repeated controls,
36 source-I/O profiles per experimental build and prototype observations receive
no additional ordinary-conformance or safe-exception credit. Historical source
inputs, arguments, environments, output/status/filesystems, bounds, executable
identities and source-evidence hashes remain intact. Runtime source, dependencies
and the allocation C observer are unchanged; no GNU implementation or tests were
copied or translated and no GNU runtime fallback was added.

## Rejected experiments

Several correct experimental runtimes passed all 269 preceding live-source
models plus the three profiling controls on both builds. They also passed all
36 source-I/O profiles per build. Candidate labels in those historical reports
identify experimental executables, not the product shipped by this change.

The experiments moved local active-variable keys into immutable shared eval
contexts and reclaimed unique storage afterward; retained snapshots took a
copy. Reader variants kept eval frames inline, used heap parent storage only
on suspension and isolated whole-frame temporaries from the hot reader loop.
A name-filter variant retained exact comparisons for every possible hit.
All experimental runtime changes were rejected after elapsed measurements.
The original patches and frozen observations remain available for review.

Two repeated glibc request profiles per engine/depth produced identical
summaries for the final experiment:

| Depth | Parent / experimental requests | Parent / experimental requested bytes |
|---|---:|---:|
| 0 | 1,407 / 1,407 | 77,347 / 77,362 |
| 32 | 52,812 / 43,212 | 3,569,970 / 1,334,185 |
| 96 | 155,608 / 126,808 | 10,561,178 / 3,851,793 |

At depths 32 and 96, requests fell about 18% and requested bytes about 63–64%.
These are experimental results. The observer excludes aligned allocations and
helper initialization; requested bytes are not live/peak memory, and sampled
stacks are hints rather than CPU timings. Path/environment representation also
affects startup requested bytes. Lower request totals did not establish a
repeatable elapsed gain.

The unchanged parent was also rebuilt in the same worktree, target directory
and pinned toolchain as the experiment. Its binary hash differs from the
published parent despite identical runtime source. Two 101-sample core batches
retain that build-control variation. Six further primary reports compare the
experiment against this same-worktree parent, with two batches of 101 alternating
samples per engine/workload and two warmups. GNU drives A; frozen experimental
rmake drives B through the top-level Makefile. All timed inputs are valid on GNU,
parent and experiment, with exact output agreement. No unrelated local build,
test or probe campaign ran during timing. Host activity remains uncontrolled;
there are no confidence intervals or performance thresholds.

| Workload | GNU / experiment / rebuilt parent median ms A | GNU / experiment / rebuilt parent median ms B | Paired parent/experiment A / B |
|---|---:|---:|---:|
| tiny-noop | 0.634 / 0.750 / 0.747 | 0.526 / 0.611 / 0.611 | 0.998 / 0.997 |
| large-noop | 10.671 / 28.806 / 28.941 | 10.929 / 29.362 / 29.597 | 1.004 / 1.004 |
| expansion | 2.655 / 5.046 / 4.885 | 2.819 / 5.275 / 5.133 | 0.972 / 0.976 |
| function-bodies-0 | 0.859 / 1.360 / 1.322 | 0.967 / 1.486 / 1.471 | 0.977 / 0.989 |
| function-bodies-16 | 3.582 / 7.181 / 6.937 | 3.739 / 7.575 / 7.289 | 0.970 / 0.962 |
| function-bodies-48 | 9.451 / 22.576 / 21.301 | 9.889 / 23.714 / 22.512 | 0.943 / 0.952 |
| nested-evals-0 | 0.628 / 0.846 / 0.824 | 0.632 / 0.826 / 0.838 | 0.980 / 1.009 |
| nested-evals-32 | 2.376 / 5.420 / 5.352 | 2.433 / 5.374 / 5.391 | 0.982 / 1.005 |
| nested-evals-96 | 5.109 / 12.298 / 12.279 | 4.968 / 11.777 / 11.798 | 1.002 / 1.000 |

Ratios above one favor the experiment. Results show repeated core-expansion and
function-body slowdowns and no dependable nested-eval elapsed improvement.
The shipping runtime therefore retains the validated PR #142 implementation.
No product allocation reduction, general speedup or performance parity is claimed.

A separate comparison against the originally published parent, both build-control
batches, four earlier prototype series and every allocation profile remain in
[the report index](benchmarks/eval-allocation-profiles-index.md). The intermediate
patches are original and uninstalled; [final experimental patch](../tests/research/eval-final-allocation-experiment.patch)
preserves the exact rejected runtime changes. Historical evidence receives no
extra conformance credit.

Complete [M3/M6](m3-m6-audit.md), remaining CLI/debug/database presentation,
native modules, enabled Guile, self-exec/stdin lifetime, broader Linux/locales/
integrations, general performance parity and persistent cache correctness remain
open. The pinned oracle/profile/source identities are preserved in the evidence.
