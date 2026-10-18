# rmake

A planned Rust-based implementation of GNU Make.

Maintained by [Jeff DaSilva](https://github.com/jeffdasilva) at
[jeffdasilva/rmake](https://github.com/jeffdasilva/rmake).

## Status

Implementation has started with the Rust workspace, pinned reference tooling,
an independent differential harness, 148 original fixtures, and GitHub CI.
The executable currently supports help/version only; Makefile evaluation is not
implemented. See [implementation progress](docs/progress.md) for completed work
and remaining gates.

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

Clone the repository and view the available scaffolding command:

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
