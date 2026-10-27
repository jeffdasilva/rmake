# M5 runtime measurements

The final runtime-flags implementation was measured against the preceding
`356bc011764eff7a96bdd98c9f7c6ffb330be06c` product revision and the pinned GNU
Make 4.4.1 oracle. All three optimized executables were frozen. The Linux run
used 101 measured samples after two warmups, alternating engine order, with no
concurrent local build or conformance campaign. Each invocation had identical
paths, environment and arguments, and exact output and successful status were
checked before accepting its timing.

| Workload | GNU median ms | Current rmake ms | Previous rmake ms |
| --- | ---: | ---: | ---: |
| Tiny no-op | 0.675 | 0.785 | 0.783 |
| 10,000-rule no-op | 11.168 | 30.184 | 30.301 |
| 5,000 expansions | 2.782 | 4.584 | 4.440 |
| Parallel branches, depth 1 | 87.512 | 88.367 | 88.371 |
| Parallel branches, depth 4 | 87.147 | 87.696 | 87.937 |

The expansion median is about 3.2% higher than the preceding revision; the other
selected workloads are essentially unchanged or slightly faster. There is no
CPU isolation or confidence interval, so small differences are not established
performance gains. GNU remains faster on every selected workload. Parallel
sleep/process costs dominate that synthetic suite. These measurements do not
establish general parity or a representative real-project result.

The disabled output path creates no spool or owner-name allocation; an unchanged
runtime option receiver returns without cloning options, and recursive flag
quoting returns directly when no dollar is present. Persistent caching remains
M8 work, with separate effect, invalidation and corruption acceptance gates.

Raw samples, source hashes, arguments, platform and identities are retained in
the ignored local `artifacts/bench/m5-final-core.json` and
`artifacts/bench/m5-final-parallel.json`. Executable SHA-256 identities:

- Candidate: `40312a9372decf153e636048ae4120ec2954594efaf28140c8b26097a6c5d5a9`
- Previous rmake: `2e81c3b69b0fe714b29077dee83d296811acaad343d69e6fcfa9d98927e1f3ec`
- GNU: `b8806e8eb7452ed9b7ed5e18cb7e76ff9abd488c9d62a266a398d8044fade974`

Reproduce with frozen optimized binaries through the top-level Makefile:

```sh
make bench-compare BENCH_CANDIDATE=/path/to/current/rmake \
    BENCH_BASELINE=/path/to/previous/rmake BENCH_SUITE=core BENCH_REPEAT=101
make bench-compare BENCH_CANDIDATE=/path/to/current/rmake \
    BENCH_BASELINE=/path/to/previous/rmake BENCH_SUITE=parallel-branches BENCH_REPEAT=101
```
