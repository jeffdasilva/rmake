# Avoid unused prepared-startup copies

Prepared startup already owns the original invocation used to reconstruct a
Makefile restart. The engine now retains an Options snapshot only for the legacy
startup path that actually reads it. Startup preparation also avoids rebuilding
a second set of command-variable names when every assignment has already been
processed. Remaining legacy definitions still build the membership set and keep
the existing deduplication behavior. No evaluation, filesystem lookup or command
effect is cached or suppressed.

This is a bounded allocation improvement, not full startup unification or a new
compatibility feature. The ordinary corpus remains 10,710 required cases, zero
pending, with 17 separately gated approved safe-error exceptions. Full M3/M6 and
general performance parity remain open.

## Validation

Formatting, warnings-denied lint, 457 Rust and 419 tooling tests passed. All 463
combined startup controls match independent exact models and two pinned GNU
observations under both frozen builds. Debug SHA256:
`eb12da013efed7762a59977a94902abd526938950b8977d93fd0702ebdddae7c`.
Release SHA256:
`dd87a34e8184ca62175d933abcb47667a500e201cb125ac589bca62b24ca4ca6`.
Full frozen rmake-driven development/optimized acceptance and exact-head CI are
required before merge; the PR records their final outcomes.

Six sequential reports contain 101 alternating, exact-output-checked samples per
engine. No build, test or probe campaign ran concurrently; host activity was not
controlled. Small changes are mixed and no statistical or general speedup is
claimed. The legacy large-noop workload does not exercise the prepared path, so
its lower observed median cannot establish a causal improvement from this change.
GNU remains faster on several common workloads.

| Workload | GNU ms | Current ms | Preceding rmake ms |
| --- | ---: | ---: | ---: |
| tiny-noop | 0.547 | 0.665 | 0.659 |
| large-noop | 11.431 | 29.128 | 29.893 |
| expansion | 2.870 | 4.921 | 4.984 |
| startup-arguments-0 | 0.659 | 0.778 | 0.777 |
| startup-arguments-1000 | 2.130 | 1.982 | 1.986 |
| startup-arguments-5000 | 25.923 | 8.327 | 8.268 |
| parallel-branches-depth-1 | 88.039 | 90.399 | 90.501 |
| parallel-branches-depth-4 | 88.021 | 90.384 | 90.484 |
| eval-arguments-0 | 0.731 | 0.886 | 0.895 |
| eval-arguments-1000 | 3.090 | 2.033 | 2.002 |
| startup-effects-0 | 0.677 | 0.769 | 0.763 |
| startup-effects-1000 | 2.532 | 2.985 | 3.045 |
| initial-flags-0 | 0.652 | 0.773 | 0.786 |
| initial-flags-1000 | 2.585 | 2.881 | 2.943 |

Raw timing reports: [core](benchmarks/startup-retention-core.json), [startup](benchmarks/startup-retention-startup.json), [parallel](benchmarks/startup-retention-parallel.json), [eval](benchmarks/startup-retention-eval.json), [effects](benchmarks/startup-retention-effects.json), [inherited](benchmarks/startup-retention-inherited.json).

Two executions per engine and workload repeat these exact glibc allocation
counters, using equal-length trace/helper paths:

| Request metric | Preceding | Current |
| --- | ---: | ---: |
| effects: calls | 28,747 | 26,743 |
| effects: requested bytes | 2,050,289 | 1,912,513 |
| inherited: calls | 28,824 | 26,818 |
| inherited: requested bytes | 2,307,088 | 2,177,261 |

These are 2,004 and 2,006 fewer requests (about 7 percent). Requested bytes fall
by 137,776 (about 6.7 percent) and 129,827 (about 5.6 percent), respectively.
The profiler observes glibc malloc/calloc/realloc/free after initialization,
excludes aligned entry points and measures requested bytes rather than live or
peak memory. Sampled stacks are hints, not CPU measurements. The trace prefix
differs from the previous report; do not sum byte deltas across reports.

Raw allocation observations: [effects/current/0](benchmarks/startup-retention-alloc-effects-current-0.json), [effects/current/1](benchmarks/startup-retention-alloc-effects-current-1.json), [effects/priorxx/0](benchmarks/startup-retention-alloc-effects-priorxx-0.json), [effects/priorxx/1](benchmarks/startup-retention-alloc-effects-priorxx-1.json), [inherited/current/0](benchmarks/startup-retention-alloc-inherited-current-0.json), [inherited/current/1](benchmarks/startup-retention-alloc-inherited-current-1.json), [inherited/priorxx/0](benchmarks/startup-retention-alloc-inherited-priorxx-0.json), [inherited/priorxx/1](benchmarks/startup-retention-alloc-inherited-priorxx-1.json).

## Include-metadata work identified next

[Original research](../tests/research/include-metadata-phases.json) retains 19
modeled cells, two pinned GNU observations and one frozen debug observation per
cell, with exact bytes, file snapshots, directory inputs, binary identities and
bounds. Thirteen finite mismatches remain ordinary compatibility work with zero
conformance, curated pending or safety-exception credit. Six controls match.

The gaps cover defaults and command identity visible too early during
.INCLUDE_DIRS append evaluation, imported SHELL origin, shell-function makelevel,
full eval effects while the old include search is still active, and passive late
GNUMAKEFLAGS include switches. GNU deliberately evaluates ordinary append
semantics here; treating directory names as literal strings would suppress real
effects. The new resolved search is published after metadata evaluation, so a
nested include can still use the preceding search.

One initial MAKE_COMMAND model assumed the default already existed. Source review
confirmed that its definition follows include construction. The refined model
matches both GNU observations and exposes a candidate mismatch; the original
incomplete model is retained. No expected result was automatically changed to
match candidate behavior. These inputs are not passing fixtures.

GNU Make 4.4.1 main.c/read.c were consulted from the verified archive, SHA256
`dd16fb1d67bfab79a72f5e8390735c49e3e8e70b4945a15ab1f81ddb78658fb3`.
GNU source is copyright Free Software Foundation, Inc., GPL-3.0-or-later. Rust,
models and tooling are original; no upstream implementation or tests were copied
or translated. No unsafe Rust or GNU execution fallback is added.
