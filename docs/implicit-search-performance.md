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

## Follow-up with default-recipe support

Repeating the same 31-sample campaign after `.DEFAULT` and selected-pattern flag
support produced these measurements. Labels refer to the recorded binaries,
not future revisions; GNU remained faster on both workloads.

| Irrelevant patterns | Previous rmake median | Candidate median | GNU median | Median paired previous/candidate |
| --- | --- | --- | --- | --- |
| 64 | 2.648 ms | 1.655 ms | 0.976 ms | 1.63 |
| 256 | 13.872 ms | 2.346 ms | 1.469 ms | 5.92 |

Candidate SHA256: `282dafc48e02f72d6e339a9564165b9d9192f2714be020727289d4fdb7b3378d`.
Raw evidence: local ignored `artifacts/bench/implicit-filter-defaults.json`.

## Large command-line goal lists

Large invocations index their requested names for repeated membership checks
during implicit search and intermediate traversal. The engine keeps the ordered
operands for execution. A live GPATH alias still uses canonical goal names
during intermediate checks.

`make bench-compare BENCH_SUITE=explicit-goals BENCH_SILENT=1 BENCH_REPEAT=17`
uses four workloads with overlapping implicit prerequisites and empty recipes.
Set `BENCH_BASELINE` to a previous optimized build to compare it in the same
campaign. `BENCH_VALIDATE=1` checks the streams, exit status and filesystem state
on fresh inputs.

An alternating warm-cache campaign pinned all engines to the same CPU on a
shared host. The validation gate passed all four workloads.

| Goals | Prior rmake median | Indexed rmake median | GNU Make median |
| --- | --- | --- | --- |
| 64 | 4.211 ms | 3.688 ms | 2.276 ms |
| 512 | 23.851 ms | 16.592 ms | 5.404 ms |
| 2,048 | 177.197 ms | 71.528 ms | 23.833 ms |
| 8,192 | 2,495.696 ms | 368.488 ms | 136.377 ms |

The largest workload ran 6.8 times as fast as the prior rmake. GNU Make
remained faster in all four workloads. These measurements include startup.

- Prior binary SHA-256 `76d6f5edcdef96b01b273c572e24acb1fb84a83cb6a83445a1e3c50e66b0737d`
- Indexed binary SHA-256 `7902dddec53ffc45b46754481ef27a86f70cbd418e167c3fce0e1764401d5d7b`
- GNU binary SHA-256 `dc60c90b3fc6fd64be2c2459483de37b96b23a8a18068a95cd50ea7ab5eccd44`
