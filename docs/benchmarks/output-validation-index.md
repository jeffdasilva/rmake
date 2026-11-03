# Output-sync validation ordering measurements

Six output-checked paired reports use 101 alternating samples per engine. They
compare the selected frozen release, the preceding approved nested-short release
and pinned GNU Make 4.4.1. A/C use GNU Make drivers; B/D use frozen rmake. Every
local validation or profiling process had exited before elapsed timing began.
Host activity, CPU placement and frequency were not controlled. These are
workload observations without confidence intervals or general acceptance proof.

| Input / batch | GNU ms | Selected ms | Preceding ms |
| --- | ---: | ---: | ---: |
| tiny-noop / A | 0.695 | 0.853 | 0.853 |
| tiny-noop / B | 0.552 | 0.639 | 0.635 |
| large-noop / A | 11.480 | 31.269 | 30.922 |
| large-noop / B | 11.553 | 31.098 | 30.932 |
| expansion / A | 3.043 | 5.432 | 5.403 |
| expansion / B | 2.956 | 5.340 | 5.342 |
| 512 overrides, 100 updates / A | 1.275 | 3.853 | 3.886 |
| 512 overrides, 100 updates / B | 1.298 | 3.756 | 3.245 |
| 512 overrides, 100 updates / C | 1.252 | 3.464 | 3.596 |
| 512 overrides, 100 updates / D | 1.270 | 3.362 | 3.622 |

The first two batches were mixed, including a higher runtime-update median in B.
Two further runtime batches retain lower medians in several update cells, with
substantial cross-batch variation. No repeatable improvement or regression is
established. GNU remains faster on these core and runtime controls. The fix
preserves required effects; it does not claim general performance parity.

All raw samples, identities, source hashes, arguments and output checks remain in:

- Core [A](output-validation-core-a.json) / [B](output-validation-core-b.json).
- Runtime arguments [A](output-validation-runtime-arguments-a.json) / [B](output-validation-runtime-arguments-b.json) / [C](output-validation-runtime-arguments-c.json) / [D](output-validation-runtime-arguments-d.json).

Twelve glibc allocation-request profiles cover each selected/preceding input
twice. Request counts and free counts are identical between builds:

| Input | Requests | Frees | Preceding requested bytes | Selected requested bytes |
| --- | ---: | ---: | ---: | ---: |
| large-noop | 244,699 | 234,749 | 20,327,434 | 20,327,430 |
| expansion | 66,074 | 61,048 | 2,659,115 | 2,659,111 |
| runtime-arguments-512-updates-100 | 21,071 | 19,142 | 1,211,238 | 1,211,234 |

The four-byte requested-size difference is not treated as an optimization. These
requests are not live/peak memory, CPU time or complete allocator coverage.
Reports and unmodified traces are retained; compiled preload libraries are excluded.

- preceding large-noop: [1](output-validation-profiles/preceding-large-noop-1/report.json) / [2](output-validation-profiles/preceding-large-noop-2/report.json).
- selected large-noop: [1](output-validation-profiles/selected-large-noop-1/report.json) / [2](output-validation-profiles/selected-large-noop-2/report.json).
- preceding expansion: [1](output-validation-profiles/preceding-expansion-1/report.json) / [2](output-validation-profiles/preceding-expansion-2/report.json).
- selected expansion: [1](output-validation-profiles/selected-expansion-1/report.json) / [2](output-validation-profiles/selected-expansion-2/report.json).
- preceding runtime-arguments-512-updates-100: [1](output-validation-profiles/preceding-runtime-arguments-512-updates-100-1/report.json) / [2](output-validation-profiles/preceding-runtime-arguments-512-updates-100-2/report.json).
- selected runtime-arguments-512-updates-100: [1](output-validation-profiles/selected-runtime-arguments-512-updates-100-1/report.json) / [2](output-validation-profiles/selected-runtime-arguments-512-updates-100-2/report.json).
