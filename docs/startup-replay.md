# Ordered startup assignment replay

Makefile restart now reconstructs CLI assignment state before repeating eager
expansion effects. The saved state precedes assignments and retains original
include operands and imported export policies. The constructor and restart use
one shared startup preparation routine, followed by the ordinary `-E` and reader
phases. This resolves the previously recorded finite CLI eval file-effect gap.

A post-assignment snapshot cannot support this correctly: applying assignments
again to final values repeats appends, retains obsolete computed names and
changes conditional assignments. The new path restores pre-assignment state,
updates the imported restart count, re-evaluates ordered assignments, then builds
fresh command-variable snapshots, defaults and include search. CLI diagnostics
retain anonymous versus real-file identity, including failure before rereading
the main Makefile. Live metadata protection activates after this phase.

An existing imported MAKELEVEL receives the preceding effective numeric depth,
as GNU's re-exec environment does. Early CLI shell requests use startup depth
zero; ordinary reader/recipe requests still use the initialized depth. Missing
startup MAKEFLAGS warnings repeat at their original phase. Recursive children
receive winning simple CLI values rather than re-evaluating their original RHS.

## Ownership and performance boundaries

Only invocations whose assignments may expand make syntax or run a shell, whose
include operands require tilde preparation, or whose startup warnings must repeat
retain pre-assignment state. Literal arguments and ordinary no-argument builds
keep the prior restart snapshot path. Shell classification is checked only for
arguments containing `!`; any dollar conservatively selects replay. This changes
state retention, not whether any initial assignment or effect runs.

The optional snapshot owns parsed database and variable state. It does not retain
filesystem observations, timestamp assumptions, process resources or native
pointers. File reads remain fresh; completed recipe/shell launches invalidate the
existing directory observations before replay. The shared preparation routine
keeps the existing cancellation, shell, output and include boundaries. No unsafe
code, persistent semantic cache or GNU execution fallback is added.

This remains an in-process restart. Complete process replacement, stdin lifetime,
raw option and directory timing, inherited option expansion, native/Guile state,
profiles and locale behavior retain their [M3/M6 audit](m3-m6-audit.md). This slice
does not establish full milestone completion or general performance parity.

## Independent evidence

67 required controls cover the exact historical reproducer, four builtin
profiles, eager/lazy effects, appends, conditional priority, computed names,
includes, current file/shell/wildcard observations, raw bytes, multiple restarts,
job/output modes, errors, warnings, imported depth, eval ordering and recursion.
Four combined-stream cases require exact stdout/stderr interleaving. File-effect
models also assert complete final bytes and absence of skipped-effect outputs.
Each control matches two bounded pinned GNU observations and the candidate.

Two output-checked scaling fixtures observe every side binding produced by zero
or 1,000 eager CLI eval assignments. A middle-value mutation fails their explicit
model. The curated corpus is 10,247 ordinary required cases, zero pending, and 17
separately gated safe-error exceptions. The earlier 10,178-case corpus passed
after the refactor. Complete final development/optimized acceptance and exact-head
CI remain merge gates.

Historical prototype observations in
[`cli-assignment-restart-effect.json`](../tests/research/cli-assignment-restart-effect.json)
are preserved. Its exact input is now required as
`cli-assignment-restart-effect`; it never received safe-error credit.

The pinned GNU source was consulted for startup ordering and re-exec environment
updates. The shared preparation routine relocates rmake's own existing Rust
implementation. No upstream implementation or tests were copied or translated.

## Frozen validation and timing observations

Formatting, warnings-denied lint, 457 Rust and 398 tooling tests passed. All 69
focused semantic/scaling controls match independent models and two pinned GNU
observations under both frozen candidates. Debug SHA256:
`984271f7564464f056df314dc2a9729057e27d68aab8674668a5503b92f1a1e4`.
Release SHA256:
`7762db039fade72cc61e38e0d77d7590fb997e9e690e56306bb8ac5cb340d249`.
Full final development/optimized acceptance and exact-head CI remain merge gates.

Five reports retain 101 alternating samples per implementation, exact output
checks and binary/profile identities. No build or test campaign ran concurrently;
other host activity was not controlled. Results remain mixed and do not prove
a statistical speedup or general parity. The new 1,000-effect workload is about
3 percent lower than preceding rmake in this run, while GNU remains faster there.

| Workload | GNU ms | Current ms | Preceding rmake ms |
| --- | ---: | ---: | ---: |

| tiny-noop | 0.681 | 0.818 | 0.810 |
| large-noop | 11.752 | 31.137 | 31.379 |
| expansion | 2.812 | 4.981 | 5.076 |
| startup-arguments-0 | 0.587 | 0.686 | 0.673 |
| startup-arguments-1000 | 2.192 | 2.087 | 2.057 |
| startup-arguments-5000 | 26.118 | 8.498 | 8.633 |
| parallel-branches-depth-1 | 87.677 | 89.813 | 89.812 |
| parallel-branches-depth-4 | 87.292 | 89.614 | 89.719 |
| eval-arguments-0 | 0.706 | 0.874 | 0.868 |
| eval-arguments-1000 | 3.479 | 2.192 | 2.192 |
| startup-effects-0 | 0.540 | 0.659 | 0.657 |
| startup-effects-1000 | 2.534 | 3.011 | 3.090 |

Raw reports: [core](benchmarks/startup-replay-core.json), [startup arguments](benchmarks/startup-replay-startup-arguments.json), [parallel branches](benchmarks/startup-replay-parallel-branches.json), [eval buffers](benchmarks/startup-replay-eval-arguments.json), [startup effects](benchmarks/startup-replay-startup-effects.json).
