# Rule filename sequences

The Linux GNU Make 4.4.1 profile uses filename syntax, rather than ordinary
function-word splitting, for targets and prerequisites. This M4 follow-up keeps
those filename boundaries through parsing, secondary expansion and implicit
search. It does not complete M4 or the reader/CLI compatibility gates.

## Implemented behavior

- A space or tab separates filenames. An odd run of backslashes before a blank
  quotes the blank; pairs of backslashes collapse at that boundary. Other
  backslashes remain filename bytes. Inter-token whitespace includes vertical
  tab, form feed and carriage return; those bytes inside a filename remain.
- Literal colons lose their quoting before rule-name processing; paired
  backslashes immediately before the rule delimiter collapse as well. The first
  unquoted pipe starts order-only prerequisites. Quoting a pipe is specific to
  the normal side: subsequent pipes and backslashes on the order-only side
  remain names. Matched filenames containing a pipe never change that role.
- Targets and target-specific bindings accept quoted blanks and ordinary
  backslashes. Percent quoting still has its own pattern semantics. Raw target
  words normalize whitespace during prefix reading, while whitespace supplied
  by a variable reference survives that phase. In particular, a raw quoted tab
  and a tab supplied through a reference can name different targets under GNU.
- Ordinary target/prerequisite globs reuse the invocation's directory observer
  and native matcher. Results use NUL separators, so space/tab/raw-byte names
  remain individual filenames. Sorting, escaped metacharacters and retention
  of unmatched patterns follow the existing rule-glob boundary. Target globs
  run before prerequisite-tail expansion; prerequisite globs run after that
  expansion. Archive groups retain their separate member glob behavior.
- Secondary expansion parses quoted blanks and normal/order-only boundaries
  before expanding globs. A glob match is already one filename and cannot
  introduce another separator.
- Immediate implicit prerequisites get a single-filename pass after stem
  replacement. Embedded blanks and pipes remain bytes, while wildcards
  introduced by a stem still expand. Leading whitespace is skipped only in
  that single-filename pass. Deferred implicit prerequisites retain their
  separate list parsing and directory reconstruction.
- Static-pattern validation checks the full target. GNU's subsequent stem
  calculation substitutes words separately; prerequisite replacement does so
  again but retains the result as one filename. Preserve literal escaped
  percents when there is no replacement, handle empty stems and remove an
  erased prerequisite.

`filenames::Words` borrows unchanged names and owns only tokens whose quoted
blanks consume backslashes. Archive/function/include sequences share this
scanner. The common raw target prefix also stays borrowed until appended to the
expanded line, avoiding its previous temporary allocation. These changes add no
unsafe code, dependencies, persistent graph cache or new filesystem cache.
Existing directory-observation invalidation and expansion limits remain in use.

## Independent evidence

109 original required Makefiles cover backslash parity one through six, leading/
trailing/repeated blanks, raw bytes and internal control whitespace, targets and
scopes, colon/pipe quoting, duplicate automatic variables, static/implicit rules,
read/eval/secondary expansion, glob sorting and timing, archive names and
normal/dry/query/touch/parallel controls. All stdout, stderr and exit statuses
have independently authored anchors; the differential runner also compares
complete observed file bytes, types, modes and links. Twenty pass and eighty-nine
fail on preceding main's release executable. No prior expectation is relaxed.

Two original ten-step stateful scenarios add exact file bytes and fixed
nanosecond timestamps through input/output deletion, restoration, order-only
changes, wildcard membership changes, current/outdated queries, dry runs,
parallel builds and forced rebuilding. The existing stateful validator's
sixteen-step bound remains unchanged.

Corpus after this slice: 4,020 curated cases, with 4,008 required matches and
12 unchanged pending cases; 63 required stateful scenarios / 261 invocations per
engine. Rust tests: 383. Tooling tests: 279. Run the development gates through
`make ci`; optimized comparisons use `make CANDIDATE=$PWD/target/release/rmake
test-diff test-stateful`. `make BENCH_SUITE=rule-filenames bench-compare` adds four
no-child workloads for quoted declarations, ordinary globs, secondary globs and
implicit names with spaces. Fixture preparation is outside timing.

## Performance

Five paired campaigns use 101 timed samples per engine, two warmups, alternating
engine order and warm OS caches. They check identical stdout/stderr and success
status. Fixture preparation and builds are outside timing, and no local
functional tests run concurrently. These are selected elapsed-time measurements,
without CPU isolation, CPU/RSS/I/O profiles, confidence intervals or release
acceptance thresholds. Reduced copying does not establish a general speedup.

The new workloads cannot run on preceding main, which lacks filename quoting
and ordinary rule globs. Their baseline is an **unreleased development
prototype** with the same new behavior, before the boundary-based filename scan
and fast unquoted colon/pipe paths. It is not a released-version comparison.

| Workload | Current rmake ms | Prototype ms | GNU ms |
| --- | ---: | ---: | ---: |
| rule-filenames-quoted-2000 | 6.994 | 7.084 | 4.835 |
| rule-filenames-glob-1000 | 2.685 | 2.625 | 1.594 |
| rule-filenames-secondary-1000 | 2.495 | 2.431 | 1.546 |
| rule-filenames-implicit-500 | 3.364 | 3.304 | 1.864 |

The remaining controls use the **preceding main release**, from merge
`53835c52ad38b3e227117acbbdc6c5c67b41c44c`. All values, including costs, are retained:

| Workload | Current rmake ms | Previous main ms | GNU ms |
| --- | ---: | ---: | ---: |
| tiny-noop | 0.604 | 0.592 | 0.555 |
| large-noop | 30.141 | 29.568 | 11.648 |
| expansion | 4.780 | 4.617 | 2.817 |
| wildcard-cold | 0.906 | 0.899 | 1.116 |
| wildcard-warm | 11.222 | 11.209 | 37.365 |
| wildcard-complex | 15.191 | 15.063 | 40.415 |
| wildcard-literal | 1.055 | 1.057 | 0.966 |
| implicit-unmatched-64 | 1.698 | 1.666 | 0.924 |
| implicit-unmatched-256 | 2.283 | 2.229 | 1.340 |
| secondary-ready-256 | 1.428 | 1.404 | 0.833 |
| secondary-ready-1024 | 3.784 | 3.641 | 1.540 |
| secondary-empty-256 | 1.131 | 1.100 | 0.998 |
| secondary-targets-1000 | 4.608 | 4.576 | 3.111 |

GNU remains faster in the new filename workloads. Some older wildcard controls
remain faster in rmake, but the full set does not establish general GNU parity
or a blanket improvement over previous main. The explicit/deferred filename
semantics require additional parsing; selected control costs remain open
performance work. No persistent caching or skipped filesystem observations were
introduced to improve a benchmark.

Executable SHA-256 identities:

- Current release: `203e6e03db7835fb3c1f0d8985a6f71586e1b73c0f53eb4c5ed872dc08fc327d`.
- Previous main: `a1d20621627c0c18f43303fe5b9639d7231826f53a34f5ef011ae5a7214db305`.
- Unreleased scanner prototype: `39c00ef633e8dc61803d1805c031608e9743bedd0eeba3e4a8f8501220fc61c9`.
- GNU Make 4.4.1 oracle: `b8806e8eb7452ed9b7ed5e18cb7e76ff9abd488c9d62a266a398d8044fade974`.
- Oracle profile: `92525c9784f4dd471cceb12ce66a9d367e267b638e1384cdc29962b738d16de7`.

The ignored raw reports retain executable identities, workload source hashes,
input descriptions, arguments, all samples and paired ratios:

- `artifacts/bench/rule-filenames-final-rule-filenames-101.json`.
- `artifacts/bench/rule-filenames-final-core-101.json`.
- `artifacts/bench/rule-filenames-final-wildcard-101.json`.
- `artifacts/bench/rule-filenames-final-implicit-search-101.json`.
- `artifacts/bench/rule-filenames-final-secondary-101.json`.

Reproduce through `make BENCH_SUITE=<suite> BENCH_REPEAT=101
BENCH_CANDIDATE=<current> BENCH_BASELINE=<baseline> bench-compare`. Use the
prototype only for `rule-filenames` and previous main for the controls; omitting
`BENCH_BASELINE` still compares current rmake with GNU. The earlier development
campaigns are retained separately and are not represented by the final values.

## Source reference and remaining gates

Consulted the verified GNU Make **4.4.1** release archive, SHA-256
`dd16fb1d67bfab79a72f5e8390735c49e3e8e70b4945a15ab1f81ddb78658fb3`:

- `src/read.c`: `get_next_mword`, `eval`, `find_map_unquote`,
  `find_char_unquote`, `unescape_char`, `parse_file_seq`, `record_files`.
- `src/file.c`: `split_prereqs`, `enter_prereqs`.
- `src/implicit.c`: `pattern_search`, specifically immediate single-word and
  deferred list passes, globbing and order-only roles.

References establish phase boundaries and observable semantics. The Rust
implementation, fixture Makefiles, scripts and output anchors are independently
written; no upstream implementation or tests are copied or translated.

Broader escaped-semicolon reader/replay behavior and tilde expansion in rules
remain open. So do post-read MAKEFLAGS changes, wider file-record/directory/cache
phase interactions, archive/runtime/locale/profile matrices, release-scale
compatibility and persistent-cache correctness. This slice supplies selected
filename and glob cases, not a claim that every filename/reader interaction
matches GNU. M4 and M8 remain open, and general GNU speed parity is unproven.
