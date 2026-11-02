# Ordered live warning policy and diagnostic context

Live MAKEFLAGS warning-enable options now share the owned argument cursor with
payload definitions. The interpreter starts with the entry policy, visits each
definition or policy event in argument order, and publishes the state actually
reached. Nested decodes can enable warnings inside a definition or move the
cursor beyond a later warning flag. Unvisited flags do not become active.

The runtime receiver applies policy without decoding again, constructing another
include search or repeating effects. Quiet library closures retain their adapter;
warning-aware hosts implement the receiver's policy method. Positions are
transient results captured only for new live events. Initial parsing, pure
validation and serialized prefix options retain no policy-position vector.

Definitions retain invocation provenance; evaluation retains the active caller
location. This includes reader/eval/include locations and the selected recipe
locus during secondary expansion. Passive post-read definitions keep their
separate unlocated context. No unsafe code, dependency, expansion cache or
filesystem-observation cache is added. Cursor replay limits and safety errors
retain their existing boundaries.

## Independent evidence

1,213 required original Makefiles comprise 960 broad phase/order/assignment/
origin/priority/byte cases, 160 finite nested resets, 60 finite long-option cursor
skip/retain cases, 30 merged info/warning/file-order cases and the three exact
historical sources from [the original discovery](../tests/research/live-warning-phase-discovery.json).
Eager/simple, immediate-recursive, recursive and conditional definitions retain
their distinct timing. Stronger stored values do not suppress eager RHS effects;
pre-build override MAKEFLAGS can replay payloads after reading. Printed file
snapshots remain distinct from final file contents after that replay.

Every independent model agrees with two bounded GNU Make 4.4.1 observations.
Preceding rmake has 713 mismatches and 500 controls; both selected frozen builds
match exact output bytes, statuses and filesystem bytes/types/modes/links, without
reached bounds or leftover children. New programs use serial invocations; the
existing parallel/runtime gates remain required. [Raw evidence](../tests/research/live-warning-phases.json)
retains inputs, modes, arguments, environments, models, topologies and observations.

275 rejected hypotheses remain unchanged with zero credit: nine sources consumed
an eval quoting layer too soon, two used the prerequisite rather than recipe
locus, and 264 incorrectly skipped eager effects under stronger values or omitted
override replay. Separately named refinements model those phase rules. The earlier
two wrong discovery locations also remain unchanged; the exact three inputs now
have correctly anchored required cases.

Two Rust tests protect positions, prefix filtering, arity, policy order, active
locations and final activation. Six tooling tests protect the full inventory,
raw observations, exact historical inputs, bytes, replay witnesses and prototype
archive. Formatting, Clippy, 522 Rust and 546 tooling tests pass. The preceding
18,516-case corpus and all 68 safety cells passed during prototype development.
Full selected frozen rmake-driven `m5-acceptance` and exact-head development/
optimized CI remain merge gates; final outcomes are recorded in the PR.

The corpus becomes 19,729 required / zero pending across 147 families, with the
same 17 separately gated approved safe-error exceptions. No research record or
prototype adds extra conformance, timing or exception credit.

## Open boundaries

This orders warning events among live decoded payloads. General effects before
later malformed option values, complete passive GNUMAKEFLAGS payload ordering,
contextual runtime HOME, the finite nested getopt mismatch, startup usage,
directory/debug/database output, real exec/stdin, native loading, enabled Guile,
Linux/locale/integration acceptance and general performance parity remain ordinary
M3/M6 work. Persistent caching keeps its separate safety design.

The reference is x86-64 Linux/glibc, C locale and disabled Guile.

Later [passive policy](passive-warning-phases.md) and
[sequential source completion](passive-source-order.md) cover the post-read
ordering gaps left by this stage. Within-source malformed-value effects,
contextual HOME and the other ordinary acceptance boundaries remain open.

Profile: `92525c9784f4dd471cceb12ce66a9d367e267b638e1384cdc29962b738d16de7`.
Oracle SHA256: `b8806e8eb7452ed9b7ed5e18cb7e76ff9abd488c9d62a266a398d8044fade974`.
GNU definition phase order was consulted from the verified 4.4.1 archive,
SHA256 `dd16fb1d67bfab79a72f5e8390735c49e3e8e70b4945a15ab1f81ddb78658fb3`.
All Rust, models, fixtures and observers are original; no GNU implementation or
tests were copied or translated. GNU is never a runtime implementation fallback.

Selected debug SHA256: `a9cdc624d02e467361c71bc4137df186fb24e03d68ff74e2175b12687d67ed9c`.
Selected release SHA256: `fe4dbaf9f02214f179a97e200e75c72b4607bd9d21a1c2f5eed40f10d11c0264`.

## Prototype and elapsed observations

The first correct prototype stored positions on persistent Options and captured
prefix events before discarding them. Its then-current 1,153 models passed.
Its [source patch and hash manifest](../tests/research/live-warning-capture-prototype.json)
and [complete compressed raw proof](../tests/research/live-warning-capture-prototype.json.gz)
remain unchanged, with compressed/uncompressed hashes checked by tooling.

Twelve retained timing reports include four prototype reports and eight selected
reports. Each has 101 alternating samples per engine/workload, two warmups and
exact output agreement. Selected A/B use a same-worktree preceding control;
C/D compare the correct capture prototype. GNU drives A/C; selected rmake drives
B/D through the top-level Makefile. No unrelated local build/test/probe campaign
runs during timing. Host activity is uncontrolled; there are no confidence
intervals, CPU/RSS/I/O measurements or statistical acceptance claims. All timed
inputs already match on their controls; new fixes receive no speed credit.

| Workload | GNU / selected / preceding median ms A | B | Paired preceding/selected A / B |
|---|---:|---:|---:|
| tiny-noop | 0.699 / 0.838 / 0.840 | 0.533 / 0.639 / 0.638 | 1.010 / 1.005 |
| large-noop | 11.461 / 31.003 / 30.327 | 11.380 / 31.068 / 30.385 | 0.980 / 0.977 |
| expansion | 2.853 / 5.158 / 5.090 | 2.849 / 5.113 / 5.094 | 0.988 / 0.994 |
| 0 definitions / 100 updates | 0.927 / 2.374 / 2.244 | 0.876 / 2.654 / 2.523 | 0.964 / 0.991 |
| 64 definitions / 100 updates | 0.910 / 2.456 / 2.503 | 0.921 / 2.792 / 2.749 | 1.003 / 0.984 |
| 512 definitions / 100 updates | 1.278 / 3.198 / 3.175 | 1.291 / 3.438 / 3.416 | 0.986 / 0.928 |

Ratios above one favor the selected build. Large-noop is about 2% slower than
preceding rmake in both batches; some runtime medians are 5–6% higher, while
paired results vary. Direct prototype controls also remain mixed, including
roughly 1–2% higher large-noop medians. Removing bookkeeping did not eliminate
the small no-op slowdown and does not establish a speed advantage over the
correct prototype. This is a compatibility change, not a performance win. GNU
remains faster; general parity and warning-heavy measurements remain open.

Selected/preceding reports: [core A](benchmarks/live-warning-core-a.json),
[core B](benchmarks/live-warning-core-b.json),
[arguments A](benchmarks/live-warning-runtime-arguments-a.json),
[arguments B](benchmarks/live-warning-runtime-arguments-b.json).
Prototype controls: [core C](benchmarks/live-warning-prototype-control-core-c.json),
[core D](benchmarks/live-warning-prototype-control-core-d.json),
[arguments C](benchmarks/live-warning-prototype-control-runtime-arguments-c.json),
[arguments D](benchmarks/live-warning-prototype-control-runtime-arguments-d.json).
Original prototype: [core A](benchmarks/live-warning-capture-core-a.json),
[core B](benchmarks/live-warning-capture-core-b.json),
[arguments A](benchmarks/live-warning-capture-runtime-arguments-a.json),
[arguments B](benchmarks/live-warning-capture-runtime-arguments-b.json).
