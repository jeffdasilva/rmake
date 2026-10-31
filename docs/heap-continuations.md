# Heap expression continuations

General variable bodies, function arguments, lazy branches, computed names,
substitution references, lexical loops and recursive user calls can now suspend
on an explicit heap job stack. A body can evaluate a function at every level,
rather than only at the end of a literal chain. The previous 128-level native
call guard is unchanged. Dispatch thresholds choose a bounded native fast path
or the heap evaluator; they do not reject a language expression.

The expression reader advances only to its next reference. It does not parse
later syntax before earlier file, shell, eval or diagnostic effects. Function
argument ranges share a borrowed source or an owned variable-body snapshot;
lookup results are not cached. Arguments run left to right, lazy branches run
only when selected, and each loop body performs fresh lookups. Eval rebindings
cannot invalidate suspended source bytes. Canonical eager builtin operations
are shared by direct and heap dispatch, rather than implemented twice.

Shallow literal references and whole eager calls use bounded native preparation.
The dispatch choice is fixed before argument effects: after preparation starts,
it cannot fall back and replay those arguments. Eligibility checks perform no
callbacks or warning-policy queries. Additional machinery takes over before the
native guard, including for nested lazy calls. Existing pure alias, text-frame
and one-reference continuation paths remain available. Rejected preparation
and later syntax failures retain earlier effects exactly once.

Heap jobs record logical depth, the nearest error location and resumptions for
arguments, selected variables, append layers, substitutions, loops and calls.
Active variable identities, first-binding warning context and numeric/lexical
scopes are restored on success, error or cancellation. Numeric call masks retain
lexical bindings while hiding surplus arguments from earlier calls. Completed
iterations release their continuation storage; work completed is not a fuel
limit. Foreach retains its existing separator-versus-body-error precedence.

Each expansion input/value retains its 16 MiB limit. At logical depths beyond
the old native window, each active heap machine additionally bounds live source
and continuation storage to 16 MiB using conservative metadata/payload leases.
This is a control-storage bound, not a measurement of process peak memory.
Completed jobs and shared owned sources release reservations on drop; outputs,
ordinary shallow evaluation and native callbacks retain their existing limits.
Deep scopes no longer fail merely because they cross 128 user-call frames.
Parser eval/include recursion and genuinely reentrant native callbacks retain
separate native guards and broader resource acceptance remains open. None of
these internal limits is an approved GNU crash/hang exception.

457 original required cases include all 144 unchanged preceding function-body
research models, 310 additional control/effect/scope models and three scaling
workloads. They cover bodies at depths 3 through 512, eager and lazy functions,
computed/substitution names, real recursive calls, numeric masking, target and
private inheritance, scoped appends, fresh eval rebindings, exact warning/cycle
locations, ordered before/after file effects and raw NUL returns with later
effects. Every independent model agrees with two bounded pinned GNU Make 4.4.1
observations. Frozen debug and release match all 457; preceding rmake fails 296
and retains 161 controls. This closes all 80 failures in the original 144-model
research set. No expected-model refinement or safety-exception promotion was
needed. Historical research receives zero separate conformance credit.

[Raw evidence](../tests/research/heap-continuations.json) retains exact input
bytes/modes, arguments, environments, output streams, statuses, resulting files,
limits and executable identities. The ordinary corpus is now 14,629 required /
zero pending, plus the unchanged 17 separately gated approved safe errors.

Ten additional Rust tests cover 1,024 function/effect levels, 512 recursive
calls, lexical/error cleanup, foreach error precedence, cancellation, live
storage release over 10,000 iterations, oversized continuation rejection before
leaf effects, live warning-policy observations and effects before syntax errors.
These stress tests receive no GNU conformance credit. Seven tooling tests
protect inventory/evidence, unchanged research expectations, file order, scope
and mutation semantics, fatal diagnostics, raw NUL effects and every scaling
read, including a wrong-value negative control. Formatting, lint, 493 Rust and
487 tooling tests pass. Complete frozen development/optimized acceptance and
final-commit CI remain merge gates; their final outcomes are recorded in the PR.

Debug SHA256:
`c90942ad29714a783dda9c1ac29a1b720902cce29907025b9b9bf89bb56d50d4`.
Release SHA256:
`af17fb968c4a8f051ef6b832f1961a63348054a29aa8c900b23b9d0e74fb605f`.
Pinned GNU binary SHA256:
`b8806e8eb7452ed9b7ed5e18cb7e76ff9abd488c9d62a266a398d8044fade974`.
Oracle profile:
`92525c9784f4dd471cceb12ce66a9d367e267b638e1384cdc29962b738d16de7`
(x86-64/glibc, C locale, disabled Guile).

GNU Make 4.4.1 expansion/context behavior was consulted from verified archive
SHA256 `dd16fb1d67bfab79a72f5e8390735c49e3e8e70b4945a15ab1f81ddb78658fb3`.
GNU source is copyright Free Software Foundation, Inc., GPL-3.0-or-later. Rust,
fixtures and tooling are original; no upstream implementation/tests were copied
or translated. No unsafe Rust, GNU runtime fallback, semantic result cache or
additional approved safety exception is introduced.

This closes another M3/M6 slice. Parser eval/include continuations, native
callback and append/resource limits, native GNU modules, enabled Guile, real
self-exec, runtime HOME/tilde, complete CLI/debug/database output, broader Linux
and locale/integration profiles, persistent caching and general performance
parity remain in the [M3/M6 audit](m3-m6-audit.md). M3/M6 is not complete.

## Measurements

Ten sequential reports contain two batches of 101 alternating samples per engine/workload, two warmups and exact output agreement. Pinned GNU Make drives batch A; frozen rmake drives B through the top-level Makefile. No unrelated local test/build/probe campaign runs during timing. All timed inputs are correct on GNU, preceding rmake and the candidate; the newly corrected deep inputs receive no speed credit. Host activity is uncontrolled, with no confidence intervals or timing thresholds.

| Workload | GNU / candidate / preceding median ms (A) | GNU / candidate / preceding median ms (B) | Paired preceding/candidate A / B |
|---|---:|---:|---:|
| tiny-noop | 0.622 / 0.725 / 0.733 | 0.537 / 0.621 / 0.623 | 1.006 / 0.997 |
| large-noop | 10.660 / 27.593 / 27.708 | 10.896 / 28.445 / 28.640 | 0.996 / 1.001 |
| expansion | 2.698 / 4.750 / 4.699 | 2.687 / 4.792 / 4.791 | 0.993 / 1.001 |
| literal-chains-0 | 0.852 / 1.307 / 1.309 | 0.889 / 1.348 / 1.346 | 1.003 / 0.998 |
| literal-chains-32 | 2.959 / 3.879 / 3.904 | 3.021 / 3.914 / 3.964 | 0.999 / 1.007 |
| literal-chains-96 | 7.706 / 7.792 / 7.955 | 7.603 / 7.643 / 7.802 | 1.020 / 1.020 |
| text-frames-0 | 0.895 / 1.388 / 1.382 | 0.887 / 1.364 / 1.372 | 0.999 / 1.002 |
| text-frames-32 | 3.603 / 6.362 / 6.306 | 3.598 / 6.362 / 6.276 | 0.990 / 0.982 |
| text-frames-96 | 8.440 / 15.384 / 15.271 | 8.445 / 15.410 / 15.207 | 0.990 / 0.986 |
| linear-leaves-0 | 0.579 / 0.753 / 0.736 | 0.578 / 0.742 / 0.731 | 0.987 / 0.987 |
| linear-leaves-32 | 0.818 / 1.277 / 1.274 | 0.822 / 1.308 / 1.275 | 0.995 / 0.984 |
| linear-leaves-96 | 1.295 / 2.361 / 2.366 | 1.313 / 2.421 / 2.438 | 1.004 / 0.995 |
| function-bodies-0 | 0.877 / 1.354 / 1.372 | 0.903 / 1.363 / 1.366 | 1.014 / 1.006 |
| function-bodies-16 | 3.679 / 8.043 / 6.999 | 3.774 / 8.126 / 7.044 | 0.868 / 0.868 |
| function-bodies-48 | 10.229 / 25.579 / 22.074 | 10.215 / 25.297 / 21.892 | 0.857 / 0.861 |

Ratios above one favor the candidate. The 96-reference literal workload improves by about 2%; core and zero-depth controls are mixed, text workloads regress roughly 1–2%, and function-body 16/48 workloads regress roughly 15–17%. GNU remains faster. This change prioritizes finite language compatibility while retaining these performance deficits as open work; no general parity, allocation reduction or peak-memory improvement is claimed.

Raw reports: [core A](benchmarks/heap-core-a.json), [core B](benchmarks/heap-core-b.json), [literal-chains A](benchmarks/heap-literal-chains-a.json), [literal-chains B](benchmarks/heap-literal-chains-b.json), [text-frames A](benchmarks/heap-text-frames-a.json), [text-frames B](benchmarks/heap-text-frames-b.json), [linear-leaves A](benchmarks/heap-linear-leaves-a.json), [linear-leaves B](benchmarks/heap-linear-leaves-b.json), [function-bodies A](benchmarks/heap-function-bodies-a.json), [function-bodies B](benchmarks/heap-function-bodies-b.json).
