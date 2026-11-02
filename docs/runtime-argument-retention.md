# Retain invocation inputs without copying on live flag resets

Live MAKEFLAGS resets now move saved Makefile names, directories, goals,
command-line definitions, argument contexts and warning-position metadata into
the updated option state. These inputs already survive resets unchanged. The
move happens after fallible decoding, output-mode validation and fresh include
search construction succeed. No evaluator callback runs while the short mutable
option borrow is held; later nested callbacks observe the published state.

This removes repeated buffer copies. It does not cache expansions, include
metadata, filesystem observations, command results or dependency graphs. Fresh
directory observations, metadata evaluation, nested effects, warnings, output
policy and cancellation keep their existing paths. The core evaluator and CLI
decoder are unchanged; no unsafe code or dependency is added.

## Independent controls

Ten original required models agree with two bounded GNU Make 4.4.1 observations,
preceding rmake and both final frozen builds. Six recipe-free timing/profile
controls vary saved definitions (0/64/512) and live updates (0/100), observing
every value before and after, both requested goal names, an auxiliary Makefile
and every include-metadata effect. Four further reader/eval/include/recipe cases
actually execute both requested goals and check command-variable origin/flavor.
All streams, statuses and filesystem bytes/types/modes/links remain exact, with
no reached bounds or leftover children. These are already-compatible controls,
not newly fixed semantic mismatches. [Raw observations](../tests/research/runtime-argument-retention.json)
retain models, inputs, modes, arguments, environments and binary identities.

The `runtime-arguments` benchmark suite and allocation profiler use the same
required sources, arguments and auxiliary inputs. Six tooling tests protect
inventory/observations, complete output and input anchors, trace integrity,
profiler failures and uncredited warning discovery. Formatting, Clippy, 520 Rust
and 540 tooling tests pass. Standard top-level Makefile gates discover the cases;
full frozen rmake-driven `m5-acceptance` and exact-head development/optimized CI
remain merge gates, with final results recorded in the PR.

The corpus becomes 18,516 required / zero pending across 146 families, with the
same 17 separate approved safe-error exceptions. This does not complete M3/M6
or establish general GNU performance parity. The qualified reference remains
x86-64 Linux/glibc, C locale and disabled Guile. Profile ID:
`92525c9784f4dd471cceb12ce66a9d367e267b638e1384cdc29962b738d16de7`.
All Rust, Makefiles, observers and models are original; no GNU implementation or
upstream tests were copied or translated, and GNU is never a runtime fallback.

Frozen debug SHA256: `e9288b536c2ac443c4fc412610f8d0a87feb0b7efa19e62ccd4698dff11dc39a`.
Frozen release SHA256: `59008ffddff17ae10e6cee5613146a679fae47e5c3e90f186fbbe8496f8cfef0`.
Preceding release SHA256: `3ab6131d069f871016629667136d9cbabcff0ae7ba939d05ce4049f2aa8d2ca5`.

## Allocation observations

Two sequential observations per build/workload retain complete raw traces,
summaries, sampled addresses, source/helper/binary hashes and limitations in
[twelve profile reports](benchmarks/runtime-retention-profiles/). Each matching
pair has identical request/free/byte counts. glibc interposition counts selected
allocation requests after initialization; aligned entry points are excluded.
Requested bytes are not retained or peak memory, and samples are not CPU time.
The frozen binary paths differ; the four-byte zero-update difference receives
no optimization credit.

| Definitions / updates | Requests before / after | Requested bytes before / after |
|---|---:|---:|
| 0 / 0 | 525 / 525 | 62,492 / 62,488 |
| 0 / 100 | 10,393 / 9,593 | 546,156 / 531,752 |
| 512 / 100 | 72,844 / 20,544 | 4,092,341 / 1,181,537 |

The largest selected workload uses about 72% fewer observed requests and 71%
fewer requested bytes. This is a workload-specific allocation reduction.

## Elapsed observations

Six reports retain alternating engine order, 101 samples per engine/workload,
two warmups, exact output agreement and a same-worktree preceding control. GNU
drives A/C; frozen rmake drives B/D through the top-level Makefile. No unrelated
local build/test/probe campaign runs during measurement. Host activity is not
controlled; there are no confidence intervals, CPU/RSS/I/O observations or
statistical performance-acceptance claims. All measured inputs match before
optimization; there is no speed credit for accepting different behavior.

| Workload | Candidate / preceding median ms A | B | C | D |
|---|---:|---:|---:|---:|
| 0 definitions / 100 updates | 2.625 / 2.908 | 3.233 / 2.970 | 2.958 / 3.289 | 2.823 / 2.870 |
| 64 definitions / 100 updates | 3.066 / 3.331 | 3.306 / 3.597 | 3.529 / 4.346 | 2.453 / 3.221 |
| 512 definitions / 100 updates | 3.546 / 5.748 | 3.570 / 5.569 | 2.968 / 5.125 | 3.234 / 5.566 |

The 512-definition medians are about 34–42% lower than preceding rmake, with
paired preceding/candidate ratios of 1.586/1.502/1.676/1.700. GNU medians are
1.258–1.338 ms on that workload and remain faster. Zero-update controls are
close. The zero-definition B median is about 9% slower, which prompted C/D
instead of suppressing the result; those later medians are lower than their
controls. Smaller-workload results remain variable. No general speedup or
parity follows from these selected observations.

Core A/B candidate/preceding medians in ms are tiny-noop 0.787/0.788 and
0.642/0.652, large-noop 30.382/31.049 and 29.775/30.213, expansion 5.102/5.181
and 5.042/5.061. GNU remains faster. Raw reports: [core A](benchmarks/runtime-retention-core-a.json),
[core B](benchmarks/runtime-retention-core-b.json),
[arguments A](benchmarks/runtime-retention-runtime-arguments-a.json),
[arguments B](benchmarks/runtime-retention-runtime-arguments-b.json),
[arguments C](benchmarks/runtime-retention-runtime-arguments-c.json),
[arguments D](benchmarks/runtime-retention-runtime-arguments-d.json).

## Newly recorded warning gap

[Uncredited exploratory observations](../tests/research/live-warning-phase-discovery.json)
record three original live payload hypotheses. The late-warning model agrees
with two GNU runs: a later `--warn-undefined-variables` must not warn about an
earlier eager definition. Current rmake warns too early. Two other initial
models incorrectly predicted program-only diagnostic locations; GNU uses the
active Makefile line. Their original inputs, expectations and all observations
remain unchanged. No case is promoted, excused or given performance credit.

A complete fix must preserve ordered policy and diagnostic context through
nested flag resets, not merely overwrite the final warning bit. This remains
ordinary M3/M6 work alongside runtime HOME, the finite nested decoder mismatch,
startup usage/presentation, real exec/stdin, native loading, enabled Guile,
Linux/locale/integration acceptance and persistent-cache correctness.
