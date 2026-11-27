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
  The database records one lookup per post-read flag source with warnings enabled.
  An unchanged `MAKEFLAGS` replay also records its encoded command-line eval
  reference when present.
  When the reader changes canonical `MAKEFLAGS`, post-read command replay
  consumes the already expanded default command-variable suffix. The ordinary
  non-POSIX suffix also reuses the result of a file-defined recursive
  `MAKEOVERRIDES` so its expansion effects run once before the first recipe.
  A changed canonical POSIX suffix that directly names the automatic
  command-line snapshot replays that expanded snapshot. When the reader
  republishes canonical `MAKEFLAGS` without its public override suffix,
  post-read replay has no override words. A later `MAKEOVERRIDES` definition
  can supply the suffix on the next publication. A literal `override MAKEFLAGS`
  in the Makefile supplies its own replay operands, so post-read replay skips
  the separate `MAKEOVERRIDES` lookup. A recursive `override MAKEFLAGS` with a
  literal option prefix and an explicit public or hidden command-variable
  reference decodes the complete expansion during the read and after it. The
  post-read decode consumes those operands without a separate lookup.
  Canonical flags with the automatic command-line eval reference use the same
  replay path. The encoded eval arguments remain flag data. The original
  command-line source is read again in the next process image after a restart.
  The startup preload list uses one expansion of `MAKEFILES` after command-line
  eval. An absent binding can warn at that point, while a defined empty binding
  leaves the preload list empty.
  Startup failure dumps mark the file records already selected by command-line
  goals, including when a command assignment supersedes `MAKECMDGOALS`.
  An exact duplicate temporary stdin operand leaves ownership with the
  earlier source record.
  Repeated assumed-old operands check prepared names before expanding HOME
  or recording another filename lookup.
  The initial file-table publication records each retained old operand and
  then each active new operand, even when the name shares an age record.
  After a restart, the delayed file-table publication records all retained
  new operands in order. Shared names keep one logical age record.
  The database prints the published what-if timestamp for an unvisited
  ordinary file record, including a dump before a generated source restart.
  An unvisited assumed-old record also shows its published successful
  update. An active what-if operand for the same file replaces its age.
  A reader failure before publication preserves the unchecked file state.
  Startup include options cache each accepted operand after filename preparation.
  Search construction records each existing directory in scan order and preserves
  its cache event even when a later `-I-` clears it from the search list.
  An accepted startup `-C` operand also enters the cache during decoding,
  before later goals or assignment effects and before the actual directory change.
  Live `MAKEFLAGS` and post-read flag decoders cache each accepted include
  operand at its argument boundary, preserving the events completed before a
  later assignment or terminal option error.
  Active include search rebuilds cache each successful directory scan before
  any later reset or HOME failure. Passive decoding leaves the search intact.
- [Output validation and mutex failures](output-validation-order.md) and the
  [startup output-lock gate](../tests/startup-output-mutex/README.md). Completed
  definitions and output remain observable when later validation or opening fails.
  Inherited regular output files use append mode. The startup output gate
  checks the original flags on pipes and devices and the flags of recipe spools.
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
  A named append to an existing file-local automatic in an ordinary ungrouped
  rule evaluates its simple suffix before origin precedence preserves the
  automatic value. This also applies to explicit single-colon groups without
  filename aliases when the named peer has its own automatic binding. The
  database models check suffix effects and lookup counts. An ordinary simple
  or recursive override of a retained automatic also checks the caller's origin
  after the automatic binding survives the assignment. A first named or
  pattern append checks the binding's own table before storing its deferred
  suffix. Expanding an inherited append in a recipe or environment starts
  a fresh lookup and records the logical tables visited as it finds each part.
  A suppressed named conditional read from global scope retains the file's
  variable table and records the caller lookup if the selected origin permits it.
  An ordinary active recipe eval probes the named file's visible variable
  chain before applying qualifiers. It publishes updated privacy before the
  caller's origin lookup, including when the caller inherits that named file.
  A lexical binding can stop the caller lookup.
  A command-line or environment-override binding in a separate caller can
  replace the selected ordinary named file binding in place. The destination
  keeps its definition location and export policy.
  When the named conditional selects the global binding, the same
  transfer updates that global slot for subsequent readers.
  A retained inherited binding can transfer into its owning named
  declaration. Later descendants and named reads see that owner's update.
  A plain double-colon head prints its current named declaration after a
  later branch changes it. Each branch retains its own automatic bindings.
  A selected activated pattern slot can also receive a command value.
  An inherited lookup updates the first visible pattern table it selects.
  Cached reads share the replacement through that table's identity.
  The database prints a pattern declaration with the flavor adopted from a
  command-line or environment-override binding. Activation uses the original
  assignment operator.
  A recipe or shell environment keeps the binding selected for each name
  during assembly. Later export expansion sees changes to that binding and
  reads its dependencies in the live scope.
  Recipe and shell environments copy inherited export policy during assembly.
  Changes to the selected binding itself are visible at emission.
  Standalone export and unexport lists record the visible-scope lookup for
  each name before updating its policy or creating an empty global binding.
  A list lookup refreshes the global .VARIABLES cache when that special
  binding is visible. A selected shadow keeps its ordinary value.
  Recipe and shell child environments use the current stored .VARIABLES
  bytes. A recursive assignment expands those bytes in the child environment
  context. An ordinary lookup can refresh the stored bytes as names change.
  An inactive pattern override preserves the stored global `.VARIABLES` value
  for the reader and recipe environments.
  An explicit single-colon group with an activated pattern binding can also
  receive a stronger caller command-line value during conditional eval.
  An undefine probes the global definition table before checking origin.
  User-defined calls record the initial body lookup and the lookup in the
  argument frame when a literal callee has a body. An empty callee still
  evaluates its arguments before returning an empty result.
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
  An accepted include appends its filename to the raw value of the visible
  MAKEFILE_LIST binding and writes the result globally. It preserves the
  selected flavor and starts a recursive value when no binding is visible.
  A rejected append still counts its lookup of the global definition.
  The optional-file database marker belongs to the active remake visit.
  A serial fatal dump retains the marker from the interrupted visit.
  A scoped include during a serial goal records a failed open as a missing timestamp.
  An optional include in an ordinary recipe body also records its failed open
  when the file has no completed visit and the parallel job queue is idle or
  holds one separate ordinary recipe.
  Required includes in the same recipe-body phase retain the failed open
  while the parallel job queue is idle or holds one separate ordinary recipe.
  A parse failure in a required source just opened by that idle recipe
  leaves its parent file unupdated and retains the included source location.
  The same result is visible when the recipe expansion enables `-p` before
  opening the malformed source.
  With one launched phony sibling, a required source parse failure also leaves
  the expanding target and its waiting ordinary single-colon ancestors unupdated.
  In a serial build, waiting ordinary single-colon ancestors retain this
  update state when recipe, shell, or export expansion fails before the
  affected command runs. This includes required reader parse failures,
  user errors, and built-in arity diagnostics. This also applies when serial
  shell or export preparation fails after an optimized no-op, before the
  first child launches.
  Optional includes from target-specific shell configuration also retain
  their missing timestamps while the parallel job queue is idle or holds
  one separate ordinary recipe.
  The ordinary recipe's exported bindings record the same observation
  while preparing a child environment with an idle queue or one separate
  ordinary recipe running.
  Idle target-specific shell configuration and child export preparation also
  retain the failed open of a required include.
  An accepted reopen, including a required read from those idle preparation
  sites, clears that timestamp while preserving the update verdict.
  A newly opened required source that fails parsing during idle command
  preparation leaves the target unupdated before any recipe command starts.
  Fatal recipe expansion in an idle explicit single-colon group leaves the
  selected member and its unvisited peers unupdated. The selected member
  retains its observed timestamp.
  For a missing target with two plain double-colon declarations, the interrupted
  branch remains unupdated. On a stop in the second branch, the earlier
  successful row keeps its own clock phase, including unchecked time after an
  optimized recipe.
  Startup preloads intern each list operand and the selected read name at their
  respective reader boundaries. The file table probes the normalized name
  before entering the selected spelling when the probe misses.
  When default makefile discovery misses, the reader interns all three
  optional remake candidate names as it enters their file records.
  With no selected goal, the diagnostic reads the raw MAKEFILE_LIST
  binding after default-goal expansion.
  Default-goal selection checks the complete name against existing file
  records before splitting it into words. That includes named and included
  Makefile sources, including an optional source that is absent.
  A serial missing-rule stop occurs before completing the file update.
  The active ordinary prerequisite chain retains that state in a dump.
  A direct serial remake of a missing required include also retains the
  uncompleted file record when no rule can rebuild it and keep-going is off.
  When a declared required include depends on an unbuildable prerequisite,
  the dump also leaves that prerequisite and its waiting single-colon
  ancestors unupdated. A source that opened successfully keeps its physical
  timestamp in the dump.
  Parallel makefile remakes with a pipe jobserver retain the same interrupted
  update state when no worker is active or one existing optional source is
  running a separate recipe.
  A launched child retains its completed failure while its waiting ordinary
  ancestors remain unupdated when traversal stops.
  An ordinary serial keep-going prerequisite consumer samples the failed
  child’s local timestamp while retaining its failed update.
  A retained VPATH alias samples its existing canonical physical file.
  When no VPATH search group applies to the resolved spelling, the first
  prerequisite observation records a missing timestamp on the same failed
  file record.
  Borrowing a .DEFAULT recipe preserves the file’s target classification.
  Named `.INTERMEDIATE` policy is visible on ordinary file rows before
  a visit and when the file is an explicit goal. Ordinary serial double-colon
  branches retain that marker when unvisited or when their first visit finds
  no file.
  A serial parent that rebuilds a deferred named intermediate consumes its
  settled timestamp before cleanup. Completed plain double-colon branches
  report the shared final timestamp or checked absence on both records.
  The post-read file policy marks .PHONY prerequisites as targets before
  visits begin, including when a later setup diagnostic stops traversal.
  This publication also enables the .PRECIOUS marker for file and pattern
  records. A conflict in intermediate policy retains completed named file
  markers and the lookup history through the conflicting directive.
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

The serial command-override restart cases record the original command-line
snapshot at both process entries. The remake and goal witnesses distinguish
the omitted public suffix from explicit republication and the POSIX hidden
snapshot. Separate one-child cases check the exported flag bytes and the
child's command-line values and origins.
Inherited command-line eval cases also check effects in both processes and
literal delimiter bytes in the child's received flags.

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
  An inherited closed stdin stays closed across reader, recipe, and restart
  execs. An explicitly selected child input can occupy that descriptor.

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

Top-level serial implicit search outside makefile remaking records the current
directory for a fixed, immediately resolved basename with an explicit
declaration in a nonterminal candidate when dry-run or question mode is active
alone at both search entry and the observation, with no path search configured.
Enabling dry-run in a rejected secondary expansion preserves the directory
observation of the search already in progress.

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
  Ordinary serial recipes report the last launched executable's failed
  recheck after a later missing-executable, permission, or non-directory
  launch error. This also covers ignored launch failures and prior
  executables that lose owner execute permission or become empty.
  Recursive output synchronization publishes the prior-executable diagnostic
  before pending recipe output, with the failed launch and recipe error
  remaining in their original order inside that output. When a launched
  recipe child exits 127, serial and parallel recipes check the selected
  path before printing the recipe error. A parallel recipe continuation
  also retains the latest launch across a no-child line and ignored
  launch failures. Target, line and recursive sync keep their respective
  output dump boundaries around the recheck. A launch rejected with `E2BIG` or
  `ENAMETOOLONG` also rechecks the prior executable in direct and target-sync
  output. A local symlink loop triggers the recheck in ordinary serial output.
  In a serial dry run or query, a forced direct command that fails to find
  its executable also triggers the recheck. This covers ignored failures and
  recursive output synchronization. The forced direct lookup recheck also
  applies with an owned two-slot pipe jobserver and no other scheduled recipe.
  Target synchronization keeps the forced recipe's direct output order.
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
This also covers the three-source layout with a single trailing optional worker
and an explicit required refresh before the later missing synthetic rule.
The required synthetic failure can also settle before the optional worker
finishes. Both worker orders preserve the optional failure for ordinary
dependents when the worker exits unsuccessfully and both timestamps stay fixed.
When the optional worker leaves a new timestamp that stays current, a
successful remake can reread the source, and an unsuccessful remake preserves
the ordinary dependency error without causing a reread.
For the qualified inline refresh without a relevant time assumption, a trailing
file changed and then restored by the required worker uses its final timestamp
for the restart decision and ordinary goal visits.
A successful sole trailing worker can also leave a reread pending if the
required worker later restores the optional timestamp. In the selected mixed
layout with an earlier optional failure, ordinary dependents instead see the
restored final timestamp. Both layouts also cover a real failure from the final
declared required recipe under keep-going. The recipe diagnostic and cached
dependency failures remain available. Other failure shapes and completion
orders still need qualification.
The ordinary reread also covers an earlier unruled optional source or a direct
explicit declaration that stayed current. The held declaration includes a
recipe effect that would reveal any expansion. The corresponding assumed
refresh retains the tagged missing-rule
failure for dependents and can reread after the required worker restores the
optional file's timestamp.
When the trailing worker fails and its timestamp is restored, its cached
failure remains available to ordinary dependents. The required synthetic
failure still reports its missing rule and independent goals can proceed.
A successful declared required worker can supply a new physical clock to
the source decision before the sole tagged synthetic branch fails. The
held cases cover both reaping orders and a genuine required-worker failure.
A successfully reaped required worker can leave a reread pending when a
trailing optional worker later restores the required file’s timestamp.
The held cases retain the tagged missing-rule diagnostic and check the
final physical clock used by ordinary goals.
With an earlier blocked or absent optional read under keep-going, a
restored required timestamp can govern ordinary prerequisite freshness
and the set of newer prerequisites in the original pass.
A genuine declared required failure can also expose a rewritten clock
before restoration. The held reread cases retain the actual recipe
failure and any matching assumed branch diagnostic in order.
When both actual workers fail, the same observed restoration can produce
a reread in current and sole layouts. Ordinary dependents still see the
appropriate cached failures when the original pass proceeds. A failed
optional worker can also restore the successful required worker’s clock
and leave a reread pending in current and sole layouts. When a blocked or
absent earlier read keeps the original pass, ordinary freshness can use
the restored required clock while dependents still receive the optional
file’s recorded failure.
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
cases still differ. A serial two-rule implicit chain with one immediate
stem-derived prerequisite per rule retains the attached child's lookup
history in the startup and late-print cases. Keep the original goal order when
reporting a discrepancy, since earlier goals can contribute to the lookup
counters. The database cases also
cover ordinary touch timestamps and when a later prerequisite observes a
forced recipe's physical timestamp after an earlier failure.

The native AArch64 glibc gate covers three C and C.UTF-8 locale cases and the
startup debug-option and directory-error diagnostics in C and a pinned French
locale. It runs the startup usage and getopt models on both build profiles
using the same locale data. It also checks selected filesystem and build
diagnostics in C, French, German and Japanese on both builds, including raw
paths and warning order. The reports retain the native comparisons.
The native gate also exercises localized parser diagnostics, adopted
stdin cleanup, and temporary-file write errors on both builds.
Localized built-in arity reports preserve each evaluated argument effect before
the error and retain the original function name, argument count and location.
Adopted stdin cleanup reports the localized action and errno with the raw
source path, including reader failure and early help.
The localized file-size checks verify the stdin write failure before reader
startup, using the recorded temporary name, retained bytes, and file mode.
Recipe cleanup uses the localized unlink report when an owned changed
target cannot be removed. The NLS checks cover raw paths and retain the
recipe diagnostic, deletion announcement, and filesystem outcome in order.
Built-in recipe failures report the translated location label for numeric
exit and signal statuses, including ignored errors and parallel output.
Physical touch opens ordinary targets for reading and writing. The locale
checks retain the target's contents and timestamp after a denied open.
Interrupted intermediate cleanup localizes deletion and unlink failures while
preserving raw filename and system error bytes.
Diagnostic prefixes use the raw final component of `argv[0]`, with `make`
as the fallback for an empty invocation argument.
Native setup-name inference classifies basename bytes under the active C
locale, including letters in a single-byte locale.
A restart that cannot return to the original directory emits the localized
system warning and fatal message in order. It reports a failed unlink of
the owned stdin source before the jobserver token report and finalizers.
The dynamic musl gate checks C.UTF-8 wildcard byte widths, a C raw-target
diagnostic beside a user warning, and ordinary inherited, private, and
pattern export scopes. Broader diagnostic producers, native and
Guile callbacks, cache history, resource failures and musl NLS remain open. See the
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

- A failed reader temporary spool retains one retry for the next built-in
  output. Direct reader info already emitted stays in order. A warning or
  fatal diagnostic pending during another creation failure is replaced by
  the suppression message. Fatal status and the separate post-read replay
  remain observable.
  [Reader spool fixture](../tests/cases/output-sync-reader-spool-retry/case.toml)
- [Output spool pressure](../tests/resource-fd-spool/README.md),
  [compact pipe pressure](../tests/resource-fd-pipe/README.md),
  [FIFO pressure and protected-directory failures](../tests/resource-fd-fifo/README.md)
  and [overlapping recipe descriptors](../tests/resource-fd-parallel/README.md).
- [Jobserver token release](../tests/jobserver-release/README.md) includes a
  held implicit recipe while the parent reports a shell configuration error.
  The word and wordlist index, built-in arity, and intcmp number cases
  record the paid slot through the fatal wait and the returned byte after the
  worker ends on a borrowed pipe or FIFO. Index errors in the recipe body
  check the before-reservation boundary.
- [Private wake slots](../tests/jobserver-wake-slots/README.md) and
  [owned/borrowed pipe flags](../tests/jobserver-read-flags/README.md).
- [Stdin file-size limits](../tests/resource-file-size/README.md) and
  [stdout file-size limits](../tests/stdout-file-size/README.md).

## Native modules and Guile

**What remains**

Extend native setup, callbacks, shutdown, source restarts and standard-stream
handling across startup phases, profiles and resource failures. Preserve
registration, allocator and callback ownership through those combinations.
When a remake pass returns a query verdict, unloaded modules stay closed.
The [native lifecycle gate](native-modules.md#remaking-and-finalization)
checks query, dry-run and ordinary reload behavior. In the successful unchanged
ordinary serial cases, modules reload in read order when each has an admitted
read record. The unsuccessful ordinary serial cases include failed recipes
for optional sources and optional sources without rules. Already-unloaded
modules stay closed after these failures. Terminal load diagnostics in the
reader keep their original error text and give existing finalizers the
selected file and active lexical scope. A fatal zero or negative index in the
word function keeps the diagnostic definition location and gives finalizers
the active caller’s location and scope. Built-in arity failures preserve
the same distinction after eager and call arguments have expanded, or
before a direct lazy call expands any argument. A registered native
function’s minimum-arity error also retains the caller, preserving its
existing eager, raw, or call argument behavior.
GNU Make remains an external test reference and cannot serve as a runtime
implementation fallback.

The hosted AArch64 glibc job for PR #1006 matched two GNU runs for eight selected
marker-only lifecycle models in each build. The hosted x86-64 dynamic musl job
for PR #1009 matched four marker-only reload models and two setup controls in
the optimized build.
[AArch64 glibc run](https://github.com/jeffdasilva/rmake/actions/runs/46014378962/job/140557643523) ·
[Dynamic musl run](https://github.com/jeffdasilva/rmake/actions/runs/46017304171/job/140567601576)

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

The [AArch64 GNU reference workflow](../.github/workflows/guile-aarch64-reference.yml)
checks the pinned GNU Make 4.4.1 Guile feature and a Scheme scalar result. The
native AArch64 glibc job uses that SDK and reference to gate seventeen application
models against the enabled rmake release, including five live converter cases
and five public `gmk-var` shape and module binding cases.
The full application and component gates use the x86-64 glibc SDK.

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
header-rebuild phases and independent executable checks. All 19 pinned profiles
matched two GNU runs in each default build at `8f75a838b`. The comparison
restored and hash-checked 640 raw records, including the Zstandard pipe
fallback with an innermost `-j1` build.
[Exact project comparison](https://github.com/jeffdasilva/rmake/pull/1007)

The hosted AArch64 glibc and x86-64 dynamic musl gates check these phases and
the checksum for pinned xxHash 0.8.3. AArch64 glibc also runs both pinned Lua
profiles, covering pipe and FIFO jobservers and recovery of a removed executable
in the pipe profile. The x86-64 job runs the Lua pipe profile using its verified
dynamic musl GNU Make and rmake executables.
[Native musl Lua run](https://github.com/jeffdasilva/rmake/actions/runs/46007027825/job/140532647293)

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
