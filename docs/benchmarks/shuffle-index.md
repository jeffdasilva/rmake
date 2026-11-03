# Shuffle opt-out performance controls

These output-checked observations use the selected frozen optimized implementation
with shuffle disabled, the preceding main implementation and pinned GNU Make.
Each elapsed report has 101 alternating samples per engine. A uses the GNU
top-level driver; B uses frozen rmake. All local validation campaigns had exited
before measurements began. Host activity is not controlled. These are workload
observations, not statistical or general performance acceptance.

| Core input | GNU A / B ms | Selected A / B ms | Preceding A / B ms |
| --- | --- | --- | --- |
| Tiny no-op | 0.745 / 0.705 | 0.900 / 0.830 | 0.897 / 0.851 |
| 10,000 explicit targets | 11.821 / 11.858 | 33.704 / 33.476 | 33.999 / 33.747 |
| Expansion | 3.103 / 3.171 | 5.484 / 5.536 | 5.406 / 5.428 |

Large-noop medians are about 0.8% lower and expansion medians 1.4–2.0% higher
than preceding rmake. The largest runtime-argument/update input is 0.3% and 4.0%
higher in A/B; other runtime inputs are mixed. These results establish no general
improvement; GNU remains faster. All samples, identities, source hashes and exact
output checks remain in [core A](shuffle-core-a.json),
[core B](shuffle-core-b.json), [arguments A](shuffle-runtime-arguments-a.json)
and [arguments B](shuffle-runtime-arguments-b.json).

Two repeated glibc allocation-request observations per engine have unchanged
request counts: 244,699 for large-noop and 66,074 for expansion. Selected requested
bytes are respectively 20,327,351 and 2,659,032, versus preceding 20,327,195 and
2,659,004. This is not retained/peak memory, full allocator coverage or CPU time.
Each directory retains its report and unmodified sampled stack trace:

| Input | Preceding 1 / 2 | Selected 1 / 2 |
| --- | --- | --- |
| Large-noop | [1](shuffle-alloc-preceding-large-noop-1/report.json) / [2](shuffle-alloc-preceding-large-noop-2/report.json) | [1](shuffle-alloc-selected-large-noop-1/report.json) / [2](shuffle-alloc-selected-large-noop-2/report.json) |
| Expansion | [1](shuffle-alloc-preceding-expansion-1/report.json) / [2](shuffle-alloc-preceding-expansion-2/report.json) | [1](shuffle-alloc-selected-expansion-1/report.json) / [2](shuffle-alloc-selected-expansion-2/report.json) |
