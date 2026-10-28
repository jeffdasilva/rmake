# GNU 4.4.1 option inventory: source audit, acceptance still open

This is a source inventory, not complete passing conformance evidence. The fixed GNU
`src/main.c` switch table and the current Rust decoder were read directly. An
accepted spelling alone does not establish all interactions or output behavior.

| GNU switch family | Current implementation boundary |
| --- | --- |
| `-b`, `-m` | Accepted and ignored; original alias fixtures exist. |
| `-B`, `-e`, `-i`, `-k`, `-L`, `-n`, `-q`, `-r`, `-R`, `-s`, `-S`, `-t` and aliases | Decoding/runtime slices implemented and exercised; full startup ordering and malformed-option matrix remain open. |
| `-h`/`--help`, `-v`/`--version` | Deliberately distinct branding/help. Early-exit interactions and malformed-option usage behavior still need acceptance. |
| `-C`/`--directory`, `-f`/`--file`/`--makefile` | Direct CLI support plus restart namespace slice. Well-formed startup inherited flags are covered by the profile-input slice. Command-variable flag payload startup order, duplicate/expanded paths, usage errors and true self-exec remain open. |
| `-I`/`--include-dir` | Include search and assignment phase slices exist. Full combined ordering/filename matrix remains open. |
| `-j`/`--jobs`, `-l`/`--load-average`/`--max-load`, `-O`/`--output-sync` | Local scheduling/load/output gates implemented; all unrelated option interactions remain part of M3. |
| `--jobserver-auth`, `--jobserver-fds`, `--jobserver-style`, `--sync-mutex` | Linux transport/authentication slices and gates exist; lifecycle must survive actual self-exec and native integrations. |
| `--no-silent`, `--no-keep-going`, `--stop` | Negative spellings, origins and recursive propagation implemented; full startup inventory remains open. |
| `--no-print-directory` | State/spelling retained; complete directory reporting is still open. |
| `-w`/`--print-directory` | Unsupported: needs origin-aware policy, deferred entering/leaving, synchronized spool boundaries, constructor errors and restart behavior. |
| `-d`, `--debug[=...]` | Unsupported: must implement debug categories, ordering, restart/child identities and output. |
| `-p`/`--print-data-base` | Unsupported: needs semantic database validator with narrow treatment of documented nondeterministic fields. |
| `--trace` | Unsupported: needs exact trigger, recipe/dependency/location and timing observations. |
| `-E`/`--eval` | Unsupported: startup evaluation before ordinary files, ordering and recursive propagation need original cases. Ordinary `$(eval ...)` is separate and implemented. |
| `-o`/`--old-file`/`--assume-old` | Unsupported: filename handling and old-file update semantics need implementation. |
| `-W`/`--what-if`/`--new-file`/`--assume-new` | Unsupported: logical timestamp/update semantics need implementation. |
| `--warn-undefined-variables` | Unsupported: definition/evaluation scopes, function arguments, automatic variables, timing and locations need implementation. |
| `--shuffle[=...]` | Unsupported: GNU seed/mode, goal/edge permutation, barriers, recursion and reproducibility need implementation. |
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
