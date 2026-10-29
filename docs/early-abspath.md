# Early absolute-path directory selection

GNU's relative `abspath` has no starting directory during inherited flag and CLI
expansion. Relative words contribute no result, absolute words still normalize,
and every argument expands with its effects before that selection. The logical
directory becomes available after CLI processing and directory changes. Recursive
CLI definitions evaluated later therefore retain ordinary path behavior.

The variable store now distinguishes an unconfigured library context, pending
startup directory and selected logical directory. Pure library callers retain
their existing unconfigured-context error. The startup plan still owns the
initial directory needed for later CURDIR/identity construction; its existence
does not prematurely enable relative `abspath`. Each reread restores pending
state before replaying original inputs. No filesystem access or semantic-result
cache is added.

132 new required original controls cover GNUMAKEFLAGS, MAKEFLAGS and eager CLI
definitions; relative, absolute, mixed, empty and UTF-8/backslash inputs;
directory and output modes; deferred definitions; computed variable names;
argument file effects; and generated-include rereads. Two bounded pinned GNU
observations agree with every independent model. Both frozen builds match all
132, while the preceding build fails 76 and passes 56 controls. Relative computed
names correctly produce the GNU empty-variable-name error; absolute names remain
valid. Argument file effects still happen once at their actual phase.

[Raw inputs, outputs, files, bounds and identities](../tests/research/early-abspath.json)
are retained separately from the fixtures' ordinary conformance credit. The corpus
is 11,737 required ordinary cases / zero pending, plus 17 unchanged separate
approved safe-error exceptions. No new crash/hang exception is used. This closes
a bounded startup gap; complete M3/M6 and general performance parity remain open.

Formatting, warnings-denied lint, 462 Rust and 451 tooling tests and the separate safety gate pass.
Debug SHA256:
`1fc8739ea8e3f40fcea796516ff070840c58d2f489ebd3a978fe68affb08fb88`.
Release SHA256:
`25888be46ef8083237de755455341a09848ac3a43f9a509d3c633a7c5f1441f7`.
Full frozen development/optimized acceptance and CI for the final commit remain
merge gates; the PR records their results.

GNU Make 4.4.1 main.c/function.c directory-selection behavior was consulted from
the verified archive SHA256
`dd16fb1d67bfab79a72f5e8390735c49e3e8e70b4945a15ab1f81ddb78658fb3`.
GNU source is copyright Free Software Foundation, Inc., GPL-3.0-or-later. Rust and
models are original; no upstream implementation/tests copied or translated, no
unsafe Rust added, and no GNU execution fallback introduced.

Include operand history/runtime HOME decoding, broader startup/CLI/output, true
self-exec, native loading, enabled Guile, locales/Linux profiles and integration
remain part of the M3/M6 audit. The prepared include-operand hypotheses have not
yet earned compatibility credit.

## Measurement evidence

Eight sequential reports contain two batches of 101 alternating samples per
engine/workload, two warmups and exact output agreement. These workloads have
identical behavior in GNU and both rmake builds; cases with a known incorrect
preceding result are not used for performance credit. No other local build/test/
probe campaign ran during timing. Host activity was not controlled, and there
are no confidence intervals or performance thresholds. Costs are mixed: large
no-op paired ratios are about 1.03 here, while several startup observations are
slower. This does not establish a causal general speedup or GNU parity.

| Workload / batch | GNU ms | Current ms | Prior rmake ms |
| --- | ---: | ---: | ---: |
| tiny-noop / A | 0.634 | 0.790 | 0.784 |
| large-noop / A | 11.728 | 30.040 | 31.034 |
| expansion / A | 2.840 | 5.065 | 5.038 |
| tiny-noop / B | 0.562 | 0.672 | 0.680 |
| large-noop / B | 11.718 | 30.768 | 31.624 |
| expansion / B | 2.942 | 5.222 | 5.239 |
| startup-arguments-0 / A | 0.570 | 0.690 | 0.692 |
| startup-arguments-1000 / A | 2.189 | 1.983 | 1.991 |
| startup-arguments-5000 / A | 25.980 | 8.386 | 8.434 |
| startup-arguments-0 / B | 0.663 | 0.766 | 0.754 |
| startup-arguments-1000 / B | 2.181 | 1.947 | 1.987 |
| startup-arguments-5000 / B | 26.103 | 8.275 | 8.297 |
| startup-effects-0 / A | 0.565 | 0.673 | 0.661 |
| startup-effects-1000 / A | 2.500 | 2.973 | 3.011 |
| startup-effects-0 / B | 0.592 | 0.676 | 0.679 |
| startup-effects-1000 / B | 2.506 | 2.961 | 2.920 |
| initial-flags-0 / A | 0.591 | 0.698 | 0.702 |
| initial-flags-1000 / A | 2.524 | 2.817 | 2.803 |
| initial-flags-0 / B | 0.608 | 0.696 | 0.689 |
| initial-flags-1000 / B | 2.576 | 2.910 | 2.864 |

Raw timing reports: [core A](benchmarks/early-abspath-core-a.json), [core B](benchmarks/early-abspath-core-b.json), [startup-arguments A](benchmarks/early-abspath-startup-arguments-a.json), [startup-arguments B](benchmarks/early-abspath-startup-arguments-b.json), [startup-effects A](benchmarks/early-abspath-startup-effects-a.json), [startup-effects B](benchmarks/early-abspath-startup-effects-b.json), [initial-flags A](benchmarks/early-abspath-initial-flags-a.json), [initial-flags B](benchmarks/early-abspath-initial-flags-b.json).

Repeated glibc-request observations use matched-length binary/helper/trace paths.
GNU make drives batch A, frozen rmake drives B; required fixture arguments,
environment and output are checked. Both observations repeat these exact counts:

| Workload | Prior calls | Current calls | Prior requested bytes | Current requested bytes |
| --- | ---: | ---: | ---: | ---: |
| tiny-noop | 288 | 287 | 45,343 | 45,303 |
| large-noop | 244,677 | 244,676 | 20,415,238 | 20,415,198 |
| startup-arguments-1000 | 20,590 | 20,589 | 1,313,146 | 1,313,106 |
| startup-arguments-5000 | 105,237 | 105,236 | 7,286,180 | 7,286,140 |

There is one fewer request and 40 fewer requested bytes on these input profiles;
this small change does not establish elapsed-speed improvement. The profiler
observes glibc malloc/calloc/realloc/free after initialization, excludes aligned
entry points and measures requested bytes rather than peak/live memory. Do not
sum byte deltas across different path prefixes. All sixteen reports and raw
traces are retained in the [allocation observations](benchmarks/early-abspath-allocations.json).
