# Nested-branch scheduling measurements

The `parallel-branches` benchmark suite measures sixteen independent branches,
with either one or four dependency levels above each 20 ms sleep recipe. Each
sample rebuilds the graph without file outputs. All engines receive `-rR`,
`--no-print-directory`, `-j4` and `--jobserver-style=pipe`. The command arguments
are retained in the report. This workload measures concurrency across dependency
branches, including process startup, rather than pure parser or expansion speed.

Reproduce a paired comparison through the top-level Makefile:

```sh
make bench-compare BENCH_SUITE=parallel-branches BENCH_REPEAT=31 \
    BENCH_BASELINE=/path/to/previous/rmake
```

A local Linux run compared the optimized candidate at `3c028811` with a separately
rebuilt optimized `9ba788fb` baseline and the pinned GNU Make 4.4.1 oracle. Each
engine had two warmups and 31 measured samples, with alternating engine order.
Exit status and exact output matched on every invocation.

| Dependency levels | Previous rmake median | Current rmake median | GNU median | Median paired previous/current |
| --- | ---: | ---: | ---: | ---: |
| 1 | 345.646 ms | 88.588 ms | 86.993 ms | 3.888x |
| 4 | 347.763 ms | 88.929 ms | 87.487 ms | 3.909x |

The improvement comes from using four available recipe slots across nested
branches. GNU remains slightly faster on these workloads. These measurements do
not establish general rmake superiority or complete compatibility. There was no
CPU pinning or isolation, no confidence interval, and no timing acceptance
threshold. Sleep/process costs dominate this deliberately small synthetic suite.
Functional marker, deep/shared graph and mixed-jobserver tests separately guard
the scheduling behavior and concurrency bounds.

Raw samples, Makefile hashes, arguments, platform, oracle profile and binary
identities are retained in the ignored local
`artifacts/bench/ready-branches-comparison.json`. The recorded SHA256 identities
are:

- Candidate: `a4e5c8e4e0a353121b434ba4782a2a7cc1627381feb599b2fbae622c716e9311`
- Rebuilt previous rmake: `5b994762ca4512903c9a70c0721e7bca3953ad84121978f48ccb0c9a0b31fee8`
- Oracle: `d129996c7f842a77375573eae5b188a6e24510a076658df5e0dd9ffd92571625`
