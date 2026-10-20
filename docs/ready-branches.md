# Resumable dependency branches

Single-colon dependency traversal now yields while a prerequisite runs, so an
independent sibling branch can start. A target with no commands still remains
pending until its prerequisites finish. The interpreter owns suspended rule
frames alongside recipe continuations; workers continue to own only prepared
child commands. No GNU process is used at runtime.

A frame retains the expanded rule, declaration cursor, dropped circular edges,
missing-intermediate decisions and accumulated outcome. Secondary expansion and
first-parent scope ownership are established once. `.WAIT` suspends only that
frame's earlier prerequisites; named `.NOTPARALLEL` suspends between its children.
A suspended target is distinct from a currently visited ancestor, so shared
pending dependencies do not produce spurious circular diagnostics. Dependents
use final timestamps and statuses before deciding staleness or expanding recipes.

Completion events and outer scheduler passes advance an epoch. Each blocked
frame is considered once per epoch, preventing exponentially repeated traversal
of deep and shared subgraphs while a child is still running. This is transient
scheduling state, with no persistent dependency or filesystem cache. Existing
jobserver permits still bound complete recipes, including recursive children.
Serial, dry-run, touch, query and makefile-remaking flows retain their execution
boundaries and diagnostic order. An invocation-wide stop records unrecoverable
errors: already-started recipes finish, while suspended parents and new branches
do not start work after a missing prerequisite, child failure or fatal expansion.
Keep-going permits recoverable independent work; fatal expansion still stops it.

Thirty-two new required original differential fixtures exercise nested and
80-level graphs, layered diamonds, independent command-line goals, empty parents,
shared targets, first-parent scopes, secondary expansion exactly once, duplicate
edges, local barriers, automatic lists, implicit/suffix rules, eval, cycles,
recursion, errors and keep-going. The existing `wait-graph-siblings` case is now
required. Bounded marker handshakes prove overlap; these are functional tests,
not speed thresholds. Exact output, exit status and filesystem observations are
compared with pinned GNU Make 4.4.1 without normalization.

At introduction, two new pending fixtures exposed synchronous double-colon and
missing-intermediate paths. [Double-colon continuations](double-colon-scheduling.md)
resolve the first boundary. [Intermediate continuations](intermediate-scheduling.md)
now resolve the second. The current corpus has 2,245 cases: 2,232 required and 13 pending. Pending
counts are not a complete inventory of missing GNU behavior; grouped outputs,
archive rules, implicit secondary expansion, signals, load limits and output
synchronization also remain.

The rule-frame implementation and all fixtures are original Rust/Makefile work;
no upstream implementation or test material was copied for this change.

## Completion during traversal

A completion observed while visiting a later branch can make an earlier frame
ready. The scheduler now compares the pass's epoch before choosing to block in
both goal and per-target wait loops. If a completion changed it, another pass
visits the newly ready work first. Without this rescan, a running recipe waiting
for that work can stall until its bounded fixture timeout.

`make test-graph-progress` repeats four original parallel barrier and grouped
output cases in fresh workspaces. `PROGRESS_REPEAT` defaults to 40; the underlying
`tools/conformance.py --repeat` accepts 1–1000 with exact case selection. Every
iteration retains oracle expectation checks and compares exact status, streams
and filesystem state. GNU self-comparisons precede candidate comparisons in CI.
The campaign supplements the required corpus rather than treating repeated
observations as new language features or guaranteeing absence of all races.

The first local negative-control run reproduced the old failure once in 40
replays. The fixed binary passed 160 consecutive replays of that case and the
new four-case repeated gate. These observations support the regression fix;
the bounded marker checks test progress, not machine-speed thresholds.
