# Owned argument decoder and runtime HOME measurements

Six paired reports use 101 alternating samples per engine, with exact output
checks against GNU Make 4.4.1 and the preceding output-sync release. A/C use GNU
Make drivers; B/D use frozen rmake. All local validation/profiling processes had
exited before each elapsed batch. Host activity, CPU placement and frequency were
not controlled. These are observations without confidence intervals or general
performance acceptance proof.

| Input / batch | GNU ms | Selected ms | Preceding ms |
| --- | ---: | ---: | ---: |
| tiny-noop / A | 0.699 | 0.843 | 0.863 |
| tiny-noop / B | 0.535 | 0.624 | 0.626 |
| large-noop / A | 11.346 | 30.384 | 30.578 |
| large-noop / B | 11.230 | 30.487 | 30.498 |
| expansion / A | 2.893 | 5.272 | 5.234 |
| expansion / B | 2.924 | 5.209 | 5.204 |
| 512 overrides, 100 updates / A | 1.298 | 3.184 | 3.211 |
| 512 overrides, 100 updates / B | 1.290 | 3.684 | 3.432 |
| 512 overrides, 100 updates / C | 1.247 | 3.691 | 3.593 |
| 512 overrides, 100 updates / D | 1.263 | 2.653 | 2.667 |

Timing is mixed, including higher runtime-update medians in some batches and
lower medians in others. These controls do not establish a general speedup or
performance parity. GNU remains faster. No effectful HOME evaluation is cached.

Raw samples, identities, arguments, source hashes and output checks:

- Core [A](runtime-home-core-a.json) / [B](runtime-home-core-b.json).
- Runtime arguments [A](runtime-home-runtime-arguments-a.json) / [B](runtime-home-runtime-arguments-b.json) / [C](runtime-home-runtime-arguments-c.json) / [D](runtime-home-runtime-arguments-d.json).

Twelve glibc allocation-request profiles repeat each build/input twice. Counts
and frees are unchanged. The four-byte requested-size difference is not treated
as an optimization. Requests are not live/peak memory, CPU time or complete
allocator coverage. Compiled preload libraries are excluded.

| Input | Requests | Frees | Preceding bytes | Selected bytes |
| --- | ---: | ---: | ---: | ---: |
| large-noop | 244,699 | 234,749 | 20,327,406 | 20,327,402 |
| expansion | 66,074 | 61,048 | 2,659,087 | 2,659,083 |
| runtime-arguments-512-updates-100 | 21,071 | 19,142 | 1,211,210 | 1,211,206 |

Unmodified reports and traces:

- preceding large-noop: [1](runtime-home-profiles/preceding-large-noop-1/report.json) / [2](runtime-home-profiles/preceding-large-noop-2/report.json).
- selected large-noop: [1](runtime-home-profiles/selected-large-noop-1/report.json) / [2](runtime-home-profiles/selected-large-noop-2/report.json).
- preceding expansion: [1](runtime-home-profiles/preceding-expansion-1/report.json) / [2](runtime-home-profiles/preceding-expansion-2/report.json).
- selected expansion: [1](runtime-home-profiles/selected-expansion-1/report.json) / [2](runtime-home-profiles/selected-expansion-2/report.json).
- preceding runtime-arguments-512-updates-100: [1](runtime-home-profiles/preceding-runtime-arguments-512-updates-100-1/report.json) / [2](runtime-home-profiles/preceding-runtime-arguments-512-updates-100-2/report.json).
- selected runtime-arguments-512-updates-100: [1](runtime-home-profiles/selected-runtime-arguments-512-updates-100-1/report.json) / [2](runtime-home-profiles/selected-runtime-arguments-512-updates-100-2/report.json).
