# M3/M6 compatibility work still open

M3 and M6 remain open. M3 covers dynamic Makefile behavior, including evaluation,
scopes, effects and restarts. M6 covers the remaining command-line, diagnostic,
extension, locale and integration behavior needed for GNU Make compatibility.
The [implementation plan](implementation-plan.md) defines their exit criteria.

M4 rule resolution and M5 Linux execution have passed their acceptance gates on
the pinned development profile. Complete compatibility still requires the areas
below to work together across the required profiles. A zero-pending curated
corpus establishes results for its recorded cases. The wider inventory remains
incomplete.

## Startup and option handling

**What remains**

Complete malformed option decoding, nested flag evaluation and the ordering of
startup, reader and live option effects. Qualify constructor metadata, option
activation, diagnostic policy and resource lifetime together. Extend coverage of
ambiguous options, missing operands, contextual `HOME` values and failures during
output synchronization. Earlier completed effects must survive later failures.

The [option inventory](m3-option-inventory.md) tracks individual surfaces,
including trace, directory reports, eval, file assumptions, shuffle and undefined
variable warnings. Startup safety conflicts retain the explicit exception policy
at the end of this audit.

**Evidence**

- [Flag assignment evaluation](flag-payloads.md),
  [inherited path switches and reference boundaries](profile-inputs.md),
  [missing option operands](live-option-arity.md),
  [long-option prefixes](long-option-prefixes.md) and
  [invalid-option continuation](live-invalid-options.md).
- [Live warning policy](live-warning-phases.md),
  [post-read warning policy](passive-warning-phases.md) and
  [sequential source completion](passive-source-order.md). These records retain
  the order of definitions, warnings and `MAKEFLAGS` expansion.
- [Output validation and mutex failures](output-validation-order.md) and the
  [startup output-lock gate](../tests/startup-output-mutex/README.md). Completed
  definitions and output remain observable when later validation or opening fails.
- [Initial inherited expansion](initial-flag-expansion.md),
  [ordered CLI startup](ordered-cli-startup.md),
  [literal startup](literal-startup.md) and
  [constructor metadata](constructor-metadata.md).
- [CLI assignment effects](cli-assignment-effects.md),
  [command-line eval](command-line-eval.md),
  [startup replay](startup-replay.md),
  [switch origins](switch-origins.md) and
  [runtime flag inspection](runtime-flag-inspection.md).

## Evaluation, scopes and source reading

**What remains**

Extend parser and effect coverage across target, pattern, lexical and automatic
variable scopes. Qualify deep evaluation together with includes, native callbacks,
Guile stack and garbage-collection behavior, and stack, heap and output limits.
Broaden adapter coverage beyond the selected x86-64 and AArch64 glibc
checks. Internal resource guards receive ordinary compatibility review and no
safety-exception credit by default.

Special metadata, POSIX mode, late built-in changes and child environment
selection need broader option, extension and profile combinations. The current
source and evaluation tests preserve source mutation, ordered effects, raw bytes,
NUL boundaries, rebinding and cancellation.

**Evidence**

- [Target scopes](target-scopes.md), [pattern scopes](pattern-scopes.md) and
  [conditional delimiters](conditional-whitespace.md). The target gate includes
  recipe-time `eval` in the active file scope, nested declarations and shell status.
- [Variable metadata](variable-metadata.md), [POSIX mode](posix-mode.md),
  [late built-in disabling](late-builtins.md) and
  [byte-preserving child environments](environment-vectors.md).
- [Literal reference chains](literal-chains.md),
  [text and reference frames](text-frames.md),
  [linear evaluator continuations](linear-continuations.md),
  [general expression continuations](heap-continuations.md) and
  [inherited appends](deep-appends.md). These records cover the progression from
  simple chains to full effects, nested scopes and shared binding history.
- [Live source streams and parser continuations](live-sources.md) retain open-file
  identity, read/error/EOF behavior, source locations and owned cleanup.
- [Eval allocation investigation](eval-allocation-profiles.md) retains experiments
  rejected after elapsed-time regressions and the selected runtime controls.

## Paths, includes and restarts

**What remains**

Qualify malformed and nested include-directory changes, runtime `HOME` and tilde
handling, source identity, duplicate operands and diagnostic ordering together.
Broader path and profile acceptance includes byte limits and pure-library calls
whose directory context has not been configured.

Process-image replacement is implemented. Remaining restart work combines startup
assignment replay, winning command variables, directory namespaces, inherited
state, native standard streams, failure/resource limits and enabled Guile.
Named sources must retain the selected operand while startup effects replay.

**Evidence**

- [Startup include search](startup-include-search.md),
  [runtime `HOME` operands](runtime-home-includes.md) and
  [second-stage include search](secondary-include-search.md).
- [Runtime include metadata](runtime-includes.md),
  [startup metadata recursion](startup-metadata-recursion.md),
  [include operand history](include-operands.md) and
  [interrupted include metadata reads](include-stat-interruptions.md).
- [Early absolute-path selection](early-abspath.md) and
  [Linux absolute-path bounds](abspath-bounds.md).
- [Imported restart state](restart-state.md),
  [directory trace lifetime](imported-directory-trace.md) and
  [restart directories and recursive identity](restart-directories.md).
- [Named Makefiles and stdin](stdin-sources.md),
  [process-image replacement](../tests/process-restarts/README.md),
  [raw restart environments](../tests/restart-environment/README.md),
  [inherited pipe restarts](../tests/restart-pipe/README.md) and
  [native standard-stream restarts](../tests/native-restart-stdio/README.md).

## Rules and target state

When an implicit rule is accepted, its inferred child rules become available to
later searches. They remain available if an enclosing candidate fails. If the
enclosing candidate succeeds, its own earlier selection can replace a nested
selection for a shared child. Direct repeats can still expand more than once.
The ownership fixtures check selection, visible expansion effects and hidden
prerequisites against GNU Make.

When two searches select the same intermediate, the selected rule supplies the
pattern scope. A pattern binding that another accepted selection already merged
can fall back to a named, inherited or global value. A named assignment during
search refreshes the pattern table used by later merges for that target. The
latest search's runtime metadata is still available to the recipe.

Named-target `MAKEFLAGS +=` assignments preserve their current outer global
value through live option changes, recipe expansion and recursive export.

**What remains**

Extend combined rule, resource and profile tests around implicit search, aliases,
cycle freshness, automatic variables, file assumptions and traversal order.
Broader shuffle coverage includes malformed usage and interactions with trace,
search and live options. Keep declaration order and actual filesystem observations
in the expected behavior.

Wider combinations of pattern scope, runtime metadata and rule selection
still need qualification, including combined late registration, unusual
search spellings and blocked visits. The
[makefile remake work](#parallel-modes-makefile-remakes-and-jobserver-paths)
covers mixed required and optional failure policies.

**Evidence**

- [Shared-child first selection](../tests/cases/shared-owner-diamond/Makefile)
  and the [accepted descendant after rejection](../tests/cases/implicit-accepted-descendant-rejected-ancestor/Makefile)
  fixtures exercise shared ownership during search. See GNU Make's
  [implicit rule search algorithm](https://www.gnu.org/software/make/manual/html_node/Implicit-Rule-Search.html)
  and [secondary expansion](https://www.gnu.org/software/make/manual/html_node/Secondary-Expansion.html).
- [Repeated intermediate pattern scope](../tests/cases/implicit-pattern-owner-forward/Makefile)
  checks which bindings and shell status reach the recipe. A
  [named assignment during search](../tests/cases/implicit-pattern-owner-named-refresh/Makefile)
  checks that a later candidate still sees the refreshed pattern table.
- [Named-target flag appends](../tests/cases/makeflags-logical-append-implicit-add-0-0/Makefile)
  check live changes and recursive export. The [target-scope guide](target-scopes.md)
  describes variable inheritance.
- [Implicit prerequisite spelling](implicit-prerequisite-spelling.md) covers
  directory and read phases, byte/backslash cases and negative rule selection.
- Late GPATH cases cover queued summaries for raw and canonical operands,
  named and unnamed visited destinations with prerequisite ancestry, and a
  separate row for a recreated raw spelling. Unvisited canonical rows preserve
  their own status and stem. The models cover runtime bindings imported into a
  predeclared canonical table and eager declarations
  after vacant and mention-only renames. Archive cases cover absent containers
  and permanent moves into vacant or pre-entered destinations.
  [Goal fixtures](../tests/cases/gpath-late-merge-visited-destination-status/) ·
  [Database models](../tests/database-output/search-spelling.json)
- With `-p`, the tested ordinary serial ungrouped single-colon recipes for
  non-archive files retain expansion effects, including `.SHELLSTATUS`. The
  database reports an unattempted file when line expansion fails before dispatch.
  The tested unignored serial launch errors record the first cleanup-policy
  lookup in the file table and invalidate the displayed nonphony timestamp
  when a direct executable or the configured recipe shell fails to start.
  This covers fully forced recipes under `-n`, `-q`, and `-t`. In those modes,
  the database retains the simulated timestamp of a nonphony file when its mixed
  forced and ordinary recipe reaches an unignored launch failure or nonzero
  child exit. The same timestamp is recorded when serial `-q` stops at an
  ordinary line after a real child or an ignored executable launch failure.
  This leaves the existing target cleanup rules in effect.
  Ordinary serial recipes now report a failed stat of the last launched
  executable after an unignored missing-executable error, before the recipe's
  terminal error. Ignored errors, child exit 127, other prior executable
  failures, and recursive output synchronization remain open.
  Cleanup after a real child error follows the existing policy. Archive and remake
  paths and wider scheduler combinations still need qualification.
  [Database phase models](../tests/database-output/phases.json)
- [Old and what-if file records](assumed-files.md) cover logical ages, physical
  timestamp witnesses, recursion, VPATH and Makefile remakes.
- [Shuffle traversal](shuffle.md) covers ordinary, grouped and double-colon rules,
  implicit `.WAIT`, targeted `.NOTPARALLEL`, scopes and recursive jobservers.
- [Forced recipe modes](forced-touch.md) cover parallel execution and resumed
  double-colon work after a later goal enables keep-going. Further grouped and
  live-option combinations remain open.

## Parallel modes, makefile remakes, and jobserver paths

**What remains**

Extend directory-qualified sources, expanded source identities and combinations
of load limits, grouped remakes, build modes and debug output. Tests now cover
serial and parallel explicit grouped rules that restart after leaving a read peer
unchanged or missing. The serial tests include an empty recipe. Implicit grouped
single-colon rules have serial and held parallel cases. Grouped double-colon
cases now cover read peers, real restarts, settled serial branch failures and
the number of repeated verbose visits. Serial tests now also cover a failed
grouped recipe whose read peer gets a newer timestamp, including a later recipe
that restores the timestamp. A required peer with a single grouped declaration
also retains the result of a failed optional makefile recipe seen earlier,
including when `-k` is absent. More cases remain for duplicate peers, mixed
failure policies, and parallel scheduling and tracing.

An unavailable prerequisite during a parallel remake now emits the failure
event under basic debugging for optional sources and for required remakes with
`-k`. The [remake debug fixtures](../tests/debug-output/remake-cache-phase.toml)
cover missing leaves, order-only inputs, shared prerequisites and cache visits.
The held abort fixture also checks that a required failure still drains the
running worker without announcing another prerequisite wait.

Cached prerequisite failures already reported by a required remake now retain
their reporting state through an optional visit, as checked by the
[shared-failure regression](../tests/cases/optional-cached-failure-required-two/Makefile).
Retained missing-rule messages now follow the latest actual prerequisite edge,
including a [later ordinary-goal visit](../tests/cases/retained-failure-parent-later-new-parent/Makefile),
while variable inheritance keeps its first owner. An explicit visit to a
settled optional double-colon makefile now repeats its retained diagnostic for
each declared branch. The [verbose replay fixture](../tests/cases/optional-double-colon-goal-replay-repeated-verbose/Makefile)
checks the order of those diagnostics and the cached branch reports.
Current parallel double-colon sources have consideration and same-pass pruning
coverage for repeated includes and earlier prerequisite visits. The held
synthetic cases check consideration before implicit search, cached declaration
checks after reap and the owned jobserver drain message before restart. Wider
combinations with load limits, diagnostic masks and `-W` remain open.
[Structured remake models](../tests/debug-output/remake-cache-phase.toml)

Local jobserver acceptance also needs broader combinations of transports,
recursive builds, restarts, loaded modules and resource pressure. Preserve the
actual token, child and descriptor relationships in those tests.

**Evidence**

The [held remake gate](../tests/remake-parallel/README.md) uses controlled real
workers for bounded and unbounded parallel builds. It covers required and
optional includes, named primary Makefiles, file prerequisites, repeated sources,
grouped recipes, stopped workers, keep-going and both FIFO and pipe transports.
Load and release observations qualify child admission and exact trace boundaries.

Dry-run, question and touch profiles check effects and restart verdicts, including
combined question/touch. Named includes and primary sources exercise multiple
read positions, ordinary and order-only prerequisites, implicit rules, recursive
recipes and workers already started by earlier includes. Held shuffle controls
separate ordinary goal order from remake order.

For ordinary explicit grouped targets under `-q` without `-k`, a query verdict
ends the current member's dependency scan. Later members still begin their own
scans. A forced recipe can stop the whole query. The
[grouped query fixtures](../tests/cases/grouped-query-expands-and-runs-first-peer-edge/Makefile)
check expansion effects, forced recipes and missing prerequisites. The
[double-colon fixtures](../tests/cases/double-grouped-query-expands-and-runs-peer/Makefile)
cover the same member scan and its later branch verdict. The
[parallel models](../tests/debug-output/grouped-single-wait-frontiers.toml)
also cover live workers, `.WAIT` boundaries and cached debug events. Combined
query modes and later queued goals still need broader validation.

A retained query verdict suppresses the ordinary success summary after a
single-colon goal or prerequisite is found out of date. Double-colon goals can
still report a current head when a later branch supplies the query verdict. The
[named query fixtures](../tests/cases/named-remake-query-summary-double-head-control/Makefile)
check the summary and exit status together.

[Serial remake diagnostics](debug-output.md) cover double-colon failure recovery,
returned failures and cached visits to required and optional declarations.
Existing includes changed or removed by a successful grouped recipe are reread
when their timestamps change. An already loaded source keeps its earlier file
observation when an earlier serial recipe removes it. If the later source's
own recipe refreshes its timestamp, the successful serial restart comparison
uses that earlier observation. The removal and recreation cases cover
required and optional sources, empty and single double-colon recipes, and
recreation at the original timestamp. Successful ungrouped declarations
that leave an existing source share its final physical timestamp. A later
declaration can restore the original timestamp after an earlier write.
The loaded-source comparison also covers a serial explicit declaration
that inherits an ungrouped pattern recipe. A source already rebuilt as
an earlier prerequisite contributes its settled timestamp when later
visited as a source, including after an intervening recipe retimes it.
The [cached prerequisite fixture](../tests/cases/include-serial-cached-source-transitive-restore/)
and the [inherited recipe fixture](../tests/cases/include-serial-implicit-source-restore/)
exercise those boundaries.
For a failed serial remake under keep-going, a settled real refresh also
uses the earlier timestamp when the declared head succeeded or the failed
source is required. The [failed inherited recipe fixtures](../tests/cases/include-serial-failed-implicit-source-required-retime/)
cover the initial required visit and the replay of an optional failure as
required. A blocked declaration preserves the earlier observation
when no branch refreshes it. Multi-declaration remakes that leave the source
absent need separate qualification. Existing current-source cases cover
unruled, explicit and order-only forms. The
[late timestamp restoration test](../tests/cases/include-failed-group-clock-late-restored-required-owner-first/)
checks that a required read peer can still trigger a restart based on the time
observed when the failed grouped recipe finished.

An earlier failed prerequisite visit retains the settled time of ordinary,
implicit grouped, and explicit grouped owners. A directly visited explicit
group owner also retains its completion sample. A directly visited implicit
owner uses the final physical comparison. The
[restored explicit owner fixture](../tests/cases/include-serial-failed-direct-explicit-group-owner-restore/)
checks the restart after a later source restores the old timestamp.
The same completion sample applies to read peers after an actual explicit
group recipe fails under dry-run or question mode. The [forced query fixture](../tests/cases/include-serial-named-failed-explicit-peer-query-forced-owner/)
checks this after a later source restores the peer's original timestamp.
An unrelated source failure can also trigger a reread using the successful
explicit peer's sampled physical change, or the logical update of a required
named source simulated under `-k -n`. The [forced peer fixture](../tests/cases/include-serial-successful-forced-explicit-peer-combined-optional-after/)
and [held required-failure models](../tools/remake_parallel.py) cover the
error pass, including a second failure after a restart.

A terminal current named include can also supply dry-run mode when the
remake pass reaps one earlier required worker. The held cases cover a
completed command, an ordinary continuation printed under `-n`, and rereads
with unchanged physical timestamps.
A forced continuation retains that mode through its own child completion.
The held observer checks the command line while the second child is live and
the source clock after it exits.

Under parallel keep-going, the held models cover trailing optional reads that
find a missing source, run a worker or inline command, or reuse a cached
completion. A cached optional refresh can trigger a reread after a synthetic
worker restores the original timestamp.
Other models preserve the successful read head through a failed declared tail
and a later assumed branch. After a successful explicit refresh, a current
synthetic declaration can propagate the logical clock. A delayed missing
synthetic rule retains its diagnostic while the successful read head allows
independent goals to proceed after a passive optional visit or a verified
inline optional refresh while the explicit worker is live. The actual failures
remain visible to ordinary dependents. The held earlier-only cases cover an
absent optional source and a declared optional with a missing prerequisite.
An earlier failure can also coexist with a trailing read of an unruled source
or a current declared source that settles at its original timestamp before the
required worker is reaped. The declared cases check that the recipe stays
unexpanded. An unruled trailing source can also settle through a direct
missing-rule visit. That failure remains available to ordinary dependents
while an independent goal can still proceed.
The trailing declaration can also run inline while the required worker is
active when its completion and final physical timestamp both match the
original timestamp.
The selected two-worker cases retain the optional child's successful completion
while the required child is still pending and both files keep their timestamps.
Other completion orders and changed worker clocks still need qualification.
[Held remake campaign](../tools/remake_parallel.py) ·
[Structured remake models](../tests/debug-output/remake-cache-phase.toml)

The [composed validation from PR #374](https://github.com/jeffdasilva/rmake/pull/374)
and [grouped double-colon database models](../tools/database_grouped_double_output.py)
record secondary prerequisites, stems, branch ownership, physical timestamps,
query paths and repeated grouped owners.

Startup jobserver tests distinguish malformed authentication from closed
descriptors and unavailable FIFOs. Authentication added while reading remains
in recursive metadata and preserves an adopted pipe.

## Diagnostics and locales

**What remains**

Complete debug, trace, directory and database behavior across asynchronous work,
implicit search, default Makefile discovery, recursive builds, restarts and live
option changes. Qualify deterministic ordering and statistics, and use specific
relationship validators for inherently variable process or time fields.

The database now records accepted implicit rules before a recipe enables
`-p`, including earlier goals that selected a rule. A small case such as
`%.out: ; @$(eval MAKEFLAGS += -p):` can check this behavior. Other late-print
cases still differ. Keep the original goal order when reporting one, since
earlier goals can contribute to the lookup counters. The database cases also
cover ordinary touch timestamps and when a later prerequisite observes a
forced recipe's physical timestamp after an earlier failure.

The native AArch64 glibc gate covers three C and C.UTF-8 locale cases and the
startup debug-option and directory-error diagnostics in C and a pinned French
locale. It runs the startup usage and getopt models on both build profiles
using the same locale data. The reports retain the native comparisons.
The dynamic musl gate has one C.UTF-8 wildcard byte-width case. Broader diagnostic
producers, native and Guile callbacks, cache history, resource failures and
musl NLS remain open. See the
[native AArch64](../.github/workflows/native-aarch64.yml) and
[dynamic musl](../.github/workflows/native-musl.yml) workflows.

**Evidence**

- [Database output](database-output.md) records execution history, search aliases,
  table/string events and native numeric formatting.
- [Structured debug output](debug-output.md),
  [implicit traversal](../tests/debug-traversal/README.md),
  [verbose decisions](../tests/debug-verbose/README.md) and the
  [child-reap regression](../tests/debug-reap-race/README.md). These include
  `.DEFAULT` selection, grouped freshness, question mode and cached visits after
  a child is reaped.
- [Actual-child JOBS output](job-debug.md) validates live identity, ordering and
  token relationships. Broader scope, transport and lifecycle combinations remain
  part of the open work above.
- [Trace output](trace.md), [undefined-variable warnings](undefined-warnings.md)
  and [directory reporting](directory-reporting.md).
- [Startup locale selection](startup-locale.md),
  [wildcard collation](locale-collation.md),
  [localized diagnostics](nls-diagnostics.md),
  [release catalog coverage](../tests/catalog-coverage/README.md) and
  [startup usage output](startup-usage.md).

## Linux resources and output ownership

**What remains**

Extend descriptor, file-size and process-resource testing across directory and
stdio profiles, fatal parent exits and shared-token recovery. Parent fatal token
loss remains open. Broader reader-output combinations must preserve output that
was already emitted before a temporary-file failure and any later retry.

FIFO and named output mutex publication preserve relative path bytes. Current
output tests cover recursive directory changes, selected temporary-file failures,
and line/target synchronization. Further combinations remain required.

**Evidence**

- [Output spool pressure](../tests/resource-fd-spool/README.md),
  [compact pipe pressure](../tests/resource-fd-pipe/README.md),
  [FIFO pressure and protected-directory failures](../tests/resource-fd-fifo/README.md)
  and [overlapping recipe descriptors](../tests/resource-fd-parallel/README.md).
- [Jobserver token release](../tests/jobserver-release/README.md),
  [private wake slots](../tests/jobserver-wake-slots/README.md) and
  [owned/borrowed pipe flags](../tests/jobserver-read-flags/README.md).
- [Stdin file-size limits](../tests/resource-file-size/README.md) and
  [stdout file-size limits](../tests/stdout-file-size/README.md).

## Native modules and Guile

**What remains**

Extend native setup, callbacks, shutdown, source restarts and standard-stream
handling across startup phases, profiles and resource failures. Preserve
registration, allocator and callback ownership through those combinations.
When a remake pass returns a query verdict, unloaded modules stay closed.
The [native lifecycle gate](native-modules.md) checks query, dry-run and
ordinary reload behavior.
GNU Make remains an external test reference and cannot serve as a runtime
implementation fallback.

The optional Guile application needs broader asynchronous entry points, value
conversions, error backtraces and resource acceptance. The reference-variable
fatal-wait behavior remains open. The independent Guile component safety gate
continues to be required alongside application comparisons.

**Evidence**

- [Native modules](native-modules.md),
  [native startup and exit ownership](native-startup-exits.md) and
  [load reader preparation](load-reader.md). These cover the GNU ABI,
  `.LOADED`, source context, signed setup results, loader diagnostics, remaking
  and process finalization.
- [Guile application comparisons](guile-frontend.md) cover Scheme conversion,
  mutable pairs, live procedure bindings, callbacks and signal handling.
- [Detached serial waits](../tests/guile-detached/README.md),
  [serial descriptor pressure](../tests/guile-eventfd-pressure/README.md),
  [detached parallel waits](../tests/guile-parallel-detached/README.md) and
  [parallel descriptor pressure](../tests/guile-parallel-pressure/README.md).

The [native AArch64 glibc workflow](../.github/workflows/native-aarch64.yml)
covers host metadata, jobservers, native standard streams, developer flows,
deep evaluation and five optimized file and archive ownership cases. The
[dynamic musl workflow](../.github/workflows/native-musl.yml) covers selected
optimized x86-64 cases, including native module setup, process restart and an
inherited jobserver pipe.

A separate hosted ARM job verified the pinned GNU Make 4.4.1 Guile feature and
a Scheme scalar result. The enabled rmake application and component gates use
the x86-64 glibc SDK. See the
[AArch64 GNU reference workflow](../.github/workflows/guile-aarch64-reference.yml).

## Integration and performance

**What remains**

Expand the project, compiler and developer-environment matrix. Qualify complete
self-hosted workflows with both development and optimized builds, including
bootstrap and cleanup failures. General performance parity, persistent-cache
correctness and complete workload/profile coverage remain open. New rmake plugin
interfaces retain their separate compatibility and correctness requirements.

**Evidence**

The [pinned project corpus](project-integration.md) includes serial leaf builds,
recursive builds and shared jobservers. It covers xxHash, zlib, Lua, GNU gzip,
GNU Patch and GNU Make itself, with exact comparisons across build, no-op and
header-rebuild phases and independent executable checks. All 18 pinned
optimized project comparisons passed after the visited GPATH ownership changes.
The hosted AArch64 glibc and x86-64 dynamic musl gates also run these phases and
the checksum check for pinned xxHash 0.8.3.
[Project comparison evidence](https://github.com/jeffdasilva/rmake/pull/775)

The [developer-flow gate](../tests/developer-flows/README.md) exercises the actual
top-level flows under GNU Make and rmake. It covers warm oracle bootstrap,
cleanup modes, benchmark failures and positive untimed reports. Cold/warm
bootstrap checks use verified inputs and disposable installation prefixes.

Performance evidence includes [startup retention](startup-retention.md),
[automatic variable snapshots](automatic-snapshots.md) and
[JOBS measurements](benchmarks/job-debug.md). Historical measurements for
[combined include/goal work](https://github.com/jeffdasilva/rmake/pull/374),
[intermediate goal checks](https://github.com/jeffdasilva/rmake/pull/384) and
[implicit suffix filtering](https://github.com/jeffdasilva/rmake/pull/391)
retain their inputs, correctness checks and comparison limits. Their measured
improvements apply to those workloads. General speed parity requires broader
evidence.

[Large explicit goal lists](implicit-search-performance.md#large-command-line-goal-lists)
measure indexed membership during implicit traversal. The 8,192-goal
workload improved from 2,496 ms to 368 ms on the measured host. Its
GNU Make median was 136 ms.

## Approved safety exceptions

The owner-approved [safe-error policy](safety-exceptions.md) covers **34 recorded
inputs across S001–S006**. These include documented crashes and hangs, plus the
explicitly approved fifteen normal-exit S004 inputs from the unstable nested
short-option class. The two stalled double-colon query inputs remain within S006.
The [exception manifest](../tests/safety/manifest.json) and
[normal-exit approval](../tests/safety/nested-short-normal-approval.json) define
the exact scope.

The separate candidate-only gate runs 134 cells twice, for 268 observations,
and earns **zero exact-conformance credit**. The ordinary corpus retains the
eighteen nested-short boundary controls and successful CLI eval/restart cases.
The finite S006 keep-going variant also remains in ordinary conformance.
Other ordinary discrepancies and resource guards require compatibility work or
an explicit new decision. They do not inherit these exceptions.

## Completion criteria

Continue with language and metadata correctness, complete option and presentation
behavior, native/profile integration and broader combined acceptance. For each
change, retain the original expectations and observations, run the affected gates,
and validate the full required suite on the proposed commit.

M3 completion requires scope, effect and restart interactions to match without
suppressing required evaluation. M6 completion requires all required profiles to
cover the remaining language and execution surfaces, with curated and integration
suites passing under the strict [compatibility contract](compatibility.md).
The recorded open work above remains part of those criteria even when every
currently selected fixture passes.
