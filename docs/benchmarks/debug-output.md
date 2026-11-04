# Debug-output measurement evidence

The selected deterministic debug slice preserves the disabled-debug allocation
counts on four original controls. Elapsed results are mixed and include
regressions. These measurements establish no broad speedup or GNU parity.

[performance-v2.json](../../tests/debug-output/performance-v2.json) retains all
three timing reports and sixteen allocation reports with their sixteen raw
traces, tool/helper identities and the corresponding
[qualification snapshot](../../tests/debug-output/qualification-v2.json).
The selected frozen release has SHA256 prefix `3c338225f894`; its preceding
directory-reporting control is `9058499815b5`. Complete identities, inputs and
samples remain in the archive. Both selected builds separately pass all 54 debug,
215 native and 40 directory models. Ordinary and full developer acceptance are
separate requirements.

The profile is x86-64 Linux/WSL2/glibc, C locale, Guile disabled and the pinned
GNU Make 4.4.1 reference. Debug is disabled in these controls; rendering throughput,
native callbacks and the complete build ecosystem are outside these measurements.
Elapsed campaigns ran sequentially after other local campaigns stopped and
inherited CPU23 affinity. This shared CPU was not exclusive and its frequency was
not controlled. The original generic benchmark limitations remain recorded;
actual inherited affinity is qualified separately in the archive wrapper.

Each report alternates engine order with identical paths, arguments, environment
and checked output. Core and startup controls use 101 samples per engine; runtime
controls use 301. These are whole-process wall times with warm OS page caches.
There are no confidence intervals, CPU/RSS/I/O counters or performance acceptance
thresholds. Independent medians and paired changes summarize different
observations and can have different signs.

Medians are milliseconds. The final column is the reciprocal of the median paired
preceding/candidate ratio minus one, expressed as a percentage; negative means
less candidate time.

| Workload | GNU median | Preceding median | Candidate median | Paired time change |
| --- | ---: | ---: | ---: | ---: |
| tiny-noop | 2.118 | 2.324 | 2.333 | +3.14% |
| large-noop | 11.966 | 31.091 | 31.209 | -0.13% |
| expansion | 3.723 | 6.065 | 6.142 | +1.28% |
| startup-arguments-0 | 1.592 | 1.695 | 1.702 | +0.14% |
| startup-arguments-1000 | 3.083 | 2.922 | 2.925 | -0.06% |
| startup-arguments-5000 | 26.315 | 8.464 | 8.511 | +1.02% |
| runtime-arguments-0-updates-0 | 1.726 | 1.879 | 1.881 | +0.05% |
| runtime-arguments-0-updates-100 | 2.461 | 3.057 | 3.067 | +0.61% |
| runtime-arguments-64-updates-0 | 1.787 | 2.009 | 2.016 | +0.15% |
| runtime-arguments-64-updates-100 | 2.503 | 3.166 | 3.171 | +0.27% |
| runtime-arguments-512-updates-0 | 2.014 | 2.443 | 2.451 | +0.09% |
| runtime-arguments-512-updates-100 | 2.533 | 3.369 | 3.370 | +0.02% |

Tiny no-op and expansion take 3.14% and 1.28% more paired time. Large no-op
takes 0.13% less; startup ranges from -0.06% to +1.02% and runtime from +0.02%
to +0.61%. The 1,000- and 5,000-argument medians remain below GNU, while GNU
medians are lower for the other controls. All regressions remain retained.

Allocation profiling uses the original glibc preload helper, with two repeats
per build/control. Exact input/argument/environment and output checks precede
each summary. Every build/control has identical summaries across its repeats.

| Control | Preceding calls / frees | Candidate calls / frees | Preceding requested bytes | Candidate requested bytes |
| --- | ---: | ---: | ---: | ---: |
| tiny-noop | 290 / 269 | 290 / 269 | 46,987 | 46,946 |
| large-noop | 244,702 / 234,750 | 244,702 / 234,750 | 20,852,127 | 20,852,086 |
| startup-arguments-1000 | 19,612 / 18,507 | 19,612 / 18,507 | 1,347,134 | 1,347,093 |
| runtime-arguments-512-updates-100 | 20,066 / 18,135 | 20,066 / 18,135 | 1,179,449 | 1,179,485 |

Calls/frees remain unchanged. Requested bytes decrease by 41 on tiny, large and
startup controls and increase by 36 on runtime. These counts cover
malloc/calloc/realloc/free after helper initialization and exclude aligned
allocation entry points. Requested bytes are requests, not retained/peak memory
or RSS; sampled stacks are hints, not CPU or elapsed time. CPU23 affinity applies
only to elapsed campaigns. Compiled helper libraries are excluded from archives.

The [evidence index](../../tests/debug-output/README.md) preserves the earlier
discovery, incorrect hypotheses, v1 controls and final v2 qualification unchanged.
General performance parity, persistent-cache correctness and complete M3/M6
remain open.
