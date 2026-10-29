# Constructor metadata and output phases

Include-directory metadata now runs through the full database evaluator before
late defaults and automatic definitions. Absent shell/compiler/version/host,
Make identity, default-goal and D/F bindings remain absent at that phase. Imported
SHELL retains its early execution/origin and later normalization. Eval can include
variables using the preceding, unpublished search rather than the new path.

The numeric process recursion level is selected once after CLI effects. Metadata
may change the public MAKELEVEL binding without reselecting that level, child
depth or diagnostic identity. The constructor changes inherited output buffering
to the requested mode before directory/metadata work, retaining the current
buffer in its failure guard. Internal output diagnostics receive the selected
program identity. GNU's initial logical directory is retained without -C; a real
directory list refreshes it. Public CURDIR overrides remain independent.

Prepared startup is mandatory for executable and focused unit construction.
Duplicate Options/database snapshots and legacy replay branches are removed;
reconstruction owns and decodes the original plan. The two focused constructor
tests now enter through real argument preparation. Alias reset state is replaced
with the fresh engine, so its unused mutation helper is removed. The invocation
and initial logical directory remain owned startup state. File/include observations
retain their existing effect boundaries; no persistent cache or suppressed
expansion is introduced.

539 required original controls comprise 368 default/automatic-origin profiles,
12 eval/imported-shell profiles, 128 output transitions, four directory-rename
profiles, eight recursion-mutation profiles and nineteen historical probes. Two
bounded pinned GNU observations agree with every model; both frozen builds match.
The preceding build fails 346 and passes 193 controls. All nineteen historical
cells now match, closing the eleven recorded finite constructor metadata gaps.
The ordinary corpus becomes 11,605 required / zero pending; 17 approved safe-error
exceptions remain separate and unchanged. These bounded results do not complete
M3/M6 or the broader constructor/profile inventory.

[Raw inputs, bytes, files and identities](../tests/research/constructor-metadata.json)
retain 52 rejected initial models. Shared stdout/stderr uses one spool, so the
initial two-spool assumption was incomplete. The corrected Linux models use a
completed child write through its parent's /proc descriptor to make buffering
observable; missing or reordered markers fail. Initial rename models lacked an
explicit early shell and did not rename; corrected inputs provide /bin/sh and
verify the watched work directory moved into watched HOME. GNU observations
qualified those changes before candidate acceptance. No normalization or new
crash/hang exception is used.

Existing regression tests exposed origin/export initialization errors during
development. Frozen rejected reproductions preserve the wrong MAKELEVEL origin,
missing MAKEFLAGS export and affected environment vectors. Both final builds pass
those original controls. Exploratory failures earn no compatibility credit.

Debug SHA256:
`edbcc0f3f88a386bbdaf6b7a993c4179a9c5fd8577650db408827ed7211e5509`.
Release SHA256:
`245ae9f1d4ab1247597eabf66ca1bf758548dda609299a4f3f803b74c47cf6d3`.
Formatting, lint, 460 Rust and 447 tooling tests pass. Full frozen development/optimized
acceptance and CI for the final commit remain merge gates; the PR records results.

GNU Make 4.4.1 main.c/variable.c/read.c phase behavior was consulted from the
verified archive SHA256
`dd16fb1d67bfab79a72f5e8390735c49e3e8e70b4945a15ab1f81ddb78658fb3`.
GNU source is copyright Free Software Foundation, Inc., GPL-3.0-or-later. Rust,
models and observers are original; no upstream implementation or tests copied or
translated, no unsafe Rust added, and no GNU execution fallback is used.

Remaining M3/M6 work includes broader constructor/default/output lifecycles,
include-operand multiplicity and runtime decoding, real self-exec, remaining CLI
options/presentation, native loading, enabled Guile, locales and broader Linux and
integration profiles. General performance parity and persistent-cache correctness
retain their separate gates.

## Measurement evidence

Two batches contain 101 alternating samples per engine/workload, two warmups and
exact output checks. Ten reports ran sequentially without other local build,
test or probe campaigns during timing. Host activity was not controlled; there
are no confidence intervals or performance thresholds. Results are mixed:
several workloads are slower than preceding rmake, and the apparent runtime
include improvement in batch A does not repeat in B. No general elapsed-speed
or GNU parity claim is made. Large no-op/expansion costs remain above GNU.

| Workload / batch | GNU ms | Current ms | Prior rmake ms |
| --- | ---: | ---: | ---: |
| tiny-noop / A | 0.722 | 0.872 | 0.879 |
| large-noop / A | 11.604 | 32.372 | 31.398 |
| expansion / A | 2.914 | 4.975 | 4.987 |
| tiny-noop / B | 0.527 | 0.618 | 0.628 |
| large-noop / B | 11.397 | 30.577 | 30.239 |
| expansion / B | 2.805 | 4.951 | 4.939 |
| startup-arguments-1000 / A | 2.142 | 1.915 | 1.904 |
| startup-arguments-5000 / A | 25.922 | 8.246 | 8.014 |
| startup-arguments-1000 / B | 2.095 | 1.867 | 1.843 |
| startup-arguments-5000 / B | 26.036 | 8.146 | 8.022 |
| startup-effects-1000 / A | 2.591 | 2.995 | 2.939 |
| startup-effects-1000 / B | 2.566 | 3.087 | 2.979 |
| initial-flags-1000 / A | 2.508 | 2.804 | 2.791 |
| initial-flags-1000 / B | 2.560 | 2.819 | 2.797 |
| runtime-includes-1000 / A | 2.687 | 9.515 | 10.929 |
| runtime-includes-1000 / B | 2.722 | 10.224 | 10.128 |

Raw timing reports: [core A](benchmarks/constructor-metadata-core-a.json), [core B](benchmarks/constructor-metadata-core-b.json), [startup-arguments A](benchmarks/constructor-metadata-startup-arguments-a.json), [startup-arguments B](benchmarks/constructor-metadata-startup-arguments-b.json), [startup-effects A](benchmarks/constructor-metadata-startup-effects-a.json), [startup-effects B](benchmarks/constructor-metadata-startup-effects-b.json), [initial-flags A](benchmarks/constructor-metadata-initial-flags-a.json), [initial-flags B](benchmarks/constructor-metadata-initial-flags-b.json), [runtime-includes A](benchmarks/constructor-metadata-runtime-includes-a.json), [runtime-includes B](benchmarks/constructor-metadata-runtime-includes-b.json).

Two observations per engine/workload repeat these glibc request counters, using
matched-length binary/helper/trace paths. GNU make drives batch A; the frozen
rmake drives B. Profile inputs and output match their required fixtures.

| Workload | Prior calls | Current calls | Prior requested bytes | Current requested bytes |
| --- | ---: | ---: | ---: | ---: |
| tiny-noop | 297 | 288 | 61,742 | 45,335 |
| large-noop | 244,686 | 244,677 | 20,431,637 | 20,415,230 |
| startup-arguments-1000 | 20,603 | 20,590 | 1,330,864 | 1,313,138 |
| startup-arguments-5000 | 105,250 | 105,237 | 7,303,898 | 7,286,172 |

Tiny no-op requested bytes fall by about 26.6%; this is not a peak/live-memory or
CPU measurement. The profiler observes glibc malloc/calloc/realloc/free after its
own initialization, excludes aligned allocation entry points and samples stacks.
Do not sum byte deltas across reports with different path prefixes. All sixteen
profiles and raw traces are retained in the [allocation observations](benchmarks/constructor-metadata-allocations.json).

Input directories contain tracked original marker files so checkout preserves their existence. A dedicated tooling check rejects empty fixture directories; packaged inputs were revalidated with both GNU observations and both frozen builds before acceptance.
