# Initial inherited flag expansion

Inherited MAKEFLAGS and GNUMAKEFLAGS that require make expansion, shell assignment
or tilde preparation now use a database-aware initial stage before option
decoding. GNUMAKEFLAGS expands and applies its assignment tokens first, then
clears its environment binding at the current origin. MAKEFLAGS expands against
that resulting state. Explicit CLI options retain their normal switch and job
priority. Literal sources keep the existing direct decoder path.

The prepared database, filesystem runtime, inherited export policies and command
names transfer into the constructor. Already evaluated assignments and prepared
include operands are skipped there. Effects execute once at their source phase;
there is no substitution-only evaluator, duplicate environment import or replay
against the final variables. Empty results still execute expansion effects, but
do not reparse a large earlier argument stream.

The initial context contains early specials and imported variables, including the
original no-export shell. It precedes builtin/default metadata, directory changes
and inherited output synchronization. Real early includes retain filenames and
line numbers; anonymous errors retain program identity. Initial fatal expansion
also precedes help/version exits. Dollar quoting receives one expansion pass
before SP/TAB/backslash option tokenization.

For this path, Makefile restart reconstructs an initial session iteratively from
owned invocation/environment inputs, including the numeric restart/depth updates,
before constructing the next engine. It repeats early effects and fresh reads
without growing the Rust call stack. Stdin bytes stay in the existing owned
reader lifetime. This remains an in-process restart, not full process replacement.

## Evidence and boundaries

111 original semantic controls have independent exact status/stream and intended
file-effect models, checked against two pinned GNU observations and the candidate.
They cover both sources, builtin profiles, variable/function expansion, source
ordering/clearing, priority, includes, raw bytes, errors, early metadata, recipe
prefixes, recursion, directory phases, merged streams and serial/parallel restarts.
Two scaling fixtures observe every inherited effect and a raw-dollar sentinel;
middle-value and sentinel mutations must fail.

Initial shell interruption adds 42 staged signal/mode cells to the existing
runtime gate, requiring exact wait status, no later effects and owned-child
cleanup. These are runtime comparisons, separate from curated conformance credit.
The corpus is 10,360 ordinary required cases, zero pending entries, plus the
unchanged 17 separately gated safe-error exceptions.

This does not finish M3/M6. Complete ordered decoding and operand-policy handling, remaining early/default
metadata timing, reentrant initial flag resets, malformed/abbreviated options,
directory reporting, native/Guile adapters, locales, full self-exec and profile/
integration coverage remain in the [audit](m3-m6-audit.md). The finite nested
getopt research remains ordinary compatibility work. No broad output exemption,
unsafe Rust, semantic cache or GNU execution fallback is added. GNU source was
consulted for phases/origins; no implementation or upstream tests were copied or
translated.

## Frozen validation and performance observations

Formatting, warnings-denied lint, 457 Rust and 404 tooling tests passed. All 113
semantic/scaling controls match independent models and two pinned GNU observations
under both frozen candidates. Debug SHA256:
`3ba622853e5c1fe7b3f31b67d4d7f0aa37308c435914f18a1693bea3e2cf7111`.
Release SHA256:
`366b2d1b65aa19e2583ad12581a2005a227ca312318917aeb7d4074b6b9153f7`.
The prior 10,247-case corpus passed during development; full final
rmake-driven development/optimized acceptance and exact-head CI remain merge gates.

The parser continues from owned prior state, including option origins, explicit
job priority, eval deduplication and assignment offsets. It does not parse the
whole inherited prefix again for each source or for explicit arguments. The
no-inherited-token path still decodes explicit arguments once.

Six reports retain 101 alternating samples per implementation, exact output,
workload source and environment, and binary/profile identities. No build or test
campaign ran concurrently; other host activity was not controlled. Results remain
mixed: the large no-op median is about 2 percent higher than preceding rmake,
while the 1,000-inherited-effect median is slightly lower. No statistical speedup
or general performance parity is claimed. GNU remains faster on several common
workloads, including the new inherited-effect workload.

| Workload | GNU ms | Current ms | Preceding rmake ms |
| --- | ---: | ---: | ---: |

| tiny-noop | 0.690 | 0.817 | 0.831 |
| large-noop | 11.540 | 31.077 | 30.353 |
| expansion | 2.891 | 5.087 | 5.124 |
| startup-arguments-0 | 0.573 | 0.684 | 0.689 |
| startup-arguments-1000 | 2.142 | 2.028 | 2.006 |
| startup-arguments-5000 | 25.875 | 8.424 | 8.403 |
| parallel-branches-depth-1 | 87.683 | 89.833 | 89.953 |
| parallel-branches-depth-4 | 87.715 | 90.136 | 90.073 |
| eval-arguments-0 | 0.685 | 0.791 | 0.777 |
| eval-arguments-1000 | 2.992 | 1.893 | 1.904 |
| startup-effects-0 | 0.556 | 0.668 | 0.664 |
| startup-effects-1000 | 2.493 | 2.943 | 2.954 |
| initial-flags-0 | 0.573 | 0.688 | 0.693 |
| initial-flags-1000 | 2.553 | 2.851 | 2.872 |

Raw reports: [core](benchmarks/initial-expansion-core.json), [startup arguments](benchmarks/initial-expansion-startup.json), [parallel branches](benchmarks/initial-expansion-parallel.json), [eval buffers](benchmarks/initial-expansion-eval.json), [CLI effects](benchmarks/initial-expansion-effects.json), [inherited effects](benchmarks/initial-expansion-inherited.json).
