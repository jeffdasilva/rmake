# Rule tilde expansion and filename phases

This Linux GNU Make 4.4.1 follow-up expands `~`, `~/path` and `~user` in rule
filenames and default-goal fallback parsing. It extends the
[filename sequence slice](rule-filenames.md); M4 and the broader compatibility
milestones remain open.

## Supported behavior

- Split filename sequences and remove leading `./` before tilde expansion.
  A nonempty expanded `HOME` supplies `~` and `~/...`; an empty or undefined
  make binding falls back to the original process environment and the existing
  bounded Linux account helper. Named users bypass make's `HOME` binding.
  Failed account resolution retains the original name.
- Expand `HOME` once for each applicable filename, without recursively
  interpreting the resulting bytes as another make expression or tilde request.
  Preserve spaces, tabs, raw bytes, colons, pipes, literal dollars and repeated
  slashes as filename bytes. Globs introduced through the home value still
  expand, and their results remain individual NUL-separated filenames.
- Resolve target names and target-specific binding names before the
  prerequisite tail or assignment value. Static target patterns retain GNU's
  no-glob/no-tilde parsing; ordinary/static prerequisites still expand tildes.
- Defer immediate prerequisite filename processing for declarations containing
  a tilde until the declaration closes. Recipes, blank lines and conditionals
  can leave it open. The next active statement or EOF supplies the diagnostic
  context; condition effects can therefore change the home selected for a
  prerequisite after its text was expanded. Preserve the missing-group-recipe
  and missing-endif error boundaries before these deferred effects.
- Evaluate post-read filename `HOME` using global bindings rather than a
  target's later automatic/scoped values. This applies to explicit secondary
  expansion and implicit search. Preserve the binding's diagnostic provenance
  after the expansion finishes, so a recipe context cannot replace it.
- Keep leading `./` introduced by the home value. GNU's complete-name lookup
  and fallback filename parsing are distinct: default goals first look up the
  normalized complete spelling, then parse a filename sequence if absent.
  That fallback supports quoted blanks, tildes and globs and still rejects
  multiple names. The tests retain GNU's unusual distinction between an
  automatically selected `./house/target` and an explicit `~/target` default.

The deferred metadata is boxed and exists only on a declaration that needs it.
Ordinary prerequisites retain their previous path. Pending consumers use stable
explicit/double-colon/implicit rule identities, preserve recipe-first ordering,
and adjust existing secondary-clause indexes when dependencies are inserted.
A single consumer takes ownership of resolved names; multiple consumers copy
only for earlier targets and move the final vector into the last consumer.
No new unsafe code, dependency, persistent graph cache or cached `HOME` value is
introduced. Existing filesystem observation/invalidation and expansion bounds
remain in force; oversized tilde fallback results are also rejected.

## Original evidence

76 required Makefiles have independently authored stdout/stderr/status anchors,
with full differential file/type/mode/link observations. They cover ordinary,
scoped, static, double-colon and grouped rules; read/eval/shell effects; global
versus target-local `HOME`; target/prerequisite/statement/EOF phases; diagnostics;
raw/blank/metacharacter paths; successful and failed account lookup; original
process-environment fallback; default goals; and normal/dry/query/touch/parallel
controls. Sixty-nine fail and seven pass on preceding main's release. No existing
fixture expectation or pending classification is relaxed.

The account test checks the selected user's home independently through Python's
passwd interface. Environment fallback preparation verifies that its destination
is the trusted runner's owned sibling `home` directory before writing there;
both engines' complete home-directory observations are compared.

`tilde-home-transitions` adds twelve stateful invocations with exact bytes and
fixed nanosecond timestamps. It changes `HOME` through an include, switches to a
home directory containing a space, changes wildcard membership, distinguishes
order-only and unrelated-home writes, and exercises query/dry/parallel/forced
builds. The sixteen-step scenario bound is unchanged.

Inventory: **4,096 curated cases / 4,084 required matches / 12 unchanged pending
fixtures**, plus **64 required stateful scenarios / 273 invocations per engine**.
Internal checks: **385 Rust tests / 280 tooling tests**. Developer gates run
through `make ci`. Optimized checks also run the required corpus and stateful
suite, self-hosted Makefile flows and repeated parallel graph cases.

## Performance

Five paired campaigns record 101 timed samples per engine, two warmups,
alternating engine order, warm OS caches and identical stdout/stderr/success
status. No local functional tests run concurrently; preparation and builds are
outside timing. These are selected elapsed-time measurements, without CPU
isolation, CPU/RSS/I/O profiles, confidence intervals or release acceptance
thresholds. Reduced copying does not establish a general speedup.

The new no-child workloads cannot run on previous main, which lacks rule tilde
expansion. They compare the finalized release directly with GNU:

| Workload | Current rmake ms | GNU ms |
| --- | ---: | ---: |
| rule-tilde-literal-2000 | 4.383 | 2.288 |
| rule-tilde-glob-1000 | 2.496 | 1.631 |
| rule-tilde-secondary-1000 | 2.377 | 1.575 |
| rule-tilde-implicit-500 | 3.387 | 1.576 |

Existing controls compare the finalized release with preceding main from merge
`8bb96b6411429cdce35c96eead8fffffa5606c9f`. All values, including costs, are retained:

| Workload | Current rmake ms | Previous main ms | GNU ms |
| --- | ---: | ---: | ---: |
| tiny-noop | 0.703 | 0.714 | 0.648 |
| large-noop | 30.059 | 29.807 | 11.329 |
| expansion | 4.668 | 4.742 | 2.838 |
| rule-filenames-quoted-2000 | 7.070 | 6.916 | 4.875 |
| rule-filenames-glob-1000 | 2.748 | 2.772 | 1.687 |
| rule-filenames-secondary-1000 | 2.577 | 2.547 | 1.622 |
| rule-filenames-implicit-500 | 3.248 | 3.277 | 1.791 |
| implicit-unmatched-64 | 1.682 | 1.686 | 0.911 |
| implicit-unmatched-256 | 2.286 | 2.301 | 1.364 |
| secondary-ready-256 | 1.357 | 1.340 | 0.758 |
| secondary-ready-1024 | 3.603 | 3.576 | 1.408 |
| secondary-empty-256 | 0.974 | 0.974 | 0.868 |
| secondary-targets-1000 | 4.565 | 4.532 | 3.044 |

These measurements do not establish general GNU parity or an across-the-board
improvement over previous main. GNU is faster in the new tilde workloads.
Existing control costs remain performance work. No persistent caching or skipped
filename effects were added to improve a measurement.

Executable SHA-256 identities:

- Final release: `454d70c8ac4aee69807840a1cf4769e9fd43fe4497f384647c12e25f3b63d661`.
- Previous main: `203e6e03db7835fb3c1f0d8985a6f71586e1b73c0f53eb4c5ed872dc08fc327d`.
- GNU oracle: `b8806e8eb7452ed9b7ed5e18cb7e76ff9abd488c9d62a266a398d8044fade974`.
- Oracle profile: `92525c9784f4dd471cceb12ce66a9d367e267b638e1384cdc29962b738d16de7`.

The ignored raw reports retain executable identities, input descriptions,
arguments, source hashes, all samples and paired ratios:

- `artifacts/bench/rule-tilde-final-rule-tilde-101.json`.
- `artifacts/bench/rule-tilde-final-core-101.json`.
- `artifacts/bench/rule-tilde-final-rule-filenames-101.json`.
- `artifacts/bench/rule-tilde-final-implicit-search-101.json`.
- `artifacts/bench/rule-tilde-final-secondary-101.json`.

Reproduce with `make BENCH_SUITE=<suite> BENCH_REPEAT=101
BENCH_CANDIDATE=<current> bench-compare`. Add `BENCH_BASELINE=<previous-main>`
for the controls; omit that baseline for `rule-tilde`.

## References and open work

Consulted the verified GNU Make **4.4.1** release archive, SHA-256
`dd16fb1d67bfab79a72f5e8390735c49e3e8e70b4945a15ab1f81ddb78658fb3`:

- `src/read.c`: `tilde_expand`, `parse_file_seq`, `eval`, its
  `record_waiting_files` boundary, `record_files`, and `record_target_var`.
- `src/implicit.c`: `pattern_search`, immediate single-word and deferred
  filename parsing after scoped expansion.
- `src/main.c`: complete default-goal lookup followed by `PARSE_SIMPLE_SEQ`.

The Rust implementation, Makefiles, scripts and anchors are independently
written. No upstream implementation or tests are copied or translated.

A known additional gap is GNU's shared filename scratch buffer: recursive
filename-sequence functions inside `HOME` can overwrite the name being parsed.
For example, a manually probed target-specific binding using
`HOME=$(if $(wildcard marker),late,early)` and an immediate value that creates
`marker` records the unexpected target `earlyarker` in the pinned GNU profile.
Owned Rust filenames do not reproduce that corruption. Wider recursive-query
and allocator/profile behavior remains open; this is not counted as a passing
compatibility case or hidden behind a new pending exemption.

Target-specific assignment RHS `eval` is another existing M3 gap. Broader
escaped-semicolon reading, post-read MAKEFLAGS replay, warning-option/NSS/locale
profiles, file-record phase interactions and persistent-cache correctness also
remain open. The original [filename notes](rule-filenames.md) describe earlier
measurements and scope, rather than a claim of complete reader compatibility.
