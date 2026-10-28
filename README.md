# rmake

A Rust implementation of GNU Make, under active development.

Maintained by [Jeff DaSilva](https://github.com/jeffdasilva) at
[jeffdasilva/rmake](https://github.com/jeffdasilva/rmake).

## Status

M4's rule-resolution implementation is complete for the pinned Linux development
profile. The [M4 acceptance audit](docs/m4-exit-audit.md) records its combined
CI gate and limits. M5's Linux runtime implementation is complete for that
profile; its [acceptance audit](docs/m5-exit-audit.md) defines the combined
development and optimized gate. M3 and broader compatibility/performance work
remain open.

The latest follow-ups add [ordered export effects](docs/export-order.md) and
[shell argv/residual execution](docs/shell-argv-replay.md), with required
collision, scope, configuration-byte and execution-mode tests.
[Flag payload assignments](docs/flag-payloads.md) now use the ordinary evaluator;
the [M3/M6 audit](docs/m3-m6-audit.md) tracks remaining compatibility surfaces.
[Variable metadata](docs/variable-metadata.md) adds ordered `.VARIABLES` lookup
and cache lifetime, pinned metadata values and scoped `MFLAGS` updates.
[Documented safety exceptions](docs/safety-exceptions.md) return fatal errors for
17 recorded GNU crash/hang inputs and are gated separately from exact conformance.

The executable now reads Makefiles, expands core variables/functions, and builds
explicit and implicit dependencies with local parallel scheduling. An independent differential harness compares it
with pinned GNU Make 4.4.1, and GitHub CI checks each implementation slice.
Target-specific and pattern-specific variables support inheritance, private
bindings, assignment timing, and export policies. Seeded graph tests check their
interactions with shared prerequisites and recipe environments.
The `file`, `realpath` and `wildcard` functions use explicit Linux filesystem
effects, with byte-preserving paths, archive-member queries and tests for buffered
I/O failures and directory-cache refresh timing.
This is an experimental subset, **not yet a GNU Make replacement**. Includes,
implicit rules, process expansion effects, recursive jobservers and grouped
outputs have implemented slices; many compatibility details remain unfinished. See [implementation progress](docs/progress.md) for exact
scope, required fixtures and open milestone gates.
Expansion diagnostics preserve raw bytes and GNU's distinct caller/definition
locations. The [`error` function and fatal aborts](docs/fatal-errors.md),
[`info`/`warning` effects](docs/diagnostic-functions.md), and remaining context
gaps are documented in [diagnostic notes](docs/diagnostics.md).
The [runtime continuation notes](docs/fatal-continuations.md) cover post-child
fatal configuration and ordered shutdown. [Linux load and stdin](docs/load-limits.md)
and [signal shutdown](docs/runtime-signals.md) have controlled runtime gates.
[Output synchronization](docs/output-sync.md),
[interruption and completion](docs/runtime-cancellation.md), and
[recursive runtime flags](docs/runtime-flags.md) share that acceptance gate.
[Source-name sharing](docs/shared-source-names.md)
reduces repeated location allocations; `make profile-allocations` provides an
isolated development profiler.
The [hidden-prerequisite notes](docs/extra-prerequisites.md) describe
`.EXTRA_PREREQS`, snapshot effects and automatic-variable exclusion.
The [file-record notes](docs/file-record-order.md) describe deterministic intermediate
cleanup, collisions, growth and dependency registration phases.
The [library lookup notes](docs/library-resolution.md) describe `-l` prerequisites,
pattern/directory priorities and record merging.
The [rule declaration notes](docs/declaration-recording.md) explain when rules,
prerequisite filename observations and nested evaluations enter the database.
The [assignment syntax notes](docs/assignment-syntax.md) explain lexical token
boundaries, whitespace handling and remaining assignment gaps.

The target is a Linux-only Rust reimplementation of GNU Make 4.4.1, with extensive
side-by-side testing. GNU Make's source may be used as a reference under its GPL
terms. Compatibility comes first; safe performance improvements and opt-in
extensions follow.

## Design and implementation plan

- [Compatibility contract](docs/compatibility.md): exact scope, reference profiles,
  source-reference policy, and observable behavior.
- [Architecture](docs/architecture.md): interpreter, rule resolution, Linux
  execution, safe caching, and extension boundaries.
- [Testing strategy](docs/testing.md): original Makefile corpus, differential
  harness, generated cases, mutation tests, and continuous compatibility gates.
- [Implementation plan](docs/implementation-plan.md): milestones, acceptance
  criteria, Makefile-driven flows, and performance objectives.
- [Licensing and provenance](docs/licensing.md): GPL terms, reference-source use,
  attribution, and the license transition.

## Getting started

Clone the repository and run the development workflows:

```sh
git clone https://github.com/jeffdasilva/rmake.git
cd rmake
make help
make bootstrap
make check
```

Bootstrap requires Linux x86-64, Python 3.11+, a C toolchain, GNU Make, and network
access to the pinned GNU/Rust downloads. Tools are installed inside the ignored
`.tools/` directory; no system packages or shell profiles are changed. The current
fixture runner is for trusted project fixtures only, not untrusted Makefiles.
Use `make build`, `make test`, `make fmt-check`, `make lint`, and `make bench` for
individual flows. The build binary is `target/debug/rmake`.

`make test-generated` runs 200 reproducible programs containing 6,400 expression
observations by default. Increase the campaign with `SEED`, `GENERATED_CASES` and
`EXPRESSIONS`; `START_INDEX` replays a recorded program. `make test-graphs` runs
100 seeded explicit dependency graphs, with `GRAPH_SEED`, `GRAPH_CASES` and
`GRAPH_NODES` controls. `make test-modes` checks 512 ordinary/one-shell combinations
of dry-run, query, touch, ignore-error and silent modes; `MODE_CASE` selects a
recorded case for replay. `make test-stateful` compares multi-step builds and
nanosecond timestamp boundaries. These are CI gates.
`make test-intcmp` checks 200 programs with 6,400 independently predicted integer
comparison results, including GNU 4.4.1's observed negative-number ordering.
Use `INTCMP_SEED`, `INTCMP_CASES`, `INTCMP_EXPRESSIONS`, and `START_INDEX` to
scale or replay that campaign. See [integer comparison notes](docs/intcmp.md).
`make test-diagnostics` checks 206 fixed output-function scenarios with independent
stream, status and filesystem expectations, including merged output ordering.
`DIAGNOSTIC_CASE` replays one recorded cell. This is also part of `make ci`.
`make test-filesystem` checks twenty-one Linux/glibc scenarios, including partial
reads, close failures and external directory mutations; `FILESYSTEM_CASE` selects
one for replay. `make test-directory-search` checks 176 original shared-cache and
I/O programs; `DIRECTORY_CASE` selects one. `make test-symlink-times` checks 112
ordinary/link/library timestamp syscall-fault programs; `SYMLINK_CASE` selects one. `make test-archive-times` checks 2,600 original archive timestamp, read/seek and
completion fault programs; `ARCHIVE_TIME_CASE` selects one. See
[archive observations](docs/archive-observations.md). `make test-wildcards` runs 100 seeded filesystem query programs
with independent matching and complete filesystem anchors. Use `WILDCARD_SEED`,
`WILDCARD_CASES`, `WILDCARD_EXPRESSIONS` and `START_INDEX` to scale or replay them.
See [wildcard semantics](docs/wildcard.md) for the implemented profile and limits.
`make test-whitespace` checks pure-function whitespace, local bindings and effect
ordering; `WHITESPACE_CASE` selects one case.
`make test-word-index` checks `word` and `wordlist` integer boundaries, raw-byte
errors and expansion ordering; `WORD_INDEX_CASE` selects one case. See the
[word index notes](docs/word-indices.md) for the accepted numeric grammar.
Their reports and failure reproductions are written under `artifacts/`.

`make bench-compare` builds release rmake and records alternating paired timings
against the verified oracle, checking exact output before reporting times. These
early workloads are measurement tools, not evidence of complete compatibility or
a general speed advantage.
Use `BENCH_SUITE=word-spans` for six large-list workloads covering selected ranges,
full scans, `let` remainders and literal `patsubst`. The default `core` suite
retains the original three workloads; `BENCH_SUITE=all` includes every suite.
`BENCH_SUITE=target-contexts` isolates variable-scope and automatic-value costs
without launching children. See [target context measurements](docs/target-context-performance.md).
`BENCH_SUITE=wildcard` measures cold, repeated, complex and literal queries over
1,000 input files prepared outside the timed invocations.
`BENCH_SUITE=bindings` measures repeated reads of large simple global and local
values, with eight references per assignment and five replacement assignments.
`BENCH_SUITE=implicit-search` measures resolution with many irrelevant patterns.
`BENCH_SUITE=archives` measures existing archive members with fixed historical dates.
Set `BENCH_BASELINE=/absolute/path/to/previous/rmake` to compare a previous
candidate in the same campaign. See the [implicit search measurements](docs/implicit-search-performance.md).
`BENCH_SUITE=vpath-search` measures ordinary/archive search policy;
[compiled VPATH measurements](docs/vpath-search-performance.md) record the current
lookup optimization and its limits.
`BENCH_SUITE=secondary` measures deferred clauses with large ready lists,
empty clauses and many targets; see [secondary expansion](docs/secondary-expansion.md).
`BENCH_SUITE=implicit-rejections` measures repeated rejected pattern searches;
see [implicit secondary expansion](docs/implicit-secondary-expansion.md).
`BENCH_SUITE=gpath` measures permanent filename lookup, aliases and implicit rules
over fixed-time files; see [GPATH behavior and measurements](docs/gpath.md).
`BENCH_SUITE=intermediates` measures lazy inspection of missing intermediates
with current outputs; see [intermediate-file policy](docs/intermediate-policy.md).
`BENCH_SUITE=lazy-records` measures grouped and double-colon intermediate
inspection; see [lazy rule records](docs/lazy-rule-records.md).
`BENCH_SUITE=vpath-records` measures declared searched records and alias pairs;
see [declared VPATH file records](docs/vpath-file-records.md).
`BENCH_SUITE=directory-search` measures shared input probes and repeated VPATH misses;
see [shared directory observations](docs/shared-directory-search.md).
`BENCH_SUITE=archive-names` measures GNU/BSD long names and repeated archive wildcards;
see [archive name encodings and borrowed scans](docs/archive-name-encodings.md).
`BENCH_SUITE=symlink-times` measures `-L` over ordinary files and live/dangling chains;
see [symlink timestamps](docs/symlink-timestamps.md).
`BENCH_SUITE=rule-filenames` measures quoted declarations, ordinary/deferred
globs and implicit names with spaces; see [rule filename sequences](docs/rule-filenames.md).
`BENCH_SUITE=rule-tilde` measures literal/deferred tilde prerequisites and implicit search;
see [rule tilde expansion](docs/rule-tilde.md).
`BENCH_REPEAT` controls measured samples per engine (default 15).

The engine can drive the repository's `build` and `check-core` targets:

```sh
target/debug/rmake -rR --no-print-directory build check-core
```

`make test-selfhost` exercises those leaf flows and selected mode/diagnostic replays
without recursively invoking the full CI target. Simple commands use GNU's
direct-execution path; commands
requiring shell interpretation retain the selected shell and flags. Full process,
signal and diagnostic compatibility remains under development.
See [grouped output rules](docs/grouped-targets.md) for explicit `&:` and
implicit multi-output behavior. [Low-resolution timestamp comparisons](docs/timestamp-comparisons.md)
preserve GNU rebuild decisions without changing stored file metadata.
See [archive members](docs/archive-members.md) for member timestamps, automatic
variables, touch behavior and the remaining archive rule-resolution work.
The [archive resolution slice](docs/archive-rule-resolution.md) supports member
pattern search, archive suffix rules and the built-in archiver recipe.
[Archive groups](docs/archive-groups.md) work in rule and variable declarations,
including explicit secondary expansion.
[Archive rule wildcards](docs/archive-rule-wildcards.md) preserve complete member
names and GNU observation timing.
[Rule filename sequences](docs/rule-filenames.md) add quoted blank/colon/pipe
handling, ordinary rule globs and preserved static/implicit filename boundaries.
[Rule tilde expansion](docs/rule-tilde.md) adds global HOME evaluation, declaration-close
prerequisite timing, account/environment fallback and default-goal filename parsing.
The [adaptive archive reader](docs/archive-reader-performance.md) reduces dense
header scan I/O while retaining fresh reads between effects.
[Archive member-name encodings](docs/archive-name-encodings.md) preserve signed/wrapped
indexes, inline lengths and NUL-terminated markers while avoiding unnecessary name copies.
[Archive-first VPATH resolution](docs/archive-vpath.md) searches the archive name
before checking members and preserves GNU search/rebuild observations.
[GPATH permanent filenames](docs/gpath.md) retain searched rebuild paths and merge
rule, scope and job identities, including overlapping parallel alias workers.
[Declared VPATH records](docs/vpath-file-records.md) allow missing searched outputs
and ordinary VPATH record merges with target eligibility and GNU phase timing.
[Symlink timestamps](docs/symlink-timestamps.md) support `-L` / `--check-symlink-times`,
including dangling chains, search-policy boundaries and archive second comparisons.
[Shared directory observations](docs/shared-directory-search.md) preserve cached
name membership, incremental streams, refresh timing and search I/O errors.
[Secondary-expansion clauses](docs/secondary-expansion.md) retain individual
static stems, literal prerequisites, GNU traversal order and diagnostic context.
[Implicit secondary expansion](docs/implicit-secondary-expansion.md) evaluates
candidate words during search, preserving lazy effects, stem reconstruction,
temporary chain contexts and prerequisite cleanup roles.
Failed-search observations share the [filesystem invalidation boundaries](docs/implicit-search-invalidation.md)
used by directory queries, so commands and writes can make a rejected input viable.
[Intermediate-file policy](docs/intermediate-policy.md) handles global/named
secondary and not-intermediate precedence, conflicts and lazy missing-file checks.
[Lazy rule records](docs/lazy-rule-records.md) preserve grouped peer phases,
double-colon head inspection, cycle removal and parent rebuild evidence.
`make test-secondary` runs a seeded graph campaign with mixed variable scopes.
See [shell configuration notes](docs/shell-configuration.md) for literal arguments,
empty-shell behavior and the remaining newline/complex-flag boundaries.
The [invocation-depth notes](docs/makelevel.md) distinguish `MAKELEVEL` startup
state, child exports and the still-incomplete command-line replay phase.

Required/optional includes, [include search directories](docs/include-search.md),
startup [MAKEFILES loading](docs/startup-makefiles.md),
and generated makefile restarts are supported in the
[current include slice](docs/dynamic-makefiles.md). Shell functions, `!=`, and
`eval` now use explicit mutable interpreter state; see
[stateful expansion](docs/stateful-expansion.md) for supported behavior and limits.

Local `-jN` scheduling, `.NOTPARALLEL`, and recursive command bindings are now
available. The [local parallelism notes](docs/local-parallelism.md) describe the
current single-command scheduling scope and remaining jobserver work.

## Repository layout

| File | Purpose |
| --- | --- |
| `README.md` | Project overview and current status |
| `Makefile` | Development command entry point |
| `LICENSE` | GNU General Public License version 3 |
| `.gitignore` | Ignore Rust build output and local editor/OS artifacts |
| `.editorconfig` | Consistent whitespace and line endings |
| `docs/` | Proposed architecture, compatibility contract, test strategy, and roadmap |
| `crates/` | Rust command-line program and compatibility core |
| `tools/` | Toolchain/oracle bootstrap, independent conformance and benchmarks |
| `tests/` | Original fixture corpus and requirement inventory |
| `oracle/` | Pinned GNU Make source and profile configuration |

## License

Copyright 2026 Jeff DaSilva.

This project is licensed under the GNU General Public License as published by
the Free Software Foundation, either version 3 of the License, or (at your option)
any later version. SPDX identifier: `GPL-3.0-or-later`.

It is distributed without any warranty, including the implied warranties of
merchantability or fitness for a particular purpose. See [LICENSE](LICENSE) for
the full terms and [licensing and provenance](docs/licensing.md) for the
source-reference policy and historical license information.

`make m4-acceptance` runs development CI followed by optimized rule-resolution
regressions. `make test-m4-release` runs the optimized corpus, stateful and archive
fault suites, repeated parallel cases and self-hosted flows. The
[recursive HOME notes](docs/recursive-home-names.md) describe the safe byte model
and [M4 audit](docs/m4-exit-audit.md) records milestone acceptance and open work.

The [owned environment-vector slice](docs/environment-vectors.md) adds byte/order-preserving Linux child launches and export activation phase checks. [M3/M6 remain open](docs/m3-m6-audit.md).

[POSIX mode](docs/posix-mode.md) now covers reader activation, defaults, folding, shell error flags, suffix eligibility and recursive command variables; M3/M6 remain open.

[Negative switches and option origins](docs/switch-origins.md) now cover silent/keep-going aliases, explicit propagation, precedence and scoped flag normalization. M3/M6 remain open.

[Restart directory namespaces and recursive invocation identity](docs/restart-directories.md) now follow original `-C` replay, actual-directory refresh and default `MAKE_COMMAND` indirection, with 348 required cases. M3/M6 remain open.

[M3 profile inputs](docs/profile-inputs.md) now cover inherited file/directory switches, Guile-free lookup and expression reference boundaries. The [option inventory](docs/m3-option-inventory.md) lists the remaining CLI work; M3/M6 remain open.

[Explicit startup arguments](docs/startup-arguments.md) now use one parse when no inherited tokens need merging. Three required fixtures and an output-checked `startup-arguments` benchmark cover up to 5,000 overrides. General performance parity remains open.

[Late built-in disabling](docs/late-builtins.md) now reconciles `-r`/`-R` after reading, preserving user overrides and suffix recipes. Cleanup visits the fixed built-in tables, with 207 required cases and an output-checked scaling benchmark. M3/M6 remain open.

[Undefined-variable warnings](docs/undefined-warnings.md) now cover reader/runtime scopes, option sources, automatic variables and raw diagnostic order. Numeric lexical bindings survive nested calls correctly. Startup ordering and broader M3/M6 acceptance remain open.

[Command-line eval](docs/command-line-eval.md) now evaluates ordered buffers before ordinary Makefiles, with recursive transport, metadata, source diagnostics and restart replay. Broader startup and M3/M6 acceptance remain open.

[Trace output](docs/trace.md) now reports rebuild reasons, source locations and command echo across execution modes, recursion and Makefile remake phases. Broader debug, startup and profile acceptance remain open.

[Automatic snapshot preparation](docs/automatic-snapshots.md) avoids unused owned prerequisite/value snapshots while preserving freshness and grouped-target views. Raw allocation and timing evidence accompanies the change; general performance parity remains open.
