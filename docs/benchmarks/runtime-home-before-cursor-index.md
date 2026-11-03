# Owned argument decoder and runtime HOME measurements

Four paired reports use 101 alternating samples per engine, with exact output
checks against GNU Make 4.4.1 and the preceding output-sync release. A uses GNU
Make drivers; B uses frozen rmake. All local validation/profiling processes had
exited before each elapsed batch. Host activity, CPU placement and frequency were
not controlled. These are observations without confidence intervals or general
performance acceptance proof.

| Input / batch | GNU ms | Selected ms | Preceding ms |
| --- | ---: | ---: | ---: |
| tiny-noop / A | 0.705 | 0.857 | 0.875 |
| tiny-noop / B | 0.609 | 0.714 | 0.707 |
| large-noop / A | 11.350 | 30.267 | 30.421 |
| large-noop / B | 11.133 | 29.928 | 30.089 |
| expansion / A | 2.809 | 5.202 | 5.151 |
| expansion / B | 2.886 | 5.205 | 5.167 |
| 512 overrides, 100 updates / A | 1.268 | 2.557 | 2.502 |
| 512 overrides, 100 updates / B | 1.227 | 3.645 | 3.612 |

Timing is mixed, including higher runtime-update medians in some batches and
lower medians in others. These controls do not establish a general speedup or
performance parity. GNU remains faster. No effectful HOME evaluation is cached.

Raw samples, identities, arguments, source hashes and output checks:

- Core [A](runtime-home-byte-core-a.json) / [B](runtime-home-byte-core-b.json).
- Runtime arguments [A](runtime-home-byte-runtime-arguments-a.json) / [B](runtime-home-byte-runtime-arguments-b.json).

Twelve glibc allocation-request profiles repeat each build/input twice. Counts
and frees are unchanged. The four-byte requested-size difference is not treated
as an optimization. Requests are not live/peak memory, CPU time or complete
allocator coverage. Compiled preload libraries are excluded.

| Input | Requests | Frees | Preceding bytes | Selected bytes |
| --- | ---: | ---: | ---: | ---: |
| large-noop | 244,699 | 234,749 | 20,327,426 | 20,327,422 |
| expansion | 66,074 | 61,048 | 2,659,107 | 2,659,103 |
| runtime-arguments-512-updates-100 | 21,071 | 19,142 | 1,211,230 | 1,211,226 |

Unmodified reports and traces:

- preceding large-noop: [1](runtime-home-byte-profiles/preceding-large-noop-1/report.json) / [2](runtime-home-byte-profiles/preceding-large-noop-2/report.json).
- selected large-noop: [1](runtime-home-byte-profiles/selected-large-noop-1/report.json) / [2](runtime-home-byte-profiles/selected-large-noop-2/report.json).
- preceding expansion: [1](runtime-home-byte-profiles/preceding-expansion-1/report.json) / [2](runtime-home-byte-profiles/preceding-expansion-2/report.json).
- selected expansion: [1](runtime-home-byte-profiles/selected-expansion-1/report.json) / [2](runtime-home-byte-profiles/selected-expansion-2/report.json).
- preceding runtime-arguments-512-updates-100: [1](runtime-home-byte-profiles/preceding-runtime-arguments-512-updates-100-1/report.json) / [2](runtime-home-byte-profiles/preceding-runtime-arguments-512-updates-100-2/report.json).
- selected runtime-arguments-512-updates-100: [1](runtime-home-byte-profiles/selected-runtime-arguments-512-updates-100-1/report.json) / [2](runtime-home-byte-profiles/selected-runtime-arguments-512-updates-100-2/report.json).
