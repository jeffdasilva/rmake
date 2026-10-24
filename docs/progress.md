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

## Assignment whitespace boundaries — 2026-10-18

Assignment parsing now skips all six C whitespace bytes at the beginning of raw
names and values, including vertical tab. Only space and tab are trimmed from
the end of a raw variable name; other whitespace there belongs to the name.
Whitespace introduced by expanding a name remains literal. The change is local
to assignments and preserves reader continuation rules and directive parsing.

Thirty new original fixtures cover the supported operators, CLI/file boundaries,
exact raw and expanded names, RHS whitespace, continuation behavior and non-ASCII
negative controls. Twenty-five are required; five retain broader internal-name
and separated-colon grammar gaps. Promoting the previous vertical-tab regression
brings the corpus to 730: 648 required exact matches and 82 pending requirements.
Full `make ci`, including self-host flows and the diagnostic matrix, passed with
244 Rust tests and 183 tooling tests. Oracle expectations, candidate comparisons
and the requirement ledger all agree; no unexpected passes remain.

An independent frozen investigation covers 576 matrix cells representing 564
unique source/argv inputs. The fix corrects 64 cells; the remaining 41 mismatches
belong to the explicitly identified broader grammar gaps. All 68 supported
independently authored semantic anchors pass, including 24 expanded-name and
15 continuation observations. These exploratory totals are separate from the
permanent curated corpus and are not presented as full conformance. No GNU
implementation source or upstream tests were consulted for this slice.

## Assignment-name recognition — 2026-10-18

The assignment parser now distinguishes literal name separators from blanks
inside make references. Raw internal spaces/tabs reject assignment syntax;
quotes and backslashes do not protect them. CLI arguments then become goals,
while file lines follow the normal rule/directive path, preserving its expansion
effects. Computed names may still contain any expansion-produced whitespace.

The scanner preserves one-byte references before removing trailing syntax
blanks. In particular, `X$ =value` defines `X`; the final space belongs to `$ `,
so the preceding whitespace slice's trimming rule needs this token boundary.
Balanced references retain their internal blanks without allocating name copies.
This small parsing change makes no measured performance claim.

Twenty-eight new original fixtures contain 23 required matches and five pending
boundaries. Four historical raw-blank regressions are promoted. The corpus now
contains 758 cases: 675 required exact matches and 83 pending requirements.
Full `make ci` passed with 247 Rust tests, 183 tooling tests, all generated and
stateful comparisons, the diagnostic matrix and top-level self-host flows.
Short-reference operator selection, backslash before an assignment delimiter,
export-list empty bindings, CLI replay, and colon-rule diagnostics remain visible
pending cases, not supported behavior.

An independent campaign of 112 distinct inputs passes authored GNU expectations
and exact candidate output, status and complete filesystem comparisons. It covers
literal and computed names, short references, modifiers and rejected-name effect
timing; its explicitly excluded boundaries are retained in the curated corpus.
No GNU implementation source or upstream tests were consulted for this slice.

## Assignment operator tokens — 2026-10-18

Assignment recognition now uses one allocation-free lexical scan instead of
generic separator searches followed by operator extraction and name validation.
Short references consume operator bytes before classification, long references
protect their internal delimiters, and backslashes remain literal. Unconsumed
hashes reject assignment syntax even in CLI arguments. The existing rule parser
and actual shell-assignment execution remain separate. The
[assignment syntax notes](assignment-syntax.md) record the observed boundaries.

Twenty-six new original fixtures add 25 required matches and one pending generic
CLI replay case; two prior operator regressions are promoted. The corpus has
784 fixtures: 702 required exact matches and 82 pending requirements. Full
`make ci` passed with 251 Rust tests, 183 tooling tests, all generated/stateful
and diagnostic comparisons and the top-level self-host flows.

An independently authored campaign passes all 148 distinct inputs across ten
families, including 20 `define` headers, delimiter/reference controls and malformed
inner expressions. Exact GNU expectations and candidate byte streams, statuses
and complete filesystem observations agree. No GNU implementation source or
upstream tests were consulted for this slice.

A paired release comparison against exact preceding commit `e2ee4a9` used 31
alternating trials after three warm-ups with identical output. Before/after
medians were 0.542/0.529 ms for tiny no-op, 31.140/30.798 ms for the 10,000-node
graph, and 4.315/4.287 ms for 5,000 assignments. Median paired after/before ratios
were 0.984, 0.989 and 1.005. These unisolated wall-time observations show no material
change on the three early workloads; they do not establish a general speedup,
GNU performance parity or completion of the performance milestone.

## Export and unexport name-list bindings — 2026-10-18

Name-list directives now create absent variables as empty simple bindings with
file origin before applying export policy. Existing bindings retain their value,
flavor, origin and source context. Names are expanded once for the list and kept
literal during insertion. This fixes conditional-assignment suppression,
immediate append effects, and empty-versus-unset child environment behavior.
The [export-binding notes](export-bindings.md) document the remaining boundaries.

Twenty-eight new original fixtures add 21 required matches and seven pending
cases; the earlier export-fallback regression is promoted. The corpus contains
812 fixtures: 724 required exact matches and 88 explicit pending requirements,
with no failures or unexpected passes. Full `make ci` passed with 255 Rust tests,
183 tooling tests, all generated/stateful/diagnostic gates and self-host flows.

An independent campaign passes all 131 distinct inputs against authored GNU
expectations and candidate byte streams, status and complete filesystem checks.
It includes existing-origin/source preservation, append effects and real child
captures for literal `=` names. Those child captures match using the existing
process launcher; no environment serialization change was necessary.

Pending cases retain policy lifetime after `undefine`, empty expanded-list
toggles, vertical-tab token splitting and modifier grammar. Other export-effect
ordering gaps remain in the earlier corpus. No GNU implementation source or
upstream tests were consulted for this slice. No performance or complete-export
compatibility claim is made.

## Export-list token and global-mode boundaries — 2026-10-18

Only syntactically empty export/unexport directives now toggle export-all mode.
Computed empty or whitespace-only lists retain the prior mode and still perform
their expansion effects. Expanded list names split on all six C whitespace
bytes, including vertical tab, while non-ASCII space-like bytes remain literal.
Both changes are confined to name-list handling; general directive recognition
and other language tokenizers remain separate compatibility work.

Twenty-one original fixtures add 20 required matches and one pending
keyword-adjacent vertical-tab case. Two previous regressions are promoted. The
corpus now has 833 fixtures: 746 required exact matches and 87 explicit pending
requirements. Full `make ci` passed with 259 Rust tests, 183 tooling tests, all
generated/stateful/diagnostic comparisons and top-level self-host flows. There
are no hidden failures or unexpected passes.

An independent campaign passes all 150 distinct inputs against authored GNU
expectations and candidate bytes, status, child environments and complete
filesystem observations. It covers prior global states, raw versus computed
emptiness, retained and fatal effects, mode skips, all C separators and non-ASCII
negative controls. No GNU implementation source or upstream tests were consulted.
This completes these two recorded regressions, not the broader export, language
or performance milestones.

## Explicit target-variable scopes — 2026-10-18

Explicit target declarations now have separate binding layers, preserving
per-target immediate expansion, deferred recursive values, inherited appends,
command-line snapshots, first-parent ownership of shared prerequisites and
private/export/unexport policy. Variable-only declarations do not create build
rules or choose a default goal. Global private bindings remain visible while
reading and disappear on descent. Environment override origins are promoted at
the observed collision point, including startup metadata and undefine.

Sixty-seven new original fixtures include 17 pending pattern-scope cases for the
next slice. The curated corpus contains 900 cases: 823 required exact matches and
77 observed pending mismatches, with no failures or unexpected passes. Full
`make ci` passed, including 288 Rust tests, 185 tooling tests and the existing
generated/stateful/diagnostic and self-host gates. New `make test-scopes` runs
100 seeded dependency graphs in CI. A larger 1,000-graph campaign passed exact
oracle/candidate output, status and filesystem comparisons; five minimized
assignment-metadata regressions from that campaign are now curated fixtures.

Immutable `Arc` binding maps let each recipe add automatic variables without
copying the globals. A comparison against exact previous main commit `69f74b0`
used 31 alternating paired release trials after three warmups, with identical
paths, environment, arguments and outputs. Before/after medians were
0.687/0.676 ms for tiny no-op, 30.027/23.894 ms for 10,000 explicit targets, and
4.350/4.369 ms for 5,000 assignments. Median paired after/before ratios were
1.005, 0.800 and 1.016. This is an unisolated early-workload measurement, not a
general performance claim or GNU speed parity; persistent caching is absent.

Pattern scopes require ordered declarations with activation-time conditional and
append semantics and remain explicitly unsupported. Shell/file/eval functions,
includes/restarts, complete rules and parallel execution remain later milestones.
The [scope notes](target-scopes.md) describe the implemented model and the next
activation boundary. This slice used original code and oracle probes; no GNU
implementation source or upstream tests were incorporated.

## Pattern-variable activation and environment policy — 2026-10-18

Pattern declarations now retain their ordered assignment semantics and activate
lazily in each target's first-parent context. Matching uses full target names,
escaped percent characters, nonempty stems, specificity and declaration order.
Immediate values, deferred appends/conditionals, command-line snapshots and
private/export policy follow the observed GNU Make 4.4.1 boundaries. Generated
target colons preserve the unexpanded assignment RHS and its effects.

Environment preparation distinguishes private pattern selection from reference
expansion, reconstructs inherited appends, and remembers resolved export policy
on the selected binding. A conditional alias that resets an imported global's
policy reevaluates export eligibility from the selected value's origin.
Up-to-date, missing, empty-recipe and touch-skipped targets avoid deferred effects.

Fifty-six original fixtures were added. The corpus contains 956 cases: 897
required exact matches and 59 observed pending mismatches, with no failures or
unexpected passes. Full `make ci` passed with 315 Rust tests, 186 tooling tests,
all existing generated/stateful/diagnostic gates and self-host flows. The new
`make test-pattern-scopes` gate runs 100 mixed-scope dependency graphs in CI.
An extended 1,000-graph campaign passed; its append regressions and independent
export-policy review findings are preserved as focused fixtures.

A paired release comparison against preceding implementation commit `daedf53`
used 31 alternating trials after three warmups, identical paths, environment,
arguments and output. Before/after medians were 0.533/0.554 ms for tiny no-op,
23.270/21.829 ms for 10,000 explicit targets, and 4.292/4.058 ms for 5,000
assignments. Median paired after/before ratios were 1.024, 0.932 and 0.947.
These unisolated early workloads do not establish a general speedup or GNU
performance parity. Persistent caching remains absent.

The [pattern-scope notes](pattern-scopes.md) describe the implementation and GNU
source references consulted. Rust code and fixtures are independently authored;
no GNU code or upstream tests were copied or translated. This completes the
recorded variable-scope slice, not M3 or full GNU Make compatibility.

## File and realpath effects with Linux I/O faults — 2026-10-18

`file` now reads, truncates and appends, and `realpath` observes existing paths.
Explicit callbacks keep OS effects out of the core. The new `rmake-linux` crate
provides byte-preserving I/O and an owned, reviewed stdio adapter that retains
GNU's buffered write/close error stages. Core and application code still forbid
unsafe Rust. No observation cache or atomic file replacement is introduced.

Expansion preserves file data through the correct NUL consumer boundaries,
newline treatment and caller/definition diagnostic contexts. Ordinary recipes
now execute expansion-generated command fragments separately, retaining raw
modifiers, sticky recursive status and distinct direct/shell continuation rules.
The [filesystem notes](filesystem-effects.md) describe safety, limits and source
references. All implementation and tests are independently authored.

Sixty-seven original fixtures were added; eight earlier file/path and empty-shell
regressions are promoted. The corpus contains 1,023 cases: 969 required exact
matches and 54 observed pending mismatches, with no failures or unexpected
passes. Full `make ci` passed with 328 Rust tests, 197 tooling tests and all prior
gates. `make test-filesystem` adds eleven durable, fault-injected comparisons to
CI, including interrupted opens/reads/path observations, partial reads, buffered
write failures and errors after exactly one actual close. A selected close-fault
case also runs through the top-level Makefile using rmake. Independent expression,
NUL, scope, recipe-mode and continuation probes matched all 432 inputs.

A release comparison against preceding implementation commit `9ecc095` used 31
alternating pairs after three warmups with identical output and execution
context. Before/after medians were 0.628/0.629 ms for tiny no-op, 24.693/24.781 ms
for 10,000 explicit targets, and 4.514/4.769 ms for 5,000 assignments. Median paired
after/before ratios were 0.997, 1.012 and 1.055. The assignment workload was about
5% slower in this unisolated run; the expanded evaluator needs later profiling.
These measurements establish neither general performance nor GNU speed parity.

Three added pending cases preserve broader `.ONESHELL` quoted-newline replay,
pure-function vertical-tab and literal-NUL reader gaps. Wildcard, process and
dynamic evaluation functions, includes/restarts, complete rule resolution,
parallel execution, persistent caching and plugins remain later work.

## Pure-function whitespace and local bindings — 2026-10-18

Word/list and filename functions now use all six C-locale whitespace bytes while
preserving non-whitespace control and non-ASCII bytes. Function-specific spans
remain intact: `wordlist` preserves selected separators, literal `patsubst`
preserves untouched input, and the final `let` binding receives the exact
remaining bytes. `foreach` selects its first expanded name token; `or` and `and`
trim each raw argument before expansion without discarding whitespace produced
by expansion. The [function notes](function-whitespace.md) record these rules.

Sixty-four original fixtures were added and the earlier vertical-tab regression
was promoted. The corpus contains 1,087 cases: 1,027 required exact matches and
60 explicit pending cases, with no failures or unexpected passes. Full `make ci`
passed with 333 Rust tests, 210 tooling tests and all existing gates. New
`make test-whitespace` adds 297 authored byte/status/filesystem scenarios to CI,
including effect ordering and non-whitespace controls; a selected case also runs
through rmake's top-level Makefile. An additional constrained campaign matched
6,400 expressions across 200 programs.

Seven added pending cases expose six existing invalid-number diagnostics and a
GNU Make 4.4.1 `let` SIGSEGV. The positive campaign explicitly excludes the
upstream crash family, while the curated case records its actual signal. rmake
keeps safe empty bindings; the crash is not counted as a compatibility pass.

The common no-NUL path now uses safe slice membership checks before scalar
position scans, retaining all file-function truncation and effect boundaries.
A comparison against preceding commit `bb504a3` used 31 alternating release
pairs after three warmups with identical output/context. Before/after medians
were 0.713/0.709 ms for tiny no-op, 21.814/22.016 ms for 10,000 explicit targets,
and 4.396/4.181 ms for 5,000 assignments. Median paired after/before ratios were
0.990, 1.006 and 0.955. The assignment workload improved about 4.5% in this
unisolated run; this is not a general speed or GNU parity claim. Persistent
caching and the broader compatibility/performance milestones remain incomplete.

## Word and wordlist numeric validation — 2026-10-18

`word` and `wordlist` now accept the pinned profile's signed 64-bit decimal
grammar, including signs, leading zeroes and all C whitespace. Bounds and
numeric-prefix overflow preserve GNU's validation precedence, exact raw-byte
diagnostics and definition locations. `wordlist` accepts negative zero as an
empty ending range. Supplied arguments expand before numeric or argument-count
validation, preserving earlier output and file effects when a later error
aborts expansion. The [word index notes](word-indices.md) describe the boundary
and GNU source references consulted; all code and tests are independently authored.

Forty-eight original fixtures were added and six prior numeric diagnostics were
promoted. The curated corpus contains 1,135 cases: 1,081 required exact matches
and 54 observed pending mismatches, with no failures or unexpected passes.
Full `make ci` passed with 338 Rust tests, 226 tooling tests and all prior gates.
The new `make test-word-index` adds 220 independently anchored byte, status,
source-context and filesystem scenarios to CI; a selected effect-before-error
case also runs through rmake's top-level Makefile. A separate authored campaign
validated 807 inputs against both engines.

A comparison against preceding implementation commit `4d535f4` used 31
alternating release pairs after three warmups, identical paths, environment,
arguments and output. Before/after medians were 0.675/0.683 ms for tiny no-op,
20.752/21.811 ms for 10,000 explicit targets and 4.686/4.772 ms for 5,000
assignments. Median paired after/before ratios were 1.003, 1.012 and 1.020.
These unisolated early workloads establish neither a speedup nor GNU performance
parity. Broader argument-count behavior, dynamic semantics and later milestones
remain incomplete.

## Streaming word positions — 2026-10-18

`wordlist`, `let` and literal-pattern `patsubst` now consume positions from a
safe iterator instead of collecting every word's offsets. Selected ranges and
local bindings scan only the positions they need; literal replacement still
scans the full list. Argument expansion, effects, numeric validation, exact
separators, binding lifetimes and output limits remain unchanged. Temporary
position storage is constant size, without introducing any cache or unsafe code.
The [function notes](function-whitespace.md#streaming-word-positions) describe
the allocation boundary and the Makefile benchmark flow.

Eleven original fixtures were added. The curated corpus contains 1,146 cases:
1,092 required exact matches and 54 observed pending mismatches, with no failures
or unexpected passes. Full `make ci` passed with 338 Rust tests, 231 tooling tests
and all existing gates. Whitespace matrix v2 adds 24 independently anchored cases
for 321 total, preserving all 297 previous programs unchanged. Independent review
also matched 225 range probes, including a 1.87 MB input, and 6,400 nested
expressions. The known GNU `let` crash remains explicitly pending.

The new `BENCH_SUITE=word-spans` contains six fixed 200,000-word workloads; the
original three `core` workloads remain unchanged. Both GNU Make and rmake drove
`bench-compare` through the top-level Makefile with exact matching observations.
A comparison against preceding implementation commit `0ee93e1` used 31
alternating release pairs after three warmups and identical paths, environment,
arguments and output:

| Workload | Before median (ms) | After median (ms) | Median paired after/before |
| --- | ---: | ---: | ---: |
| Wordlist prefix | 4.567 | 3.545 | 0.774 |
| Wordlist middle | 4.529 | 3.704 | 0.817 |
| Wordlist full range | 5.866 | 5.724 | 0.966 |
| Wordlist out of range | 4.584 | 3.933 | 0.859 |
| Let final remainder | 5.203 | 4.271 | 0.818 |
| Literal patsubst | 8.064 | 7.382 | 0.902 |

The core tiny/no-op graph/assignment workloads had median paired ratios of
0.989, 1.000 and 0.988. A separate four-million-word prefix case used GNU `time`
for seven alternating pairs after two warmups: median peak resident memory fell
from 97,404 KiB to 33,700 KiB. These are unisolated workload-specific measurements,
not general performance guarantees. rmake remained slower than GNU Make in all
six new workloads. Persistent caching and the wider compatibility/performance
milestones remain incomplete.

## Wildcard expansion and invocation directory observations — 2026-10-18

`wildcard` now supports the current C-locale Linux/glibc profile through an
invocation-owned filesystem runtime. Filename-sequence quoting, raw bytes,
dot/link handling, path spelling, scoped/effectful HOME expansion and per-pattern
ordering are preserved. Archive groups, traditional/GNU/BSD names, thin-archive
fallbacks and GNU's matching-member repetition quirk have original fixtures.
Malformed numeric headers and unsupported archive-symbol references preserve
program-only fatal diagnostics through parsing and recipes without panics.

Directory observations have path and device/inode keys, including missing paths.
Successful file-write opens and completed recipe dispatch invalidate them.
Reads retain them, and literal existence checks remain fresh. Cached owned
names, compiled patterns, safe single-star matching and one-time directory sorting
reduce repeated work without suppressing expansion effects. The
[wildcard notes](wildcard.md) record semantics, source references, safety limits
and the still-open locale/profile validation.

The corpus adds 125 original fixtures and promotes three prior cases: 1,271 total,
with 1,220 required exact matches, 51 explicit pending mismatches and no failures
or unexpected passes. Full `make ci` passed with 344 Rust tests, 240 tooling tests,
all prior gates and a selected wildcard self-host flow. `make test-wildcards`
adds 100 independently modeled ASCII query programs to CI; an extended 1,000
program campaign passed with a separate seed. Additional original campaigns
matched 1,596 filename inputs and eight external cache-mutation scenarios.

The filesystem interposer now has 19 required scenarios, adding six external
mutation checks and two interruption boundaries. Both engines must match
authored streams, status, complete filesystem and injection traces. The tests
retain GNU's one-shot literal lstat and archive-open fallback, distinct from
directory-discovery retries. The simple-star optimization also matches direct
libc results across raw-byte names, dot boundaries and slash cases.

`BENCH_SUITE=wildcard` adds cold, repeated, complex and literal query workloads
over 1,000 untimed input files. `BENCH_REPEAT` controls paired measured samples.
In 31 alternating release trials after two warmups, oracle/rmake median times
were 1.176/0.908 ms cold, 38.117/11.585 ms repeated, 40.955/15.544 ms complex and
0.926/1.122 ms literal. Repeated and complex queries are faster in these local
measurements; literal queries remain slower than GNU. Both engines also drove
the benchmark target through the top-level Makefile.

Against preceding implementation `add18bb`, 31 alternating core-workload pairs
after three warmups had median paired after/before ratios of 1.005 for tiny no-op,
0.990 for 10,000 explicit targets and 1.005 for 5,000 assignments. These unisolated
wall-time observations have no confidence intervals or CPU/RSS/I/O coverage and
establish no general performance guarantee. Non-C profiles, process/eval effects,
includes/restarts, complete rules, parallel execution, persistent caching and
plugins remain open milestones.

## Wildcard allocation follow-up — 2026-10-18

Filename sequences without archive grouping now retain their owned tokens rather
than cloning each token into another list. Unescaped literal wildcard queries
borrow qualified paths for the same fresh existence check and return their owned
result directly; single-result glob queries also avoid a final copy. No lookup is
cached or omitted. GNU's `//name` root spelling and interior repeated separators
are preserved, with a focused Linux adapter regression check.

Full `make ci` passed with 345 Rust tests and 240 tooling tests. The curated corpus
remains 1,271 cases: 1,220 required matches and 51 explicit pending mismatches,
with no failures or unexpected passes. The generated wildcard, filesystem fault,
external cache-mutation and self-host gates remain green.

Against preceding production source `f0acfb8`, 31 alternating release pairs after
three warmups measured literal-query medians of 1.148 ms before and 1.052 ms after;
the median paired after/before ratio was 0.903. Cold, repeated and complex query
ratios were 1.001, 0.998 and 1.007. Core tiny no-op, 10,000-target and
5,000-assignment ratios were 0.959, 0.992 and 0.996. The same paths, arguments,
environment and output were used within each pair.

In a separate 31-pair comparison after two warmups, GNU/rmake medians were
1.140/0.891 ms cold, 37.504/11.865 ms repeated, 40.003/15.313 ms complex and
0.937/0.988 ms literal. Literal queries still trail GNU in this run. These are
unisolated local wall-time measurements without confidence intervals or
CPU/RSS/I/O coverage; they establish no general speed guarantee or full parity.

## Export policy and binding lifetime — 2026-10-18

Successful `undefine` directives now discard export/unexport membership with the
binding. Imported export policy is stored on the binding itself, retained by
ordinary assignments and internal literal/metadata updates, and discarded by
deletion. A startup name-list entry alone no longer exports a new file-origin
definition or an override of an ordinary command-line binding. Refused undefines
retain policy, and explicit target/pattern policies stay independent.

Internal updates preserve imported `MAKELEVEL`, `CURDIR` and `MAKEFILE_LIST`
export behavior, including expansion errors before the numeric child-depth
override. The existing MAKELEVEL regression gate caught a discarded policy during
development; both it and new internal-update fixtures pass. No shell/eval effect,
command-line replay, new cache or GNU runtime fallback is introduced. The
[export-binding notes](export-bindings.md) record semantics and source references.

Twenty-four original fixtures were added and three pending fixtures promoted.
Full `make ci` passed with 346 Rust tests, 240 tooling tests and all existing
generated, fault, stateful and self-host gates. The curated corpus contains 1,295
cases: 1,247 required exact matches and 48 observed pending mismatches, with no
failures or unexpected passes. A separate 792-program matrix matched both engines
against independent stream/status anchors across origins, policies, deletion,
assignment operators, export-all and local target policies.

Against preceding production source `5b069a3`, 31 alternating release pairs after
three warmups measured before/after medians of 0.688/0.693 ms for tiny no-op,
22.610/22.522 ms for 10,000 explicit targets and 4.173/4.177 ms for 5,000
assignments. Median paired after/before ratios were 1.029, 1.019 and 1.008.
These unisolated wall-time measurements establish no speed improvement and have
no confidence intervals or CPU/RSS/I/O coverage. Broader environment behavior,
effect ordering, dynamic semantics and the remaining milestones stay incomplete.

## Variable-modifier fallback — 2026-10-18

The parser now retains the original statement while recognizing assignment
modifiers. When no variable operation is found, export/unexport lists and rules
use that original text. Keyword-shaped list names are materialized and exported
with their proper metadata. Bare or misplaced modifiers retain GNU's syntax
errors and prior expansion effects. Rule targets named `override` or `private`
also retain their names and default-goal order. Valid assignments and qualified
undefines keep their modifier handling. GNU Make 4.4.1 `src/read.c`
`parse_var_assignment` and `eval` informed the independently authored change.

Thirty-seven original required fixtures were added, and two pending fixtures
were promoted. Full `make ci` passed with 346 Rust tests, 240 tooling tests and all
existing gates. A larger campaign compared 2,380 original modifier combinations:
2,104 matched exactly; 276 differed only because rmake warns about repeated
targets even when a declaration has no recipe. The same mismatch reproduces with
`all all:` on the preceding implementation. One new pending graph fixture
preserves that warning gap; comparisons were not normalized or excused.

After adding that pending fixture, refreshed tooling, oracle and candidate gates
passed. The curated corpus contains 1,333 cases: 1,286 required exact matches and
47 observed pending mismatches, with no failures or unexpected passes. This
bounded parser correction introduces no cache or performance claim. Complete
directive grammar, warning timing, dynamic semantics and later milestones remain
open.

## Literal NUL input and file-stream boundaries — 2026-10-18

The reader now models GNU's visible stream chunks when a Makefile contains a
literal NUL. Discarded tails, subsequent appended reads, persistent buffer growth,
synthetic newlines, continuation/recipe handling and observed line counts retain
the pinned reference behavior. Warnings are emitted at read time, with the
definition reader's distinct positions for `define` bodies. The ordinary reader
remains the path for files without NUL bytes.

Capacity is represented as an integer rather than a large allocation. The
reference's signed 32-bit read-count conversion is preserved safely. A tiny
NUL-bearing input can reach that boundary; an extended campaign exposed it and
the regression fixture checks the resulting input stop. The
[NUL reader notes](nul-reader.md) record phase behavior and source references.

Thirty-two original fixtures were added and one pending fixture promoted. Full
`make ci` passed with 348 Rust tests and 240 tooling tests; formatting, Clippy and
Rust tests were repeated after marking the rare reader path cold. The curated
corpus contains 1,365 cases: 1,319 required exact matches and 46 observed pending
mismatches, with no failures or unexpected passes. A separate seeded campaign of
400 original stream programs matched the oracle, including buffer growth and
mixed tails/continuations.

Against preceding production source `516dc1a`, 31 alternating release pairs after
three warmups measured tiny no-op medians of 0.591/0.621 ms, 10,000-target medians
of 21.121/22.263 ms and 5,000-assignment medians of 4.251/4.362 ms before/after.
Median paired after/before ratios were 1.039, 1.043 and 1.027. These unisolated
wall-time observations establish no speed improvement; CPU/RSS/I/O profiles and
broader release-scale validation remain open. Full GNU compatibility is not
claimed by this reader slice.

## Post-read command binding replay — 2026-10-18

Startup now snapshots winning command-line bindings and defines the corresponding
automatic snapshot and default `MAKEOVERRIDES` metadata. After all Makefiles are
read, active overrides are expanded and decoded into environment-origin
assignments, respecting `-e`, ordinary precedence and binding export lifetime.
The invocation-level outer diagnostic context remains distinct from nested
definition locations. The [command replay notes](command-replay.md) record the
phase model, source references and remaining flags/recursive-transport gaps.

Thirty original required fixtures were added and five pending cases promoted.
Full `make ci` passed with 350 Rust tests, 240 tooling tests and all existing
generated, fault, stateful and self-host gates. The curated corpus contains 1,395
cases: 1,354 required exact matches and 41 observed pending mismatches, with no
failures or unexpected passes. A separate 480-program replay campaign matched
both engines' streams and statuses across operators, values, imports, precedence
and source mutations. General flag decoding and recursive transport remain open.

Against preceding production source `5fb7f4c`, 31 alternating release pairs after
three warmups measured before/after medians of 0.689/0.691 ms for tiny no-op,
21.811/22.181 ms for 10,000 targets and 4.370/4.415 ms for 5,000 assignments.
Median paired after/before ratios were 1.032, 1.014 and 1.007. These unisolated
wall-time observations establish no speed improvement and do not cover CPU,
RSS or I/O. No persistent cache or runtime fallback was introduced; complete
compatibility and release-scale performance validation remain unfinished.

## Copy contiguous expansion literals — 2026-10-18

The expression evaluator now copies literal runs with one bounded append. Dollar
references, unmatched terminal dollars, NUL consumer boundaries and expansion
effects retain their original order. A new resource-boundary test checks exact
limit success, failure before a later effect, and a terminal dollar at the limit.
No cache, unsafe code or source-reference dependency is added.

Full `make ci` passed with 351 Rust tests, 240 tooling tests and all existing
curated, generated, fault, stateful and self-host gates. The corpus remains 1,395
cases: 1,354 required exact matches and 41 observed pending mismatches, with no
failures or unexpected passes.

Against preceding production source `88a3f03`, 31 alternating release pairs after
three warmups observed 10–19% less elapsed time on six large-text word-span
workloads. Core workload median paired ratios were 1.004, 0.970 and 0.999 for
tiny no-op, 10,000 targets and 5,000 assignments. The
[literal expansion notes](literal-expansion.md) record the individual ratios,
output checks and measurement limitations. Full compatibility, persistent cache
validation and broader performance work remain open.

## Defer recipe-dependent rule warnings — 2026-10-18

The parser now retains local declaration state and emits duplicate-target and
recipe-replacement diagnostics when the declaration closes. Repeated targets
without a recipe do not warn. Conditional effects, read-time warnings, assignment
effects, target occurrence order and EOF/fatal paths keep their distinct phases.
The [rule warning notes](rule-warnings.md) describe the boundaries, graph scope
and GNU source references used for this independently authored implementation.

Twenty-nine original fixtures were added and one pending case promoted. Full
`make ci` passed with 351 Rust tests, 240 tooling tests and all existing generated,
fault, stateful and self-host gates. The curated corpus contains 1,424 cases:
1,384 required exact matches and 40 observed pending mismatches, with no failures
or unexpected passes. A 440-program warning matrix matched exact streams and
statuses. The earlier 2,380-program modifier campaign now also matches exactly;
its 276 prior duplicate-warning differences are resolved without normalization.

Against preceding production source `65032a9`, 31 alternating release pairs after
three warmups measured before/after medians of 0.701/0.700 ms for tiny no-op,
21.478/21.340 ms for 10,000 targets and 4.457/4.505 ms for 5,000 assignments.
Median paired after/before ratios were 1.004, 1.000 and 1.008. These unisolated
wall-time observations establish no speed improvement; CPU/RSS/I/O and broader
release validation remain open. Complete dynamic input, rule families, recursive
options, parallel execution and full GNU compatibility are still unfinished.

## Assignments before directive dispatch — 2026-10-18

Lexically recognized assignments now precede conditional and directive dispatch.
Keyword-shaped names such as `ifdef`, `define` and `undefine` retain ordinary
variable behavior. Ignored assignments have no name/value effects and do not
alter the conditional stack; active assignments close the preceding declaration
before their immediate effects. The [assignment notes](assignment-syntax.md)
record the independently authored correction and its GNU source reference.

Forty-two original required fixtures were added. Full `make ci` passed with
351 Rust tests, 240 tooling tests and all existing generated, fault, stateful and
self-host gates. The curated corpus contains 1,466 cases: 1,426 required exact
matches and 40 observed pending mismatches, with no failures or unexpected passes.
An original 324-program matrix across eighteen keywords, six operators and three
spacings now matches exactly, resolving its 96 previously observed differences.

Against preceding production source `1fc55d7`, 31 alternating release pairs after
three warmups measured before/after medians of 0.678/0.678 ms for tiny no-op,
20.653/20.957 ms for 10,000 targets and 4.509/4.263 ms for 5,000 assignments.
Median paired after/before ratios were 1.001, 1.027 and 0.952. The selected
assignment workload improved by about 5% in this run; the graph workload was
slower. These unisolated wall-time observations have no confidence intervals or
CPU/RSS/I/O measurements and establish no general speedup. Directive whitespace,
definition grammar and later compatibility milestones remain separate work.

## Directive C-whitespace and active/ignored definitions — 2026-10-18

Directive dispatch now recognizes C-whitespace keyword boundaries and leading
vertical tabs while retaining assignment-first interpretation and separate rule
filename tokenization. Active definition bodies keep their narrower space/tab
keyword boundaries, preserve other literal separators and emit `endef` extra-text
warnings at the observed closing position. Ignored definitions instead follow
standalone closure, recipe context, effect suppression and conditional EOF rules.
The [directive whitespace notes](directive-whitespace.md) record those phases,
remaining grammar scope and source references for the independent implementation.

One hundred seventeen original fixtures were added and the export-keyword
vertical-tab case promoted. Final full `make ci` passed with 351 Rust tests,
240 tooling tests and all generated, fault, stateful and self-host gates. The
curated corpus contains 1,583 cases: 1,544 required exact matches and 39 observed
pending mismatches, with no failures or unexpected passes. Separate campaigns
matched 160 directive/body programs, 336 ignored-definition programs, 972 keyword
and C-separator programs, and all 2,380 modifier programs without normalization.

Against preceding production source `b2e24e5`, 31 alternating release pairs after
three warmups measured before/after medians of 0.714/0.708 ms for tiny no-op,
22.298/22.221 ms for 10,000 targets and 4.370/4.493 ms for 5,000 assignments.
Median paired after/before ratios were 0.974, 0.986 and 1.032. These unisolated
wall-time observations establish no general speedup; the assignment workload was
slower. CPU/RSS/I/O, complete definition/conditional grammar and the remaining
dynamic, rule, runtime and release-validation milestones remain unfinished.

## Copy simple values directly — 2026-10-18

Simple-variable expansion now copies stored bytes once without cloning the whole
binding first. Recursive values retain their existing owned-binding, provenance,
cycle and append-layer paths. No cache or expansion effects are introduced.
Four permanent repeated-read workloads are available through
`make bench-compare BENCH_SUITE=bindings`; the `all` suite includes them.

Full `make ci` passed with 351 Rust tests, 240 tooling tests and all existing
curated, generated, fault, stateful and self-host gates. The corpus remains
1,583 cases: 1,544 required exact matches and 39 observed pending mismatches,
with no failures or unexpected passes. The permanent binding benchmark also
matched GNU Make 4.4.1 output and successful status for all four workloads.

Against source `8081183`, 31 alternating release pairs after three warmups
observed about 9–22% lower elapsed time on repeated reads of 256 KiB and 1 MiB
global/local simple values. Existing core and word-span workloads showed no
consistent improvement. A separate GNU comparison still found rmake slower for
the two global workloads and slightly faster for the two local workloads. The
[simple-copy notes](simple-value-copy.md) record exact medians, paired ratios,
workload construction and limitations. These unisolated wall-time observations
have no confidence intervals or CPU/RSS/I/O coverage and establish no general
speed advantage. Complete compatibility and broader performance work remain open.

## Initial UTF-8 BOM phase — 2026-10-18

The parser now removes one initial UTF-8 BOM from each input's first observed
logical line after stream reading. Other positions, doubled/partial BOMs and raw
value/body/recipe bytes remain literal. Keeping this after reading preserves NUL
buffer behavior and source positions. The [initial-BOM notes](initial-bom.md)
record the source reference, behavior and remaining generated-input/debug scope.

Twenty-six original required fixtures were added. Full `make ci` passed with
351 Rust tests, 240 tooling tests and all existing generated, fault, stateful and
self-host gates. The curated corpus contains 1,609 cases: 1,570 required exact
matches and 39 observed pending mismatches, with no failures or unexpected passes.
A separate seeded campaign of 400 BOM-prefixed NUL-stream programs matched exact
streams and statuses, including buffer growth and discarded tails.

Against source `c407c57`, 31 alternating release pairs after three warmups
measured before/after medians of 0.693/0.692 ms for tiny no-op, 21.789/22.222 ms
for 10,000 targets and 4.500/4.502 ms for 5,000 assignments. Median paired
after/before ratios were 1.006, 1.001 and 0.993. These unisolated wall-time
observations establish no speed improvement and have no confidence intervals or
CPU/RSS/I/O coverage. Broader syntax, dynamic semantics, rule resolution, runtime
and release validation remain incomplete.

## Conditional operand and name whitespace — 2026-10-18

Conditional name validation now keeps GNU's first-token rule after expansion,
including invalid leading separators, valid trailing separators and literal
non-ASCII name data. Parenthesized comparisons trim only raw left-side blanks,
skip raw right-side C whitespace and preserve expanded/trailing operand bytes.
Quoted-operand gaps and final separators use their own C-whitespace handling.
The [conditional notes](conditional-whitespace.md) record phase ordering, source
references and the still-open broader grammar scope.

One hundred thirty original required fixtures were added. Full `make ci` passed
with 351 Rust tests, 240 tooling tests and all existing generated, fault, stateful
and self-host gates. The curated corpus contains 1,739 cases: 1,700 required exact
matches and 39 observed pending mismatches, with no failures or unexpected passes.
The 122-program conditional operand matrix matches independently authored anchors
and both engines, resolving 26 prior differences. Eight additional fixtures
preserve effect/error ordering, skipped branches and non-ASCII behavior.

Against source `a97640a`, 31 alternating release pairs after three warmups measured
before/after medians of 0.687/0.703 ms for tiny no-op, 21.645/21.708 ms for 10,000
targets and 4.360/4.429 ms for 5,000 assignments. Median paired after/before
ratios were 1.020, 0.993 and 1.014. These unisolated wall-time observations
establish no performance improvement and lack confidence intervals or CPU/RSS/I/O
measurements. General definition grammar, shell/eval effects, includes/restarts,
complete rule resolution, parallel runtime and release validation remain open.

## Qualified define and header/body phases — 2026-10-18

Global modifiers now apply to definitions, including export/unexport, override
and private policy. Assignment priority is retained after modifiers for variables
named `define` and `undefine`. Definition headers warn about ignored extra text,
expand and trim names once, and reject empty/error names before body reads.
Prepared names then reach the shared definition path without re-expansion;
immediate body evaluation retains its closing-line context and header provenance.
Modifier lexing retains its borrowed assignment result instead of rescanning it.
The [qualified-definition notes](qualified-definitions.md) record semantics,
source references and remaining shell/eval/generated-input scope.

Sixty original required fixtures were added. Final full `make ci` passed with
351 Rust tests, 240 tooling tests and all generated, fault, stateful and self-host
gates. The curated corpus contains 1,799 cases: 1,760 required exact matches and
39 observed pending mismatches, with no failures or unexpected passes. A
336-program modifier/header/origin matrix resolves 308 differences against the
preceding binary and now matches exact streams/statuses. The previous
2,380-program modifier fallback matrix also remains exact.

Against source `342f446`, 31 alternating release pairs after three warmups
measured before/after medians of 0.685/0.682 ms for tiny no-op, 21.066/20.833 ms
for 10,000 targets, 4.691/4.568 ms for 5,000 assignments and 2.013/1.970 ms for
2,000 ordinary simple definitions. Median paired after/before ratios were
1.000, 0.990, 0.977 and 0.978. These unisolated wall-time observations establish
no general speedup and lack confidence intervals or CPU/RSS/I/O coverage. Shell
assignments, eval/includes/restarts, complete rules, parallel execution and full
GNU compatibility remain unfinished.

## Dynamic include and restart slice

Added required/optional nested includes and makefile remake/restart staging.
The corpus now has 1,824 cases: 1,792 required and 32 pending. Include support
is partial: search directories and the full remake failure semantics remain.
See [dynamic makefiles](dynamic-makefiles.md).

## Stateful shell and evaluation slice

Added shell capture, shell assignments and dynamic eval with target/global
scope separation. There are now 1,905 fixtures: 1,883 required exact passes and
22 pending mismatches. See [stateful expansion](stateful-expansion.md) for
architecture, safety limits and remaining boundaries.

## Pattern, static, secondary and double-colon resolution

Added a direct implicit-rule search slice, static patterns, VPATH and selective
vpath search, secondary expansion and independent double-colon branches. There
are 1,943 cases: 1,928 required and 15 pending. Full local CI passed; see
[rule resolution](rule-resolution.md) for the remaining search boundaries.

## Local parallel execution and recursive command bindings

Added a bounded local job pool, target completion tracking, .NOTPARALLEL and
recursive command-line transport. There are 1,971 fixtures: 1,961 required exact
passes and ten pending mismatches. The local scheduler and recursion still have
documented compatibility limits; see [local parallelism](local-parallelism.md).

## Implicit chains and intermediate lifecycle

Transactional implicit-chain planning, terminal and match-anything selection,
intermediate cleanup/retention, incremental checks of missing intermediates, and
implicit optional-include remaking are implemented. See
[implicit chains](implicit-chains.md) for boundaries and source references.
Added 39 original differential cases (37 required, two pending) and six stateful
scenarios (12 invocations). Corpus: 2,010 cases, 1,998 required and 12 pending.

## Linux built-ins and suffix transformations

Added default command variables, embedded Linux suffix/fixed rule declarations,
post-read suffix conversion, override/cancellation priority, disabled-rule modes
and built-in diagnostic locations. The data verifier and regenerator are
available through the top-level Makefile; CI verifies the data against the pinned
oracle. See [built-ins](builtins.md) for provenance and remaining boundaries.
Added 66 original differential cases and three tooling tests. All 2,064 required
cases pass; 12 pending mismatches remain, out of 2,076 total cases.

## Parallel recipe continuations

Complete ordinary recipes and recursive invocations can now run concurrently,
with interpreter-owned continuation state and one slot retained per target.
Success and ignored failures resume the next command; unignored failures stop
that target. Export environments are frozen once per target, while shell
configuration still evaluates per command. Stdin ownership survives continuation.
Added 24 required fixtures and one pending GNU fatal-resumption case. The corpus
contains 2,101 cases: 2,088 required and 13 pending. See
[local parallelism](local-parallelism.md) for boundaries and validation.

## Shared Linux jobservers

FIFO and pipe jobservers now share a concurrency limit with recursive GNU/rmake
children in either direction. Permits remain attached to whole recipe
continuations and return on ordinary completion/error paths. Added unavailable
server diagnostics, explicit-child reset handling, owned/borrowed resource
lifecycle and recursive-only pipe descriptor inheritance. The mixed-engine
campaign is a top-level Makefile/CI gate with 61 checks per run.
Added 15 original differential fixtures, six Linux adapter tests, a CLI transport
case, and three independent tooling-anchor tests. The corpus has 2,116 cases:
2,103 required and 13 pending. See [jobservers](jobserver.md) for precise coverage,
procfs/resource assumptions, and unfinished signal/load/output-sync behavior.

## Prerequisite phase barriers

Implemented `.WAIT` prerequisite phases with local waiting, automatic-variable
exclusion, error/keep-going behavior, secondary/static/implicit expansion and
special-target diagnostic timing. Added 28 original required cases plus one
pending fixture exposing incomplete scheduling across independent graph branches.
Corpus: 2,145 cases, 2,131 required and 14 pending. See
[phase barriers](wait-barriers.md) for coverage and the remaining ready-graph work.

## Implicit-search comparison reduction

Moved replacement/cancellation checks after pure pattern matching, and computed
the target directory prefix once per search. Added an original implicit-search
benchmark suite, optional previous-candidate timing through `BENCH_BASELINE`, and
a tooling test for workload structure/order. A 31-sample release comparison
measured paired improvements of 1.64x and 6.08x over the previous rmake on the two
synthetic workloads; GNU remained faster on both. No persistent cache was added.
See [performance evidence](implicit-search-performance.md) for hashes, raw-report
location, measured medians, methodology and limitations.

## Default recipes and selected-pattern flags

Implemented `.DEFAULT` command fallback with explicit/implicit precedence,
cancellation at declaration completion, correct automatic variables, target
contexts, error locations and generated includes. Corrected `.PRECIOUS` and
`.NOTINTERMEDIATE` inheritance to use the exact selected implicit pattern rather
than arbitrary filename matches. Added 42 original required differential cases.
Corpus: 2,187 cases, with 2,173 required passes and 14 pending. See
[default recipes](default-recipes.md) for scope and source references.

## Fail-safe implicit-search limits

Final review found that an exhausted bounded search could be mistaken for a
proven absence of implicit rules and select `.DEFAULT`. Distinguish exhaustion,
propagate a fatal outcome through ordinary builds and makefile remaking, and make
the resolver result must-use. A 131-rule regression in a private empty directory verifies failure
without installing or executing a fallback. The corpus remains 2,173 required
and 14 pending; Rust coverage gains this separate safety-boundary test.

## Jobserver descriptor identity

Validate that incoming pipe endpoints have the same device/inode identity before
changing inherited descriptor flags or accepting the server. Extend the invalid
authentication test with two valid but unrelated pipes and verify both original
pools retain their tokens. Valid GNU/rmake interoperability is unchanged; the
mixed-engine campaign remains part of CI.

## Resumable dependency branches

Yield single-colon graph traversal while prerequisite recipes run, preserving
secondary expansion, first-parent scope ownership, local barriers, circular-edge
diagnostics and final staleness decisions. Per-pass epochs bound repeated visits
to shared pending subgraphs. Add 32 required original cases, promote the existing
independent-sibling case, and record two remaining synchronous paths as pending.
Corpus: 2,221 cases, with 2,206 required and 15 pending. See
[ready branches](ready-branches.md).

## Double-colon graph continuations

Yield double-colon dependency branches to unrelated ready work while keeping
commands and declarations for one target sequential. Preserve aggregate status
and shared ownership through all branches. Add twelve required differential
cases and promote the previous double-colon sibling probe. Corpus: 2,233 cases,
with 2,219 required and 14 pending. See
[double-colon scheduling](double-colon-scheduling.md).

## Nested-branch performance follow-up

Add an optional `parallel-branches` benchmark suite and retain per-workload
arguments in reports. Two 16-branch workloads at different dependency depths
showed about 3.9x improvement against the previous optimized rmake, with exact
outputs/statuses and 31 alternating measured samples. GNU remained slightly
faster. No performance threshold or persistent graph cache is introduced. See
[nested-branch measurements](branch-scheduling-performance.md) for limits,
reproduction and binary identities. Default benchmark workloads are unchanged.

## Missing-intermediate graph continuations

Yield both inspection and late materialization of missing intermediates so
independent graph branches can progress. Base creation decisions on finalized
input timestamps, retaining old-timestamp and per-parent semantics without
marking absent files updated. Honor local barriers during inspection. Add twelve
required differential cases and promote the previous intermediate sibling probe.
Corpus: 2,245 cases, with 2,232 required and 13 pending. See
[intermediate scheduling](intermediate-scheduling.md).

## Leaf scheduling fast path

Rules without prerequisites now use existing recipe finalization directly,
avoiding pending-frame hash-table work. Successful outcomes skip an unnecessary
parallel-policy lookup in the failure guard. The full corpus and gates are
unchanged. A 101-sample paired local comparison improved the 10,000-target no-op
workload by about 8%; tiny startup and expansion do not show meaningful speedup,
and GNU remains faster. See [measurements and limits](ready-leaf-performance.md).

## Grouped explicit and implicit outputs

Implement explicit `&:` and implicit multi-output rules with shared recipe
completion, peer readiness, automatic variables, timestamp comparison, secondary
expansion, local barriers, override diagnostics and peer cleanup. Shared implicit
rule identities prevent sibling patterns from recurring in a search chain;
selected peer patterns retain `.PRECIOUS` and `.NOTINTERMEDIATE` flags.
Add 83 original required differential cases, promote `chain-shared-middle`, and
add eight stateful scenarios covering 17 more invocations. The corpus now has
2,328 cases: 2,316 required and 12 pending. Stateful coverage is 37 scenarios and
90 invocations. Independent overlapping `&::` declarations remain a next slice.
See [grouped outputs](grouped-targets.md) for boundaries and reference policy.

## Independent grouped double-colon declarations

Implement `&::` with separate declaration identities, completion and active-job
ownership across shared targets. Retain each target's initial double-colon
timestamp and each branch's expansion state; a peer completed by one declaration
still visits its other declarations. Suspended peer frames recheck recipe
ownership after shared prerequisites finish. Add 40 required original fixtures
and three stateful scenarios with six invocations. Corpus: 2,368 cases, with
2,356 required and 12 pending. Stateful coverage: 40 scenarios / 96 invocations.
See [grouped output rules](grouped-targets.md) for remaining scope.

## Ready-work rescan after observed completion

A main-branch CI replay exposed an intermittent barrier stall: a child completed
while a later dependency branch was being visited, but the scheduler blocked on
another child before revisiting earlier newly ready work. Both outer-goal and
per-target wait loops now rescan after an observed completion. This preserves
the per-epoch sharing bound while allowing the ready branch to launch.

Add bounded exact-case repetition to the independent conformance harness and
`make test-graph-progress` to normal CI. Each gate runs four existing barrier and
grouped-output scenarios 40 times against two GNU executions, then 40 times
against rmake, in fresh workspaces. Two tooling tests verify repetition's
validation and exact outcome handling. The fixture corpus remains 2,356 required
and 12 pending, with 40 stateful scenarios / 96 invocations.

## Low-resolution timestamp comparisons

Implement `.LOW_RESOLUTION_TIME` target-side comparisons through the last
nanosecond of the timestamp's second. Preserve actual timestamps for
prerequisite outcomes, no-op metadata and changed-output error cleanup. Match
GNU's high-resolution warning timing, including no-command files, secondary
expansion, double-colon branches and grouped peers. Pattern spelling and empty
flag lists retain their literal GNU behavior. Add 33 required original cases
and four stateful scenarios / ten invocations. Corpus: 2,401 cases, with 2,389
required and 12 pending. Stateful coverage: 44 scenarios / 106 invocations.
See [timestamp comparisons](timestamp-comparisons.md) for scope and limitations.

## Keeping ordinary rules small

Move grouped-rule metadata and peer traversal state behind optional owned
allocations, and cache the named `.NOTPARALLEL` decision per prerequisite phase.
Ordinary rules no longer carry the unused peer vectors, declaration metadata or
second recipe view inline. Leaf rules skip the serialization lookup. This changes
storage and repeated pure policy lookup without caching expansions, filesystem
state or graph semantics across invocations. The required corpus is unchanged.
A 101-sample local comparison improves the 10,000-target no-op workload by about
4% against the immediately preceding build; tiny startup is unchanged and
expansion shows no improvement. GNU remains faster. See
[rule storage measurements](rule-storage-performance.md) for exact evidence and
limitations.

## Repeated double-colon target occurrences

Keep each repeated target occurrence in ordinary `::` and grouped `&::`
declarations as a separate branch. Recipes attach to each branch, while grouped
completion remains shared by declaration. Peer cursors retain occurrence
ordinals so secondary expansion and circular-edge removal address the correct
branch. This prevents duplicated automatic prerequisites and lost expansion
effects. Also scan surviving expanded prerequisite bytes for static-pattern
colons with GNU's escape parity; literal escaped references do not hide them.
Add 35 required original differential cases, including controls for ordinary
single-colon and grouped single-colon duplication. Corpus: 2,436 cases, with
2,424 required and 12 pending. Stateful coverage remains 44 scenarios / 106
invocations.

## Include search and recursive include-directory options — 2026-10-23

Implement ordered `-I` / `--include-dir` search, resets, `.INCLUDE_DIRS`, configured
default directories, recursive flag escaping, and runtime reader reuse across
eval and recipes. Resolve directory arguments in command-line assignment order;
preserve requested source names separately from resolved remake/list names.
Expose command-variable overrides in startup MAKEFLAGS without expanding them
merely to check whether the flags binding changed. Correct directory-read errors.
Add 54 required original fixtures and two CLI boundary tests. Corpus: 2,490
cases, with 2,478 required and 12 pending. Stateful coverage remains 44 scenarios
/ 106 invocations. See [include search](include-search.md) for precise scope.

## Target-variable context allocation — 2026-10-23

Allocate pattern scopes only when a declaration matches; share empty local maps
through copy-on-write and share immutable invocation-directory bytes. Keep all
activation/effect ordering unchanged. Add a Makefile-selectable target-context
benchmark suite and its workload validation test. Full local CI and optimized
conformance pass with unchanged corpus counts. Alternating 101-sample runs show
small improvements in expansion and target-context workloads; GNU remains
faster. See [measurements and limits](target-context-performance.md).

## MAKEFILES preloads, main discovery and export — 2026-10-23

Read a frozen expanded MAKEFILES list before discovering the main file. Suppress
automatic default selection across preloads and their includes, while retaining
explicit assignments and eval's GNU default-selection behavior. Discover main
files created during startup and update missing default candidates. Preserve
optional-remake failure handling and required-use promotion. Add binding-local
export-if-set policy without freezing the untouched default's initial answer.
Add 52 original required fixtures. Corpus: 2,542 cases, with 2,530 required and
12 pending. Stateful coverage remains 44 scenarios / 106 invocations. See
[startup makefiles](startup-makefiles.md) for exact boundaries.

## Archive-member metadata and execution

Share the existing Linux archive reader between wildcard queries and build
metadata. Implement member dates, standalone-file freshness, one-second target
comparison, archive/member automatic variables, header-only touch, and
non-destructive failure cleanup. Validate completed recipes' headers without
reading partial headers from running jobs or invalidating completed targets.
Add 52 original required cases and three stateful scenarios / twelve invocations.
Corpus: 2,594 cases, with 2,582 required and 12 pending. Stateful coverage is
47 scenarios / 118 invocations. Add a Makefile-selectable archive benchmark.
See [archive members](archive-members.md) for remaining resolution work.

## Archive implicit search and suffix conversion

Search complete archive-member names before member-only candidates, retaining
directory bytes and building the fallback list only when needed. Enable the
built-in archiver recipe and intermediate chains, and generate archive and
ordinary patterns from `.X.a` suffix rules. Preserve GNU's literal member-only
peer names. Rewrite deferred static-pattern percents before activation, with
non-archive controls for the shared behavior. Add 34 original required cases
and a four-step deterministic-archive scenario. Corpus: 2,628 cases, with 2,616
required and 12 pending. Stateful coverage: 48 scenarios / 122 invocations.
See [archive rule resolution](archive-rule-resolution.md) for remaining M4 scope.

## Archive descriptor lifetime across recipe workers

A regression test reproduces archive-descriptor inheritance while a scan remains
open across a child exec. Open read descriptors with atomic close-on-exec, so a
worker launching concurrently with an interpreter scan cannot inherit them.
Preserve the single open attempt required by the archive EINTR profile. The
functional corpus and stateful inventory remain unchanged; the new Rust test
checks the actual child boundary rather than just the configured flag.

## Archive groups and bounded filename recognition

Expand archive member groups in explicit/static/pattern declarations, scoped
variables and deferred explicit prerequisites. Preserve empty, incomplete and
member-only forms, metadata projection and grouped completion. Reuse the shared
filename parser and replace repeated suffix look-ahead with one reverse scan.
Add 36 original required fixtures and a Makefile-selectable unclosed-sequence
benchmark. Corpus: 2,664 cases, with 2,652 required and 12 pending. Stateful
coverage remains 48 scenarios / 122 invocations. The 31-sample local workload
comparison shows a specific 2.2x/8.2x improvement over the preceding build; a
separate 101-sample core check shows no general speed gain. See
[archive groups](archive-groups.md) for scope, measurements and remaining work.

## Archive rule wildcards and default filename lookup

Add a distinct filesystem effect for complete archive-member rule names, with
NUL-separated transport preserving blanks, tabs and arbitrary bytes. Expand
target globs before prerequisite effects, glob clauses once before static stem
replacement, and defer secondary reads and errors until activation. Preserve the
wildcard function's separate GNU output behavior. Look up complete known default
filenames before splitting them. Add 50 original required cases and two injected
archive-open fault cases. Corpus: 2,714 cases, with 2,702 required and 12 pending.
Stateful coverage remains 48 scenarios / 122 invocations; the filesystem fault
gate now has 21 cases. See [archive rule wildcards](archive-rule-wildcards.md).

## Adaptive archive reader I/O

Buffer dense small-member header runs within one scan; use direct header reads
and payload seeks for larger members. Track logical offsets and restore the
cursor when discarding read-ahead. Keep member data fresh across effects and
retain absolute header-only touch offsets, open attempts and close-on-exec.
An initial fixed-buffer experiment regressed realistic object payload scans and
was replaced before publication. Extend archive benchmarks with 4 KiB/64 KiB
payloads and add 26 original boundary/mutation/format required fixtures. Corpus:
2,740 cases, with 2,728 required and 12 pending. Stateful coverage remains
48 scenarios / 122 invocations. See
[archive reader measurements](archive-reader-performance.md).

## Archive-first VPATH resolution

Search archive filenames before member lookup and stop at the first existing
archive, including empty, missing-member and zero-date cases. Keep implicit
viability's literal-name search separate. Preserve resolved automatic variables
and goal summaries, local rebuild spelling, phony checks and GNU's per-invocation
archive observations. Add 64 required original fixtures and two stateful scenarios
with eleven invocations. Corpus: 2,804 cases, with 2,792 required and 12 pending.
Stateful coverage is now 50 scenarios / 133 invocations. Full development gates
and optimized conformance pass; paired core/archive checks show no general speed
change. See [archive VPATH](archive-vpath.md) for behavior and remaining M4 gates.

## Compiled VPATH lookup policy

Compile each selective percent pattern when declared and borrow matching
directories through a short-circuiting iterator. Keep original clearing keys and
all filesystem observations/effect ordering unchanged. Add nine original required
policy regressions and a Makefile-selectable VPATH benchmark suite with prepared
ordinary files and an archive. Corpus: 2,813 cases, with 2,801 required and
12 pending. Stateful coverage remains 50 scenarios / 133 invocations. Full gates
and optimized conformance pass. A 101-sample paired run improves the selected
lookup workloads by 1.2x-6.3x over the preceding build; three of four measured
medians beat GNU, without establishing general performance parity. See
[VPATH lookup measurements](vpath-search-performance.md).

## Per-clause secondary expansion

Preserve each deferred static clause's stem, recipe-first traversal order and
all ready immediate prerequisites. Keep literal dollars intact, reproduce
pairwise duplicate role promotion, and use GNU's selected recipe diagnostic
context. Borrow ready names when producing automatic values. Add 69 original
required fixtures (61 fail on the preceding build), a 100-graph mixed-scope
secondary campaign in CI and a separate 100-graph/32-node local campaign.
The top-level Makefile exposes `test-secondary` and a four-workload `secondary`
benchmark suite. Corpus: 2,882 cases, with 2,870 required and 12 pending.
Stateful coverage remains 50 scenarios / 133 invocations. See
[secondary-expansion clauses](secondary-expansion.md) for behavior, measurements
and the remaining M4 gates.

## Implicit candidate secondary expansion

Evaluate deferred prerequisite words during viability search, preserving lazy
word effects, explicit-only automatic values, directory reconstruction and
program-level diagnostics. Keep temporary chain scopes separate from parent
inheritance, retain selected pattern activation, and preserve explicit filename
roles for cleanup. Add NUL-separated rule globs and GNU's bounded-lifetime
impossible-file observations; retry compatibility search only when needed.
Avoid boxed iterators and complete recipe clones on rejected branches.
Add 96 original required fixtures, four lexer tests and a seven-step stateful
scenario. Corpus: 2,978 cases, with 2,966 required and 12 pending. Stateful
coverage is 51 scenarios / 140 invocations. A paired 101-sample local rejection
benchmark improves about 2.9x/5.5x over the preceding build; GNU remains faster,
and the core/control results do not establish general speed parity. See
[implicit secondary expansion](implicit-secondary-expansion.md) for behavior,
measurements and the remaining M4 gates.
