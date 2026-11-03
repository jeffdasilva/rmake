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
| Unambiguous long-option prefixes | [Valid-prefix resolution](long-option-prefixes.md) adds 1,068 required cases for implemented switches, aliases, attached/separate operands, origins and byte-preserving effects. Exact names win; the complete namespace prevents false unique matches. Ambiguity, invalid usage and broader decoder/profile behavior remain open. |
| `-b`, `-m` | Accepted and ignored; original alias fixtures exist. |
| `-B`, `-e`, `-i`, `-k`, `-L`, `-n`, `-q`, `-r`, `-R`, `-s`, `-S`, `-t` and aliases | Decoding/runtime slices implemented and exercised; full startup ordering and malformed-option matrix remain open. |
| `-h`/`--help`, `-v`/`--version` | Deliberately distinct branding/help. Early-exit interactions and malformed-option usage behavior still need acceptance. |
| `-C`/`--directory`, `-f`/`--file`/`--makefile` | Direct CLI support plus restart namespace slice. Well-formed startup inherited flags are covered by the profile-input slice. Command-variable flag payload startup order, duplicate/expanded paths, usage errors and true self-exec remain open. |
| `-I`/`--include-dir` | Include search and assignment phase slices exist. [Ordered explicit startup](ordered-cli-startup.md) adds argument-position, tilde/effect and duplicate-operand controls. Complete combined ordering/filename acceptance remains open. |
| `-j`/`--jobs`, `-l`/`--load-average`/`--max-load`, `-O`/`--output-sync` | Local scheduling/load/output gates implemented; all unrelated option interactions remain part of M3. |
| `--jobserver-auth`, `--jobserver-fds`, `--jobserver-style`, `--sync-mutex` | Linux transport/authentication slices and gates exist; lifecycle must survive actual self-exec and native integrations. |
| `--no-silent`, `--no-keep-going`, `--stop` | Negative spellings, origins and recursive propagation implemented; full startup inventory remains open. |
| `--no-print-directory` | State/spelling retained; complete directory reporting is still open. |
| `-w`/`--print-directory` | Entry-point support remains open: needs origin-aware policy, deferred entering/leaving and synchronized spool boundaries. [Imported restart-trace closure](imported-directory-trace.md) now covers its recorded final-message slice, including constructor errors and remakes. |
| `-d`, `--debug[=...]` | Unsupported: must implement debug categories, ordering, restart/child identities and output. |
| `-p`/`--print-data-base` | Unsupported: needs semantic database validator with narrow treatment of documented nondeterministic fields. |
| `--trace` | [Trace output](trace.md) has 165 required cases for rebuild reasons, identity, echo, modes, synchronization and remake reactivation. Full debug-category, startup, option/profile and locale interactions remain open. |
| `-E`/`--eval` | [Command-line evaluation](command-line-eval.md) has 194 required semantic cases and two output-checked scaling cases across phases, metadata, diagnostics, effects, priority, recursion and restarts. Broader startup/usage/self-exec/profile acceptance remains open. |
| `-o`/`--old-file`/`--assume-old` | [Invocation-only file assumptions](assumed-files.md) add filename preparation, ordinary rule/traversal suppression, logical ages, modes, source phases, recursion, VPATH and makefile-remake controls. Complete decoder/profile interactions remain open. |
| `-W`/`--what-if`/`--new-file`/`--assume-new` | [Logical newest file records](assumed-files.md) add prerequisite traversal, virtual completion comparisons, phony and ordered double-colon behavior, physical-time witnesses and initial/non-exported flag phases. Complete decoder/profile interactions remain open. |
| `--warn-undefined-variables` | [Reader/runtime warning emission](undefined-warnings.md) now covers scopes, automatic variables, option sources/phases and raw diagnostics. [CLI assignment defaults](cli-assignment-effects.md) close the two recorded startup warning probes; broader inherited expansion and complete startup/option/profile acceptance remain open. |
| `--shuffle[=...]` | [Owned shuffle traversal](shuffle.md) has 1,995 required models for modes/seeds, goals/edges, barriers, scope/phase policy, recursion, restarts and failures, plus a separate random/zero-seed permutation/replay gate. Malformed usage, cycle freshness/trace and broader lifecycle/profile cross-products remain open. |
| `--temp-stdin` | Unsupported internal restart option: stdin currently survives an in-process restart; real self-exec needs owned temporary-source lifetime and exact argument replacement. |

Decoder acceptance must also cover abbreviations/ambiguity, option clusters,
standalone `-`, end markers, optional arguments, repeated directories and
makefiles, source origins, invalid arguments, original raw argument bytes,
MAKEFLAGS/GNUMAKEFLAGS precedence, and all permitted recursive propagation.
Do not turn oracle crashes/hangs into passing fixtures or deliberately reproduce
memory corruption. The owner approved [separately gated safe errors](safety-exceptions.md)
for recorded crash/hang inputs. Finite ordinary mismatches remain compatibility work.

Other M3/M6 surfaces include enabled Guile profiles, broader disabled-profile interactions, native loading
and APIs, locales, Linux/build metadata, startup/reexec semantics, pinned external
project builds and full top-level developer flows. General performance parity
and persistent caching have independent correctness and measurement gates.
