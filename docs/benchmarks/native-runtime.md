# Native-runtime measurement evidence

The retained reports show mixed overhead changes and small improvements from a later lookup change. They do not establish general GNU Make performance parity. These controls use ordinary core and runtime-argument workloads without loading a native module; native callback throughput and module compilation/loading costs are outside these measurements.

Every original report is retained byte-for-byte in [the benchmark archive](../../tests/native/benchmark-evidence.json), including raw samples, workload identities, output checks, executable digests and generic limitations. No negative or mixed report was discarded. The [147-model qualification archive](../../tests/native/qualification-147.json) separately retains both passing builds and the preceding build's failures.

The runs use GNU Make 4.4.1 on the recorded x86-64 Linux/WSL2/glibc profile, with Guile disabled and C-locale observations. Wall time includes process startup, two warmups per engine and warm OS page cache. Engine order alternates; each engine receives the same arguments, environment, path and checked output. Local campaigns were serialized.

| Reports | Comparison | Samples per engine/workload | Affinity |
| --- | --- | ---: | --- |
| benchmark-core-a/b.json | Initial native runtime versus preceding reader-only build | 101 | Unpinned |
| benchmark-runtime-a/b.json | Initial native runtime versus preceding reader-only build | 101 | Unpinned |
| benchmark-runtime-cpu-a/b.json | Initial native runtime versus preceding reader-only build | 301 | Inherited CPU23 |
| benchmark-lookup-core.json | Qualified lookup build versus initial native runtime | 101 | Inherited CPU23 |
| benchmark-lookup-runtime.json | Qualified lookup build versus initial native runtime | 301 | Inherited CPU23 |

The archive wrapper records inherited taskset affinity for CPU23. The original benchmark reports retain their generic no-pinning limitation, and benchmark-followups-environment.json remains unchanged. CPU23 was shared, with no frequency control or exclusive isolation. The runs have no confidence intervals, CPU/RSS/I/O counters, CI performance thresholds or representative full-project acceptance. Absolute times from different reports should not be combined.

The initial native build is identified by SHA256 prefix 97b22d41e8cd; the preceding reader-only build is c278e6725dcd; the qualified lookup build is 4866fc922134. Full digests are in each raw report. Packaging-time source hashes accompany the archives and are labeled separately from historical compilation provenance.

Initial unpinned core reports show paired candidate time changes of +0.08% to +2.90% against the preceding build. Runtime reports are mixed, ranging from -3.98% to +28.77%, with substantial movement between independent medians and paired observations. Both CPU23 follow-ups show initial-native overhead on every runtime control: +0.42% to +1.37% paired time. These earlier negative observations remain part of the evidence.

The later lookup comparison has the following core observations. Medians are milliseconds. The final column is the reciprocal of the median paired baseline/candidate ratio minus one, expressed as a time change; a negative value means less candidate time in that report.

| Workload | GNU median | Initial native median | Lookup median | Paired lookup time change |
| --- | ---: | ---: | ---: | ---: |
| tiny-noop | 1.706 | 1.816 | 1.808 | -0.40% |
| large-noop | 11.851 | 30.604 | 31.309 | +1.59% |
| expansion | 3.619 | 6.033 | 6.014 | -0.43% |

The large no-op control is slower in the lookup report: its medians are 30.604ms before and 31.309ms after, with +1.59% paired time. Tiny no-op and expansion show roughly 0.4% lower paired time. This is mixed evidence rather than a general core speedup.

| Runtime control | GNU median | Initial native median | Lookup median | Paired lookup time change |
| --- | ---: | ---: | ---: | ---: |
| 0 arguments / 0 updates | 1.629 | 1.752 | 1.747 | -0.33% |
| 0 arguments / 100 updates | 2.527 | 3.137 | 3.110 | -0.77% |
| 64 arguments / 0 updates | 1.801 | 2.028 | 2.017 | -0.74% |
| 64 arguments / 100 updates | 2.526 | 3.178 | 3.148 | -0.57% |
| 512 arguments / 0 updates | 1.983 | 2.416 | 2.397 | -0.65% |
| 512 arguments / 100 updates | 2.549 | 3.377 | 3.343 | -1.26% |

The six runtime controls show small paired reductions of 0.33% to 1.26% against the initial native build. Every GNU median in this lookup report remains lower. The changed baselines and affinity conditions prevent treating these reductions as a cumulative improvement over the preceding reader-only build. General performance parity, persistent caching and broader native/profile acceptance remain open.
