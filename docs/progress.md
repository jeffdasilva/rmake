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

This does not complete M2 or M3. The initial slice sent recipes through a shell;
the process follow-up below adds direct execution. There is no signal
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

## Linux command preparation and launch — 2026-10-18

The process module now selects direct execution for GNU's supported simple
command forms and preserves shell interpretation for expansions, metacharacters,
built-ins and custom shell configuration. It keeps byte-valued arguments and
empty quoted arguments. Launch failures use GNU's status/diagnostic behavior and
respect ignored errors. Executable text without a shebang falls back through
`/bin/sh`, including custom shell scripts, with preserved arguments/environment.

PATH search retains the actual file that returned `ENOEXEC`; it does not guess
executability from permission bits and accidentally interpret a different file.
An effective-permission regression test covers that boundary. The module has 35
unit tests, and 30 original process fixtures join the required differential set:
the curated corpus now has 200 cases, 152 required matches and 48 pending cases.
An additional independent 200-case command-quoting campaign matched the oracle.
The larger seeded pure-expansion campaign also passed 1,000 programs containing
128,000 observations (seed 20261018). These observations extend coverage without
changing the open compatibility or performance gates.

## Automatic-variable metadata and scope evidence — 2026-10-18

The engine installs GNU's 14 recursive automatic D/F variables before reading
Makefiles, preserving their exact raw expressions, origin and precedence even
under `-R`. Ordinary explicit recipes also receive the empty `%` automatic
variable. Tests cover parse-time metadata, later recipe values, repeated lists,
path edge cases, byte-valued names and failed attempts to override the built-ins.
The variable core now evaluates immediate right-hand sides before discarding a
lower-priority definition; conditional assignments still skip their unused RHS.

The curated corpus has 236 cases: 165 required matches and 71 pending cases.
The additions include 13 required automatic-variable fixtures, one pending
explicit-suffix stem fixture, and 22 pending target-scope fixtures. The
[scope design notes](target-scopes.md) record observed assignment and inheritance
requirements for a future layered implementation; scope support is not claimed.

## Merged prerequisite ordering and generated graphs — 2026-10-18

An independent 200-graph campaign exposed ordering errors when separate rule
declarations mixed normal and order-only prerequisites. Rules now retain their
complete ordered occurrence stream. A recipe-bearing declaration moves its whole
dependency block forward. Traversal preserves that order; normal occurrences
promote a previously order-only name without moving it. `$<` and `$+` retain the
original normal occurrences, while `$^`, `$?` and `$|` use the canonical roles and
order. Repeated cycles retain repeated diagnostics; missing-target failures are
memoized. Special targets also recognize names written after the order-only bar.

All 200 discovery graphs and 19 independent focused review probes now match the
oracle. Six parser regressions and 17 original required fixtures cover the fix.
The curated corpus has 253 cases, with 182 required matches and 71 pending cases.
`make test-graphs` adds a permanent CI gate: 100 generated graphs by default,
each with eight nodes, original trace/file anchors and strict differential
comparison. It retains seed/index/source identity and full failure evidence.
Its 23 tooling tests include deliberately altered outputs and failure handling.
This campaign covers fresh explicit acyclic builds; stateful and cyclic behavior
remain covered separately, and full graph-language compatibility is unfinished.
An expanded campaign also passed 500 graphs with 16 nodes each (seed 20261018),
both oracle-versus-oracle and oracle-versus-candidate.

## One-shell execution and mode timing — 2026-10-18

`.ONESHELL` now joins and expands a rule's recipe as one script, retains its
parse-time recipe prefix, and handles GNU's internal prefix stripping for
recognized Bourne shells. Custom shells receive literal executable names with
GNU's script-prefix handling. First-line modifiers control the combined command;
direct `MAKE` references force it as GNU does. Failure locations identify the
first recipe line, and supported forced one-shell commands work with touch mode.

Ordinary and one-shell commands now expand shell configuration before dry-run
and query decisions. Tests distinguish truly empty expansions from nonempty
text that reduces to blanks/modifiers. Combined `-nt` echoes a touch without
writing, while `-qt` performs the touch. Raw empty/modifier-only recipes have
separate timing coverage. Forced exit code 1 under query mode reports an outdated
build, with the distinct ignore-error and delete-on-error behavior preserved.
Query/keep-going mode suppresses the final blocked-target summary as GNU does.
The curated suite has 318 cases: 241 required matches and 77 pending cases,
including 65 new one-shell/configuration/mode fixtures.

This remains a bounded execution slice. GNU's quoted-newline direct-command
tail replay and complex one-shell `.SHELLFLAGS` syntax are explicitly unsupported
on execution; exact oracle fixtures preserve these open requirements. Some
variable-definition diagnostic locations also remain pending. Ordinary mixed
forced recipes with touch mode, signal cleanup, parallel execution and the
previously recorded dynamic-language features remain unfinished.

## Explicit suffix metadata and stems — 2026-10-18

Explicit rules now compute `$*` from the first matching entry in the final
`.SUFFIXES` list, preserving path and non-UTF-8 bytes. The default suffix list and
the independent default `SUFFIXES` variable follow the pinned Linux profile,
including `-r`/`-R`. Empty suffix declarations clear dependencies while retaining
recipes; ordering, order-only entries, expansion timing and recipe-block merging
are covered. Default-goal selection excludes single/double suffix-rule names
using the list present before the current declaration is merged.

Thirty new required suffix fixtures and the previously pending explicit-stem
fixture match GNU. Two independent generated campaigns also matched: 100 suffix
ordering programs (seed 44110) and 200 default-goal programs (seed 44111). Eight
focused Rust regressions bring the Rust suite to 178 tests. This implements
suffix metadata for explicit rules; implicit search and suffix-rule conversion
remain unfinished.

The curated corpus now contains 368 cases: 275 required matches and 93 pending
requirements. Twenty original forced-touch fixtures preserve three matching
behaviors and seventeen open mismatches. The [forced-touch notes](forced-touch.md)
record their file-state expectations and the subsequent source-informed lifecycle
investigation. Those pending cases are not compatibility passes, and the ordinary
forced-touch execution guard remains until the transition model is implemented.

The paired release benchmark was refreshed on the same development host, with
two warm-ups and fifteen alternating samples per engine. Median GNU/rmake times
were 0.508/0.522 ms for the tiny no-op, 11.834/30.864 ms for the 10,000-node no-op,
and 2.653/3.840 ms for 5,000 assignments. The large-graph slowdown remains an open
performance issue. These unisolated wall-clock samples do not establish a
statistically controlled regression or speed advantage; no persistent cache is
enabled, and compatibility gates still precede acceleration work.

## Ordinary recipe lifecycle and mode matrix — 2026-10-18

The ordinary forced-touch guard is replaced by an explicit lifecycle model.
Raw recursion flags select expansion, reached expanded modifiers update command
flags, and completion retains GNU's one/two-touch distinction. Execution returns
a named status and cleanup policy. Failed launches, real-child failures, query
continuation and completion-touch failures preserve their different deletion
rules. Exact prepared colon invocations can complete without launching a child;
precious/phony targets, directories and symlink referents remain protected as
observed. Touch-open diagnostics retain their operation and target name.

A final special-target interaction campaign exposed global `.SILENT` behavior.
It now suppresses completion touches, ignored-error summaries and no-work
messages without adding `-s` to `MAKEFLAGS` or silencing recursive children.
Target-specific `.SILENT` keeps its narrower recipe-echo effect. The already
accepted `--no-print-directory` option is now preserved in `MAKEFLAGS`, including
GNU's leading space when no short flags precede it and duplicate coalescing.

Twenty-five new original fixtures and 19 formerly pending cases bring the curated
corpus to 393 cases: 319 required matches and 74 pending requirements. The 36
forced-touch fixtures all match. Five focused helper/CLI tests bring the Rust
suite to 183 tests, and twelve matrix-runner tests bring the tooling suite to 138.
An independent review replay matched 85 additional lifecycle/colon probes,
including fixed timestamp and link-state observations.

Eight new required stateful scenarios add 24 invocations, bringing that gate to
23 scenarios and 61 invocations per engine. They independently assert preserved
content, exact old mtimes, newer touched targets, query deletion, and absence of
skipped recipe output. `make test-modes` preserves the original 512-case Cartesian
matrix as a CI gate: ordinary/one-shell recipes, eight recipe shapes, and all
combinations of `-n`, `-q`, `-t`, `-i`, and `-s`. Reports retain exact reproducers;
`MODE_CASE` selects a recorded cell. Neither gate permits pending mismatches.

The implementation remains serial and incomplete. Expansion effects, target
scopes, includes/restarts, implicit rules, parallel/jobserver behavior, signals,
full diagnostic compatibility and persistent caching remain open. The lifecycle
code and later fixtures are independently written but source-informed; consulted
GNU files/functions and archive hashes are recorded in
[the provenance notes](forced-touch.md#reference-provenance).
