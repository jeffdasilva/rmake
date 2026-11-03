# Second-stage include search measurements

Four reports use 101 alternating samples per engine with exact output checks.
A uses pinned GNU Make drivers; B uses the selected frozen rmake. The baseline is
PR157's selected release. All local test, probe and profiling campaigns exited
before elapsed measurement. CPU placement, frequency and other host activity
were not controlled. No confidence intervals or general acceptance are claimed.

| Input / batch | GNU ms | Selected ms | Preceding ms |
| --- | ---: | ---: | ---: |
| tiny-noop / A | 0.633 | 0.744 | 0.727 |
| tiny-noop / B | 0.637 | 0.737 | 0.739 |
| large-noop / A | 11.244 | 29.899 | 30.380 |
| large-noop / B | 11.378 | 30.142 | 30.743 |
| expansion / A | 2.882 | 5.278 | 5.170 |
| expansion / B | 2.807 | 5.200 | 5.115 |
| runtime-arguments-512-updates-100 / A | 1.257 | 4.002 | 3.648 |
| runtime-arguments-512-updates-100 / B | 1.280 | 3.525 | 3.440 |

The selected large no-op medians are lower in both batches. Expansion and
runtime-update medians are higher; tiny no-op results are mixed. These controls
establish neither a general speedup nor parity. GNU remains faster.

Raw samples, identities, source hashes and checked outputs:

- Core [A](secondary-search-core-a.json) / [B](secondary-search-core-b.json).
- Runtime arguments [A](secondary-search-runtime-arguments-a.json) / [B](secondary-search-runtime-arguments-b.json).

Twelve glibc request profiles repeat each build/input twice. Request and free
counts are unchanged. Small requested-byte differences include path/identity
lengths and are not treated as an optimization. These are allocator requests,
not live/peak memory, CPU measurements or complete allocator coverage. No
compiled preload libraries are retained.

| Input | Requests | Frees | Preceding bytes | Selected bytes |
| --- | ---: | ---: | ---: | ---: |
| large-noop | 244,699 | 234,749 | 20,327,474 | 20,327,536 |
| expansion | 66,074 | 61,048 | 2,659,155 | 2,659,217 |
| runtime-arguments-512-updates-100 | 21,071 | 19,142 | 1,211,278 | 1,211,274 |

Unmodified reports and adjacent traces:

- preceding large-noop: [1](secondary-search-profiles/preceding-large-noop-1/report.json) / [2](secondary-search-profiles/preceding-large-noop-2/report.json).
- selected large-noop: [1](secondary-search-profiles/selected-large-noop-1/report.json) / [2](secondary-search-profiles/selected-large-noop-2/report.json).
- preceding expansion: [1](secondary-search-profiles/preceding-expansion-1/report.json) / [2](secondary-search-profiles/preceding-expansion-2/report.json).
- selected expansion: [1](secondary-search-profiles/selected-expansion-1/report.json) / [2](secondary-search-profiles/selected-expansion-2/report.json).
- preceding runtime-arguments-512-updates-100: [1](secondary-search-profiles/preceding-runtime-arguments-512-updates-100-1/report.json) / [2](secondary-search-profiles/preceding-runtime-arguments-512-updates-100-2/report.json).
- selected runtime-arguments-512-updates-100: [1](secondary-search-profiles/selected-runtime-arguments-512-updates-100-1/report.json) / [2](secondary-search-profiles/selected-runtime-arguments-512-updates-100-2/report.json).
