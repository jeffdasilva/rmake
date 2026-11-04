# BASIC child diagnostic measurements

The child diagnostic correction adds no allocation request/free calls on the
four selected disabled-debug controls. Requested-byte totals increase, while
the paired elapsed observations are modestly lower in this campaign. These
results establish no general speedup or GNU performance parity.

[performance-children.json](../../tests/debug-output/performance-children.json)
retains all three timing reports, sixteen allocation reports and sixteen raw
traces, with tool/helper bytes, hashes and exact workload inputs. The selected
frozen release is identified by SHA256 prefix `f974e8e99b87`; the preceding debug
build is `3c338225f894`. Complete identities and samples are in the archive.
[Qualification](../../tests/debug-output/qualification-children.json) separately
retains both builds passing 64 debug models and the preceding 62-pass/two-fail
control. Ordinary and full developer acceptance remain separate requirements.

This is the pinned x86-64 Linux/WSL2/glibc, C-locale, Guile-disabled profile with
GNU Make 4.4.1. These controls disable debug; enabled rendering throughput and
representative external builds are outside the measurements. Campaigns ran
sequentially after other local campaigns stopped and inherited shared CPU23
affinity. That CPU was not exclusive and its frequency was not controlled.
The generic benchmark limitations are preserved with actual affinity recorded
separately. There are no confidence intervals, CPU/RSS/I/O measurements or
performance acceptance thresholds.

Each timing report alternates engine order at identical paths, arguments and
environment, checking exact output. Core and startup controls use 101 samples
per engine; runtime controls use 301. These are whole-process wall times with
warm OS page caches. Medians below are milliseconds. The last column is the
reciprocal of the median paired preceding/candidate ratio minus one, expressed
as a percentage; negative means less candidate time. Independent medians and
paired changes can differ in sign.

| Workload | GNU median | Preceding median | Candidate median | Paired time change |
| --- | ---: | ---: | ---: | ---: |
| tiny-noop | 1.703 | 1.816 | 1.814 | -0.27% |
| large-noop | 11.766 | 30.962 | 30.940 | -0.67% |
| expansion | 3.669 | 5.973 | 5.911 | -1.06% |
| startup-arguments-0 | 1.590 | 1.683 | 1.678 | -0.42% |
| startup-arguments-1000 | 3.123 | 2.961 | 2.951 | -0.11% |
| startup-arguments-5000 | 26.582 | 8.479 | 8.449 | -0.52% |
| runtime-arguments-0-updates-0 | 1.688 | 1.829 | 1.813 | -0.49% |
| runtime-arguments-0-updates-100 | 2.476 | 3.105 | 3.082 | -0.69% |
| runtime-arguments-64-updates-0 | 1.850 | 2.091 | 2.073 | -0.74% |
| runtime-arguments-64-updates-100 | 2.605 | 3.299 | 3.280 | -0.74% |
| runtime-arguments-512-updates-0 | 1.997 | 2.427 | 2.420 | -0.43% |
| runtime-arguments-512-updates-100 | 2.548 | 3.372 | 3.350 | -0.82% |

Paired elapsed changes range from -0.11% to -1.06% in this report. The 1,000-
and 5,000-argument startup medians remain below GNU; GNU medians are lower on
the other controls. These small observations are not proof of a code-only
improvement. The [preceding debug campaign](debug-output.md), including every
regression, remains unchanged. Independent campaigns must not be pooled or
interpreted as cumulative percentage changes.

Allocation profiles use the original glibc preload helper with two repeats per
build/control. Each profile checks fixture/input/argument/environment and output.
Every build/control has identical summaries across its repeats.

| Control | Preceding calls / frees | Candidate calls / frees | Preceding requested bytes | Candidate requested bytes |
| --- | ---: | ---: | ---: | ---: |
| tiny-noop | 290 / 269 | 290 / 269 | 46,998 | 47,090 |
| large-noop | 244,702 / 234,750 | 244,702 / 234,750 | 20,852,138 | 20,852,230 |
| startup-arguments-1000 | 19,612 / 18,507 | 19,612 / 18,507 | 1,347,145 | 1,347,237 |
| runtime-arguments-512-updates-100 | 20,066 / 18,135 | 20,066 / 18,135 | 1,179,537 | 1,179,541 |

Calls/frees are unchanged, while requested bytes increase by 92 on tiny, large
and startup controls and four on runtime. Those increases are retained. Build
layout and injected profiler/output path costs are not independently isolated;
these totals should not be attributed to the diagnostic code alone.

Counts cover malloc/calloc/realloc/free after helper initialization and exclude
aligned allocation entry points. Requested bytes are allocation requests, not
retained/peak memory or RSS; sampled stacks are hints, not elapsed or CPU time.
CPU23 affinity applies only to elapsed campaigns. Compiled preload libraries
are excluded from archives. The [evidence index](../../tests/debug-output/README.md)
links complete discovery and previous observations. Persistent-cache correctness,
general performance parity and complete M3/M6 remain open.
