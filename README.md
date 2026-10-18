# rmake

A Rust implementation of GNU Make, under active development.

Maintained by [Jeff DaSilva](https://github.com/jeffdasilva) at
[jeffdasilva/rmake](https://github.com/jeffdasilva/rmake).

## Status

The executable now reads Makefiles, expands core variables/functions, and builds
explicit dependencies serially. An independent differential harness compares it
with pinned GNU Make 4.4.1, and GitHub CI checks each implementation slice.
This is an experimental subset, **not yet a GNU Make replacement**. Includes,
implicit rules, expansion effects, parallel jobs and many compatibility details
remain unimplemented. See [implementation progress](docs/progress.md) for exact
scope, required fixtures and open milestone gates.

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
Their reports and failure reproductions are written under `artifacts/`.

`make bench-compare` builds release rmake and records alternating paired timings
against the verified oracle, checking exact output before reporting times. These
early workloads are measurement tools, not evidence of complete compatibility or
a general speed advantage.

The serial engine can drive the repository's `build` and `check-core` targets:

```sh
target/debug/rmake -rR --no-print-directory build check-core
```

`make test-selfhost` exercises those leaf flows and a selected mode-matrix replay
without recursively invoking the full CI target. Simple commands use GNU's
direct-execution path; commands
requiring shell interpretation retain the selected shell and flags. Full process,
signal and diagnostic compatibility remains under development.

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
