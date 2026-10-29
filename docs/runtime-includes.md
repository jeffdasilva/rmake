# Runtime include metadata and current RHS state

Special MAKEFLAGS assignments now evaluate .INCLUDE_DIRS append effects with the
complete database. A new resolved include search remains owned until metadata
finishes. Nested resets can publish their own search while the outer metadata
continues; the outer completion then publishes its original search. No mutable
RefCell borrow crosses an evaluation effect. An error or cancellation drops an
unfinished publication. Each include still opens its input afresh.

MAKEFLAGS and MFLAGS canonicalization follows metadata construction. Finalization
selects the current option state after nested effects rather than restoring an
older snapshot. A passive post-read GNUMAKEFLAGS decode accumulates include
operands without reconstructing search; a subsequent actual MAKEFLAGS reset
activates those operands. These are distinct GNU phases.

Immediate RHS effects can also change the binding being assigned. Append now
uses its current value while retaining the originally selected append flavor;
final priority is checked after RHS evaluation, so a newly installed stronger
binding survives. The removed-binding append fallback is unchanged. This does
not establish complete safety or compatibility for arbitrary destructive nested
redefinitions.

A shared typed database definition entry preserves source identity independently
of evaluation location and chooses RHS timing through the assignment engine.
Literal RHS values avoid setting up a database evaluator, while still invoking
special setters. Shell assignments and values containing dollars retain full
database evaluation. No effect or filesystem observation is cached or skipped.

## Original coverage and limits

155 required original cases comprise 138 include/effect/phase controls, three
finite failure controls, twelve current-RHS value/priority controls and two
output-checked scaling controls. Include coverage crosses reader, eval and recipe
sites, direct and prepared startup, serial, pipe-jobserver and synchronized modes.
It includes old search during evaluation, nested publication, weak simple versus
recursive metadata bindings, flag-normalization ordering and passive/active late
include switches. Both frozen builds match independent models and two pinned GNU
observations for every final fixture. The preceding build fails 102 cases and
passes 53 controls. The ordinary corpus becomes 10,865 required / zero pending;
17 approved safe-error exceptions remain separately gated with zero conformance
credit.

Three core tests protect old-search visibility, failure disposal and
nested publication with current inner flags. New staged interruption coverage
adds 42 cells over seven signals and three buffering modes, including shell
capture inside metadata eval. Three repetitions pass under both builds and the
GNU self-test; candidates reap owned capture children and execute no forbidden
later effect. All preceding 378 interruption programs and arguments retain their
exact fingerprint; the full inventory is now 420.

The initial nested-metadata model omitted the plain inner directory appended by
the nested construction. Two GNU observations and source review corrected that
incomplete model; [historical evidence](../tests/research/runtime-include-phases.json) retains the original expected bytes and observations.
The initial scaling input appended bare s after complex option words, where it is
ignored. The refined input uses explicit -s to exercise silent activation while
retaining the independent expected output. No output model was rewritten to fit
candidate behavior, and neither incomplete model earned conformance credit.

The earlier nineteen startup-metadata research cells now have twelve finite
ordinary mismatches; the passive late-include gap is closed. Startup default and
identity visibility, imported SHELL, startup shell depth and eval using the old
initial search remain open. This slice does not complete M3/M6. Real exec,
directory/debug/database output, old/new-file assumptions, shuffle, native/Guile,
locales, broader profiles and integration/performance gates remain in the
[audit](m3-m6-audit.md). Development resource limits are not general compatibility
exceptions.

GNU Make 4.4.1 main.c reset/post-read phases, read.c include construction and
variable.c definition behavior were consulted from the verified archive, SHA256
`dd16fb1d67bfab79a72f5e8390735c49e3e8e70b4945a15ab1f81ddb78658fb3`.
GNU source is copyright Free Software Foundation, Inc., GPL-3.0-or-later. Rust,
models and tooling are original; no upstream implementation or tests were copied
or translated. No unsafe Rust or GNU execution fallback is added.

## Frozen validation and performance

Formatting, warnings-denied lint, 460 Rust and 426 tooling tests pass. The preceding 10,863
ordinary cases pass the development comparison; both final environment-priority
controls also pass against two GNU observations under each build. Full frozen rmake-driven
development/optimized acceptance and exact-head CI are required before merge;
the PR records their final results.

Debug SHA256:
`e6397d337972dee32c2f9bb0679420e5aa46cecae47f9072319f9bbef4c5aed5`.
Release SHA256:
`71d0560f49f629629c643c973327fe79b97d9dff6b3d1e543e96d5d6fe93a904`.

The literal-RHS fast path was added after a first timing run showed extra cost in
repeated resets. Its eligibility is syntactic and conservative: shell and dollar
values keep the evaluator; all special setters still run. The new scaling suite
requires every metadata info effect, including the initial construction, and the
expected silent activation. Missing effects or ignored switch spelling cannot
pass the model.

Reproduce through `make bench-compare BENCH_SUITE=runtime-includes
BENCH_CANDIDATE=/path/to/current BENCH_BASELINE=/path/to/preceding
BENCH_REPEAT=101` on one line. Timings include process startup, use alternating
engine order and exact output checks, and are not isolated or statistically
qualified. No general speedup or GNU parity follows from these observations.

Six sequential reports contain 101 alternating samples per engine and workload,
with exact output checks and no independent local builds, tests or probes during
timing. Host activity was not controlled. GNU and rmake both drove the Makefile
benchmark flow. Several medians are higher than preceding rmake, including about
7 percent on repeated include resets. Current-binding priority probes and full
database metadata evaluation add real work. This remains performance work to
improve, not evidence of general parity.

| Workload | GNU ms | Current ms | Preceding rmake ms |
| --- | ---: | ---: | ---: |
| tiny-noop | 0.714 | 0.893 | 0.892 |
| large-noop | 11.637 | 30.637 | 30.377 |
| expansion | 3.036 | 5.665 | 5.327 |
| startup-arguments-0 | 0.695 | 0.813 | 0.803 |
| startup-arguments-1000 | 2.309 | 2.262 | 2.184 |
| startup-arguments-5000 | 27.604 | 9.186 | 8.883 |
| eval-arguments-0 | 0.650 | 0.770 | 0.759 |
| eval-arguments-1000 | 3.055 | 2.010 | 1.924 |
| startup-effects-0 | 0.651 | 0.766 | 0.762 |
| startup-effects-1000 | 2.535 | 3.197 | 2.964 |
| initial-flags-0 | 0.663 | 0.818 | 0.807 |
| initial-flags-1000 | 2.626 | 3.060 | 2.879 |
| runtime-includes-0 | 0.696 | 0.825 | 0.822 |
| runtime-includes-1000 | 3.067 | 8.278 | 7.716 |

Raw reports: [core](benchmarks/runtime-includes-core.json), [startup-arguments](benchmarks/runtime-includes-startup-arguments.json), [eval-arguments](benchmarks/runtime-includes-eval-arguments.json), [startup-effects](benchmarks/runtime-includes-startup-effects.json), [initial-flags](benchmarks/runtime-includes-initial-flags.json), [runtime-includes](benchmarks/runtime-includes-runtime-includes.json).

[Startup metadata recursion and diagnostic frames](startup-metadata-recursion.md) subsequently close the recorded shell-depth gap and add source-aware startup error coverage. Eleven of the original nineteen research cells remain finite ordinary gaps.
