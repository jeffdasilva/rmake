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
The former dry-run shortcut based only on whether any command was processed is
removed: it incorrectly treated forced recursive no-ops as synthetic updates.

## Original tests

The slice adds 76 required Makefiles with independently authored exact output,
status and filesystem comparisons:

- Real symlink loops and ENAMETOOLONG paths, with/without `-L`, in ordinary,
  dry-run, query and touch modes. Missing prerequisites, explicit recipes, empty
  recipes, repair commands and silent ENOTDIR controls are separate cases.
- Fixed-timestamp dependent graphs covering ordinary empty recipes, grouped empty
  recipes, forced no-ops, double-colon branches and mixed recursive branches.

The syscall campaign retains its 52 existing link-aware programs and adds twenty
ordinary stat programs. Its fault journal, real symlink, input files and protected
file have independent full-filesystem anchors. The new programs inject EINTR,
EIO, EACCES, ENOENT and ENOTDIR across four execution modes. A tooling regression
checks that ordinary programs disable `-L` and retain retry/missing-file anchors.

Existing required fixtures and the twelve pending classifications are unchanged.
The curated inventory is **4,245 cases / 4,233 required / 12 pending**. Stateful
coverage remains **64 scenarios / 273 invocations per engine**. Internal coverage
is **385 Rust / 281 tooling tests**. `test-symlink-times`, `test-diff`, `check`
and `ci` include this coverage through the existing top-level Makefile.

## Validation and measurements

Full local `make ci` passed with the immutable final debug executable:
4,233 required curated matches, all 64 stateful scenarios, 72 timestamp-fault
programs per engine, existing generated/fault suites, 61 jobserver programs,
720 parallel comparisons per engine, 385 Rust/281 tooling tests and self-hosting.
The optimized executable drove top-level differential/stateful flows and
216 parallel comparisons per engine successfully. Previous main fails 42 of the
76 new curated cases and twelve of the twenty new ordinary stat-fault programs.

Four alternating paired suites ran after functional validation with 101 samples
per engine, two warmups, warm OS caches and exact stream/status checks. Times
include startup. There was no concurrent local validation workload. These runs
have no CPU isolation, CPU/RSS/I/O profile, confidence interval or acceptance
threshold.

| Workload | Current rmake (ms) | Previous main (ms) | GNU 4.4.1 (ms) |
| --- | ---: | ---: | ---: |
| tiny-noop | 0.718 | 0.705 | 0.652 |
| large-noop | 31.145 | 30.484 | 11.787 |
| expansion | 4.804 | 4.961 | 2.956 |
| symlink-times-plain-2000 | 4.385 | 4.325 | 2.768 |
| symlink-times-links-1000 | 6.766 | 6.738 | 3.331 |
| symlink-times-chain-500 | 3.899 | 3.839 | 2.116 |
| symlink-times-dangling-1000 | 6.490 | 6.443 | 3.150 |
| lazy-record-grouped-1000 | 11.780 | 11.733 | 5.562 |
| lazy-record-double-1000 | 12.090 | 12.006 | 5.754 |
| secondary-ready-256 | 1.336 | 1.304 | 0.746 |
| secondary-ready-1024 | 3.675 | 3.562 | 1.433 |
| secondary-empty-256 | 1.067 | 1.065 | 0.969 |
| secondary-targets-1000 | 4.547 | 4.480 | 2.978 |

Results are close or mixed: about 2% higher large-no-op time and 3% lower
expansion time, with selected secondary controls up to about 3% higher. This
correctness change does not establish a general speedup or GNU parity. GNU is
faster in these workloads. All raw values, input and executable identities are
retained under ignored
`artifacts/bench/stat-timestamps-{core,symlink,lazy,secondary}-101.json`.

Final release SHA-256:
`e7156e043d9ef4edb9cbc75672645915471f3ea9f50d9969c7e0569da8aa18d4`.
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
