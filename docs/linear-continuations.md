# Linear continuations around full evaluator leaves

A recursive variable can suspend one literal reference surrounded by ordinary
text, `$$` or a trailing dollar. Chains of those variables now keep owned text
continuations on the heap and invoke the existing full evaluator for the leaf.
That leaf can contain functions, computed names, file/shell effects, eval,
dynamic metadata or scoped appends. The forty previously recorded finite deep
function/effect failures are closed, without raising a Rust recursion guard.

The eligibility walk performs no interpreter effects. Each suspended variable
has exactly one reference with an ASCII alphanumeric/underscore literal name.
Its remaining suffix contains no further lookup, so leaf mutation cannot change
what that continuation means. Owned prefix/suffix bytes and locations survive
eval rebinding; subsequent expansions perform fresh lookups. Branches and bodies
that cannot use this path retain the existing graph/full evaluators.

Temporary metadata/source-work budgeting rejects oversized plans before any
leaf callback. The owned leaf and text buffers are bounded, and no intermediate
expanded parent strings are retained. Full evaluation sees every suspended
parent's active name/layer key. Recursive rebinding still shares that identity;
simple rebinding bypasses it. Parent keys and first-binding warning context are
restored after success, errors or cancellation. Dynamic/scoped leaves retain
the full evaluator's existing lookup, export and effect behavior.

After the leaf finishes, each parent receives the existing raw output-size
check before its recursive NUL return boundary. A raw simple-value NUL discards
only the innermost suffix; callers retain their own suffixes. Input NULs and
literal dollars keep their separate semantics. The final output is flattened
once with exact capacity. Cancellation prevents further leaf effects and wins
over a pending value/error as before. No unsafe Rust, semantic result cache,
GNU runtime fallback or new approved safety exception is introduced.

419 original required controls include the unchanged eighty preceding research
models, 240 additional shape/leaf models, scope/warning/cycle/NUL/metadata and
rebinding controls, 24 shell/export/status models and three scaling workloads.
Each independent model agrees with two bounded pinned GNU Make 4.4.1 runs.
Frozen debug and release match all 419; preceding rmake fails 248 and passes 171.
No model refinement was needed. Leaf file effects occur once, including before
fatal cycles and after raw NUL file reads. Shell controls cover failed commands,
`.SHELLSTATUS` and active recursive exports using the original process value.

[Raw evidence](../tests/research/linear-continuations.json) retains exact input
files/modes, arguments, environments, outputs, statuses, resulting files, bounds
and executable hashes. Historical observations receive no separate conformance
or safety-exception credit. The ordinary corpus is 14,172 required / zero
pending, plus the same 17 separately gated approved safe-error exceptions.

Seven additional Rust tests cover a 5,000-frame path into a real function leaf,
selected-parent rebinding, cancellation before file effects and active-state
restoration, leaf effects before parent output-limit errors, raw NUL returns,
warning/error contexts and oversized plan rejection. The direct variable-store
unit setup does not implement eval; actual eval behavior is qualified through
the parser/database Makefile cases. These library stress tests receive no GNU
conformance credit. Formatting, lint, 483 Rust and 480 tooling tests pass; the
preceding 13,753-case corpus passes. Full frozen rmake-driven acceptance and
final-commit GitHub CI remain merge gates, with outcomes recorded in the PR.

Debug SHA256:
`9d2ec103b69954e8fd716612e803d9ac7c318de8b5fca09ca31cb1c8a59bbe25`.
Release SHA256:
`18e2e6bce602b0be76c5186cc615b3f44a3e4aaa7142043afb5a5a3cf8f78cef`.

GNU Make 4.4.1 expand.c context, active-variable and return behavior was consulted
from verified archive SHA256
`dd16fb1d67bfab79a72f5e8390735c49e3e8e70b4945a15ab1f81ddb78658fb3`.
GNU source is copyright Free Software Foundation, Inc., GPL-3.0-or-later. Rust,
models and tooling are original; no upstream implementation/tests were copied
or translated.

This closes a bounded M3/M6 slice. General function argument/body continuations,
effectful branching, deeply nested eval/include work, arbitrary nonliteral
ancestor references and broader resource/profile acceptance remain open. The
existing internal guards are not approved oracle crash/hang exceptions. Native
loading, enabled Guile, real exec, runtime HOME/tilde, complete CLI/output and
Linux/locale/integration coverage, persistent caching and general performance
parity remain in the [M3/M6 audit](m3-m6-audit.md).

## Measurements

Six sequential reports contain two batches of 101 alternating samples per
engine/workload, two warmups and exact output agreement. GNU make drives batch A
and the frozen candidate drives B through the top-level Makefile. No unrelated
local test, build or probe campaign runs during timing. Host activity is not
controlled; no confidence intervals or performance thresholds are claimed.
All timed inputs pass on GNU, preceding rmake and the candidate. Previously
incorrect deep models receive no timing credit.

Existing text controls observe every value from 1,000 eager reads at depths 0,
32 and 96. New function-leaf controls use 100 eager reads through the same depths
and a `strip` leaf. The latter also expose the preceding repeated rejected-plan
walks. Suspending the eligible path once avoids that repeated work. Core and
zero-depth controls retain their mixed/slower observations. No allocation-count
or live/peak-memory claim is made.

Run `make bench-compare BENCH_SUITE=linear-leaves BENCH_REPEAT=101`, or select
`text-frames` / `core`, through either GNU make or rmake.

| Workload / batch | GNU ms | Current ms | Preceding rmake ms |
| --- | ---: | ---: | ---: |
| text-frames-0 / A | 1.111 | 1.694 | 1.713 |
| text-frames-32 / A | 3.510 | 6.303 | 11.136 |
| text-frames-96 / A | 8.150 | 14.822 | 26.070 |
| text-frames-0 / B | 1.104 | 1.690 | 1.682 |
| text-frames-32 / B | 3.424 | 6.087 | 10.904 |
| text-frames-96 / B | 8.175 | 14.813 | 25.930 |
| linear-leaves-0 / A | 0.661 | 0.814 | 0.789 |
| linear-leaves-32 / A | 1.028 | 1.508 | 11.746 |
| linear-leaves-96 / A | 1.588 | 2.707 | 94.641 |
| linear-leaves-0 / B | 0.586 | 0.733 | 0.721 |
| linear-leaves-32 / B | 0.992 | 1.474 | 11.411 |
| linear-leaves-96 / B | 1.453 | 2.647 | 91.030 |
| tiny-noop / A | 0.574 | 0.677 | 0.667 |
| large-noop / A | 11.016 | 28.458 | 28.599 |
| expansion / A | 2.767 | 4.845 | 4.845 |
| tiny-noop / B | 0.617 | 0.708 | 0.700 |
| large-noop / B | 10.834 | 28.398 | 28.472 |
| expansion / B | 2.747 | 4.833 | 4.811 |

Paired preceding/current ratios for text reads are 1.767 / 1.793 at 32 references and 1.761 / 1.747 at 96. The previously measured 32-reference regression is removed. Function-leaf ratios are 7.778 / 7.866 at 32 and 34.998 / 34.451 at 96. GNU remains faster on these workloads; general performance parity is not established.

Raw reports: [text A](benchmarks/linear-text-a.json), [text B](benchmarks/linear-text-b.json), [leaves A](benchmarks/linear-leaves-a.json), [leaves B](benchmarks/linear-leaves-b.json), [core A](benchmarks/linear-core-a.json), [core B](benchmarks/linear-core-b.json).
