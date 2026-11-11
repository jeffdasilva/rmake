# Shuffle traversal on the pinned Linux profile

`--shuffle[=MODE]` accepts case-insensitive `none`, `identity`, `reverse`,
`random`, and decimal unsigned seeds. Bare `--shuffle` selects random; a following
word remains a goal. Startup canonicalizes mode spelling and Linux unsigned-long
conversion before command-line eval buffers run. Generated random seeds become
numeric recursive flags. Later decoded flags change requested/exported metadata,
while the running process retains its startup traversal policy and failure suffix.

The separate owned planner permutes goal and prerequisite traversal indices.
Declaration order remains available to automatic variables. Its iterative graph
walk records visited files without native recursive graph descent. Secondary
expansion and accepted implicit rules regenerate traversal at their own seed
boundary. Initial makefile-remake goals keep their read order; ordinary goals
are planned after rereading. The first static double-colon branch and subsequent
branches preserve their distinct GNU behavior.

An effective `.WAIT` prevents a new permutation of the prerequisite list.
When implicit rule selection inserts an effective barrier, traversal visits the
implicit prerequisites in declaration order, then follows any existing explicit
permutation. Trailing markers have no following edge. Empty `.NOTPARALLEL`
disables graph shuffling, and targeted serial lists retain their barriers. FIFO/pipe jobservers,
output synchronization, recursive invocations and restarts use the existing
runtime. Recipe failures append the active canonical `shuffle=MODE`, including
ignored failures. Circular traversal separates the skipped edge from GNU's
physical declaration-slot removal quirk using owned indices; no dangling pointers
or deliberate memory corruption are reproduced.

Secondary expansion retains normalized per-file `MAKEFLAGS`/`MFLAGS` in the
reusable target context without inheriting its temporary automatic variables.
A scoped MAKEFLAGS append reads the visible append base and flavor while checking
the global destination's priority separately. Nested immediate evaluation can
refresh that base; no mutable binding borrow crosses an evaluator callback.

The disabled planner has no allocated shuffle state. This adds no filesystem
cache, persistent dependency cache, unsafe code, dependency or GNU fallback.
Performance parity and safe persistent caching remain separate work.

The [opt-out controls](benchmarks/shuffle-index.md) retain four 101-sample elapsed
reports and eight repeated allocation profiles. Request counts are unchanged on
the two core inputs. Elapsed results are mixed: large-noop medians are about 0.8%
lower, expansion medians 1.4–2.0% higher; GNU remains faster. No general performance
win is claimed.

## Exact and semantic evidence

1,995 required original cases agree with two bounded pinned GNU Make 4.4.1
observations and frozen debug/optimized builds:

- 1,368 graph/mode/source/byte cells across nineteen graph shapes, twelve mode
  spellings, three startup sources and ASCII/non-UTF-8 names.
- 175 active/requested phase models, eight scope controls, eight double-colon
  and makefile-remake controls, and 45 initial graph pilots.
- 288 parallel transport/output/barrier controls, nine numeric metadata cases,
  24 failure cases, eight recursion/restart cases, 30 startup rejection/effect
  cases and 32 scoped append flavor/priority cases.

Preceding rmake fails all 1,995 cases. Exact statuses, output bytes and filesystem
types/modes/links/content agree; ordered file witnesses establish traversal and
preceding effects. No execution reaches its bound or leaves a background child.
The [proof manifest](../tests/research/shuffle-order.json) identifies the
[compressed raw observations](../tests/research/shuffle-order.json.gz).

Random and zero seeds are separately gated by `make test-shuffle`, also included
in development/optimized CI and acceptance. Its 72 engine cells have repeated
bounded observations. It checks exact metadata format, declaration-order
automatic variables, unique leaf permutations and the root completion witness.
Nonzero reported seeds must match an independent arithmetic permutation model
and replay exactly in the same engine with that explicit seed. Zero seeds
self-seed at traversal time, so their gate checks permutation properties without
inventing cross-engine or fixed-seed reproducibility. These cells earn zero
additional exact-conformance or safety-exception credit. Validator tests reject
changed status, streams, bounds, automatic order and duplicate leaves.

Original research remains in its [manifest](../tests/research/shuffle-research.json)
and [compressed archive](../tests/research/shuffle-research.json.gz), including
inputs, generation scripts and unmodified observations. The initial matrix had
114 duplicate IDs and 36 wrong cycle-slot models; separately named refinements
preserve all original records. Another 43 phase hypotheses, two scope hypotheses
and two makefile-order hypotheses were rejected and independently refined.
The first qualified implementation still failed two refined scoped append
controls; both debug/release failures are retained and closed by the selected
implementation. Historical source snapshots are unavailable; archived revision
sources identify the selected implementation only. None of this history earns
extra conformance or performance credit. Compressed/uncompressed hashes are gated.

Five proof tests and three validator tests protect these boundaries. Core gates
include formatting, Clippy, 525 Rust and 569 tooling tests. Full frozen
rmake-driven `m5-acceptance` and exact-head development, optimized and aggregate
GitHub CI remain merge gates; the PR records final results.

The ordinary corpus becomes 24,886 required / zero pending across 150 families.
The same 17 approved safe-error exceptions / 68 separately gated cells remain
unchanged. Full M3/M6 remain open. Shuffle's broad malformed-usage/lifecycle,
cycle freshness/trace, alias/VPATH and extension/profile cross-products retain
their ordinary acceptance work. In particular, empty `--shuffle=` still needs
GNU's complete usage/continuation behavior; it is not an approved safe-error
exception. Directory/debug/database output, native loading, enabled Guile,
locales, real exec/stdin and expanded integration remain in the broader audit.

All implementation, probes, mathematical models and fixtures are original.
Pinned GNU source was studied as a behavioral reference; no code or tests were
copied or translated. The profile is x86-64 Linux/glibc, C locale, disabled Guile,
identity `92525c9784f4dd471cceb12ce66a9d367e267b638e1384cdc29962b738d16de7`.
Oracle SHA256: `b8806e8eb7452ed9b7ed5e18cb7e76ff9abd488c9d62a266a398d8044fade974`.
Selected debug SHA256: `fc1a180bbdc5cfb64b215ec2c53a7ebe6efed303c577bd01a37262e70a530c36`.
Selected release SHA256: `a4f38cc0897deed35c99c401c9eff9f56cb386e918d20919d1c423563dccc8d2`.
