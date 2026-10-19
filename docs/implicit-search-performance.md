# Avoiding irrelevant implicit-rule comparisons

Candidate selection now matches each pattern before checking whether later
identical definitions replace or cancel it. An irrelevant pattern cannot affect
that target's candidates or match-anything suppression. This removes the repeated
scan through later definitions for nonmatching patterns. The target's directory
prefix is also computed once per search instead of once per rule. Both operations
are pure: prerequisite expansion, filesystem observations, rule order, cancellation,
terminal behavior, candidate planning and invalidation retain their existing
boundaries. No persistent cache or timestamp reuse is introduced.

`make bench-compare BENCH_SUITE=implicit-search` measures two original workloads
with 400 resolved targets and respectively 64 or 256 irrelevant patterns. Empty
recipes exercise resolution without process launches. `BENCH_BASELINE=/path/to/old`
adds an optional previous rmake to the same alternating, output-checked campaign.
Binary identities, workload hashes, raw samples and paired ratios are retained.
The default core suite is unchanged; `BENCH_SUITE=all` includes this new suite.

A local optimized-build campaign used 31 measured samples per engine after two
warmups, with the same executable alias, directory, environment and arguments.
All outputs and statuses matched the pinned GNU Make 4.4.1 oracle.

| Irrelevant patterns | Previous rmake median | Current rmake median | GNU median | Median paired previous/current |
| --- | --- | --- | --- | --- |
| 64 | 2.495 ms | 1.538 ms | 0.935 ms | 1.64 |
| 256 | 13.645 ms | 2.222 ms | 1.427 ms | 6.08 |

The current rmake is faster than the previous rmake on these workloads. GNU
remains faster on both. These are synthetic, warm-cache timings including startup;
there was no CPU pinning/isolation, confidence interval, or performance acceptance
threshold. They do not establish a general speed advantage or complete compatibility.
The local ignored evidence is `artifacts/bench/implicit-filter-comparison.json`.
The measured binaries were:

- Previous: `f0a951cc3ac7130fe2d26903db64c7e98c50ae8dc0b4415dda9e4c8af49b88f7`
- Current: `29779eae1860f50de5779de0e1bf29a3e464466d3839bde1471bed1d6a0853d0`
- GNU: `d129996c7f842a77375573eae5b188a6e24510a076658df5e0dd9ffd92571625`
