# Owned argument decoder and runtime HOME measurements

Four paired reports use 101 alternating samples per engine, with exact output
checks against GNU Make 4.4.1 and the preceding output-sync release. A uses GNU
Make drivers; B uses frozen rmake. All local validation/profiling processes had
exited before each elapsed batch. Host activity, CPU placement and frequency were
not controlled. These are observations without confidence intervals or general
performance acceptance proof.

| Input / batch | GNU ms | Selected ms | Preceding ms |
| --- | ---: | ---: | ---: |
| tiny-noop / A | 0.691 | 0.840 | 0.851 |
| tiny-noop / B | 0.616 | 0.727 | 0.720 |
| large-noop / A | 11.469 | 31.261 | 30.567 |
| large-noop / B | 11.223 | 30.721 | 30.352 |
| expansion / A | 2.927 | 5.285 | 5.247 |
| expansion / B | 2.808 | 5.138 | 5.194 |
| 512 overrides, 100 updates / A | 1.235 | 4.334 | 4.058 |
| 512 overrides, 100 updates / B | 1.234 | 3.685 | 3.815 |

Timing is mixed, including higher runtime-update medians in some batches and
lower medians in others. These controls do not establish a general speedup or
performance parity. GNU remains faster. No effectful HOME evaluation is cached.

Raw samples, identities, arguments, source hashes and output checks:

- Core [A](runtime-home-selected-core-a.json) / [B](runtime-home-selected-core-b.json).
- Runtime arguments [A](runtime-home-selected-runtime-arguments-a.json) / [B](runtime-home-selected-runtime-arguments-b.json).

Twelve glibc allocation-request profiles repeat each build/input twice. Counts
and frees are unchanged. The four-byte requested-size difference is not treated
as an optimization. Requests are not live/peak memory, CPU time or complete
allocator coverage. Compiled preload libraries are excluded.

| Input | Requests | Frees | Preceding bytes | Selected bytes |
| --- | ---: | ---: | ---: | ---: |
| large-noop | 244,699 | 234,749 | 20,327,442 | 20,327,438 |
| expansion | 66,074 | 61,048 | 2,659,123 | 2,659,119 |
| runtime-arguments-512-updates-100 | 21,071 | 19,142 | 1,211,246 | 1,211,242 |

Unmodified reports and traces:

- preceding large-noop: [1](runtime-home-selected-profiles/preceding-large-noop-1/report.json) / [2](runtime-home-selected-profiles/preceding-large-noop-2/report.json).
- selected large-noop: [1](runtime-home-selected-profiles/selected-large-noop-1/report.json) / [2](runtime-home-selected-profiles/selected-large-noop-2/report.json).
- preceding expansion: [1](runtime-home-selected-profiles/preceding-expansion-1/report.json) / [2](runtime-home-selected-profiles/preceding-expansion-2/report.json).
- selected expansion: [1](runtime-home-selected-profiles/selected-expansion-1/report.json) / [2](runtime-home-selected-profiles/selected-expansion-2/report.json).
- preceding runtime-arguments-512-updates-100: [1](runtime-home-selected-profiles/preceding-runtime-arguments-512-updates-100-1/report.json) / [2](runtime-home-selected-profiles/preceding-runtime-arguments-512-updates-100-2/report.json).
- selected runtime-arguments-512-updates-100: [1](runtime-home-selected-profiles/selected-runtime-arguments-512-updates-100-1/report.json) / [2](runtime-home-selected-profiles/selected-runtime-arguments-512-updates-100-2/report.json).
