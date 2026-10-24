# rmake

A Rust implementation of GNU Make, under active development.

Maintained by [Jeff DaSilva](https://github.com/jeffdasilva) at
[jeffdasilva/rmake](https://github.com/jeffdasilva/rmake).

## Status

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
`make test-filesystem` checks nineteen Linux/glibc scenarios, including partial
reads, close failures and external directory mutations; `FILESYSTEM_CASE` selects
one for replay. `make test-wildcards` runs 100 seeded filesystem query programs
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
The [adaptive archive reader](docs/archive-reader-performance.md) reduces dense
header scan I/O while retaining fresh reads between effects.
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
