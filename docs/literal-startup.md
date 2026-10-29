# Ordered literal startup and reconstruction

Every executable invocation now enters the same ordered startup plan. Literal
arguments no longer bypass the argument receiver because they contain no dollar,
tilde or shell-assignment syntax. This preserves the GNU comparison of an incoming
raw directory operand with earlier stored, normalized operands. For example,
`-C./d -Cd` enters one level while `-C./d -C./d` enters two. Duplicate Makefiles
remain permitted, and `-f./Makefile` contributes `Makefile` to `MAKEFILE_LIST`.

The same original arguments are decoded again after a generated include causes a
restart. Early missing-MAKEFLAGS warnings retain the initial program identity;
directory errors use the recursion level selected after all command arguments.
Failed return to the original directory retains the `chdir` diagnostic and GNU's
retired master-pool cleanup warning, while owned jobserver resources still close.

There are 146 new required original models across short/long options, literal and
expression input classes, repeated operands, warning/error phases, generated
includes and failed-directory reconstruction with serial, FIFO and pipe execution.
Two GNU observations agree with every independently authored expectation. The
preceding build fails 58 and passes 88 controls; both final frozen builds match
all 146, including intended generated-file effects. The ordinary corpus is now
11,066 required / zero pending. The 17 approved safe-error exceptions remain
separately gated, with no new exceptions or extra credit for historical records.

The first implementation passed the initial 116 models but failed 72 existing
namespace cases. Those raw failures are retained alongside its hash; that
candidate was rejected before the diagnostic and cleanup paths were corrected.
All 72 regression controls now pass. See the [original observations and rejected
candidate evidence](../tests/research/literal-startup.json).

Formatting, warnings-denied lint, 460 Rust and 439 tooling tests pass. Debug SHA256:
`d9e87f415e1c2f50c9edc8eb7014ee46f179c187ae6410b19ad73ae5424fe137`.
Release SHA256:
`9b25a90eecc5bcafd44e40b502cab393667ddeb064e0ce00528c5c2a1cb770cd`.
Full frozen rmake-driven development/optimized acceptance and CI for the final
commit remain merge gates; the PR records the results.

GNU Make 4.4.1 main.c argument decoding, startup and reconstruction phases were
consulted from the verified archive, SHA256
`dd16fb1d67bfab79a72f5e8390735c49e3e8e70b4945a15ab1f81ddb78658fb3`.
GNU source is copyright Free Software Foundation, Inc., GPL-3.0-or-later. The
Rust, tests, observers and models are original. No upstream implementation or
tests were copied or translated; no unsafe Rust or GNU execution fallback is added.

## Allocation and timing evidence

The production path no longer creates the legacy Options or pre-reader database
snapshots. It retains its original owned inputs for reconstruction. The two
focused unit constructors still use the older constructor/replay helpers; removing
those branches is a separate cleanup. No semantic or filesystem result is cached,
and no required expansion or restart effect is skipped.

Two observations per engine/workload repeat these exact glibc-request counters,
using equal-length binary, helper and trace paths. GNU make drives the first
profile batch; the frozen rmake drives the second. The top-level
`profile-allocations-frozen` flow now also accepts `startup-arguments-0`, `-1000`
and `-5000`, with original negative checks for output and fixture/input drift.

| Workload | Prior calls | Current calls | Prior requested bytes | Current requested bytes |
| --- | ---: | ---: | ---: | ---: |
| tiny-noop | 334 | 297 | 68,966 | 61,758 |
| large-noop | 244,723 | 244,686 | 20,438,863 | 20,431,653 |
| startup-arguments-1000 | 21,803 | 20,603 | 1,554,229 | 1,330,880 |
| startup-arguments-5000 | 111,067 | 105,250 | 8,393,271 | 7,303,914 |

Requested bytes fall by about 14.4% and 13.0% on the 1,000/5,000-literal-argument
workloads. The profiler observes glibc malloc/calloc/realloc/free after its own
initialization, excludes aligned entry points and measures requested bytes rather
than peak or live memory. Sampled stacks do not establish CPU cost. Counts and
raw traces are preserved in the [allocation observations](benchmarks/literal-startup-allocations.json).

Eight sequential timing reports contain two batches of 101 alternating samples
per engine and workload, two warmups, exact output checks and no concurrent local
build/test/probe campaigns. Host activity was not controlled; there are no
confidence intervals or performance thresholds. Literal argument workloads have
median paired prior/current timing ratios of 1.048–1.063 in these runs; no-op results are
mixed, and some expression-heavy observations are slower. This does not establish
general performance parity. GNU remains substantially faster on large no-op and
expansion workloads.

| Workload / batch | GNU ms | Current ms | Prior rmake ms |
| --- | ---: | ---: | ---: |
| tiny-noop / A | 0.594 | 0.695 | 0.683 |
| tiny-noop / B | 0.542 | 0.640 | 0.639 |
| large-noop / A | 11.402 | 29.752 | 29.725 |
| large-noop / B | 11.338 | 29.654 | 29.788 |
| expansion / A | 2.770 | 4.964 | 4.990 |
| expansion / B | 2.927 | 5.063 | 5.099 |
| startup-arguments-1000 / A | 2.116 | 1.868 | 1.972 |
| startup-arguments-1000 / B | 2.202 | 2.008 | 2.086 |
| startup-arguments-5000 / A | 26.088 | 7.972 | 8.494 |
| startup-arguments-5000 / B | 25.902 | 8.023 | 8.464 |
| startup-effects-1000 / A | 2.480 | 2.924 | 2.859 |
| startup-effects-1000 / B | 2.430 | 2.957 | 2.850 |
| initial-flags-1000 / A | 2.480 | 2.824 | 2.774 |
| initial-flags-1000 / B | 2.579 | 2.871 | 2.848 |

Raw reports: [core A](benchmarks/literal-startup-core-a.json), [core B](benchmarks/literal-startup-core-b.json), [literal argv A](benchmarks/literal-startup-startup-arguments-a.json), [literal argv B](benchmarks/literal-startup-startup-arguments-b.json), [CLI effects A](benchmarks/literal-startup-startup-effects-a.json), [CLI effects B](benchmarks/literal-startup-startup-effects-b.json), [inherited effects A](benchmarks/literal-startup-initial-flags-a.json), [inherited effects B](benchmarks/literal-startup-initial-flags-b.json).

Full M3/M6 remains open: eleven finite constructor metadata gaps, complete
default/output/eval lifecycles, include-operand multiplicity and runtime decoding,
real self-exec, remaining CLI presentation/options, native loading, enabled Guile,
locales and broader Linux/integration profiles. The input-class controls retain
their value even though both classes now select the same production startup path.
