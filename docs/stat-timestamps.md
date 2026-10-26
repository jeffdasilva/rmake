# Ordinary stat errors and completion timestamps

Ordinary target timestamp checks now use the same bounded Linux observation
adapter as `-L`, with symlink traversal disabled. An interrupted `stat` is retried.
ENOENT and ENOTDIR are silent missing-file results; other errors print GNU's
`stat: <path>: <error>` diagnostic and leave the target missing. Paths retain raw
bytes. Successful ordinary checks still perform one metadata lookup.

This changes more than error text: treating EINTR as a missing file could reject
an existing prerequisite. Conversely, an EIO or permission failure must not be
silently accepted or hidden by a second successful observation.

## Completion state

GNU does not always read the filesystem again after processing a recipe. Dry-run,
query and touch can retain a cached timestamp or mark the file assumed-new.
A recipe containing only recursive commands instead requires a real observation.
Empty recipes, grouped empty recipes and double-colon combinations have distinct
observable behavior.

The runtime now records a compact completion policy: unchanged, preserve,
assume-new or refresh. Assumed-new participates in dependent freshness and `$?`
without inventing a wall-clock timestamp. A recursive double-colon branch that
requires refresh takes precedence over an assumed-new branch. Grouped empty
recipes can preserve the old timestamp. Real execution keeps fresh filesystem
observations; simulated completion avoids the extra stat and duplicate error.
This is invocation-local rule state, with no persistent cache or effect suppression.
Parallel dispatch and intermediate recipe continuations also retain the previous
timestamp until the final child completes; they cannot inspect a partially
rewritten target or duplicate its stat diagnostic.
The former dry-run shortcut based only on whether any command was processed is
removed: it incorrectly treated forced recursive no-ops as synthetic updates.

## Original tests

The slice adds 92 required Makefiles with independently authored exact output,
status and filesystem comparisons:

- Real symlink loops and ENAMETOOLONG paths, with/without `-L`, in ordinary,
  dry-run, query and touch modes. Missing prerequisites, explicit recipes, empty
  recipes, repair commands and silent ENOTDIR controls are separate cases.
- Sixteen parallel loop/repair controls with `-B`, `-k` and `-L` interactions.
- Fixed-timestamp dependent graphs covering ordinary empty recipes, grouped empty
  recipes, forced no-ops, double-colon branches and mixed recursive branches.

The syscall campaign retains its 52 existing link-aware programs and adds twenty
ordinary stat programs. Its fault journal, real symlink, input files and protected
file have independent full-filesystem anchors. The new programs inject EINTR,
EIO, EACCES, ENOENT and ENOTDIR across four execution modes. A tooling regression
checks that ordinary programs disable `-L` and retain retry/missing-file anchors.

Existing required fixtures and the twelve pending classifications are unchanged.
The curated inventory is **4,261 cases / 4,249 required / 12 pending**. Stateful
coverage remains **64 scenarios / 273 invocations per engine**. Internal coverage
is **385 Rust / 281 tooling tests**. `test-symlink-times`, `test-diff`, `check`
and `ci` include this coverage through the existing top-level Makefile.

## Validation and measurements

Full local `make ci` passed with the immutable final debug executable:
4,249 required curated matches, all 64 stateful scenarios, 72 timestamp-fault
programs per engine, existing generated/fault suites, 61 jobserver programs,
720 parallel comparisons per engine, 385 Rust/281 tooling tests and self-hosting.
An additional 1,216 strict combined-flag comparisons match GNU. They combine
`-n`, `-q`, `-t`, `-B`, `-k` and `-j4` across the original timestamp inputs.
The sixteen parallel failures found in that campaign have dedicated required
fixtures, rather than new pending exemptions. The exploratory report remains in
ignored `artifacts/research/completion-matrix/report.json`; it is additional
evidence and is not added to the curated fixture count.

The optimized executable drove top-level differential/stateful flows and
216 parallel comparisons per engine successfully. Previous main fails 58 of the
92 new curated cases and twelve of the twenty new ordinary stat-fault programs.

Four alternating paired suites ran after functional validation with 101 samples
per engine, two warmups, warm OS caches and exact stream/status checks. Times
include startup. There was no concurrent local validation workload. These runs
have no CPU isolation, CPU/RSS/I/O profile, confidence interval or acceptance
threshold.

| Workload | Current rmake (ms) | Previous main (ms) | GNU 4.4.1 (ms) |
| --- | ---: | ---: | ---: |
| tiny-noop | 0.671 | 0.652 | 0.563 |
| large-noop | 30.200 | 29.446 | 11.425 |
| expansion | 4.712 | 4.794 | 2.839 |
| symlink-times-plain-2000 | 4.388 | 4.300 | 2.724 |
| symlink-times-links-1000 | 6.765 | 6.711 | 3.277 |
| symlink-times-chain-500 | 3.924 | 3.768 | 2.137 |
| symlink-times-dangling-1000 | 6.368 | 6.329 | 3.115 |
| lazy-record-grouped-1000 | 11.852 | 11.947 | 5.578 |
| lazy-record-double-1000 | 12.285 | 12.288 | 6.117 |
| secondary-ready-256 | 1.449 | 1.430 | 0.829 |
| secondary-ready-1024 | 3.813 | 3.719 | 1.546 |
| secondary-empty-256 | 1.099 | 1.095 | 0.980 |
| secondary-targets-1000 | 4.595 | 4.589 | 3.063 |

Results are close or mixed: about 3% higher large-no-op time and 2% lower
expansion time, with the link-chain control about 4% higher. This
correctness change does not establish a general speedup or GNU parity. GNU is
faster in these workloads. All raw values, input and executable identities are
retained under ignored
`artifacts/bench/stat-timestamps-final-{core,symlink,lazy,secondary}-101.json`.

Final release SHA-256:
`623d49911893b31051b58932bcdbb62d52e044de09bbaebb3f326943df733425`.
Previous main release SHA-256:
`2679de9f5c1fa7460620831c4040c951cb0dcc49a93c9be1cc04881ef67dd847`.

## Reference and remaining work

Consulted GNU Make **4.4.1** from the verified release archive, SHA-256
`dd16fb1d67bfab79a72f5e8390735c49e3e8e70b4945a15ab1f81ddb78658fb3`:
`src/remake.c` (`name_mtime`, `f_mtime`, `notice_finished_file`,
`update_goal_chain`) for interrupted/error observations and completion phases.
The Rust changes, Makefiles, test helpers and anchors are independently written;
no upstream implementation or tests were copied or translated.

These cases close the selected ordinary stat and simulated-completion failures.
They do not establish every timestamp/file-record interaction or complete M4.
Hidden `.EXTRA_PREREQS`, `-l` / `.LIBPATTERNS` lookup, intermediate-cleanup table ordering (`chain-two-roots`), recursive HOME filename
scratch corruption, archive standalone-member/container error ordering, wider
archive I/O and record/alias/lifecycle audits remain open. Profile, runtime,
reader and dynamic-option work remain on the broader compatibility path.
See the [M4 exit audit](m4-exit-audit.md) for minimal confirmed feature failures.
M4/M8 and general GNU speed parity remain incomplete.
