# JOBS lifecycle measurement evidence

The JOBS correction preserves allocation request/free counts on four selected
disabled-debug controls. Elapsed results are mixed, including a slower large
no-op control. These measurements establish no general speedup or GNU parity.

[performance.json](../../tests/job-debug/performance.json) retains all three
timing reports, sixteen allocation reports and sixteen raw traces with exact
inputs, tool/helper bytes and hashes. The selected frozen release is identified
by SHA256 prefix `46448db1f1a0`; its preceding BASIC-child control is
`f974e8e99b87`. Full executable identities and samples are in the archive.
[Qualification](../../tests/job-debug/qualification.json) separately records
both builds passing JOBS 9, deterministic debug 64, native 215 and directory 40.
Ordinary and full developer acceptance remain independent gates.

The profile is x86-64 Linux/WSL2/glibc, C locale, Guile disabled and the pinned
GNU Make 4.4.1 reference. Debug is disabled in these controls. Enabled rendering
throughput, native callbacks and the throughput benefit of returning a token to
a waiting recursive client are not measured here. The separate functional
FIFO/pipe witnesses prove token availability and conservation, not broad elapsed
performance.

Elapsed campaigns ran sequentially after other local campaigns stopped and
inherited shared CPU23 affinity. That CPU was not exclusive and its frequency
was not controlled. Generic benchmark limitations remain recorded, with actual
affinity described separately in the archive. There are no confidence intervals,
CPU/RSS/I/O counters or performance acceptance thresholds.

Reports alternate engine order at identical paths, arguments and environment,
checking exact output. Core and startup controls use 101 samples per engine;
runtime controls use 301. These are whole-process wall times with warm OS page
caches. Medians are milliseconds. The last column is the reciprocal of the
median paired preceding/candidate ratio minus one, expressed as a percentage;
negative means less candidate time. Independent medians and paired changes
summarize different observations and can differ in sign.

| Workload | GNU median | Preceding median | Candidate median | Paired time change |
| --- | ---: | ---: | ---: | ---: |
| tiny-noop | 1.705 | 1.816 | 1.819 | +0.07% |
| large-noop | 11.999 | 30.777 | 31.002 | +0.80% |
| expansion | 3.623 | 5.875 | 5.851 | -0.49% |
| startup-arguments-0 | 1.601 | 1.691 | 1.693 | +0.09% |
| startup-arguments-1000 | 3.131 | 2.968 | 2.932 | -1.13% |
| startup-arguments-5000 | 26.652 | 8.625 | 8.585 | -1.20% |
| runtime-arguments-0-updates-0 | 1.647 | 1.779 | 1.781 | +0.07% |
| runtime-arguments-0-updates-100 | 2.457 | 3.043 | 3.041 | -0.22% |
| runtime-arguments-64-updates-0 | 1.772 | 1.987 | 1.984 | -0.13% |
| runtime-arguments-64-updates-100 | 2.498 | 3.148 | 3.151 | +0.13% |
| runtime-arguments-512-updates-0 | 1.991 | 2.407 | 2.403 | -0.16% |
| runtime-arguments-512-updates-100 | 2.551 | 3.358 | 3.352 | -0.04% |

Tiny and large no-op take 0.07% and 0.80% more paired time; expansion takes 0.49%
less. Startup ranges from -1.20% to +0.09%, and runtime from -0.22% to +0.13%.
The argument-heavy startup medians remain below GNU, while GNU medians are lower
on the other controls. All regressions remain retained. The additional initial
duplicate/close pairs are real startup work, but their individual elapsed cost is
not isolated by these whole-process measurements. Earlier campaigns and their
regressions remain unchanged and must not be pooled as cumulative changes.

Allocation profiling uses the original glibc preload helper with two repeats per
build/control. Each profile checks exact fixture/input/argument/environment and
output. Every build/control has identical summaries across its repeats.

| Control | Preceding calls / frees | Candidate calls / frees | Preceding requested bytes | Candidate requested bytes |
| --- | ---: | ---: | ---: | ---: |
| tiny-noop | 290 / 269 | 290 / 269 | 46,994 | 46,877 |
| large-noop | 244,702 / 234,750 | 244,702 / 234,750 | 20,852,134 | 20,852,017 |
| startup-arguments-1000 | 19,612 / 18,507 | 19,612 / 18,507 | 1,347,141 | 1,347,024 |
| runtime-arguments-512-updates-100 | 20,066 / 18,135 | 20,066 / 18,135 | 1,179,445 | 1,179,449 |

Calls/frees are unchanged. Requested bytes decrease by 117 on tiny, large and
startup controls and increase by four on runtime. Build layout and injected
profiler/output-path costs are not independently isolated, so these totals should
not be attributed to diagnostic code alone. Counts cover malloc/calloc/realloc/
free after helper initialization and exclude aligned allocation entry points.
Requested bytes are requests, not retained/peak memory or RSS; sampled stacks
are hints, not CPU or elapsed time. CPU23 applies only to elapsed campaigns.
Compiled preload libraries are excluded from archives. General parity,
persistent-cache correctness and complete M3/M6 remain open.
