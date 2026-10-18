# rmake

A planned Rust-based implementation of GNU Make.

Maintained by [Jeff DaSilva](https://github.com/jeffdasilva) at
[jeffdasilva/rmake](https://github.com/jeffdasilva/rmake).

## Status

This repository contains the initial project scaffolding only. There are no Rust
source files, Cargo package, or executable yet. Compatibility goals, architecture,
and implementation milestones will be defined in the next planning step.

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
| `LICENSE` | Apache License 2.0 |
| `.gitignore` | Ignore Rust build output and local editor/OS artifacts |
| `.editorconfig` | Consistent whitespace and line endings |

## License

Copyright 2026 Jeff DaSilva.

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for the
full license text.
