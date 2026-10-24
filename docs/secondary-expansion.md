# Secondary-expansion clauses

Deferred explicit prerequisites now keep the static stem of their own
clause. A later static-pattern declaration can change the target's final recipe
stem without changing an earlier clause's stem. Ordinary deferred clauses use
the final target stem, including explicit suffix-derived stems. Recipe attachment
and overriding rotate clause metadata together with prerequisite occurrences;
implicit recipe selection preserves metadata when appending explicit clauses.

Original probes of the pinned GNU Make 4.4.1 executable establish these rules:

- Expand deferred clauses in the stored prerequisite order, starting with the
  selected recipe's clause when present. Remaining declarations retain their
  order. Do not move the recipe clause to the end for expansion.
- Automatic variables can see every immediate prerequisite, including later
  declarations, together with earlier expanded clauses. Unexpanded placeholders
  stay invisible. Preserve duplicates, first occurrences and order-only roles.
- Compute `$<` and `$+` before pairwise promotion of duplicate order-only entries.
  Promotion affects later clauses and the final recipe. `$?` is normally empty
  during this pass; `-B` populates it from known normal prerequisites.
- Keep literal prerequisite dollars intact when another clause needs secondary
  expansion. Expand and glob only the deferred occurrences.
- Attribute expansion warnings and failures to the selected recipe's first
  line, even for prerequisite clauses in included files. Without a recipe, use
  the program prefix. Clause source lines are not GNU's diagnostic context.

The core stores an optional boxed vector of deferred indices and static stems.
Ordinary dependency entries retain their compact representation, with no clause
allocation. The engine tracks readiness only during this pass and replaces each
placeholder once. Ready names are borrowed while building automatic values;
there is no filesystem, expansion-result or dependency-graph cache.

## Validation

There are 69 original required fixtures covering differing/empty/byte/directory
stems, ordinary/static mixtures, recipe placement and overrides, ready lists,
duplicate role promotion, literal dollars, scopes, eval, includes/restarts,
archive groups, double-colon/grouped targets, file effects, diagnostics and
execution modes. The preceding main build fails 61 and passes eight.

`make test-secondary` adds a deterministic generated campaign to `test`, `check`
and CI. It mixes immediate/deferred ordinary/static clauses with explicit and
pattern variable scopes. Default coverage is 100 eight-node graphs, each with
multiple interleaved declarations, duplicate normal/order-only dependencies and
shared prerequisites. Independent anchors check clause execution exactly once,
node execution and authored artifacts; the oracle determines exact trace values
and order. Seeds, declarations, profile flags, source hashes and failures are
retained for replay. A second local campaign also covers 100 graphs of 32 nodes.

At introduction, the curated corpus had 2,882 cases: 2,870 required matches and 12 pre-existing
pending mismatches. Stateful coverage remains 50 scenarios / 133 invocations.
This slice does not complete M4. The subsequent
[implicit search slice](implicit-secondary-expansion.md) adds candidate secondary
expansion. GPATH/permanent renaming, wider file-record alias behavior and the
remaining compatibility profiles still need work. No upstream implementation or
tests were copied.

## Measurement

`make bench-compare BENCH_SUITE=secondary BENCH_REPEAT=101` compares release
rmake, GNU 4.4.1 and optionally `BENCH_BASELINE=/absolute/path/to/previous/rmake`.
The fixed suite covers ready lists of 256/1,024 prerequisites, 256 empty deferred
clauses and 1,000 targets with one deferred clause each. No child recipes launch;
exact streams and status must agree before timings are accepted.

A local alternating paired comparison against the preceding main `ecd025e`
build used 101 samples per engine:

| Workload | rmake (ms) | Previous rmake (ms) | GNU 4.4.1 (ms) |
| --- | ---: | ---: | ---: |
| 256 ready prerequisites | 1.446 | 4.565 | 0.890 |
| 1,024 ready prerequisites | 3.789 | 49.715 | 1.679 |
| 256 empty clauses | 1.056 | 0.951 | 0.986 |
| 1,000 small deferred targets | 4.387 | 4.208 | 3.099 |

Skipping literal prerequisites removes the previous repeated growing-list
expansion cost. The two ready-list workloads improve substantially. The other
two workloads expose remaining costs: roughly 0.11 ms for the empty-clause case
and 0.18 ms for many small targets. A special path avoids scanning placeholders
when nothing is ready, but clause tracking/readiness still has overhead. That
small absolute cost is retained for the corrected compatibility behavior and
remains an optimization target. GNU remains faster on all four workloads.

A separate 101-sample core comparison measured startup at 0.674 ms (previous
0.678 ms), 10,000 no-op targets at 28.408 ms (28.889 ms), and expansion at
4.767 ms (4.708 ms). These small changes do not establish a general speed gain;
GNU measured 0.621, 11.630 and 2.874 ms respectively.

Both campaigns include startup, two warmups per engine and warm OS caches,
without CPU isolation, confidence intervals or CPU/memory/I/O profiling. They
ran sequentially outside functional validation. Raw samples and binary
identities remain in ignored `artifacts/bench/secondary-clauses-101.json` and
`artifacts/bench/secondary-clauses-core-101.json`. These early measurements do not
establish general performance parity or satisfy the full M8 resource/cold-cache
gates.
