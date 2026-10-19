# Simple variable value copies

Expanding a simple binding now copies its stored bytes directly. It does not
first clone the whole binding and then copy that clone's value again. Recursive
values retain the existing owned-binding path, cycle checks, append-layer
handling and definition provenance. No cache or effect suppression is introduced.

The existing core and word-span benchmarks showed no consistent improvement:
their median paired after/before ratios ranged from 0.987 to 1.050 in one
unisolated run. A single lookup, file parsing and function scanning can obscure
the cost this change removes.

Four repeated-read workloads expose that cost separately. A simple value of
256 KiB or 1 MiB is referenced eight times per immediate assignment, with five
replacement assignments. Global bindings and `foreach` local bindings are both
measured. The largest result is 8 MiB, below the existing expansion limit.
`make bench-compare BENCH_SUITE=bindings BENCH_REPEAT=31` preserves these workloads
for future GNU/candidate comparisons; `all` includes them too.

Thirty-one alternating release pairs after three warmups compared source
`8081183` with the direct-copy path, using identical paths, inputs, environment,
arguments and exact output/status checks:

| Workload | Before median (ms) | After median (ms) | Paired after/before |
| --- | ---: | ---: | ---: |
| Global, 256 KiB | 7.370 | 5.697 | 0.776 |
| Local, 256 KiB | 8.013 | 7.261 | 0.916 |
| Global, 1 MiB | 20.500 | 16.007 | 0.782 |
| Local, 1 MiB | 27.514 | 25.051 | 0.902 |

This run observed about 9–22% less elapsed time on these selected repeated-read
workloads. A separate permanent-suite comparison used two warmups and 31 paired
trials against GNU Make 4.4.1. All four outputs and successful statuses matched.
rmake's global workloads remained slower than GNU (5.510 vs 4.258 ms and
15.515 vs 12.217 ms); its local workloads measured 7.968 vs 8.426 ms and
25.411 vs 27.478 ms. These are unisolated wall-time observations without
confidence intervals or CPU/RSS/I/O measurements, not general speed claims.

Local reports are `artifacts/bench/simple-copy-before-after.json`,
`simple-copy-repeated-before-after.json` and `simple-copy-oracle-bindings.json`.
Full compatibility and broader performance validation remain open.
