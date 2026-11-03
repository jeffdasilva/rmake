# Startup include-search measurements

Four reports use 101 alternating samples per engine with exact output checks.
A uses pinned GNU Make drivers; B uses the selected frozen rmake. The baseline is
PR159's selected release. All local test, probe and profiling campaigns exited
before elapsed measurement. CPU placement, frequency and other host activity
were not controlled. No confidence intervals or general acceptance are claimed.

| Input / batch | GNU ms | Selected ms | Preceding ms |
| --- | ---: | ---: | ---: |
| tiny-noop / A | 0.635 | 0.752 | 0.766 |
| tiny-noop / B | 0.550 | 0.626 | 0.635 |
| large-noop / A | 11.103 | 30.407 | 29.517 |
| large-noop / B | 11.210 | 30.607 | 29.742 |
| expansion / A | 2.874 | 5.169 | 5.290 |
| expansion / B | 2.889 | 5.258 | 5.374 |
| runtime-arguments-512-updates-100 / A | 1.248 | 2.964 | 3.232 |
| runtime-arguments-512-updates-100 / B | 1.274 | 2.665 | 2.727 |

Large no-op medians are higher in both batches; tiny no-op, expansion and
runtime-update medians are lower. These selected, uncontrolled observations
establish neither a general speedup nor parity. GNU remains faster.

Raw samples, identities, source hashes and checked outputs:

- Core [A](startup-search-core-a.json) / [B](startup-search-core-b.json).
- Runtime arguments [A](startup-search-runtime-arguments-a.json) / [B](startup-search-runtime-arguments-b.json).

Twelve glibc request profiles repeat each build/input twice. Request and free
counts are unchanged. Small requested-byte differences include path/identity
lengths and are not treated as an optimization. These are allocator requests,
not live/peak memory, CPU measurements or complete allocator coverage. No
compiled preload libraries are retained.

| Input | Requests | Frees | Preceding bytes | Selected bytes |
| --- | ---: | ---: | ---: | ---: |
| large-noop | 244,699 | 234,749 | 20,327,552 | 20,327,548 |
| expansion | 66,074 | 61,048 | 2,659,233 | 2,659,229 |
| runtime-arguments-512-updates-100 | 20,062 | 18,133 | 1,178,997 | 1,178,993 |

Unmodified reports and adjacent traces:

- preceding large-noop: [1](startup-search-profiles/preceding-large-noop-1/report.json) / [2](startup-search-profiles/preceding-large-noop-2/report.json).
- selected large-noop: [1](startup-search-profiles/selected-large-noop-1/report.json) / [2](startup-search-profiles/selected-large-noop-2/report.json).
- preceding expansion: [1](startup-search-profiles/preceding-expansion-1/report.json) / [2](startup-search-profiles/preceding-expansion-2/report.json).
- selected expansion: [1](startup-search-profiles/selected-expansion-1/report.json) / [2](startup-search-profiles/selected-expansion-2/report.json).
- preceding runtime-arguments-512-updates-100: [1](startup-search-profiles/preceding-runtime-arguments-512-updates-100-1/report.json) / [2](startup-search-profiles/preceding-runtime-arguments-512-updates-100-2/report.json).
- selected runtime-arguments-512-updates-100: [1](startup-search-profiles/selected-runtime-arguments-512-updates-100-1/report.json) / [2](startup-search-profiles/selected-runtime-arguments-512-updates-100-2/report.json).
