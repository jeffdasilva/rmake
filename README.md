# rmake

A planned Rust-based implementation of GNU Make.

Maintained by [Jeff DaSilva](https://github.com/jeffdasilva) at
[jeffdasilva/rmake](https://github.com/jeffdasilva/rmake).

## Status

This repository contains project scaffolding and a proposed implementation plan.
There are no Rust source files, Cargo package, or executable yet.

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
```

The current Makefile requires GNU Make. Rust build, test, formatting, and lint
commands will be added when the Cargo project is initialized.

## Repository layout

| File | Purpose |
| --- | --- |
| `README.md` | Project overview and current status |
| `Makefile` | Development command entry point |
| `LICENSE` | GNU General Public License version 3 |
| `.gitignore` | Ignore Rust build output and local editor/OS artifacts |
| `.editorconfig` | Consistent whitespace and line endings |
| `docs/` | Proposed architecture, compatibility contract, test strategy, and roadmap |

## License

Copyright 2026 Jeff DaSilva.

This project is licensed under the GNU General Public License as published by
the Free Software Foundation, either version 3 of the License, or (at your option)
any later version. SPDX identifier: `GPL-3.0-or-later`.

It is distributed without any warranty, including the implied warranties of
merchantability or fitness for a particular purpose. See [LICENSE](LICENSE) for
the full terms and [licensing and provenance](docs/licensing.md) for the
source-reference policy and historical license information.
