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
Validate the required adapters beyond the current x86-64 profile. Internal
resource guards receive ordinary compatibility review and no safety-exception
credit by default.

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

Named-target `MAKEFLAGS +=` assignments preserve their current outer global
value through live option changes, recipe expansion and recursive export.

**What remains**

Extend combined rule, resource and profile tests around implicit search, aliases,
cycle freshness, automatic variables, file assumptions and traversal order.
Broader shuffle coverage includes malformed usage and interactions with trace,
search and live options. Keep declaration order and actual filesystem observations
in the expected behavior.

Repeated searches can still differ in which pattern-specific values reach a
recipe. In a rule for `%.out` that lists `%.mid` before a `%.tmp` prerequisite
that also requires `%.mid`, both implementations can expand `%.mid` twice.
GNU Make can leave the `%.mid` pattern-specific value unset in the recipe while
rmake retains it. Wider interactions with runtime metadata, including
`.SHELLSTATUS` and `MAKEFLAGS`, still need qualification. Mixed required and
optional failure policies remain part of the
[makefile remake work](#parallel-modes-makefile-remakes-and-jobserver-paths).

**Evidence**

- [Shared-child first selection](../tests/cases/shared-owner-diamond/Makefile)
  and the [accepted descendant after rejection](../tests/cases/implicit-accepted-descendant-rejected-ancestor/Makefile)
  fixtures exercise shared ownership during search. See GNU Make's
  [implicit rule search algorithm](https://www.gnu.org/software/make/manual/html_node/Implicit-Rule-Search.html)
  and [secondary expansion](https://www.gnu.org/software/make/manual/html_node/Secondary-Expansion.html).
- [Named-target flag appends](../tests/cases/makeflags-logical-append-implicit-add-0-0/Makefile)
  check live changes and recursive export. The [target-scope guide](target-scopes.md)
  describes variable inheritance.
- [Implicit prerequisite spelling](implicit-prerequisite-spelling.md) covers
  directory and read phases, byte/backslash cases and negative rule selection.
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

Cached prerequisite failures already reported by a required remake now retain
their reporting state through an optional visit, as checked by the
[shared-failure regression](../tests/cases/optional-cached-failure-required-two/Makefile).
Retained missing-rule messages now follow the latest actual prerequisite edge,
including a [later ordinary-goal visit](../tests/cases/retained-failure-parent-later-new-parent/Makefile),
while variable inheritance keeps its first owner. Later explicit-goal replay
counts and verbose event order remain open.

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

[Serial remake diagnostics](debug-output.md) cover double-colon failure recovery,
returned failures and cached visits to required and optional declarations.
Existing includes changed or removed by a successful grouped recipe are reread
when their timestamps change. An already loaded source keeps its earlier file
observation when an earlier serial recipe removes it. Tests cover unruled,
explicit, empty, order-only and double-colon forms. The
[late timestamp restoration test](../tests/cases/include-failed-group-clock-late-restored-required-owner-first/)
checks that a required read peer can still trigger a restart based on the time
observed when the failed grouped recipe finished.

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
earlier goals can contribute to the lookup counters.

Broader locale work includes native and Guile callbacks, diagnostic producers,
cache history and resource failures. The x86-64/glibc results and selected
C/C.UTF8/private French cases retain their recorded scope. Wider Linux profiles
need separate acceptance.

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
header-rebuild phases and independent executable checks.

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
