# Runtime inspection measurements

The selected build inspects silent/keep-going/job selections during the merged
runtime parse. The baseline is PR158's selected release. Four unpinned reports
use 101 alternating samples per engine. A uses GNU Make drivers; B uses frozen
rmake. Every sample checks exact output against pinned GNU Make 4.4.1. All local
validation/profiling campaigns exited before elapsed batches. CPU placement,
frequency and other host activity were not controlled in these first reports.

| Input / batch | GNU ms | Selected ms | Preceding ms |
| --- | ---: | ---: | ---: |
| tiny-noop / A | 0.646 | 0.728 | 0.737 |
| tiny-noop / B | 0.538 | 0.637 | 0.652 |
| large-noop / A | 11.334 | 30.327 | 30.170 |
| large-noop / B | 11.261 | 30.082 | 29.788 |
| expansion / A | 2.809 | 5.260 | 5.203 |
| expansion / B | 2.826 | 5.243 | 5.241 |
| runtime-arguments-512-updates-100 / A | 1.264 | 3.160 | 3.349 |
| runtime-arguments-512-updates-100 / B | 1.211 | 4.126 | 3.769 |

Unpinned results are mixed, including opposite runtime-update movements. To
investigate that unresolved result, two further runtime suites use 301 samples
on one inherited CPU (23), with both driver types. Affinity was process-scoped;
this did not reserve the CPU, control frequency or isolate the host. The
[method](runtime-inspection-pinned-method.json) records affinity and exact argv.

| 512 overrides, 100 updates | GNU ms | Selected ms | Preceding ms | Median paired baseline/selected |
| --- | ---: | ---: | ---: | ---: |
| A | 2.587 | 3.417 | 3.441 | 1.0060 |
| B | 2.563 | 3.342 | 3.367 | 1.0079 |

Pinned medians show a small improvement in both batches. No confidence intervals
or full workload/profile acceptance are claimed. The unpinned observations remain
part of the evidence; no general elapsed speedup or GNU parity is established.

Raw samples, identities, source hashes and checked outputs:

- Core [A](runtime-inspection-core-a.json) / [B](runtime-inspection-core-b.json).
- Runtime [A](runtime-inspection-runtime-arguments-a.json) / [B](runtime-inspection-runtime-arguments-b.json).
- Pinned runtime [A](runtime-inspection-runtime-arguments-pinned-a.json) / [B](runtime-inspection-runtime-arguments-pinned-b.json).

Twelve glibc request profiles repeat each build/input twice. The runtime workload
reduces requests/frees by 1,009 (21,071 to 20,062 requests, about 4.8%). Other two
controls have unchanged counts. Small byte differences include path lengths and
are not used as optimization evidence. Requests are not live/peak memory, CPU
measurements or complete allocator coverage. Compiled preload libraries are not
retained.

| Input | Preceding requests | Selected requests | Preceding frees | Selected frees | Preceding bytes | Selected bytes |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| large-noop | 244,699 | 244,699 | 234,749 | 234,749 | 20,327,508 | 20,327,504 |
| expansion | 66,074 | 66,074 | 61,048 | 61,048 | 2,659,189 | 2,659,185 |
| runtime-arguments-512-updates-100 | 21,071 | 20,062 | 19,142 | 18,133 | 1,211,246 | 1,178,949 |

Unmodified profiles and adjacent request traces:

- preceding large-noop: [1](runtime-inspection-profiles/preceding-large-noop-1/report.json) / [2](runtime-inspection-profiles/preceding-large-noop-2/report.json).
- selected large-noop: [1](runtime-inspection-profiles/selected-large-noop-1/report.json) / [2](runtime-inspection-profiles/selected-large-noop-2/report.json).
- preceding expansion: [1](runtime-inspection-profiles/preceding-expansion-1/report.json) / [2](runtime-inspection-profiles/preceding-expansion-2/report.json).
- selected expansion: [1](runtime-inspection-profiles/selected-expansion-1/report.json) / [2](runtime-inspection-profiles/selected-expansion-2/report.json).
- preceding runtime-arguments-512-updates-100: [1](runtime-inspection-profiles/preceding-runtime-arguments-512-updates-100-1/report.json) / [2](runtime-inspection-profiles/preceding-runtime-arguments-512-updates-100-2/report.json).
- selected runtime-arguments-512-updates-100: [1](runtime-inspection-profiles/selected-runtime-arguments-512-updates-100-1/report.json) / [2](runtime-inspection-profiles/selected-runtime-arguments-512-updates-100-2/report.json).
