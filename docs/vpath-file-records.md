# Declared file records in VPATH search

Directory search can select a filename entered by a Makefile even when that file
does not exist yet. If the original name is an explicit target, a prospective
record must itself be a target; an ordinary mentioned prerequisite or scope-only
record is not sufficient. For a non-target original, those records are eligible.
An ineligible record can still be selected when its physical file exists.
An existing local file wins. Selective `vpath` entries precede global `VPATH`,
and an earlier eligible declaration wins over a later physical search result.

A search result now carries its identity independently of an optional timestamp.
Missing declared outputs are not treated as physical files with an invented date.
The engine merges into an existing searched record before rule selection, scope
activation or secondary expansion. This also happens without `GPATH`. The
searched record retains its recipe, bindings and metadata; source prerequisites
are appended and missing bindings filled. Conflicting recipes retain GNU's three
warnings. Mixed single/double-colon records fail after those warnings.

Ordinary VPATH search without an existing destination record remains tentative.
GPATH additionally retains new searched identities. Archive containers use the
same declared-record eligibility before member lookup; implicit viability still
probes the full literal archive-member prerequisite separately.

## Merge phase and rule metadata

Double-colon record merging combines the two heads. The destination keeps its
later branches, while the source's later branches do not transfer. Head recipes
and prerequisites follow normal record-merge precedence. Deferred head clauses
keep their own expansion order; a hidden source-tail error remains unevaluated.

Prerequisites and grouped peers prepare searched identities before the ordinary
already-considered check. A completed destination is reused. A newly visited
command-line goal reaches timestamp lookup after that check and can evaluate a
merged destination again, including while an earlier worker is running. The
existing independent worker continuations, job slots, error cleanup and token
handling cover these ordinary VPATH overlaps too. Already expanded command text
retains its earlier values, while a new invocation of the recipe sees merged
bindings without replaying pattern activation.

Parsed rule names, prerequisite words, deferred clauses and command-line goals
strip leading `./` components, including repeated leading separators. Ordinary
owned prerequisite names transfer directly when no stripping is required.
GNU's record lookup also strips that prefix, but its searched hash spelling is
retained. Thus `VPATH=./src` can select a declaration looked up as `src/foo`
without merging into that hash entry. GPATH and ordinary search then differ in
the chosen build name. These lexical behaviors are tested; no canonical-path or
symlink normalization has been added.

All policy and alias data are local to a Makefile read. Metadata observations
retain the existing effect timing and restart boundaries. This slice adds no
persistent cache and does not cache filesystem timestamps across invocations.

## Original coverage

The slice adds 160 required cases under `resolution.vpath-records`. The preceding
`b6af192` release passes 23 and fails 137. Cases cover missing/physical/mentioned
and scope-only destinations, original-target eligibility, priority, lexical
spellings, local precedence, implicit chains, archives, static stems, groups,
double-colon merges, automatic variables, scopes, cycles, modes and diagnostics.
The four bounded overlap cases reuse the project's original barrier helpers;
no upstream implementation or tests were copied.

Those four cases join `make test-graph-progress`: 40 oracle self-comparisons and
40 candidate comparisons per case by default, including CI. The optimized
candidate check also runs the target with 12 repetitions. The stateful
`vpath-declared-record-transitions` scenario adds twelve invocations with
independently authored stream, byte-content and exact timestamp anchors. It
checks query/build/rebuild, removal, disabling/restoring the declared record,
local-file precedence and fresh-invocation behavior.

The full corpus now has 3,602 cases: 3,590 required matches and 12 existing pending
mismatches. Stateful coverage is 57 scenarios / 198 invocations. Internal gates
include 373 Rust tests and 261 tooling tests. Both GNU and rmake drive the
supported top-level flows; the new benchmark is exposed by
`make bench-compare BENCH_SUITE=vpath-records`.

This closes the declared-record search and ordinary VPATH merge gap. M4 remains
open for shared directory-cache membership, wider entered-file and timestamp
metadata interactions, descendant-scope audits after late merges, more
Makefile-remake/lifecycle combinations and the archive format/fault matrix.
Remaining runtime and CLI/profile surfaces also block full compatibility.
Persistent graph caching remains deferred until its validation design is proved.

## Measurements

Three original workloads prepare fixed-time physical inputs before timing and
launch no children. They measure 1,000 searched records, 1,000 original/searched
pairs, and 400 implicit rules over declared searched inputs. Their streams and
status also converge on the preceding build; that convergence does not mean it
implemented record merging. Each timed invocation checks streams and status.

A local 101-sample alternating paired comparison against `b6af192` measured:

| Workload | Current rmake (ms) | Previous rmake (ms) | GNU 4.4.1 (ms) |
| --- | ---: | ---: | ---: |
| 1,000 explicit searched records | 5.185 | 4.461 | 2.601 |
| 1,000 original/searched pairs | 6.105 | 6.356 | 2.819 |
| 400 implicit rules | 3.844 | 3.448 | 2.131 |
| Startup | 0.680 | 0.670 | 0.601 |
| 10,000 ordinary no-op targets | 29.365 | 29.580 | 11.618 |
| 5,000 expansion assignments | 4.650 | 4.541 | 2.835 |

The alias-pair median is about 4% lower. Explicit-record and implicit-rule medians
are about 16% and 12% higher. Record-aware identity preparation, alias merging and
traversal of the destination rule add work omitted by the old build. Removing an
unnecessary second filename copy keeps ordinary parser allocation unchanged for
unprefixed prerequisites. Core timings are close in this run. These observations
are a measured compatibility cost, not a general performance improvement; GNU
remains faster in all six workloads. Further profiling is still needed.

Reports retain all samples, source hashes and executable identities under ignored
`artifacts/bench/vpath-records-101.json` and `vpath-records-core-101.json`.
Runs include startup, two warmups per engine and warm OS caches, with alternating
engine order. There is no CPU isolation, confidence interval, CPU/RSS/I/O profile
or performance acceptance gate. Current release SHA256:
`eaeea3a97b6b19c8d21fe5b6cf12bb078b7413bd6ea0979e8c4cc433576eb77e`. Previous release SHA256:
`fb8de3ae655bf2250f031f66ffd9703a469d6dcd048bf4d814a0716c28e41b09`.

## Reference use

GNU Make 4.4.1 `src/vpath.c` (`selective_vpath_search`), `src/file.c`
(`lookup_file`, `rehash_file`, `rename_file`), `src/remake.c` (`f_mtime`,
`update_file`, `update_file_1`), `src/read.c` (`parse_file_seq`) and `src/dir.c`
(`file_exists_p`, `dir_contents_file_exists_p`) were consulted for eligibility,
lookup spelling, record merging and traversal phases. Directory-cache integration
remains an open follow-up. Rust changes and fixture inputs are original.
