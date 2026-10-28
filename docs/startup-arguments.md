# Single-pass explicit startup arguments

When decoded inherited MAKEFLAGS/GNUMAKEFLAGS contain no tokens, there is no
option-source merge to perform. Parse the explicit arguments once and retain the
normal explicit job count and authentication finalization. Invocations with
inherited tokens keep their existing merge path. Both paths share the same
jobserver finalization helper, including explicit-job reset behavior.

This removes the redundant explicit parse and argument copies. Assignment
classification, byte handling, option order and later assignment evaluation use
the same decoder and interpreter. The optimization does not cache environment
values, suppress assignments or change when their effects run.

## Acceptance and developer flow

Add three original required Makefiles with 0, 1,000 and 5,000 explicit overrides.
Each invocation prints every value in argument order. Two GNU Make 4.4.1
observations, the candidate and preceding release match exact output, status and
filesystem observations. The middle of the large output has an independent
negative mutation guard, so an implementation cannot pass by dropping arguments.
Two tooling tests protect the benchmark workload and exact oracle anchor.

The curated corpus is 8,968 total / 8,967 required / one pending. Rust totals
remain 440; tooling totals are 359. Full development/optimized acceptance, driven
by rmake, and exact-head CI remain merge gates. The [M3/M6 audit](m3-m6-audit.md)
retains the incomplete compatibility inventory and broader exit criteria.

The same workload is available through the top-level Makefile:

```sh
make bench-compare BENCH_SUITE=startup-arguments BENCH_REPEAT=101 \
  BENCH_BASELINE=/absolute/path/to/previous-rmake
```

Set `BENCH_CANDIDATE` to a frozen binary when needed. `rmake` supports the same
flow. Every timed invocation observes every override value; there are no recipe
subprocesses in these workloads. The existing benchmark runner retains paired
samples, output comparisons, binary identities and input arguments.

## Measurements

An alternating 101-sample comparison of frozen checkout-built binaries recorded
these medians. No builds or acceptance runs were concurrent with timing; host
activity was not controlled.

| Workload | GNU | Current | Previous |
| --- | ---: | ---: | ---: |
| 0 overrides | 0.707 ms | 0.846 ms | 0.846 ms |
| 1,000 overrides | 2.591 ms | 2.375 ms | 2.406 ms |
| 5,000 overrides | 25.921 ms | 8.216 ms | 8.637 ms |
| Tiny no-op | 0.522 ms | 0.614 ms | 0.617 ms |
| 10,000-target no-op | 12.457 ms | 32.249 ms | 32.397 ms |
| 5,000 expansions | 2.781 ms | 4.776 ms | 4.729 ms |
| Parallel branches, depth 1 | 88.124 ms | 90.647 ms | 90.488 ms |
| Parallel branches, depth 4 | 88.240 ms | 90.454 ms | 90.691 ms |

The 5,000-override median is about 4.9% below the previous candidate and about
3.2 times faster than GNU in this specific workload. The expansion and one
parallel median are slightly higher than the preceding candidate. These
unisolated measurements do not establish statistical improvement/regression or
general performance parity; GNU remains faster on the standard workloads.
Raw reports retain every sample: [arguments](benchmarks/startup-arguments.json),
[core](benchmarks/startup-core.json), [parallel](benchmarks/startup-parallel.json).
Current SHA256:
`87eeb31f68fd9b9331e70db6b186b5dd3f2c13c8f767dd0282c7f5bdf7d4aac0`;
previous:
`7036659e3319b763f484abe0fdda2dca52c0aa760a4816d37f0d7cd46c3c914c`.

All code, workload definitions and fixtures are independently authored. GNU is
an external reference, never an implementation fallback.

The full local run exposed a pre-existing [terminal capture race](terminal-capture.md). The collector now drains completed streams before reusing a PTY, with deterministic regressions and unchanged raw expectations/timeouts. The product binaries and performance samples are unchanged.
