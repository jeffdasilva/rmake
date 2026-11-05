# Startup locale measurement

The locale correction has mixed whole-process timing results, including slower
large no-op and expansion controls. It does not establish general speedup or
GNU parity. All samples, allocation traces, source/helper identities, original
commands and limits are retained in the [archive](../../tests/locales/performance-ctype.json).
The candidate release SHA256 starts `aef90bcd2a27`; preceding starts `46448db1f1a0`.

Other local campaigns explicitly stopped before measurement. All commands ran
sequentially with shared CPU23 affinity. CPU frequency and exclusivity were not
controlled; no CPU/RSS/I/O counters, confidence intervals or acceptance
thresholds are supplied. Core/startup controls have 101 samples per engine;
runtime controls have 301. Engine order alternates at identical paths with exact
output checks and warm OS page caches. These are C-locale controls; C.UTF8
initialization/rendering costs remain unmeasured.

Medians are milliseconds. Paired change is the reciprocal of the median paired
preceding/candidate ratio minus one; negative means less candidate time.
Independent medians summarize different observations and can differ in sign.

| Control | GNU | Preceding | Candidate | Paired change |
| --- | ---: | ---: | ---: | ---: |
| tiny-noop | 1.709 | 1.837 | 1.828 | -0.14% |
| large-noop | 11.593 | 30.349 | 30.700 | +1.21% |
| expansion | 3.616 | 5.831 | 5.863 | +0.42% |
| startup-arguments-0 | 1.604 | 1.692 | 1.694 | +0.22% |
| startup-arguments-1000 | 3.130 | 2.931 | 2.949 | +0.25% |
| startup-arguments-5000 | 26.513 | 8.426 | 8.428 | +0.19% |
| runtime-arguments-0-updates-0 | 1.686 | 1.817 | 1.803 | +0.13% |
| runtime-arguments-0-updates-100 | 2.301 | 2.799 | 2.831 | +0.09% |
| runtime-arguments-64-updates-0 | 1.774 | 1.981 | 1.985 | +0.21% |
| runtime-arguments-64-updates-100 | 2.518 | 3.192 | 3.193 | +0.32% |
| runtime-arguments-512-updates-0 | 1.991 | 2.406 | 2.415 | +0.26% |
| runtime-arguments-512-updates-100 | 2.548 | 3.346 | 3.355 | +0.28% |

Two allocation repeats per build/control have identical summaries. Allocation
request-call counts stay unchanged. Free-call counters increase by 2 on each
control; this includes API-call accounting and does not establish lower retained
memory. Requested bytes increase by 77 on tiny, large and startup controls and
stay unchanged on the runtime control. Build layout and profiler/output path
costs are not separately isolated.

| Control | Calls before/after | Frees before/after | Requested bytes before/after |
| --- | ---: | ---: | ---: |
| tiny-noop | 290 / 290 | 269 / 271 | 46,773 / 46,850 |
| large-noop | 244,702 / 244,702 | 234,750 / 234,752 | 20,851,913 / 20,851,990 |
| startup-arguments-1000 | 19,612 / 19,612 | 18,507 / 18,509 | 1,346,920 / 1,346,997 |
| runtime-arguments-512-updates-100 | 20,066 / 20,066 | 18,135 / 18,137 | 1,179,345 / 1,179,345 |

The helper observes interposed malloc/calloc/realloc/free calls after its own
initialization; aligned and hidden internal allocation entry points are outside
its coverage. Requested bytes are not live/peak memory. Sampled call stacks are
hints rather than CPU measurements. Compiled preload binaries are not archived.
Earlier measurement archives remain separate and unchanged. Full M3/M6,
persistent-cache correctness and general performance parity remain open.
