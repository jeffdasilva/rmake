# M3/M6 compatibility work still open

This audit prevents the curated pending count from being mistaken for a complete
GNU Make inventory. Both milestones remain open. Their exit criteria in the
[implementation plan](implementation-plan.md) are unchanged, including native
loading, Guile profiles, diagnostics and locale behavior. M4/M5 acceptance on the
pinned development profile does not establish those surfaces.

| Area | Current evidence and next acceptance work |
| --- | --- |
| Changed flag assignments | [Owned payload evaluation](flag-payloads.md) covers ordinary assignment forms, origins and effects. [Well-formed inherited path switches](profile-inputs.md) now distinguish startup from later decoding. [Live arity errors](live-option-arity.md) add 770 required cases for skipped arguments, missing operands, cluster continuity and ordered definitions. Broader startup/option ordering, malformed decoding and safety conflicts remain open. |
| Live payload warning phases | [Ordered live policy](live-warning-phases.md) adds 1,213 required models, closing 713 preceding mismatches across phases, nested cursors, active locations, eager priority and replay. The three exact historical inputs now pass; original discoveries and rejected hypotheses remain unchanged. Passive source completion is covered below; within-source decoder/lifecycle phases remain open. |
| Passive payload warning phases | [Post-read policy positions](passive-warning-phases.md) add 1,872 required models and close 288 preceding warning mismatches across declaration/source phases, assignment forms, existing values, `-e` timing and raw bytes. The 48 source-policy regressions in the first prototype are closed and retained. Source completion is covered below; other decoder/lifecycle phases remain open. |
| Sequential passive sources | [Ordered source completion and clearing](passive-source-order.md) add 1,290 required models, closing every preceding mismatch across definitions, fresh effects, warnings, priorities, bytes, explicit files/goals and actual FIFO/pipe tokens. GNU definitions finish before MAKEFLAGS expansion. Within-source malformed-value effects, contextual HOME and broader decoder/lifecycle acceptance remain open. |
| Output-sync validation ordering | [Payload completion and source boundaries](output-validation-order.md) add 818 required original models, closing 553 preceding mismatches across live/passive phases, assignment forms, priorities, raw bytes and source/export effects. Invalid modes do not skip payload effects; a different source cannot rescue a fatal selection. [Output mutex completion](output-validation-order.md) adds live and per-source ordering, including 178 catalog profile cells, including held-lock failures, and 19 ordinary post-read cases. Earlier completed definitions retain their effects; later reads cannot rescue an earlier failing open. The [startup lock gate](../tests/startup-output-mutex/README.md) adds thirteen inherited and explicit source cases, including fatal version output. Broader constructor and malformed-value phases, contextual HOME and complete owned decoding remain open. |
| Runtime HOME include operands | [Owned argument suspension](runtime-home-includes.md) adds 506 required models and closes 444 preceding mismatches across live/post-read phases, operand positions, warnings, bytes, callbacks, parallel modes, fallback and fatal behavior. Invocation-only operands stay ignored; completed effects survive cancellation/failure while invocation ownership is restored. Full malformed-value, nested-reset, secondary tilde/search and broader decoder acceptance remain open. |
| Second-stage include search | [Owned search continuation](secondary-include-search.md) adds 474 required models and closes 240 preceding mismatches. Physical tilde resolution follows completed argument definitions without rewriting stored operands; nested resets retain parent cursor context, and empty streams preserve it. Directory/control identity, raw bytes, warnings, fatal effects and invalid output-sync ordering are covered. Broader nested/malformed/lifecycle acceptance remains open. |
| Runtime flag inspection | [One successful parse](runtime-flag-inspection.md) records incoming silent/keep-going/job selections without constructing a second Options tree. 96 original recipe behavior regressions cover source, origin, aliases and output modes. Error retry preserves preliminary failure precedence without interpreter effects. Repeated runtime profiles save 1,009 requests/frees; general timing/parity acceptance remains open. |
| Startup include search | [Pre-default HOME lookup](startup-include-search.md) adds 500 original models and closes 438 preceding mismatches. Constructor evaluation preserves delayed flag activation, ordered database callbacks, warning masks, stored byte operands and search spelling. Raw byte argv is replayable through args_hex. Broader constructor, decoder, profile and lifecycle acceptance remains open. |
| Shuffle traversal | [Owned traversal permutations](shuffle.md) add 1,995 required models and separate random/zero-seed replay checks. Startup policy remains distinct from live requested flags; barriers, scopes, double-colon, remake, recursion, parallel transports and failures have explicit evidence. Full malformed usage, cycle freshness/trace, alias/search and broader lifecycle/profile cross-products remain ordinary acceptance work. |
| Raw implicit prerequisites | [Literal percent substitution](implicit-prerequisite-spelling.md) adds 144 required models, closing 40 spelling mismatches while borrowing unchanged names. Directory/read phases, byte/backslash matrices, negative selection, shared graph metadata and exact profiling fixtures are covered. The slower sharing experiment is retained unselected. Full rule/resource/profile and broader M3/M6 work remain open. |
| Initial inherited expansion | [Initial source evaluation](initial-flag-expansion.md) adds 111 required semantic cases and two output-checked scaling fixtures. Early effects transfer owned state into construction and repeat on restart; 42 initial interruption cells extend runtime coverage. [Ordered explicit startup](ordered-cli-startup.md) and [early inactive-special controls](restart-state.md) add phase evidence. Complete decoding/activation/lifetime and self-exec acceptance remain open. |
| Ordered explicit startup | [Argument-phase preparation](ordered-cli-startup.md) adds 126 required semantic cases and two ordered-argv controls, plus 128 narrow cosmetic-banner exit cells and 42 staged interruption cells. Early metadata, per-position policies, filename operands, output ownership and in-process restart preparation are covered; complete activation/lifetime, malformed decoding and real self-exec remain open. |
| Literal startup unification | [One executable argument plan](literal-startup.md) adds 146 required controls for directory operand comparison, repeated Makefiles, diagnostic levels and restart file effects. It avoids legacy snapshot copies on production invocations and preserves owned cleanup diagnostics; complete constructor ordering, include multiplicity/runtime decoding and real exec remain open. |
| Early abspath directory phase | [Pending versus selected directory](early-abspath.md) adds 132 required controls for inherited/CLI expansion, absolute words, argument effects, computed names and rereads. The pure-library unconfigured boundary remains explicit; broader function/path/decoder/profile acceptance stays open. |
| Linux abspath bounds | [Raw and incremental byte limits](abspath-bounds.md) add 243 required cases for phase, UTF-8, normalization, rejected-word effects and target-context scaling. Borrowed components and exact result capacity reduce measured allocation requests; broader path/profile acceptance and general performance parity remain open. |
| Literal reference chains | [Heap-resolved pure chains](literal-chains.md) add 353 required controls and close the 136 recorded deep-chain research failures plus 30 cross-family gaps. Cycles, diagnostics, lexical/target scopes and effectful fallbacks retain exact behavior. Full function/eval/include continuations, nonliteral deep references, append/output budgets and broader resource acceptance remain open; internal guards earn no approved safety-exception credit. |
| Pure text/reference frames | [Bounded heap evaluation](text-frames.md) adds 479 required controls and closes 182 preceding finite/cycle mismatches. Branches, warnings, scopes, NUL returns and fallback effects retain exact models. General effectful/function/eval/include continuations, nonliteral names, append/output resource acceptance and full M3/M6 remain open. |
| Linear full-evaluator leaves | [Owned literal continuations](linear-continuations.md) add 419 required controls and close 248 preceding mismatches, including all forty recorded finite deep-leaf failures. Effects, recursive exports, active cycles, rebinding, scopes, NUL returns and locations retain exact models. General function/body continuations, effectful branching, eval/include/resource/profile acceptance and full M3/M6 remain open. |
| General expression continuations | [Heap jobs and bounded native preparation](heap-continuations.md) add 457 required models, close 296 preceding mismatches and all 80 prior function-body research failures. Lazy branches, computed/substitution names, recursive calls, lexical/target scopes, fresh eval lookups, ordered effects, raw NUL returns and diagnostics retain exact observations. Parser eval/include and native callback recursion, append/output/resource acceptance, profile adapters and full M3/M6 remain open. |
| Deep inherited appends | [Ordered outer snapshots and heap resumptions](deep-appends.md) add 495 required models, closing 144 preceding mismatches across direct/function-body entry, recipes, secondary expansion, exports, private inheritance, automatic context and empty suffixes. Native stack and broader output/resource acceptance, parser/native callbacks and complete M3/M6 remain open. |
| Eval allocation investigation | [Recipe-free profiling controls](eval-allocation-profiles.md) add three required every-read models and frozen allocation support. Correct ownership/frame experiments were rejected after elapsed regressions; source, observations and build controls remain recorded. Runtime and all broader M3/M6/performance criteria remain unchanged. |
| Live source and parser continuations | [Open streams, heap includes and bounded same-thread eval stacks](live-sources.md) add 269 required models, closing 158 preceding mismatches, and 36 separately gated source-I/O profiles. Source mutation, prepared filenames, sticky EOF/errors, cycle/rebinding identity, descriptor cleanup and cancellation retain original models. Native extensions, Guile stack/GC interaction, non-x86 adapters, broader resource and complete M3/M6 acceptance remain open. |
| Imported restart state | [Process count and binding policy](restart-state.md) add 214 required controls and eight early inactive-special controls. Numeric wrap, reread budgets, export lifetime and owned plan transfer are covered. [Imported directory-trace lifetime](imported-directory-trace.md) closes the two exact historical marker reproducers with 482 separately gated profiles; real exec and broader activation/profile acceptance remain open. |
| Include metadata phases | [Owned runtime publication and current RHS state](runtime-includes.md) add 155 required controls and 42 staged interruption cells. Metadata eval uses the old search, nested publication selects current flags, and passive late decoding waits for activation. [Startup metadata recursion and diagnostics](startup-metadata-recursion.md) add 55 controls; [Constructor phase controls](constructor-metadata.md) now close all nineteen historical cells with 539 required cases for early visibility, full eval, imported shell, selected recursion, output ownership and cached directories. Broader lifecycle and include/runtime operand work remains open; no safety-exception credit. |
| Include operand history | [Raw versus stored byte history](include-operands.md) adds 941 required cases for multiplicity, barrier lifetime, initial HOME effects, runtime resets, passive options, rereads and counted metadata effects. Membership indexes avoid repeated scans; every retained operand still receives fresh physical metadata. Runtime HOME/tilde decoding and broader operand/profile acceptance remain open. |
| Include-directory interruptions | [Fresh metadata retries](include-stat-interruptions.md) close six finite interruption mismatches with twelve initial separately gated Linux/glibc profiles; [operand history](include-operands.md) expands the gate to twenty-four repeated-path profiles and a qualified observer. Fatal signals prevent further filesystem observations. Broader path, startup and profile acceptance remains open. |
| Scoped assignments and directives | [Target and pattern scopes](pattern-scopes.md) cover eager and deferred RHS evaluation, lexical shell status, and target binding replacement during inherited append. Expanded `undefine` names preserve GNU whitespace and effect order; an empty expanded name stops with the GNU diagnostic. Combined required tests include target/pattern scope and variable whitespace. Eleven [conditional delimiter cases](conditional-whitespace.md) cover raw operands and ordered effects. Broader parser/effect interactions remain open. |
| Special metadata | [Variable metadata](variable-metadata.md) now implements ordered `.VARIABLES`, pinned `.FEATURES`, initial `.LOADED`, default `.RECIPEPREFIX`, `MAKE_HOST`, `.DEFAULT_GOAL` and `MFLAGS`, including lookup lifetime and scope tests. Module-updated metadata and enabled/broader Linux profiles remain tied to their adapters and acceptance gates. |
| POSIX mode | [POSIX mode](posix-mode.md) now implements sticky reader activation, default values, folding, per-command shell flags, suffix eligibility and original command-variable propagation, with 390 required cases across profiles and phases. Broader option/profile and cross-family acceptance remains in the complete audit. |
| Late built-in disabling | [Post-read -r/-R cleanup](late-builtins.md) preserves stronger origins, metadata, export policies, user suffix declarations/recipes and original restart phases, with 204 semantic comparisons and three output-checked scaling cases. Full startup/self-exec and option interactions remain open. |
| Assumed file records | [Old and what-if files](assumed-files.md) add 631 required cases for logical ages, traversal, modes, phony/double-colon queues, initial/late flag phases, recursion, VPATH, remake and physical timestamp witnesses. Broad decoder/usage/resource/profile interactions remain open. |
| Long-option prefixes | [Unambiguous prefixes](long-option-prefixes.md) have 1,068 required cases across flag metadata, literal operand bytes, actual path/eval selection, transport publication and source origins. [Live invalid-option continuation](live-invalid-options.md) closes the recorded ambiguous-reader --w form with 641 required cases; startup ambiguity and full malformed usage remain ordinary open decoder work. |
| CLI and presentation | [Negative silent/keep-going switches](switch-origins.md), their origin precedence and scoped flag metadata now have 468 required cases. The [source option inventory](m3-option-inventory.md) lists the remaining debug/database and broader trace, directory, eval, old/new-file, shuffle, undefined-variable and decoder interactions. Required raw output or narrow semantic validators must establish ordering, identities and nondeterministic fields. |
| Database output | [Borrowed execution history](database-output.md) adds 41 complete profiles and six native rounding/registration profiles, including retained search aliases and actual table/string events. Both combined builds pass these plus native fenv18, collation74 and locale183; 640 Rust/727 tooling checks pass. Default history costs, full selected-head gates and broader native/Guile/NLS/lifecycle/resource cross-products remain acceptance requirements. |
| Structured debug output | [Original requests and borrowed events](debug-output.md) retain 64 exact deterministic profiles, including GNU's omitted queued-child Success records. [Controlled actual-child JOBS](job-debug.md) adds nine strict identity/event/token models. Both frozen builds pass JOBS 9, debug 64, native 215 and directory 40; formatting, Clippy, 578 Rust/643 tooling checks and one separate archive-integrity unit pass. [Measurements](benchmarks/job-debug.md) retain mixed elapsed and unchanged allocation-call/free results. Broader JOBS transports/scopes/recursion/restarts/loaded/resource profiles, asynchronous/implicit/default-discovery/database diagnostics and locale/Linux/native/Guile/integration acceptance remain open. Complete M3/M6 and general parity are not established. |
| Directory reporting | [Ordered selection and output ownership](directory-reporting.md) has 40 original profiles, closing 36 preceding mismatches. Automatic/explicit policy, early unknown-directory metadata, quiet children, locked spool groups, restart transfer and terminal native closure are qualified. Broader database/debug, path/stdio/descriptor/resource and cross-family profiles remain open; the gate does not complete M3/M6. |
| Source files and process restart | [Named stdin sources](stdin-sources.md) pass 44 original profiles and 21 pre-exit signal cells. [Process image replacement](../tests/process-restarts/README.md) adds three cases that fail on the preceding in-process path and pass on both corrected builds. The strict 14 lifecycle profiles include named-file and adopted-stdin cleanup, live environment, failed exec, and advertised pipe identity. [Raw process environment](../tests/restart-environment/README.md) adds three original controls for duplicate/unkeyed entries and counter lifetime. [Inherited pipe restart](../tests/restart-pipe/README.md) qualifies descriptor identity, privacy, and original authentication across real exec. [Native standard-stream restart cases](../tests/native-restart-stdio/README.md) cover public flushes, private buffers and write failures. Wider fault/resource and enabled Guile profiles remain open. |
| Restart namespaces | [Directory replay and recursive identity](restart-directories.md) have 348 required cases. Complete self-exec lifecycle, winning command-variable replay and startup assignment ordering remain open. Empty `-C` usage output is recorded explicitly in research. |
| Reference boundaries | [Simple/computed/fallback names and function diagnostics](profile-inputs.md) now have 120 required cases. Broader reader, option and native registry interactions retain their own acceptance requirements. |
| Undefined-variable warnings | [Warning emission](undefined-warnings.md) has 358 required cases across scopes, phases, automatic variables and option sources. Numeric lexical/call-frame masking is corrected. [CLI assignment phases](cli-assignment-effects.md) now close both recorded default-order probes; full startup, option/profile and locale acceptance remain open. |
| Command-line eval | [Startup buffers](command-line-eval.md) have 194 required semantic cases plus two scaling controls, including anonymous/real-file diagnostics, live additions, metadata, switch phases, recursion and restart replay. Broader startup/decoder/self-exec/profile acceptance remains open. |
| CLI assignment effects | [Database-aware expansion and default phases](cli-assignment-effects.md) add 186 required cases across eager/lazy effects, scopes, includes, diagnostics, shell/default visibility, priority, exports and recursion. [Ordered startup replay](startup-replay.md) now closes the exact repeated CLI file-effect control with 67 required cases; complete startup/self-exec acceptance stays open. |
| Trace output | [Rebuild reasons and command echo](trace.md) have 165 required cases across modes, identities, synchronization, restarts and live remake reactivation. Requested flags remain separate from active remake diagnostics. Broader debug/startup/profile/locale interactions remain open. |
| Debug output | [Implicit traversal](../tests/debug-traversal/README.md) and [verbose remake decisions](../tests/debug-verbose/README.md) add 40 and 23 models, each checked in C and French with both builds. Parallel reconsideration, asynchronous timestamps and broader dynamic debug output remain open. |
| Child environment vectors | [Owned raw vectors](environment-vectors.md) now preserve the selected byte/order representation across recipe and shell launches, with 278 required cases. Broader extension/profile changes must retain the same selection and ownership gates. |
| Existing GNU native loading | [Native loading and callbacks](native-modules.md) implement shared-library setup, GNU ABI registration/allocation/expansion/eval, raw `.LOADED`, nullable source provenance, qualified remaking and process-finalizer behavior. [Startup exit ownership](native-startup-exits.md) retains selected decoder/constructor/restart state and directory/banner ordering. 226 original compiled C integration cases, including signed setup results and loader diagnostics, run in development, optimized and self-host gates. Broader startup/stdio, stdin restarts and callback/native/profile/resource acceptance remain open. [Reader preparation](load-reader.md) retains its 324 ordinary cases. GNU may not be an implementation fallback. |
| Guile profiles | The [optional application frontend](guile-frontend.md) covers 172 enabled comparisons and 14 default signal controls, including valid module conversion, live reverse and join bindings, pair mutation, and callback diagnostics. [Detached serial waits](../tests/guile-detached/README.md) add six enabled cases in both build profiles. [Late serial descriptor pressure](../tests/guile-eventfd-pressure/README.md) adds three enabled cases that keep callback admission open when the wake descriptor cannot be allocated. [Detached parallel waits](../tests/guile-parallel-detached/README.md) add ten enabled cases and [parallel descriptor pressure](../tests/guile-parallel-pressure/README.md) adds six. The independent component gate remains required. Broader asynchronous entry points, conversions, backtraces and the reference-variable fatal wait remain open. |
| Descriptor pressure | The original 29 startup, recipe and signal-mask cases are joined by [output spools](../tests/resource-fd-spool/README.md), [compact pipe](../tests/resource-fd-pipe/README.md), [FIFO](../tests/resource-fd-fifo/README.md) and [ordinary token release](../tests/jobserver-release/README.md) gates. Six [parallel descriptor cases](../tests/resource-fd-parallel/README.md) cover up to ten overlapping recipes while the Linux process has only eight descriptors. [Stdin](../tests/resource-file-size/README.md) and [stdout](../tests/stdout-file-size/README.md) file-size limits have separate exact profiles. The [owned FD4 FIFO case](../tests/resource-fd-fifo/README.md) now checks the exact failure and empty retained path in a protected directory. The [wake-slot](../tests/jobserver-wake-slots/README.md) and [read flag](../tests/jobserver-read-flags/README.md) gates cover private slot isolation and exact owned/borrowed pipe flags. Other directory profiles and parent fatal token loss remain open. |
| Locales and Linux profiles | [Startup locale selection](startup-locale.md) passes 183 separately required glibc C/C.UTF8 profiles across wildcard matching, environment precedence, startup expansion, later assignments/exports and recursion. Broader locales, NLS catalog diagnostics, native/Guile interactions and profile-specific path/build metadata remain open; glibc/x86-64 observations do not establish broader Linux acceptance. |
| Localized diagnostics | [Typed diagnostics and directory output](nls-diagnostics.md) run 161 cases, including include replay and real restarts. [Catalog coverage](../tests/catalog-coverage/README.md) exercises all 30 attributed release languages and three fallback controls. [Startup usage](startup-usage.md) checks the libc option domain with portable and separately authenticated cases. Broader producer and profile interactions remain open. |
| Integration and self-hosting | The [pinned project corpus](project-integration.md) covers xxHash and zlib as serial leaf builds, GNU gzip and GNU Patch as serial recursive builds, and Lua with a two-slot pipe jobserver. Both build profiles require exact GNU2 comparisons across builds, no-ops, header rebuilds and independently checked executable output. The selected self-host flow exercises xxHash. The developer gate checks verified warm oracle bootstrap, six cleanup modes, benchmark error handling, and positive untimed benchmark reports under both outer engines. A separate cold/warm bootstrap audit uses verified inputs in disposable installation prefixes. Broader project, compiler, developer environment and performance coverage remains open. |
| Performance | [Prepared startup retention](startup-retention.md) removes unused copies with repeated allocation observations and mixed paired timing evidence. [Automatic snapshots](automatic-snapshots.md) avoid unused owned preparation with raw glibc-request and paired timing evidence. General performance parity, persistent cache correctness and complete workload/profile acceptance remain open. |
| Unsafe oracle behavior | The owner approved [documented safe errors](safety-exceptions.md), including the explicit normal-exit nested-short extension. The separate gate covers 34 inputs, including two stalled double-colon query tails. All 134 candidate-only cells run twice and earn zero exact-conformance credit. Eighteen nested-short boundaries retain exact GNU behavior. CLI eval/restart controls remain required ordinary fixtures. Broader decoder/resource/profile work stays open. |

The live target declaration gate now includes eager recipe-time `eval` in the
active file scope. It verifies inherited, pattern and automatic variables,
nested publications, and shell status alongside inactive-target controls.
The selected named Makefile operand is also retained across process restart
while startup assignments replay. [Target scopes](target-scopes.md) ·
[Named Makefiles and stdin](stdin-sources.md)

The diagnostic gate has separate models for ordinary and intermediate
`.DEFAULT` selection, grouped peer and owner freshness, serial question mode,
and cached asynchronous prerequisite visits after a child reap. An owned pipe
test verifies the nested wait when the recipe child is already waitable. The
real-project gate also builds pinned GNU Make itself in serial and with a
recursive FIFO jobserver. [Reap regression](../tests/debug-reap-race/README.md) ·
[Real-project corpus](project-integration.md)

Startup jobserver authentication now distinguishes malformed text from closed
descriptors and an unavailable FIFO. Authentication added while reading a
Makefile remains in recursive metadata and preserves an already-adopted pipe.
The 17 original fixtures cover startup, flag phases and actual parallel work.

The implementation order is language and metadata correctness, complete option
and presentation behavior, native extension/profile adapters, then the expanded
integration/profile audit. Continue adding cross-family cases rather than
limiting acceptance to named reproductions. Persistent caching and new rmake
plugin APIs retain their separate correctness and compatibility gates.


The [locale collation slice](locale-collation.md) closes 33 observed preceding mismatches in 74 original full-stream/full-filesystem models, including malformed-byte ties, table growth and live native locale selection. Both profiles retain all 183 earlier C/CUTF8 controls. Wider locale/cache histories, localized diagnostics, resource profiles and general performance remain separate M3/M6 requirements.

## Parallel modes, makefile remakes, and jobserver paths

The held remake gate covers 2,650 profiles on Linux FIFO and pipe jobservers.
It includes 764 remake-debug and 850 controlled load profiles. The gate authenticates 239 FIFO
controller traces. The gate covers direct grouped optional
recipes, stopped optional
workers retained across load limits, named include and primary makefile goals,
and exact remake-debug prefixes under dry-run, question, and touch. The
explicit primary cases include existing normal and order-only prerequisites,
forced shell recipes, and recursive Make recipes. Later named primary sources
exercise two- and three-slot scheduling alongside earlier include workers.
Forced named includes exercise required and optional sources in three read
positions, including an early question exit. Primary prerequisite profiles
cover stock C rules, user pattern rules, and an existing source with no
applicable rule or a prerequisite already running as an earlier include.
Named includes with file prerequisites exercise optional immediate pass boundaries.
Repeated required named includes share one remake record. Plain subdirectory
goals overlap leaf-named required and optional makefile remakes. Named primary
controls cover basic and scoped verbose traces. Additional verbose profiles
cover direct single-line remakes and required includes named as goals under
dry-run, question and touch. Held shuffle cases verify ordinary goal order
separately from remake order. Basic load debug profiles cover child admission
and trace boundaries with and without an active build mode. The load gate
checks normal verbose admission and optional prerequisite recipes that
continue across a blocking reap. Verbose controls cover direct includes and
stopped or keep-going optional remakes under active build modes.
The load gate also exercises regular required includes named as goals under
dry-run, question and touch on both jobserver transports, including basic
and verbose remake debug traces and their held release prefixes. Optional
and directory-qualified named includes cover the same build modes and
controlled load states.
Combined question
and touch controls check optional touch effects and the restart verdict.
Directory-qualified sources, expanded identities, and
combined load, grouped-remake, and debug paths need broader coverage.
[Held remake gate](../tests/remake-parallel/README.md)

The ordinary inventory contains 30,996 case manifests and 230 requirements.
Serial double-colon failure recovery, returned-failure diagnostics, and cached
visits for repeated required makefiles and failed or unchanged optional
declarations have independent expectations for exit status, streams, and file effects.
[Serial remake diagnostics](debug-output.md)

Existing includes rewritten or removed by a successful serial grouped recipe
are reread if their timestamp changes. Controls cover required and optional
includes, query and dry-run, and a group whose recipe is up to date.
GNU Make can also restart after a grouped recipe runs and leaves a read peer
unchanged. That behavior remains open.
An already loaded unruled source retains its pre-remake file observation
when an earlier serial recipe removes it. Explicit empty rule debug traces
remain open.

The development and optimized builds from #374 each matched all 505
held remake profiles, 1,098 affected ordinary cases, and 167 database
observations. The database coverage includes realized secondary prerequisites,
grouped stems, and grouped double-colon branch ownership. The current database
suite also covers the physical timestamp after a grouped double-colon branch
under dry-run, later grouped peers after an unstarted query recipe,
cache timestamps for mixed branches, and completion of repeated grouped owners. [Composed validation](https://github.com/jeffdasilva/rmake/pull/374)
[Grouped double-colon database models](../tools/database_grouped_double_output.py)

The composed optimized build reduced median time from 287.37 to 196.79 ms at
5,000 includes and 5,001 explicit goals, and from 469.57 to 303.99 ms at 7,000
includes and 7,001 goals. GNU Make 4.4.1 took 104.67 and 161.48 ms. The dry-run
comparisons verified stdout, stderr, status, and file paths, contents, and
mtimes. Small controls remained within 0.4 ms of the earlier rmake build.
[Combined benchmark](https://github.com/jeffdasilva/rmake/pull/374)

Skipping explicit goal scans for ineligible intermediate targets cut median
time from 204.68 to 142.89 ms at 7,000 direct goals and from 271.38 to
171.94 ms at 5,000 goals with two existing inputs each. GNU Make 4.4.1
took 99.70 and 82.96 ms. Each dry run verified status, streams, and file
paths, contents, and mtimes. Global `.SECONDARY` and live aliases retain
the explicit-goal-first check.
[Intermediate goal benchmark](https://github.com/jeffdasilva/rmake/pull/384)

Filtering large implicit rule tables by the last literal suffix byte cut
median time from 41.64 to 31.98 ms for 512 patterns and 1,000 lookups and
from 66.31 to 45.35 ms for 1,024 patterns and 1,500 directory lookups. GNU Make
4.4.1 took 22.34 and 35.72 ms. Both workloads verified streams, status, and
file and directory state. The filter preserves declaration order and full
pattern matching, and uses the direct scan for small or dense selections.
[Implicit suffix benchmark](https://github.com/jeffdasilva/rmake/pull/391)

The [shuffle fixtures](shuffle.md) qualify implicit WAIT and targeted
NOTPARALLEL across ordinary, grouped, and double-colon rules. The
[forced recipe fixtures](forced-touch.md) qualify parallel ordinary build
modes and verify resumed double-colon work when a later goal enables
keep-going. Further grouped and live-option combinations remain open.

FIFO and named output mutex publication preserve relative path bytes.
The output gates qualify recursive directory changes and a selection of
temporary-file creation failures. Line and target synchronization also
cover one reader stdout emission before a temporary-file creation failure
and the post-read retry. Further reader-output combinations remain open.
