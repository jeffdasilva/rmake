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

## Integer comparison and byte diagnostics — 2026-10-18

`intcmp` now accepts arbitrary-length decimal operands within the expansion
budget, normalizes signed zero and leading zeroes, and follows GNU's branch
defaults and expansion order. Two negative operands preserve the pinned release's
observed magnitude-ordering quirk. Direct calls remain lazy in result branches;
`call` eagerly expands every supplied parameter before builtin dispatch. Function
separators include vertical tabs, and user-call parameter zero follows GNU's
name truncation and expanded leading-whitespace behavior.

Invalid integer diagnostics preserve original operand bytes through the core,
parser and runtime. The fixture format now supports exact hexadecimal stream
expectations, with validation rejecting conflicting or malformed encodings. A
related zero-index `word` diagnostic now matches the oracle and promotes its
existing pending fixture.

Fifty-eight new original cases include 57 required matches and one pending
recursive-macro diagnostic-location regression. The curated corpus contains 451
cases: 377 required matches and 74 pending requirements. The Rust suite has 195
tests; the independent tooling suite has 164. An independent review matched 113
additional call-name, whitespace, nesting and eager-error probes. Of 184 separate
integer boundary/context probes, 181 match; the three remaining observations
share the recorded macro-definition location gap.

`make test-intcmp` adds a required CI gate with 200 programs and 6,400 expressions.
Its independent Python numeric model is checked against GNU before differential
comparison, and reports retain exact source, invocation, seed and observations.
An expanded local campaign passed 1,000 programs with 64,000 expressions per
engine using seed 20261018. These bounded campaigns do not establish complete
function or diagnostic compatibility. Implementation and fixture provenance and
the remaining location limitation are recorded in [the integer notes](intcmp.md).

## Expansion source locations — 2026-10-18

Variable bindings now retain shared definition locations, and structured
expansion errors preserve the innermost failing context. Recursive variables and
user calls, successful nested-expansion restoration, append/override/conditional
assignments and multiline `define` header/endef distinctions follow the pinned
oracle. Command-line and environment bindings inherit their caller's location.
Runtime expansion preserves GNU's separate raw-body handling for `.SHELLFLAGS`
and `.DEFAULT_GOAL`. Recursive-cycle messages retain the original variable bytes
and identify the binding being re-entered.

Forty-five new required original fixtures cover these rules, including multiple
Makefiles and invalid-UTF-8 filenames. Six older diagnostic fixtures now match.
One newly observed literal-`SHELL` process-launch mismatch is preserved as pending.
The curated corpus has 497 cases: 428 required exact matches and 69 pending
requirements. All 497 satisfy independent oracle expectations and oracle
self-comparison. The Rust suite has 202 tests and the tooling suite has 164.

A separate generated campaign passed 400 executions covering 399 distinct inputs
across 25 source-location families. It independently predicts status and exact
diagnostic bytes, verifies empty stdout and unchanged inputs, and varies source
padding, chain depth, definition order and evaluation context. Eighteen further
probes matched byte-valued/whitespace Makefile paths across recursive errors,
cycles and appends. The ordinary and one-shell mode matrix, prior generators,
stateful builds and self-host flows remain required gates.

Simple-variable reads copy their value without cloning unused diagnostic
metadata. Stored definition metadata remains available for later redefinitions,
appends and inspection. This is a bounded allocation improvement, not persistent
caching. [The diagnostic notes](diagnostics.md) record the verified boundaries,
original provenance and remaining context work for unimplemented features.

An alternating 31-sample release comparison with the preceding commit measured
before/after medians of 0.630/0.623 ms for tiny no-op, 30.964/31.162 ms for the
large graph, and 4.147/4.494 ms for 5,000 assignments. The assignment workload's
median paired slowdown was 8.8%, even after avoiding the unnecessary simple-value
metadata clone. Exact outputs agreed. These unisolated wall-clock measurements
do not include CPU/memory profiles, but the observed regression remains open for
allocation/layout work. Required location fidelity is retained; this does not
satisfy the planned performance gate or establish an advantage over GNU.

## Literal shell configuration arguments — 2026-10-18

Ordinary recipe preparation now protects the observed metacharacters originating
in `SHELL` before configuration parsing. Literal executable names and arguments
no longer accidentally undergo shell substitution. Existing quoting/backslash
interactions, interpreted `.SHELLFLAGS`, assignment/builtin fallbacks, ENOEXEC
handling and the separate `.ONESHELL` whole-path behavior remain covered.

Fourteen new required fixtures and the previously pending literal-dollar
regression match GNU. Four additional pending fixtures preserve known LF and
empty-shell gaps. The curated corpus now contains 515 oracle-validated cases:
443 required exact matches and 72 pending requirements. Full `make ci`, including
self-host flows, passed with 206 Rust tests and 164 tooling tests.

An independent 144-case matrix matched exact argv, streams, status and filesystem
state: 128 ordinary recipes, 16 one-shell cases, and equal PATH/explicit-path
coverage. Filenames combine punctuation, whitespace and non-UTF-8 bytes; checks
assert that accidental command-substitution marker files are absent. A broader
320-probe review matched 291 observations, with 26 remaining known configuration
gaps and three oracle-instability observations recorded separately. Two minimal
GNU allocator-abort reproductions were rechecked and retained as research data.
They are not passing conformance cases or permanent scope exceptions.

[Shell configuration notes](shell-configuration.md) document the verified
boundaries, original provenance and remaining work. No GNU implementation source
or upstream tests were consulted for this slice.

## Explicit errors and fatal expansion — 2026-10-18

The `error` function now preserves GNU's message bytes, direct/call argument
distinctions, evaluation timing and caller-location rules. Fatal expansion
results stop later prerequisites and top-level goals even under `-k` or `-i`;
ordinary failed processes retain their separate keep-going and ignore behavior.
Completed prerequisite files survive, and no child-failure cleanup is invented
for an expansion error.

Imported environment values are exported raw, while file and command-line
bindings expand at their required point. Automatic command-line and export-all
selection now requires an initial ASCII letter or underscore and subsequent
ASCII letters, digits or underscores. Explicit exports and inherited environment
membership still allow other names. This prevents premature expansion of values
that GNU would not export.

Fifty-one new required fixtures and one promoted historical error case bring the
curated corpus to 569 cases: 495 required exact matches and 74 pending
requirements. Three new pending cases preserve special `MAKELEVEL` mismatches.
All oracle expectations and self-comparisons pass. Full `make ci`, including
self-host flows, passed with 215 Rust tests and 164 tooling tests.

An independent campaign passed 348 distinct programs across 29 families: 300
fatal outcomes and 48 successful lazy cases, including 116 one-shell programs.
It asserts status, exact streams, source locations and complete filesystem state,
including preservation of earlier prerequisites and absence of later side
effects. Separate probes verified export-name boundaries and fatal runtime modes.
The implementation and tests used original oracle observations and public
documentation; no GNU implementation source or upstream tests were consulted.

[Fatal-error notes](fatal-errors.md) describe these boundaries. Special
`MAKELEVEL` initialization and child-depth behavior remain recorded pending
requirements. Other expansion effects and the later compatibility milestones
remain open; this slice does not establish full GNU compatibility or a speedup.

## Empty shell values and assignment routing — 2026-10-18

An empty or undefined `SHELL` now remains empty during recipe preparation. In
ordinary recipes the first flag can become the program; with no configuration
words, the complete recipe becomes a literal executable name. Exact builtins,
the optimized colon, and equals-containing words retain GNU's distinct shell
paths. Assignments preserve metacharacters as data, while invalid assignment
names retain shell-generated diagnostics. `.ONESHELL` preserves its empty program
and internal prefixes. Tests distinguish shell-child failure cleanup from direct
launch failure, including a target created by an earlier recipe line.

Thirty-five new required fixtures and two promoted regressions bring the curated
corpus to 606 cases: 532 required exact matches and 74 pending requirements. Two
new pending fixtures retain the observed ordinary expanded-newline splitting
gap. All independent oracle expectations and oracle self-comparisons pass.
Full `make ci`, including self-host flows, passed with 221 Rust tests and 164
tooling tests.

An independent campaign matched 296 distinct fixture inputs, covering 288
distinct program/argument texts plus different executable-presence and permission
states. It checks exact argv, status, streams and filesystem results across
ordinary and one-shell modes, three empty-value forms, three flag forms and
twelve option combinations. Assignment collision tests and forbidden-file checks
ensure values cannot accidentally become shell commands or redirections.

Original probes discovered the assignment boundary. A subsequent review of the
verified GNU 4.4.1 `src/job.c` confirmed its cause; the source reference and file
digest are recorded in [shell configuration notes](shell-configuration.md).
The Rust implementation and fixtures are original, with no upstream code or test
text copied or translated. Literal-newline and complex-flag gaps remain explicit
open requirements.

A fresh paired release comparison measured GNU/rmake medians of 0.501/0.501 ms
for tiny no-op, 11.219/29.707 ms for the 10,000-node graph, and 2.685/4.208 ms
for 5,000 assignments (15 measured samples per engine after two warm-ups).
Exact outputs agreed. These are unisolated wall-clock measurements, not CPU or
memory profiles: the substantial graph/expansion performance gap remains open,
and neither this slice nor earlier diagnostic work satisfies the M8 speed gate.

## MAKELEVEL startup snapshot — 2026-10-18

Invocation depth is now read from the winning raw startup binding after
command-line assignments and before Makefile parsing. Imported/default values
are normalized with GNU's observed origin and flavor. File assignments can
change the visible variable without changing the depth sent to children.
Ordinary export expansion retains its effects before the special numeric child
override, which uses wrapping 32-bit addition. Runtime diagnostic prefixes use
the snapshot; command-line immediate failures retain their earlier plain name.

Thirty-five new required fixtures and three promoted historical regressions
bring the corpus to 646 cases: 570 required exact matches and 76 pending
requirements. Four new pending cases preserve generic command-line replay
behavior, and another preserves a generic leading-vertical-tab assignment lexer
gap. All curated oracle expectations and oracle self-comparisons pass. Full
`make ci`, including self-host flows, passed with 226 Rust tests and 164 tooling
tests.

An independent campaign matched 242 distinct inputs across 13 scenario families,
checking raw output, exit status and filesystem contents/modes. It covers numeric
conversion, command-line operators, immutable depth, precedence, export effects,
diagnostic timing and recursive children. The generic command-line replay and
vertical-tab lexer gaps remain separate pending requirements.

The [invocation-depth notes](makelevel.md) document conversion, evaluation and
export ordering, runtime prefixes and remaining recursion work. The implementation
and fixtures are independently authored from original executable probes and
public documentation; no GNU implementation source or upstream tests were
consulted for this slice. Removing per-child expansion of the current variable
avoids incorrect effects, but is not a measured performance claim.

## Informational output and warnings — 2026-10-18

`info` and `warning` now emit byte-preserving output at their actual expansion
points. An explicit, synchronous output context keeps the core independent of
process I/O; pure convenience APIs reject emissions rather than silently losing
them. Parser warnings emit in order and survive subsequent fatal errors.

Independent review also corrected first-binding contexts for warnings and
explicit errors, conditional operand/diagnostic staging, literal default-goal
selection, and `MAKEFILE_LIST` initialization and raw appends. Numeric validation
errors retain their distinct innermost-definition behavior. The
[diagnostic-function notes](diagnostic-functions.md) record these boundaries.

Fifty new required fixtures, four new pending cases and two promoted historical
fixtures bring the corpus to 700: 622 required exact matches and 78 pending
requirements. Multiple-export traversal order and make-flag effect timing remain
explicit gaps. Full `make ci` passed with 240 Rust tests, 183 tooling tests, all
curated oracle expectations, existing generated/stateful gates and self-host
flows. No failures or unexpected passes were hidden by the pending classification.

The permanent `make test-diagnostics` gate adds 206 distinct original scenarios
across 14 families, each with independent byte-output, status and filesystem
expectations. It checks merged streams as well as separate capture, and supports
exact case replay with retained failure evidence. Nineteen tooling tests verify
mutation detection, resource failures, replay and report integrity. A selected
diagnostic replay also runs through the candidate's top-level Makefile.

A paired release comparison against exact preceding commit `44c646b` measured
before/after medians of 0.716/0.700 ms for tiny no-op, 31.207/31.483 ms for the
10,000-node graph, and 4.400/4.306 ms for 5,000 assignments. Median paired
after/before ratios were 1.001, 1.000 and 0.983 over 31 alternating trials after
three warm-ups. Outputs matched. This unisolated wall-clock check found no
greater-than-5% median paired regression in these three early workloads; it is
not evidence of meeting the broader GNU performance target or the M8 gate.
