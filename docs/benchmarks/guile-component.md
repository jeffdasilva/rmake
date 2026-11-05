# Optional Guile component and native context costs

The selected default executable is slower on these whole-process controls.
The source does not activate Guile in the application. These measurements do
not establish GNU parity, an enabled-profile cost, or native-callback throughput.
The [selected archive](../../tests/guile-component/performance-selected.json)
retains every sample, allocation report/trace, source and helper identity.
The [earlier archive](../../tests/guile-component/performance-exploratory.json)
retains the separate pre-locale experiment, including every regression.

All three local development campaigns explicitly stopped before measurement.
Commands ran serially with shared CPU23 affinity. CPU frequency and exclusivity
were not controlled. No confidence intervals, CPU/RSS/I/O counters or acceptance
thresholds are supplied. Core/startup controls have 101 samples per engine;
runtime controls have 301, with alternating engine order, identical inputs and
exact output checks. Candidate SHA256 is `b3ccc5ecb1d6`; the preceding locale
executable is `aef90bcd2a27`. These C-locale workloads do not execute native
callbacks or initialize Guile; context-switch costs remain unmeasured.

Medians are milliseconds. Paired change is the reciprocal of the median paired
preceding/candidate ratio minus one. Independent medians summarize different
observations and can differ from that ratio.

| Control | GNU | Preceding | Candidate | Paired change |
| --- | ---: | ---: | ---: | ---: |
| tiny-noop | 1.694 | 1.800 | 1.843 | +2.28% |
| large-noop | 11.796 | 31.061 | 31.647 | +1.65% |
| expansion | 3.620 | 5.791 | 6.045 | +4.53% |
| startup-arguments-0 | 1.590 | 1.681 | 1.714 | +1.88% |
| startup-arguments-1000 | 3.100 | 2.926 | 2.969 | +1.42% |
| startup-arguments-5000 | 26.265 | 8.407 | 8.487 | +0.77% |
| runtime-arguments-0-updates-0 | 1.664 | 1.782 | 1.821 | +1.96% |
| runtime-arguments-0-updates-100 | 2.449 | 3.018 | 3.072 | +1.54% |
| runtime-arguments-64-updates-0 | 1.766 | 1.980 | 2.019 | +1.95% |
| runtime-arguments-64-updates-100 | 2.489 | 3.151 | 3.188 | +1.19% |
| runtime-arguments-512-updates-0 | 1.989 | 2.425 | 2.462 | +1.54% |
| runtime-arguments-512-updates-100 | 2.537 | 3.344 | 3.380 | +1.12% |

Two allocation repeats per executable/control have identical summaries. Calls,
frees and requested bytes are unchanged between the selected and preceding
executables on all four controls. Build layout and other sources of elapsed
variation were not individually isolated.

| Control | Calls | Frees | Requested bytes |
| --- | ---: | ---: | ---: |
| tiny-noop | 290 | 271 | 46,882 |
| large-noop | 244,702 | 234,752 | 20,852,022 |
| startup-arguments-1000 | 19,612 | 18,509 | 1,347,029 |
| runtime-arguments-512-updates-100 | 20,066 | 18,137 | 1,179,377 |

The helper observes interposed malloc/calloc/realloc/free calls after its own
initialization. Aligned and hidden internal allocation entry points are outside
coverage. Requested bytes are not live or peak memory. Sampled call stacks are
hints, not CPU measurements; compiled preload binaries are excluded. Full
M3/M6, enabled frontend acceptance, persistent caching and general performance
parity remain open.
