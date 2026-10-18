# Original compatibility fixtures

This corpus starts with **200 independently authored cases** across
**37 requirement families**. It is an initial behavioral sample, not a
complete GNU Make 4.4.1 specification or a claim that rmake implements these cases.
The serial slice requires **152 exact matches**; 48 cases remain explicitly
pending. Promotion to `required` needs an implementation and a successful
comparison against the pinned reference. A passing sample does not mark its whole
requirement family complete.

No GNU Make implementation files, upstream tests, or manual example programs were
used to create these fixtures. Requirement links identify public behavioral
specifications. Fixture names, examples, inputs and expected results were written
for this repository. License: GPL-3.0-or-later, matching the project.

## Layout and manifest

Each `cases/<id>/` directory contains `case.toml`, an original `Makefile`, and any
supporting input files. The manifest is not copied into the fixture workspace.
`requirements.toml` groups the cases into explicitly incomplete requirement
families; its `fixture-ready` status means examples exist, not that the family is
fully specified, implemented or passing.

The initial manifest records:

- `id`, matching its directory; `requirement`, matching a ledger family.
- `candidate`: `pending` or `required`; `description` and `provenance`.
- `args`: normally `-rR --no-print-directory`, to isolate specified behavior from
  the reference's built-in rules; special cases explicitly add required options.
- `timeout_seconds`: a per-invocation bound; these fixtures use five seconds.
- Optional `[environment]` entries for explicit input variables.
- `[expected]`: an independently specified `exit_code` and exact UTF-8 `stdout`.
  `stderr` can be asserted when appropriate; otherwise differential comparison
  still checks its exact bytes. Failure cases deliberately require GNU's failure
  status and do not invent diagnostic formatting.

Each fixture has one invocation in the initial harness. Cases use `/bin/sh`
behavior and ordinary Linux tools such as `printf`, `cat`, and `test`. Outputs do
not embed clocks, PIDs, absolute fixture roots, or executable identity. Parallel
cases assert completed artifacts before emitting a fixed final result; their
outputs do not rely on normalizing nondeterministic completion order.

Run corpus workflows through the top-level Makefile. The oracle workflow must
validate each independently written expectation before comparing runs. Passing an
oracle-versus-itself comparison checks harness repeatability and these specified
outcomes; it does **not** establish rmake compatibility. Candidate reports must
show pending cases distinctly from passing cases.

## Coverage deliberately still missing

Expand this corpus alongside the implementation ledger. Among the remaining
areas are exhaustive option interactions, all 4.4 additions, implicit-rule and
archive edge cases, grouped-target failures, timestamp-resolution behavior,
symlinks and non-UTF-8 paths, full signal/descriptor behavior, terminal output
synchronization, Guile and dynamic loading, and cache cold/warm/corrupt/concurrent
validation. The recursive parallel case is only a first jobserver smoke test; it
does not prove token accounting or FIFO/pipe interoperability.

Add small original counterexamples for every discrepancy. Preserve exact output
and side effects; do not broaden normalization to hide a failure. The separate
[stateful corpus](stateful/README.md) now provides explicit steps and deterministic
timestamp inputs. Use that interface instead of uncontrolled sleeps in single-step
cases. `make test-generated` also provides a reproducible pure-expansion campaign;
its case count is separate from this curated inventory.

`make test-graphs` generates independent, bounded explicit dependency graphs.
It exercises merged rule declarations, normal/order-only role collisions,
duplicate prerequisites, phony nodes and observable recipe order. It compares
exact streams, status and output files, with original marker/file expectations;
it does not calculate GNU's traversal order using the candidate's algorithm.
The seed, case index and exact failed program are retained for replay. This is a
fresh-build graph campaign; incremental timestamps remain in `test-stateful`.
