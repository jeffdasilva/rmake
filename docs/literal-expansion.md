# Literal expansion throughput

The expression evaluator copies contiguous literal bytes in one bounded append
instead of growing and checking the result for every byte. Dollar references
still run in their original order. An unmatched final dollar remains literal.
This uses safe slice iteration and does not cache results or skip expansions.

The existing input/result NUL boundaries remain unchanged: a NUL in source ends
the input, while a NUL produced by a file read does not suppress later effects
within that expansion. The existing file/effect suites exercise this distinction.
A resource-boundary test also checks the exact output limit, effects before and
after that boundary, and a terminal dollar at the limit.

Thirty-one alternating release pairs after three warmups compared production
source `88a3f03` with this change. Runs used the same executable alias, working
directory, environment and arguments, and checked exact output and status.
The following are median paired elapsed-time ratios, after/before:

| Workload | Ratio |
| --- | ---: |
| Tiny no-op | 1.004 |
| 10,000 explicit targets | 0.970 |
| 5,000 assignments | 0.999 |
| Large word list, prefix | 0.808 |
| Large word list, middle | 0.812 |
| Large word list, full | 0.875 |
| Large word list, outside range | 0.812 |
| `let` remainder | 0.811 |
| Literal `patsubst` | 0.902 |

These selected large-text workloads took about 10–19% less elapsed time. This is
an unisolated wall-time observation without confidence intervals or CPU/RSS/I/O
measurements, not a claim of general superiority to GNU Make. The permanent
workloads are available through `make bench-compare BENCH_SUITE=all`; the local
before/after report is `artifacts/bench/literal-runs-before-after.json`.
