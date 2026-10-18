# Implementation progress

## M0 foundation

The Rust workspace now builds with pinned Rust 1.90.0 and GPL-3.0-or-later
package metadata. The initial executable supports its own help/version output
and fails explicitly for unimplemented Makefile evaluation. It never delegates
execution to GNU Make.

The independent Python standard-library harness compares two executables using
the same disposable workspace and invocation alias, reset between runs. It checks
output bytes, exit status, file contents/types/modes and link relationships. It
checks independently authored fixture expectations and retains mismatch evidence.
Pending cases are visible mismatches, not passes; unexpected matches require an
explicit promotion to required coverage.

There are 148 original fixtures across 35 initial requirement families. The GNU
Make 4.4.1 source archive is pinned by SHA256; the locally built reference has
recorded executable/runtime digests and feature observations. GNU Make 4.3 is
rejected as a conformance reference. This first profile disables Guile and uses
C-locale observations; additional profiles remain planned.

All implemented flows are available through the top-level Makefile. GitHub CI
bootstraps the pinned tools, runs formatting/Clippy/Rust tests, tooling tests,
oracle fixture checks and candidate differential checks. Action revisions are
pinned and workflow permissions are read-only. `make bench` records original
tiny/no-op/expansion workload baselines, including startup wall time.

## Open foundation gates

M0 is not fully complete. The current harness runs trusted, reviewed fixtures with
process-group/time/output bounds; it is **not a filesystem or network sandbox**.
Do not run untrusted Makefiles through it. Full namespace/container isolation,
immutable runtime images, timestamp comparison, stateful fixture steps, PTYs and
process-event assertions remain to be implemented. The current benchmarks report
wall time only, not the full CPU/memory/I/O profile in the design.

The core semantics milestones M1-M9 are not complete. The corpus and requirement
inventory are deliberately incomplete; no claim of GNU compatibility or speedup
is made by a green foundation CI run.

The implementation keeps the conformance runner in Python initially to avoid
coupling its expected answers to the Rust engine. Moving it to Rust is optional
and must preserve this independence. This is a refinement of the proposed crate
layout, not a change to the compatibility contract.
