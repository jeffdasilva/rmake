# Compiled VPATH lookup policy

Selective VPATH declarations now compile their percent pattern once when read.
Lookups borrow matching directory slices through an ordered iterator. The
iterator stops after a successful filesystem lookup instead of materializing
all later matching entries. The original directive key remains available for
clearing declarations, including percent quoting. Parser-owned private state
keeps the compiled matcher and directory list consistent across eval, includes
and Makefile restarts.

Only pure pattern policy is stored. Directory contents, existence, timestamps,
member dates, variable expansions and effects remain outside this structure.
Filesystem lookups retain their original order and remain fresh. This does not
implement persistent dependency-graph caching.

`make bench-compare BENCH_SUITE=vpath-search BENCH_REPEAT=101` exercises 1,000
ordinary prerequisites against 64/256 unmatched patterns, a first-hit case with
64 matching declarations, and 64 archive members through 64 unmatched patterns.
The suite prepares real files before timing. Its archive uses a distinct basename
so prior benchmark inputs cannot satisfy it through a local archive. All engines
receive identical paths/environment/arguments, and exact outputs are checked.

A local alternating paired comparison against the preceding `ecdcdbb` build:

| Workload | rmake (ms) | Previous rmake (ms) | GNU 4.4.1 (ms) | Median paired previous/current |
| --- | ---: | ---: | ---: | ---: |
| 64 unmatched patterns | 2.429 | 5.934 | 2.531 | 2.464 |
| 256 unmatched patterns | 3.032 | 18.935 | 3.618 | 6.282 |
| First hit, 64 matching declarations | 2.319 | 4.881 | 2.198 | 2.149 |
| Archive, 64 unmatched patterns | 1.294 | 1.571 | 1.370 | 1.213 |

These selected lookup workloads improve over the prior rmake build. The measured
rmake medians beat GNU in three of the four; GNU remains slightly faster in the
first-hit case. This does not establish broad GNU performance parity. A separate
101-sample core run measured startup at 0.681 ms (previous 0.678 ms), 10,000 no-op
targets at 28.724 ms (28.583 ms), and expansion at 4.542 ms (4.675 ms). Core
ratios were 0.995, 0.995 and 1.034; these small differences are not attributed to
the VPATH optimization, and GNU remains faster on those three workloads.

Samples include startup, two warmups per engine and warm OS caches, without CPU
isolation, resource profiling or confidence intervals. Benchmark runs were
sequential, outside functional validation. Raw samples and identities remain in
ignored `artifacts/bench/compiled-vpath-101.json` and
`artifacts/bench/compiled-vpath-core-101.json`. Current results do not cover all
VPATH shapes, cold-cache I/O or the full compatibility workload set.

Nine original required fixtures protect declaration order, clear operations,
literal percent quoting, expansion timing, eval, includes and byte-valued names.
They pass on both the preceding build and the optimized build. The benchmark
workload test checks source identities and actual prepared files. Full development
gates and optimized conformance pass with 2,813 corpus cases: 2,801 required
matches and 12 existing pending mismatches. Stateful coverage remains 50 scenarios
/ 133 invocations. No upstream code or tests were copied. M4/M8 remain open; see
[archive VPATH](archive-vpath.md) for the resolution scope and remaining work.
