# Linux library prerequisite resolution

Library prerequisites such as `-ldemo` now resolve through GNU Make 4.4.1's
`.LIBPATTERNS` rules. Literal filenames and ordinary VPATH matches take precedence.
A local match wins over remote matches, including a later pattern's local file.
Remote matches are ranked by selective/general VPATH group and directory position
across all patterns. The standard `/lib`, `/usr/lib` and configured library
directory are searched after VPATH. Unknown or empty pattern lists retain the
original missing-target behavior. Invalid pattern words warn at their normal
position, and lookup stops before later warnings when a local match wins.

The pinned Linux default `lib%.so lib%.a` is included as declarative compatibility
data with its existing FSF/GPL attribution. `-R` suppresses the default; environment,
command-line and Makefile bindings retain normal precedence. The Rust/Cargo driver
sets compile-time `RMAKE_LIBDIR` to the pinned oracle prefix, like `RMAKE_INCLUDEDIR`.
Ordinary Cargo builds default to `/usr/local/lib`; packagers can configure it.
GNU is never invoked by rmake's runtime.

## Search, records and effects

Pattern text expands globally when lookup is required, with its normal effects
and definition diagnostics. Target-local `.LIBPATTERNS` does not replace that
lookup context. Percent quoting reuses the core's original percent-pattern type;
raw bytes are preserved and no filename globbing is added to library patterns.
A separate ranked VPATH iterator retains positions of nonmatching declarations.
Current-directory VPATH entries are omitted, matching GNU's search-list setup.

Selected filenames participate in existing record merges and automatic-variable
identity. An existing destination's rule and scope can win over the logical `-l`
record, with exact conflict diagnostics. Rebuilding without a permanent searched
name uses the logical target as GNU does. The pre-merge physical observation
survives a destination's later phony flag; normal phony records still skip stat.
A declared VPATH destination can be selected without a physical file and then
built, touched or queried. Implicit viability does not invent support for `-l%`
prerequisites when GNU rejects that candidate.

There is no new unsafe code, persistent cache, runtime dependency or synthesized
mtime. Ordinary filename lookup retains its fast path. Pattern, file and directory
observations are not cached across invocations or suppressed when effectful.

## Independently authored coverage

Add 160 required Makefiles under `rules.libraries`, with exact stream/status
anchors and full filesystem comparisons. They cover local/remote/system lookup,
cross-pattern priority, default/environment/command-line bindings, raw-byte names,
percent quoting, effects and errors, absent declared files, merged rules/scopes,
phony records, default goals, automatic variables, secondary clauses, implicit
negative controls, query, dry-run, touch, always-make and parallel execution.
The preceding main passes 29 and fails 131. No existing required expectation or
pending classification is weakened.

The existing timestamp fault campaign keeps all 72 programs and adds forty
library programs: stat EINTR/EIO/EACCES/ENOENT/ENOTDIR in default/`-L` mode and
four execution modes. Fault journals, symlinks and protected input bytes retain
independent anchors. Tooling checks the added programs and the distinction
between physical stat diagnostics and the logical missing library name.

One new fourteen-invocation stateful scenario checks changing pattern/directory
priority, library creation/deletion, older fallbacks that remain current under
GNU timestamps, new inputs, dry-run, parallel rebuilding, invalid patterns and
restoration. Exact output/content and fixed nanosecond anchors remain required.
The scenario pool bound grows from 64 to 128; per-step/time/resource bounds remain.
The corpus is **4,421 cases / 4,409 required / 12 unchanged pending**; stateful
coverage is **65 scenarios / 287 invocations per engine**. Internal coverage:
**385 Rust / 285 tooling tests**.

The manifest loader now accepts dotted GNU environment-variable names while
retaining reserved-name and invalid-name guards. Existing developer flows discover
all cases. `test-symlink-times` runs 112 fault programs. Both GNU and rmake drive
the top-level Makefile; `BENCH_SUITE=libraries` adds three no-child lookup workloads.

## Validation and measurements

Full local `make ci` passed on the immutable final debug build, including all
4,409 required curated matches, 65 stateful scenarios, 112 timestamp/library fault
programs per engine, 720 parallel comparisons per engine, 61 jobserver programs,
all generated/fault suites, 385 Rust/285 tooling tests and self-hosted flows.
The optimized build also drove top-level differential/stateful flows and
216 repeated parallel comparisons per engine.

Three paired suites ran after functional checks with 101 samples per engine,
two warmups, alternating engine order, warm OS caches and exact stream/status
checks. No concurrent local validation workload ran during the accepted campaign.
There is no CPU isolation, CPU/RSS/I/O profile, confidence interval or acceptance
threshold. The old build cannot run the new library workloads; before/after
comparisons use unchanged core and VPATH controls.

| Workload | Current rmake (ms) | Previous main (ms) | GNU 4.4.1 (ms) |
| --- | ---: | ---: | ---: |
| libraries-local-512 | 2.389 | not supported | 1.572 |
| libraries-remote-512 | 2.769 | not supported | 1.867 |
| libraries-ranked-256 | 2.208 | not supported | 1.711 |
| tiny-noop | 0.625 | 0.604 | 0.553 |
| large-noop | 31.624 | 32.262 | 11.441 |
| expansion | 5.042 | 5.006 | 3.019 |
| vpath-record-explicit-1000 | 5.513 | 5.620 | 2.473 |
| vpath-record-aliases-1000 | 6.470 | 6.440 | 2.726 |
| vpath-record-implicit-400 | 4.258 | 4.235 | 2.148 |

Controls are close/mixed: about 2% lower large-no-op time, 3% higher startup time
and less than 1% higher expansion time in these runs. GNU is faster in these
selected workloads. These results do not establish general GNU speed parity or
complete M8. Reports retain all samples, inputs and executable identities under
ignored `artifacts/bench/libraries-accepted-{libraries,core,vpath}-101.json`.

Current release SHA-256:
`95cc5ee1361082e09612f53db2cd0c1ccc35bf214d6d709e1321dd70ee73c53f`.
Previous main release SHA-256:
`623d49911893b31051b58932bcdbb62d52e044de09bbaebb3f326943df733425`.

## Provenance

Consulted GNU Make 4.4.1 from the verified release archive, SHA-256
`dd16fb1d67bfab79a72f5e8390735c49e3e8e70b4945a15ab1f81ddb78658fb3`:
`src/remake.c` (`f_mtime`, `library_search`), `src/vpath.c` (`vpath_search`,
`selective_vpath_search`, list construction), and `src/file.c` (`rehash_file`).
The Rust implementation, Makefiles, scripts, fault additions and anchors are
independently written; no upstream implementation or tests were copied or
translated. The single default-variable declaration extends the existing public
oracle database extraction and keeps its existing attribution and license.
