# GNU 4.4.1 option inventory: source audit, acceptance still open

This is a source inventory, not complete passing conformance evidence. The fixed GNU
`src/main.c` switch table and the current Rust decoder were read directly. An
accepted spelling alone does not establish all interactions or output behavior.

| GNU switch family | Current implementation boundary |
| --- | --- |
| Live invalid option results | [Ignored invalid/ambiguous options](live-invalid-options.md) add 641 required cases across scopes, origins, pure/payload continuations and raw bytes. [Arity and skipped effects](live-option-arity.md) add 770 required cases for forbidden attached values, missing operands, clusters and later definitions. Startup remains strict; registered unimplemented features are not hidden. Broader known-value/arity and complete usage behavior remain open. |
| Live warning policy | [Ordered warning events](live-warning-phases.md) add 1,213 required cases for live argument positions, nested cursor selection, active diagnostics, eager priority and replay. Within-source malformed-value effects and runtime HOME remain open. |
| Passive warning policy | [Post-read argument events](passive-warning-phases.md) add 1,872 required cases for source expansion, warning and `-e` positions, assignment forms, priorities and raw bytes. Source completion is covered below; malformed-value effects and runtime HOME remain open. |
| Passive source sequence | [GNU then ordinary source completion](passive-source-order.md) add 1,290 required cases for effects, clearing, binding locations, priorities, explicit invocation inputs and actual FIFO/pipe capacity. Within-source malformed-value effects, runtime HOME and complete decoder/lifecycle acceptance remain open. |
| Output-sync validation | [Ordered invalid-mode validation](output-validation-order.md) adds 818 required models for completed definitions, within-source replacement, per-source fatal boundaries and recursive export effects. Other malformed values, contextual HOME and full decoder/lifecycle behavior remain open. |
| Runtime HOME include operands | [Owned runtime filename steps](runtime-home-includes.md) add 506 required controls for argument-position HOME effects, normalized history, warning masks, callbacks, parallel modes, ignored invocation operands and fatal behavior. Other malformed values, nested resets, secondary tilde/search and full decoder/lifecycle behavior remain open. |
| Second-stage include search | [Owned search continuation](secondary-include-search.md) adds 474 required models and closes 240 preceding mismatches. Physical tilde resolution follows completed argument definitions without rewriting stored operands; nested resets retain parent cursor context, and empty streams preserve it. Directory/control identity, raw bytes, warnings, fatal effects and invalid output-sync ordering are covered. Broader nested/malformed/lifecycle acceptance remains open. |
| Runtime flag inspection | [One successful parse](runtime-flag-inspection.md) records incoming silent/keep-going/job selections without constructing a second Options tree. 96 original recipe behavior regressions cover source, origin, aliases and output modes. Error retry preserves preliminary failure precedence without interpreter effects. Repeated runtime profiles save 1,009 requests/frees; general timing/parity acceptance remains open. |
| Startup include search | [Pre-default HOME lookup](startup-include-search.md) adds 500 original models and closes 438 preceding mismatches. Constructor evaluation preserves delayed flag activation, ordered database callbacks, warning masks, stored byte operands and search spelling. Raw byte argv is replayable through args_hex. Broader constructor, decoder, profile and lifecycle acceptance remains open. |
| Unambiguous long-option prefixes | [Valid-prefix resolution](long-option-prefixes.md) adds 1,068 required cases for implemented switches, aliases, attached/separate operands, origins and byte-preserving effects. Exact names win; the complete namespace prevents false unique matches. Ambiguity, invalid usage and broader decoder/profile behavior remain open. |
| `-b`, `-m` | Accepted and ignored; original alias fixtures exist. |
| `-B`, `-e`, `-i`, `-k`, `-L`, `-n`, `-q`, `-r`, `-R`, `-s`, `-S`, `-t` and aliases | Decoding/runtime slices implemented and exercised; full startup ordering and malformed-option matrix remain open. |
| `-h`/`--help`, `-v`/`--version` | Deliberately distinct branding/help. Early-exit interactions and malformed-option usage behavior still need acceptance. |
| `-C`/`--directory`, `-f`/`--file`/`--makefile` | Direct CLI support plus restart namespace slice. Well-formed startup inherited flags are covered by the profile-input slice. Command-variable flag payload startup order, duplicate/expanded paths, usage errors and true self-exec remain open. |
| `-I`/`--include-dir` | Include search and assignment phase slices exist. [Ordered explicit startup](ordered-cli-startup.md) adds argument-position, tilde/effect and duplicate-operand controls. Complete combined ordering/filename acceptance remains open. |
| `-j`/`--jobs`, `-l`/`--load-average`/`--max-load`, `-O`/`--output-sync` | Local scheduling/load/output gates implemented; all unrelated option interactions remain part of M3. |
| `--jobserver-auth`, `--jobserver-fds`, `--jobserver-style`, `--sync-mutex` | Linux transport/authentication slices and gates exist; lifecycle must survive actual self-exec and native integrations. |
| `--no-silent`, `--no-keep-going`, `--stop` | Negative spellings, origins and recursive propagation implemented; full startup inventory remains open. |
| `--no-print-directory` | [Ordered directory selection](directory-reporting.md) preserves CLI priority and incoming flag order, including automatic silence and process closure. Broader profile and formatting interactions remain open. |
| `-w`/`--print-directory` | [Directory reporting](directory-reporting.md) has 40 independently modeled profiles for lazy entry, quiet child launches, synchronized spool boundaries, early metadata and restart/native closure. [Imported restart traces](imported-directory-trace.md) retain their separate gate. Database/debug, broader path/stdio/resource and cross-family acceptance remain open. |
| `-d`, `--debug[=...]` | [Structured debug output](debug-output.md) has 64 exact selected profiles for original request/reset/serialization, effective BASIC/VERBOSE/IMPLICIT/MAKEFILE/PRINT/WHY selection, reader/eval/BOM phases, TRACE interaction, controlled restart and ten BASIC-child lifecycle cases. [Controlled actual-child JOBS](job-debug.md) adds nine strict event/identity/token profiles; broader JOBS transports/scopes/recursion/restarts/loaded/resource combinations, asynchronous traversal, full implicit chains/default discovery and database/profile interactions remain open. Automatic debug GNU headers are exact; only one explicit-version error boundary permits anchored branding. |
| `-p`/`--print-data-base` | [Borrowed full database output](database-output.md) has 41 exact phase/search profiles and six native rounding/registration profiles. Anchored libc clocks and independently statted directory identities are the only dynamic slots. Broader lifecycle, locale and profile combinations remain open. |
| `--trace` | [Trace output](trace.md) has 165 required cases for rebuild reasons, identity, echo, modes, synchronization and remake reactivation. PRINT/WHY now share effective debug selection; broader debug-category, startup, option/profile and locale interactions remain open. |
| `-E`/`--eval` | [Command-line evaluation](command-line-eval.md) has 194 required semantic cases and two output-checked scaling cases across phases, metadata, diagnostics, effects, priority, recursion and restarts. Broader startup/usage/self-exec/profile acceptance remains open. |
| `-o`/`--old-file`/`--assume-old` | [Invocation-only file assumptions](assumed-files.md) add filename preparation, ordinary rule/traversal suppression, logical ages, modes, source phases, recursion, VPATH and makefile-remake controls. Complete decoder/profile interactions remain open. |
| `-W`/`--what-if`/`--new-file`/`--assume-new` | [Logical newest file records](assumed-files.md) add prerequisite traversal, virtual completion comparisons, phony and ordered double-colon behavior, physical-time witnesses and initial/non-exported flag phases. Complete decoder/profile interactions remain open. |
| `--warn-undefined-variables` | [Reader/runtime warning emission](undefined-warnings.md) now covers scopes, automatic variables, option sources/phases and raw diagnostics. [CLI assignment defaults](cli-assignment-effects.md) close the two recorded startup warning probes; broader inherited expansion and complete startup/option/profile acceptance remain open. |
| `--shuffle[=...]` | [Owned shuffle traversal](shuffle.md) has 1,995 required models for modes/seeds, goals/edges, barriers, scope/phase policy, recursion, restarts and failures, plus a separate random/zero-seed permutation/replay gate. Malformed usage, cycle freshness/trace and broader lifecycle/profile cross-products remain open. |
| `--temp-stdin` | Internal restart operand with owned named-source lifetime and exact argument rewriting in [44 stdin profiles](stdin-sources.md). General internal-option and resource interactions remain open. |

Decoder acceptance must also cover abbreviations/ambiguity, option clusters,
standalone `-`, end markers, optional arguments, repeated directories and
makefiles, source origins, invalid arguments, original raw argument bytes,
MAKEFLAGS/GNUMAKEFLAGS precedence, and all permitted recursive propagation.
Do not turn oracle crashes/hangs into passing fixtures or deliberately reproduce
memory corruption. The owner approved [separately gated safe errors](safety-exceptions.md)
for recorded crash/hang inputs and the explicitly approved fifteen-input normal-exit
nested-short class. Other finite ordinary mismatches remain compatibility work.

Other M3/M6 surfaces include enabled Guile profiles, broader disabled-profile interactions, native loading
and APIs, locales, Linux/build metadata, startup/reexec semantics, pinned external
project builds and full top-level developer flows. General performance parity
and persistent caching have independent correctness and measurement gates.


The earlier 54-profile debug qualification and its preceding-main one-pass/
53-fail control remain historical. The C1 slice passed all 64 debug
profiles; preceding debug passed 62 and failed two. Its full local/CI acceptance
is retained. The current isolated JOBS builds pass all nine selected JOBS models,
64 debug, 215 native and 40 directory profiles. Formatting, Clippy, 578 Rust and
643 tooling tests pass, plus one separate archive-integrity unit; the next full
tooling gate has 644 tests. [JOBS measurements](benchmarks/job-debug.md) preserve
mixed elapsed results and unchanged allocation-call/free counts. The ordinary ledger stays 27,766
required / zero pending across 158 families; 32 approved exceptions / 128 cells
remain separate. These counts do not complete the option inventory or M3/M6.
Enabled Guile/GC, locale/Linux/resource profiles, integration/developer flows,
persistent caching and general GNU parity retain their acceptance work.


The BASIC-child proof preserves GNU's omitted queued-child Success records;
serial stop-mode failure omits Failed while serial keep-going retains it. The
queued causal worker protocol leaves only static final observations, and
does not itself qualify JOBS PID/address output; the separate nine-case
[JOBS gate](job-debug.md) checks strict identity relationships. The [child measurements](benchmarks/debug-children.md)
are a separate comparison; earlier regressions remain retained and no general
speedup or GNU parity follows from these selected controls.


[Wildcard locale collation](locale-collation.md) adds 74 separately required models for selected C/CUTF8/French and live native choices, with private pinned data generation. It does not close localized diagnostics, the remaining CLI inventory, broader locale/resource profiles or M3/M6.

The [inherited option terminator gate](../tests/flag-terminator/README.md) checks
literal dash-prefixed goals and definitions, required option operands, distinct
source boundaries, live and passive flags, raw bytes, recursion and actual
process restart. The owned decoder retains `--` until it can tell whether an
option consumes it or whether it terminates that source.
