# Include-directory metadata interruptions

Include-directory selection now retries an interrupted metadata operation instead
of silently dropping a valid search directory. It checks pending fatal signals
before each observation, so cancellation does not issue another stat or proceed
to later Makefile effects. Other metadata failures retain ordinary directory
selection behavior. No observation is cached.

Twelve original Linux/glibc fault profiles cover startup -I, reader MAKEFLAGS
resets and recipe eval, each with no fault, one interruption, three interruptions
and interruption with a fatal signal. The preceding build fails six finite
recovery profiles and passes six controls. Two GNU observations agree with each
independent model; both frozen candidate builds match all twelve. Raw bytes,
files, trace causality and prior observations are retained in
[historical evidence](../tests/research/include-stat-interruptions.json).

The fault gate requires the intended phase, every injected interruption, recovery
and the absence of later filesystem work or forbidden effects after cancellation.
It allows the stat/stat64/statx ABI route to differ while retaining semantic trace
order. The compiler-built observer is qualified against the host null-path ABI.
An early experimental observer failed zero-fault qualification because it
inherited declaration nonnull assumptions. It was discarded, its failures were
retained as instrumentation failures and no crash exception or conformance credit
was assigned. The corrected interposer uses separate export identifiers and a
qualified null-path forwarding client.

Run `make test-include-stat`, or select one profile with
`INCLUDE_STAT_CASE=include-stat-reader-eintr-1`. The full check, optimized
acceptance and self-hosting flows include this gate. GNU and rmake both drive the
flow. Three repetitions cover all profiles under each build and the GNU self-test.
These are separate required fault profiles, not extra ordinary curated cases or
safe-error exceptions. The ordinary corpus remains 10,920 required / zero pending,
with 17 unchanged separate approved safe-error exceptions. M3/M6 and general
performance parity remain open.

GNU Make 4.4.1 read.c include-directory EINTR handling was consulted from the
verified source archive, SHA256
`dd16fb1d67bfab79a72f5e8390735c49e3e8e70b4945a15ab1f81ddb78658fb3`.
GNU source is copyright Free Software Foundation, Inc., GPL-3.0-or-later. Rust,
observer, ABI client and models are original; no upstream code or tests were
copied or translated. No GNU execution fallback or unsafe Rust is added.

Full frozen development/optimized acceptance and CI for the final commit remain
merge gates; the PR records their final results.

Formatting, warnings-denied lint, 460 Rust and 434 tooling tests pass, as does the
10,920-case development comparison. Debug SHA256:
`146dc7cccf0239d6f2d6569e73ad65a814830fd151bacc1324d523fd3ac965da`.
Release SHA256:
`d77095a5cf04f7b620e8ebb6df6fef0f09b2a3dc105c75e2c4642dc4dd12339c`.

Two sequential reports contain 101 alternating samples per engine and workload,
with exact output checks and no independent local builds, tests or probes during
timing. Host activity was not controlled. The normal-path measurements are mixed;
no speedup or parity claim is made. Cancellation checks remain part of each fresh
metadata attempt.

| Workload | GNU ms | Current ms | Preceding rmake ms |
| --- | ---: | ---: | ---: |
| tiny-noop | 0.688 | 0.828 | 0.833 |
| large-noop | 11.175 | 30.575 | 29.937 |
| expansion | 2.831 | 4.994 | 5.006 |
| runtime-includes-0 | 0.673 | 0.770 | 0.775 |
| runtime-includes-1000 | 2.709 | 9.315 | 9.220 |

Raw reports: [core](benchmarks/include-stat-core.json), [repeated include metadata](benchmarks/include-stat-runtime-includes.json).

The [include operand history follow-up](include-operands.md) expands the current gate to 24 profiles, including two retained identical physical paths and explicit trace-count rejection. The historical twelve-profile evidence above remains unchanged.
