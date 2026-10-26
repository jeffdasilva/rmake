# Hidden prerequisites and recording phases

`.EXTRA_PREREQS` contributes normal or order-only edges to update traversal,
missing-input errors and freshness. Each edge retains its hidden role through
implicit filename selection, VPATH/library merging, grouped peer handling and
secondary expansion. Hidden edges stay out of `$<`, `$^`, `$+`, `$?` and `$|`,
including role collisions with a visible order-only occurrence. Visibility does
not decide whether a normal edge makes a target stale.

The initial snapshot expands the raw global binding once, then visits live file
slots in GNU order. Explicit local bindings expand their raw values in global
variable context, including simple values containing surviving dollar references.
Any local variable set suppresses the global extra list, even when it contains
only an unrelated binding. A pattern-specific setting alone does not participate
in this snapshot. An occurrence of the current filename cancels the entire extra
list; expansion effects and entered filename records remain observable.

Only target records inherit global extras. Selected implicit/built-in recipes
promote their records to targets, including commands that leave no output file.
VPATH merges retain target identity and GNU head-only double-colon behavior. A filename carrying local bindings or
other metadata remains distinct from a declared target, so adding hidden inputs
does not accidentally turn a missing file into a successful meta-target. Extras
attach to the head of a double-colon chain. After late suffix installation and
conversion, a second global expansion contributes hidden inputs to implicit
rules. This occurs before VPATH expansion. Restarting Makefiles repeats both
phases. Neither phase is memoized: info, file, shell, eval, errors and filesystem
queries retain their ordered effects. New rule declarations remain prohibited
once the dependency snapshot begins.

Special-target flags are fixed before extras are attached. A hidden dependency
on `.SILENT`, `.IGNORE` or another special record does not change that target's
original flag interpretation. Global secondary policy captures the filename
records visited during the extra snapshot, including newly entered hidden inputs,
while preserving the existing deferred/late record boundary.

The runtime stores no cached graph or timestamp across invocations. The ordinary
no-extra path avoids the file-table scan. Rule storage no longer registers inputs
again: reading, deferred expansion and accepted implicit selection already enter
them at the correct phase. This removes redundant hashing without skipping an
expansion, filename observation or filesystem effect.

The slice adds 145 original required fixtures and one sixteen-invocation stateful
scenario. Coverage includes six modes, local/global/simple/recursive/append and
unrelated bindings, self/duplicate/normal-order collisions, effect/error phases,
unused and Makefile records, restarts, raw bytes, tilde/globs/escaped names,
archives, grouped/double-colon/implicit/suffix rules, built-in compilation through
an original checked helper, default recipes, recursion, `.WAIT`, library/VPATH
lookup and intermediate policies. The incremental scenario fixes nanosecond
inputs and checks hidden freshness, exclusion from automatic variables, local
suppression, missing order-only inputs, dry runs and parallel rebuilds. No output
normalization or new pending exemption is introduced.

Consulted verified GNU Make 4.4.1 `src/file.c` (`expand_extra_prereqs`, `snap_file`,
`snap_deps`), `src/rule.c` (`snap_implicit_rules`), `src/commands.c`
(`set_file_variables`), `src/implicit.c` (edge-role propagation), `src/remake.c`
(metadata records and remake failures) and `src/main.c` (phase ordering). Rust
code, helpers, Makefiles and anchors were authored independently; no upstream
implementation or tests are included. GNU remains an external test oracle.

M4 remains open for the archive observation/recording audit, recursive HOME scratch
behavior and combined acceptance checks. These fixtures do not prove unrestricted
GNU compatibility, runtime-profile completeness or general performance parity.

## Validation and controls

Final local core checks passed 388 Rust and 285 tooling tests. The final debug
candidate passed all 4,590 required curated cases (eleven historical mismatches
remain pending), all 66 stateful scenarios / 303 invocations, and both oracle
executions of the 145 new cases. Full `make ci` also passed during development,
before the two final metadata-merge regressions were added; the final complete
curated and stateful suites were rerun after those fixes. GitHub CI runs the
combined development and self-host gates on the pushed commit.

Final executable SHA-256 identities:

- Debug: `68955fb6b56559189ebc6de556c04a4b93e6d763d5f00780c45bf637879cb994`.
- Release: `fd9808ea88150178c8a1aa7c41df47cdb0c86ce648baae64b62b341ddeef2b19`.

Paired controls used 101 samples per engine, two warmups, alternating engine
order, warm OS cache and exact status/stream checks. No validation ran concurrently.
Median milliseconds:

| Workload | GNU | This release | Previous file-record release |
| --- | ---: | ---: | ---: |
| startup | 0.645 | 0.739 | 0.733 |
| 10,000 target no-op | 11.876 | 35.136 | 35.302 |
| expansion | 2.872 | 4.789 | 4.812 |
| 512 local libraries | 1.315 | 2.048 | 1.981 |
| 512 remote libraries | 2.006 | 2.937 | 2.901 |
| 256 ranked libraries | 1.700 | 2.252 | 2.213 |

These small differences do not establish an improvement or general GNU parity.
GNU remains faster in these controls. There is no CPU isolation, CPU/RSS/I/O
profiling or performance threshold gate. Ignored raw reports:
`artifacts/bench/extra-prereqs-final-core-101.json` and
`artifacts/bench/extra-prereqs-final-libraries-101.json`.
