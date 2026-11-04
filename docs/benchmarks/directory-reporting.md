# Directory reporting measurement evidence

The directory reporting change adds explicit and automatic enter/leave messages
with process, output-group and finalizer lifetimes. Two frozen versions passed
the separate 40-model directory and 215-model native profiles in both builds.
V2 avoids replacing an identical stored directory value. Measured elapsed changes
remain mixed; the allocation profiles retain a small call/free increase against
the preceding build. These results establish no general speedup or GNU parity.

[performance-v1.json](../../tests/directory-reporting/performance-v1.json) and
[performance-v2.json](../../tests/directory-reporting/performance-v2.json) preserve
all six original timing reports and all 32 allocation reports with 32 raw traces.
Every retained file has a byte hash; helper/tool bytes and hashes are included.
V1 links its immutable [qualification snapshot](../../tests/directory-reporting/qualification.json),
and v2 links [qualification-v2.json](../../tests/directory-reporting/qualification-v2.json).
All earlier evidence, including negative observations, remains unchanged.
Ordinary conformance and full developer acceptance are separate gates.

V1 is identified by SHA256 prefix 0c8cf50ff5e7 and v2 by 9058499815b5. Both
campaigns compare against the preceding startup-owner build da2952f9a1a2.
Full executable identities, exact inputs and raw samples appear in the archives.
The ordinary core/startup/runtime controls load no native module; callback
throughput, library loading and the complete native ecosystem are outside these
measurements.

The recorded profile is x86-64 Linux/WSL2/glibc with the pinned GNU Make 4.4.1
reference, C locale and Guile disabled. All elapsed campaigns inherited shared
CPU23 affinity, with alternating engine order and identical paths, environment,
arguments and checked output. Core and startup controls use 101 samples per
engine; runtime controls use 301. These are whole-process wall times. CPU23 was
not exclusive, and frequency was not controlled. Original generic no-pinning
limitations remain unchanged, with actual affinity recorded separately in the
archive wrappers. There are no confidence intervals, CPU/RSS/I/O counters or
performance acceptance thresholds. Independent campaigns must not be pooled
or interpreted as cumulative percentage changes.

Medians below are milliseconds. The final column is the reciprocal of the
median paired preceding/candidate ratio minus one, expressed as a percentage;
negative means less candidate time. Independent medians and paired changes can
have different signs because they summarize different observations.

V1 produced the following observations against the preceding build.

| Workload | GNU median | Preceding median | V1 median | Paired v1 time change |
| --- | ---: | ---: | ---: | ---: |
| tiny-noop | 1.684 | 1.800 | 1.797 | -0.20% |
| large-noop | 11.793 | 30.472 | 31.100 | +2.38% |
| expansion | 3.636 | 5.888 | 5.991 | +1.70% |
| startup-arguments-0 | 1.598 | 1.694 | 1.691 | -0.24% |
| startup-arguments-1000 | 3.123 | 2.935 | 2.962 | +0.80% |
| startup-arguments-5000 | 26.446 | 8.411 | 8.537 | +1.58% |
| runtime-arguments-0-updates-0 | 1.676 | 1.827 | 1.831 | -0.20% |
| runtime-arguments-0-updates-100 | 2.471 | 3.074 | 3.076 | +0.04% |
| runtime-arguments-64-updates-0 | 1.783 | 2.004 | 2.002 | -0.13% |
| runtime-arguments-64-updates-100 | 2.590 | 3.316 | 3.322 | +0.07% |
| runtime-arguments-512-updates-0 | 1.991 | 2.416 | 2.435 | +0.63% |
| runtime-arguments-512-updates-100 | 2.553 | 3.379 | 3.401 | +0.80% |

V1 tiny no-op takes about 0.20% less paired time, while large no-op and expansion
take 2.38% and 1.70% more. Startup controls range from -0.24% to +1.58%; runtime
controls range from -0.20% to +0.80%. These regressions remain part of the evidence.

The final v2 observations use their own paired preceding-build measurements.

| Workload | GNU median | Preceding median | V2 median | Paired v2 time change |
| --- | ---: | ---: | ---: | ---: |
| tiny-noop | 1.703 | 1.810 | 1.812 | -0.14% |
| large-noop | 11.909 | 30.818 | 31.186 | +0.95% |
| expansion | 3.614 | 5.914 | 5.903 | -0.70% |
| startup-arguments-0 | 1.598 | 1.687 | 1.686 | -0.09% |
| startup-arguments-1000 | 3.144 | 2.938 | 2.931 | -0.35% |
| startup-arguments-5000 | 26.640 | 8.388 | 8.434 | +0.25% |
| runtime-arguments-0-updates-0 | 1.689 | 1.825 | 1.823 | -0.09% |
| runtime-arguments-0-updates-100 | 2.475 | 3.044 | 3.046 | +0.40% |
| runtime-arguments-64-updates-0 | 1.799 | 2.019 | 2.014 | -0.11% |
| runtime-arguments-64-updates-100 | 2.592 | 3.289 | 3.299 | +0.22% |
| runtime-arguments-512-updates-0 | 2.003 | 2.417 | 2.424 | +0.12% |
| runtime-arguments-512-updates-100 | 2.558 | 3.351 | 3.374 | +0.77% |

V2 tiny no-op and expansion take 0.14% and 0.70% less paired time; large no-op
takes 0.95% more. Startup controls range from -0.35% to +0.25%, and runtime
controls from -0.11% to +0.77%. The 1,000- and 5,000-argument startup medians
remain below GNU in these reports, while GNU medians remain lower on the core
and runtime controls. Small selected improvements and remaining regressions do
not establish general elapsed parity or a broad speedup.

Allocation profiles use the original glibc preload helper, with two repeats per
build/control. Each report follows exact fixture/input/argument/environment and
output checks. Calls/frees/requested-byte summaries match between repeats for
every build/control. Both campaigns report identical preceding-build summaries,
shown once below; complete reports and traces remain individually retained.

| Control | Preceding calls / frees | V1 calls / frees | V2 calls / frees | Preceding requested bytes | V1 requested bytes | V2 requested bytes |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| tiny-noop | 289 / 268 | 291 / 270 | 290 / 269 | 47,021 | 47,005 | 46,967 |
| large-noop | 244,701 / 234,749 | 244,703 / 234,751 | 244,702 / 234,750 | 20,852,161 | 20,852,145 | 20,852,107 |
| startup-arguments-1000 | 19,611 / 18,506 | 19,613 / 18,508 | 19,612 / 18,507 | 1,347,168 | 1,347,152 | 1,347,114 |
| runtime-arguments-512-updates-100 | 20,065 / 18,134 | 20,067 / 18,136 | 20,066 / 18,135 | 1,179,395 | 1,179,467 | 1,179,429 |

V1 adds two calls and two frees on every control, with 16 fewer requested bytes
on tiny/large/startup and 72 more on runtime. V2 adds one call and one free
against the preceding build, saving one call/free and 38 requested bytes relative
to v1 in each control. Its requested-byte differences against the preceding build
are -54 on tiny/large/startup and +34 on runtime; the increase remains retained.

These counts cover malloc/calloc/realloc/free after helper initialization and
exclude aligned allocation entry points. Requested bytes are allocation requests,
not live/peak memory or RSS. Stride samples are call-stack hints, not elapsed or
CPU time. The CPU23 affinity statement applies only to elapsed campaigns, not
allocation profiles. Compiled helper libraries are excluded from the archives.
The [evidence index](../../tests/directory-reporting/README.md) links both complete
versions and the original compatibility discovery.
