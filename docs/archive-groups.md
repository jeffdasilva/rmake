# Archive member groups

Rule headers, prerequisites and target/pattern variable declarations recognize
archive groups: `lib.a(one.o two.o)` names two members. The same filename pass
runs after explicit secondary expansion. Static-pattern substitution happens on
each resulting member. Grouped outputs, ordinary and double-colon rules retain
their existing declaration order and completion behavior. Empty groups vanish;
member-only parentheses and unclosed groups retain GNU's literal spellings.

The ordinary rule path borrows word slices. Archive forms reuse the existing
byte-preserving filename parser used by includes and wildcard functions, without
adding filesystem access to the core. A single reverse scan locates the last
closing token, avoiding repeated suffix searches on unclosed archive sequences.
No expansion effects or filesystem results are cached by this optimization.

There are 36 original required fixtures, covering metadata projection, repeated
members, order-only promotion, variables, eval, includes, secondary expansion,
static rules, target/pattern scopes, grouped outputs, double-colon declarations,
query/dry-run modes and parallel builds. Before implementation, 32 failed against
the pinned GNU 4.4.1 executable. The corpus is now 2,664 cases: 2,652 required
matches and 12 known pending mismatches. Stateful coverage remains 48 scenarios /
122 invocations. No upstream implementation or tests were copied.

`make bench-compare BENCH_SUITE=archive-sequences BENCH_REPEAT=31` measures two
original unclosed-list workloads. Local alternating paired samples against the
preceding build measured 4,096 tokens at 2.012 ms (previous 4.407 ms; GNU 29.073 ms)
and 16,384 tokens at 5.584 ms (previous 46.064 ms; GNU 445.308 ms). Median paired
previous/current ratios were 2.173 and 8.160. These adversarial filename workloads
show a specific algorithmic improvement, not general performance parity.

A separate 101-sample core check measured tiny startup at 0.687 ms (previous
0.683 ms), 10,000 no-op targets at 28.806 ms (28.576 ms), and expansion at
4.710 ms (4.688 ms). Median paired previous/current ratios were 1.008, 0.986,
and 0.992; GNU remains faster on these workloads. These are local wall times
including startup, two warmups per engine and warm OS caches, without CPU pinning,
resource measurements or confidence intervals. Timings were rerun sequentially
after an exploratory overlapping run. Raw reports are ignored artifacts:
`artifacts/bench/archive-sequences-31.json` and
`artifacts/bench/archive-groups-core-101.json`.

Archive wildcard expansion in rule declarations, archive-aware VPATH/GPATH,
broader implicit secondary expansion, per-clause static stems and the remaining
format/fault matrix are still open. General escaped rule filenames remain an
explicit implementation limitation. This slice does not complete M4.
