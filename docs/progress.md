# Implementation progress

## M0 foundation

The Rust workspace now builds with pinned Rust 1.90.0 and GPL-3.0-or-later
package metadata. The first executable supported its own help/version output;
the serial slice below connects the core language implementation. The runtime
never delegates execution to GNU Make.

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
immutable runtime images, comprehensive timestamp observations, PTYs and
process-event assertions remain open. The later stateful gate below adds a
bounded set of multi-step and relational timestamp tests. Current benchmarks report
wall time only, not the full CPU/memory/I/O profile in the design.

The core semantics milestones M1-M9 are not complete. The corpus and requirement
inventory are deliberately incomplete; no claim of GNU compatibility or speedup
is made by a green foundation CI run.

The implementation keeps the conformance runner in Python initially to avoid
coupling its expected answers to the Rust engine. Moving it to Rust is optional
and must preserve this independence. This is a refinement of the proposed crate
layout, not a change to the compatibility contract.

## M1 language library slice — 2026-10-18

The core now has independent byte-preserving reader, variable and incremental
parser modules. The reader tracks physical source locations, folds continuations
according to recipe context, and handles escaped comments, CRLF and changing
recipe prefixes. The variable store implements assignment timing and precedence,
nested references, substitution references, metadata functions, pure string/list/
filename functions and lazy control forms. It exposes no process or filesystem
effects. Expansion has explicit resource bounds while the engine is experimental.

The parser evaluates assignments and conditionals as it reads, preserves deferred
recipes, and records explicit normal/order-only prerequisites and exports. Original
unit tests and focused GNU 4.4.1 probes cover these modules. This slice is not a
claim that all M1 behavior is complete: effects, includes, target/pattern scopes,
full diagnostics and several rule forms remain open. Connecting these modules to
serial filesystem/process execution is covered by the next slice below.

## M2 serial execution slice — 2026-10-18

The executable now resolves explicit rules and default/requested goals, walks
normal/order-only dependencies once per invocation, compares nanosecond mtimes,
expands automatic variables and runs recipes serially. It implements basic recipe
echo/ignore/force prefixes, phony targets, silence/error special targets,
delete-on-error with precious-file protection, and a subset of CLI flags.
Environment and command-line assignments keep their precedence. Inherited dry-run
and query flags are honored; unsupported scheduling flags fail explicitly.

The supported serial subset can run the repository's `build` and `check-core`
leaf targets under rmake. `make test-selfhost` exercises this path. Filesystem
regression tests cover equal/one-nanosecond timestamps, unchanged output on no-op,
order-only dependencies, byte-valued names and arguments, deletion on failure,
and dry-run/query behavior that must not accidentally execute a command.

The original corpus now contains 170 fixtures in 36 requirement families. All
170 pass oracle-versus-oracle validation; 122 are required exact candidate matches
and 48 remain explicitly pending. Independent generated probes also found byte
sorting and empty-pattern/empty-stem substitution discrepancies; the fixes have
dedicated regression tests. Family coverage remains partial even when all of its
current samples pass.

This does not complete M2 or M3. Recipes currently go through a shell, so GNU's
direct-exec path and its failure diagnostics remain open. There is no signal
cleanup implementation, jobserver, include/restart handling, built-in rule
database, implicit search, target/pattern scopes or persistent cache. Expansion
effects are rejected. Makefile changes to execution flags and forced commands
under touch mode are rejected until their semantics are implemented. Resource
bounds and explicit unsupported errors are development limitations, not approved
exceptions to the final compatibility contract.

## Generated and stateful test gates — 2026-10-18

`make test-generated` adds independent, seeded programs covering assignment
timing, nested names, substitution, lazy functions and byte-valued words. Its
default gate runs 200 programs with 6,400 labeled expression observations, both
oracle-versus-oracle and oracle-versus-candidate. Each program includes original
expected anchors. Campaign reports retain generator/source identity, seed and
index range, program digest and binary/profile identities; mismatches retain the
exact Makefile and raw results. Larger runs use the same bounded CLI.

`make test-stateful` adds 15 scenarios with 37 invocations per engine and
preserves each workspace across a sequence of invocations,
with deterministic input mutations and nanosecond timestamp anchors. It compares
exact outputs/status/files plus declared timestamp relations and unchanged-file
checks. Wall-clock timestamps are retained as evidence but not compared as
arbitrary absolute values between sequential executions. Transient writes restored
before observation, directory times, atime/ctime and process events remain outside
this gate. The trusted-fixture security limitation still applies.

`make bench-compare` records alternating paired wall-time samples with identical
paths, arguments, environment and exact output. The first local release-build
measurements were about 0.53/0.48 ms (rmake/GNU) for tiny no-op, 25.96/11.92 ms for
10,000 explicit no-op nodes, and 4.24/3.00 ms for 5,000 immediate assignments.
The current slice is slower on these three workloads. These are preliminary warm
OS-cache observations on one host, without CPU isolation or confidence intervals;
they guide later profiling and do not satisfy the performance milestone.
