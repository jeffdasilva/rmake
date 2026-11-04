# Native-runtime measurement evidence

The retained reports show mixed overhead changes, small improvements from a lookup change, and further mixed results from the source-name provenance correction. They do not establish general GNU Make performance parity. These controls use ordinary core and runtime-argument workloads without loading a native module; native callback throughput and module compilation/loading costs are outside these measurements.

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


The subsequent source-name provenance correction is measured separately in
[provenance-performance.json](../../tests/native/provenance-performance.json).
It preserves all three original timing reports and all 16 allocation reports and
raw traces. The frozen candidate is 60c52926ffd2, compared with the historical
lookup build 4866fc922134; its source/input hash reference is the immutable
[184-model qualification archive](../../tests/native/qualification-184.json).
These are historical executable identities, independent of later working-source
edits. The archive also retains the profiling helper source and its hash.

All three timing campaigns inherited shared CPU23 affinity. Core and nested-eval
controls have 101 samples per engine; runtime controls have 301. The raw reports'
generic limitations remain unchanged, with the affinity recorded separately in
the archive wrapper. The same timing and isolation limitations apply. The
following medians are milliseconds; paired time changes use the same formula
as above and compare this frozen candidate with the lookup baseline.

| Workload | GNU median | Lookup median | Provenance median | Paired provenance time change |
| --- | ---: | ---: | ---: | ---: |
| tiny-noop | 1.719 | 1.827 | 1.826 | +0.07% |
| large-noop | 11.836 | 31.802 | 30.981 | -1.55% |
| expansion | 3.682 | 6.098 | 5.939 | -2.19% |
| nested-evals-0 | 2.032 | 2.200 | 2.188 | -0.19% |
| nested-evals-32 | 3.134 | 4.867 | 5.005 | +3.41% |
| nested-evals-96 | 5.283 | 11.409 | 11.892 | +3.79% |
| runtime-arguments-0-updates-0 | 1.660 | 1.781 | 1.770 | -0.26% |
| runtime-arguments-0-updates-100 | 2.464 | 3.055 | 3.025 | -0.87% |
| runtime-arguments-64-updates-0 | 1.769 | 1.983 | 1.976 | -0.17% |
| runtime-arguments-64-updates-100 | 2.491 | 3.121 | 3.117 | -0.21% |
| runtime-arguments-512-updates-0 | 2.004 | 2.421 | 2.419 | +0.78% |
| runtime-arguments-512-updates-100 | 2.567 | 3.360 | 3.371 | +0.45% |

Large no-op and expansion show 1.55% and 2.19% lower paired times in this
comparison. Nested evals at depths 32 and 96 are 3.41% and 3.79% slower, despite
fewer allocation calls. Runtime observations are mixed: four controls show small
reductions and both 512-argument controls show increases. Every GNU median in
these reports remains lower than the candidate median. These observations do
not establish general performance parity or a cumulative improvement across
separate campaigns.

The allocation profiles are separate preload-helper observations, with two
repeats per build and workload. Each repeat validates the fixture inputs,
arguments, environment and expected output. Both repeats have identical
calls/frees/requested-byte summaries for each build/control. Complete reports
and traces remain individually retained, including sampled-caller ordering
variation. The CPU23 timing-affinity statement does not apply to these profiles.

| Allocation control | Lookup calls | Provenance calls | Lookup requested bytes | Provenance requested bytes |
| --- | ---: | ---: | ---: | ---: |
| profile-nested-evals-0 | 1,409 | 1,409 | 77,978 | 78,046 |
| profile-nested-evals-32 | 52,814 | 49,614 | 3,570,601 | 3,519,469 |
| profile-nested-evals-96 | 155,610 | 146,010 | 10,561,809 | 10,408,277 |
| runtime-arguments-512-updates-100 | 20,064 | 20,064 | 1,179,257 | 1,179,333 |

The nested controls save 3,200 and 9,600 calls. The zero-depth and runtime controls
retain the same call counts while requesting 68 and 76 additional bytes. These
counts cover glibc malloc/calloc/realloc/free after helper initialization and
exclude aligned allocation entry points. Requested bytes are allocation requests,
not live or peak memory or RSS; stride samples are call-stack hints, not elapsed
or CPU time. Compiled helper libraries are excluded from the retained archive.


The final measured change borrows existing source locations during expansion
and evaluation, avoiding redundant location clones. Its frozen release is
bb18d1f7f410. [borrowed-performance.json](../../tests/native/borrowed-performance.json)
retains all four original timing reports and four allocation reports with their
raw traces and helper bytes/hash. It links the immutable source/input snapshot
and both passing 184-model builds in
[qualification-borrowed-184.json](../../tests/native/qualification-borrowed-184.json).
The preceding main still fails all 184 native profiles; this separate profile
remains distinct from full developer and ordinary conformance acceptance.

All four timing campaigns inherited shared CPU23 affinity. Core controls use 101
samples per engine; both nested comparisons and runtime controls use 301. The
original limitations, paired-time formula and millisecond median units remain
unchanged. No confidence intervals or general performance acceptance follow from
these measurements. Each table below is its own controlled comparison; the
results must not be combined into a cumulative percentage.

The following reports compare the borrowed-location build with the historical
lookup build 4866fc922134, before the source-name provenance correction.

| Workload | GNU median | Lookup median | Borrowed median | Paired borrowed time change |
| --- | ---: | ---: | ---: | ---: |
| tiny-noop | 1.666 | 1.865 | 1.837 | +0.40% |
| large-noop | 12.287 | 32.669 | 32.937 | +1.36% |
| expansion | 3.604 | 6.075 | 5.773 | -4.90% |
| nested-evals-0 | 2.040 | 2.216 | 2.220 | -0.04% |
| nested-evals-32 | 3.050 | 4.897 | 4.860 | -0.75% |
| nested-evals-96 | 4.984 | 11.501 | 11.286 | -1.97% |
| runtime-arguments-0-updates-0 | 1.639 | 1.767 | 1.771 | +0.14% |
| runtime-arguments-0-updates-100 | 2.449 | 3.039 | 3.028 | +0.02% |
| runtime-arguments-64-updates-0 | 1.780 | 1.999 | 2.000 | +0.03% |
| runtime-arguments-64-updates-100 | 2.522 | 3.205 | 3.214 | +0.23% |
| runtime-arguments-512-updates-0 | 2.021 | 2.450 | 2.459 | +0.04% |
| runtime-arguments-512-updates-100 | 2.543 | 3.377 | 3.384 | +0.31% |

Core results are mixed: tiny and large no-op take 0.40% and 1.36% more paired time,
while expansion takes 4.90% less. Nested depths 32 and 96 take 0.75% and 1.97% less
paired time than this pre-provenance baseline. All six runtime controls take
slightly more paired time, from 0.02% to 0.31%. Independent medians and paired
ratios differ because they summarize different observations; all raw samples
remain retained.

The separate nested comparison below uses provenance build 60c52926ffd2 as its
baseline, directly measuring the subsequent borrowing change.

| Workload | GNU median | Provenance median | Borrowed median | Paired borrowed time change |
| --- | ---: | ---: | ---: | ---: |
| nested-evals-0 | 2.271 | 2.466 | 2.481 | +0.31% |
| nested-evals-32 | 3.042 | 4.970 | 4.854 | -2.34% |
| nested-evals-96 | 4.991 | 11.636 | 11.167 | -3.86% |

Depths 32 and 96 take 2.34% and 3.86% less paired time than the provenance build;
the zero-depth control takes 0.31% more. Every GNU median in both final
comparisons remains lower than the candidate median. The selected improvements
and regressions establish neither general speedup nor GNU performance parity.

Two repeats of each final allocation control have identical
calls/frees/requested-byte summaries. The corresponding earlier controls are
retained in the immutable provenance-performance archive.

| Frozen build | Zero-depth calls | Zero-depth requested bytes | Depth 96 calls | Depth 96 requested bytes |
| --- | ---: | ---: | ---: | ---: |
| Lookup | 1,409 | 77,978 | 155,610 | 10,561,809 |
| Provenance | 1,409 | 78,046 | 146,010 | 10,408,277 |
| Borrowed | 1,409 | 78,014 | 146,010 | 10,408,245 |

Borrowing retains the provenance build's call counts and removes 32 requested
bytes in each control. The depth 96 count remains 9,600 calls below the lookup
build; zero-depth calls remain unchanged across all three. Requested bytes at
zero depth remain 36 above the lookup baseline. The earlier helper limits still
apply: requested bytes are not live/peak memory or RSS, and sampled call stacks
are not timing measurements. CPU23 timing affinity is not asserted for these
allocation profiles; compiled helper libraries are excluded from the archive.
