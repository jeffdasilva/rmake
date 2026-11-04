# Native startup exit ownership measurements

The startup exit ownership change preserves the current interpreter phase for
native finalizers reached through help/version, decoding and construction errors,
and failed restarted startup. The selected measurements show mixed elapsed
changes and small allocation increases. They establish no general speedup or
GNU Make performance parity.

[startup-exit-performance.json](../../tests/native/startup-exit-performance.json)
retains all three original elapsed reports and all 16 allocation reports with
16 raw traces, including every negative observation. Each retained file has a
byte hash. The archive includes profiling helper/tool bytes and hashes and links
the immutable source/input snapshot in
[qualification-startup-215.json](../../tests/native/qualification-startup-215.json).
That separate native qualification contains both frozen builds passing all 215
profiles and the preceding main passing 188 and failing 27. Full developer and
ordinary conformance acceptance remain separate gates.

The frozen candidate SHA256 begins da2952f9a1a2; the preceding borrowed-location
build begins bb18d1f7f410. Full identities and exact inputs appear in the raw
reports. These ordinary controls load no native module. They measure invocation
costs; native callback throughput and library loading costs are outside this
comparison.

The campaigns use the pinned GNU Make 4.4.1 reference on the recorded x86-64
Linux/WSL2/glibc, C-locale, Guile-disabled profile. Every elapsed campaign inherited
shared CPU23 affinity, with alternating engine order and identical paths,
environment, arguments and checked output. Core and startup-argument workloads
have 101 samples per engine; runtime workloads have 301. These are whole-process
wall times. CPU23 was shared, with no exclusive isolation or frequency control.
The original generic no-pinning limitations remain unchanged; the archive wrapper
records campaign affinity separately. No confidence intervals, CPU/RSS/I/O
counters or performance acceptance thresholds were collected.

Medians below are milliseconds. The paired change is the reciprocal of the
median paired preceding/candidate ratio minus one, expressed as a percentage;
negative means less candidate time. Independent medians and paired changes can
have different signs because they summarize different observations. Every raw
sample remains available in the archive.

| Workload | GNU median | Preceding median | Candidate median | Paired candidate time change |
| --- | ---: | ---: | ---: | ---: |
| tiny-noop | 1.683 | 1.798 | 1.798 | -0.20% |
| large-noop | 11.781 | 30.862 | 30.590 | -1.18% |
| expansion | 3.618 | 5.747 | 5.889 | +2.50% |
| startup-arguments-0 | 1.583 | 1.675 | 1.672 | -0.25% |
| startup-arguments-1000 | 3.096 | 2.922 | 2.913 | -0.00% |
| startup-arguments-5000 | 26.310 | 8.340 | 8.349 | -0.27% |
| runtime-arguments-0-updates-0 | 1.733 | 1.881 | 1.878 | -0.22% |
| runtime-arguments-0-updates-100 | 2.472 | 3.053 | 3.051 | -0.17% |
| runtime-arguments-64-updates-0 | 1.772 | 1.991 | 1.985 | -0.20% |
| runtime-arguments-64-updates-100 | 2.507 | 3.162 | 3.159 | -0.09% |
| runtime-arguments-512-updates-0 | 2.069 | 2.523 | 2.512 | +0.14% |
| runtime-arguments-512-updates-100 | 2.536 | 3.330 | 3.326 | -0.13% |

Tiny and large no-op controls have 0.20% and 1.18% lower paired times; expansion
has 2.50% higher paired time. The startup-argument controls show changes of
-0.25%, approximately zero, and -0.27%. Runtime controls are mixed, from -0.22%
to +0.14%. The candidate has lower GNU-comparison medians on the 1,000- and
5,000-argument startup controls, while GNU medians remain lower for the core and
runtime controls. These selected observations do not establish broader parity
or a cumulative improvement over different historical campaigns.

The allocation campaigns use the original glibc preload helper and two repeats
per build/control. It verifies the exact fixture input, arguments, environment,
stdout and empty stderr before writing a report. Calls, frees and requested-byte
summaries agree between both repeats for each build/control; all complete reports
and traces remain individually retained.

| Control | Preceding calls / frees | Candidate calls / frees | Preceding requested bytes | Candidate requested bytes |
| --- | ---: | ---: | ---: | ---: |
| tiny-noop | 289 / 268 | 289 / 268 | 46,947 | 47,017 |
| large-noop | 244,701 / 234,749 | 244,701 / 234,749 | 20,852,087 | 20,852,157 |
| startup-arguments-1000 | 19,611 / 18,506 | 19,611 / 18,506 | 1,347,094 | 1,347,164 |
| runtime-arguments-512-updates-100 | 20,064 / 18,133 | 20,065 / 18,134 | 1,179,349 | 1,179,391 |

Tiny, large and 1,000-argument startup retain their call/free counts and request
70 additional bytes. The 512-argument/100-update runtime control adds one call,
one free and 42 requested bytes. The counts cover malloc/calloc/realloc/free
after helper initialization and exclude aligned allocation entry points.
Requested bytes are allocation requests, not live/peak memory or RSS. Stride
samples are call-stack hints, not CPU or elapsed time. The CPU23 elapsed-affinity
statement does not apply to these allocation profiles. Compiled helper libraries
are excluded from the archive. Earlier evidence remains immutable and is linked
from the [native evidence index](../../tests/native/README.md).
