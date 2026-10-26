# File records and intermediate cleanup

GNU Make's final intermediate removal order follows its file-record table. The
ordering is observable even with only two independent chains (`rm b.mid a.mid`),
and collisions mean sorting names or hashes cannot reproduce it.

The core now owns a separate filename index, independent of rule bodies and
explicit-name flags. It uses safe owned byte names, the pinned Linux string
fingerprint, linear collision resolution, deletion markers, and the 93.75%
empty-slot threshold. Growth and same-size compaction retain ascending old-slot
reinsertion order. A filename merge removes the source key while preserving the
existing destination slot. Restart databases clone the index along with rules.
There are no raw pointers, new dependencies or persistent filesystem observations.

Registration follows reading and selection phases: command goals precede suffix
metadata; explicit immediate inputs precede targets; target-only bindings and
include records enter during reading; static inputs remain distinct from explicit
mentions; deferred inputs enter when expanded; accepted implicit chains install
reverse candidate inputs and their saved child lists. Built-in suffix-name
records enter after the read phase in the pinned Linux declaration order, rather
than iterating Rust's randomized rule map. This order is declarative compatibility
data; the suffix names also occur in the existing attributed GNU default data.

Cleanup scans the index after jobs finish. An intermediate created by an
expansion-only recipe is eligible even when no child process ran. Preexisting
explicit intermediates remain protected, as in GNU. Phony intermediates can be
removed. Failed unlink operations retain GNU's partial command, exact diagnostic
and command restart behavior; missing outputs remain silent. Query, touch, silent,
dry and parallel modes retain their separate policies.

The slice adds 35 required original Makefiles: 32/256/512-chain programs cover
collisions, growth, multiple inputs, deferred/static dependencies, scope-only
records, explicit goals, built-ins, eval and includes; cleanup controls cover
phony outputs and directory unlink failures in four modes. Exact independent
stream/status anchors and complete filesystem comparisons are retained. The
historical `chain-two-roots` mismatch is promoted to required coverage. Rust tests
check observed numeric fingerprint boundaries, collision deletion/merge behavior,
large index growth, independent clones, and consistency of the declarative names.

GNU Make 4.4.1 `src/hash.c` (string fingerprint and table policy), `src/file.c`
(`enter_file`, `rehash_file`, `enter_prereqs`, `remove_intermediates`), `src/read.c`
(`record_files`), `src/implicit.c` (accepted dependency installation), and
`src/main.c` / `src/default.c` (initial and late suffix records) were consulted.
The Rust data structures and tests were authored independently; no GNU C function
or upstream test is included. Numeric vectors were measured using original,
oracle-only instrumentation of its exported string hash. GNU remains a test tool.

This establishes the covered registration and cleanup paths. Hidden prerequisite
snap effects, archive-container registration and the remaining M4 audit gates are
separate work; this slice alone does not complete M4.

Validation: `make CANDIDATE=/tmp/rmake-record-final-debug ci` passed on this
source. All 4,456 oracle cases and 4,445 required candidate cases passed; eleven
historical mismatches remain pending. The 65 stateful scenarios / 287 invocations,
112 timestamp-fault programs, 61 jobserver cells, repeated 720-cell graph progress
campaigns for each engine and top-level self-host gates passed. Core checks passed
388 Rust tests and 285 tooling tests. Debug SHA-256:
`207b7291dfcfcf7095756cc552b4fe8e0be1c0b18915196a8270fb82a8129f73`.
Release SHA-256:
`04f6e172292bcbdae29912bb107ce1ad0c8bb0672198ae7242daf94f7c5a7656`.

A paired 101-sample core benchmark (two warmups, alternating engine order, warm
OS cache, exact status/stream checks) measured median milliseconds:

| Workload | GNU | This release | Previous main |
| --- | ---: | ---: | ---: |
| startup | 0.625 | 0.706 | 0.706 |
| 10,000 target no-op | 11.472 | 34.491 | 31.459 |
| expansion | 2.811 | 4.755 | 4.755 |

The filename index adds measurable no-op cost; this is compatibility work, not a
speedup claim. Avoiding redundant registration during rule storage is follow-up
performance work. There was no concurrent validation during measurement. The
ignored report `artifacts/bench/file-record-core-101.json` retains all samples,
identities and limitations. There is no CPU isolation, CPU/RSS/I/O profiling or
threshold gate; GNU remains faster in these workloads.
