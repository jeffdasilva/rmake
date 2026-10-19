# Differential testing strategy

Status: implementation underway. Curated, generated, mode-matrix and stateful
runners, early benchmarks and GitHub CI are implemented. The
[progress record](progress.md) lists current coverage and open gates; the broader
profile, isolation, stress and release-scale requirements below remain the design
target. Current runners execute trusted fixtures in disposable directories with
bounded processes and output; they are not a filesystem or network sandbox.
The primary corpus consists of independently authored Makefiles and helpers.
GNU source may inform implementation and case design under the
[source-reference policy](compatibility.md#source-reference-and-provenance).
Any upstream-derived supplemental cases require their own provenance and notices.

The [compatibility contract](compatibility.md) defines the reference and permitted
differences. Testing must detect semantic mistakes, establish which feature
combinations are covered, and make any future optimization earn its correctness.

## Fixture format and oracle execution

Each case is a directory containing an original `Makefile`, optional included
files and input data, and a declarative `case.toml`. The manifest records:

- Stable case ID, requirement IDs, feature tags, rationale, documentation/source
  references or original experiment provenance, and required oracle profile.
- Initial files and byte contents, modes, links, directory structure and exact
  nanosecond modification times where relevant.
- One or more invocation steps: argv, cwd, environment, stdin/TTY mode, umask,
  filesystem/environment mutations between runs, and bounded resource limits.
- Output-descriptor topology: separate streams, a shared file or pipe, a PTY,
  closed descriptors, and broken pipes where the case requires them.
- Comparison mode, required observable events and precisely scoped nondeterminism.
- Reproducible seed and shrinking constraints for a generated case.

The runner creates separate isolated environments with identical absolute paths,
for example `/work/case` and `/opt/test/bin/make`. It mounts the appropriate
executable at the latter path; never use different workspace prefixes and then
blindly rewrite captured text. Pin shell/tool versions, `PATH`, `HOME`, `TMPDIR`,
locale, timezone and relevant host properties. No network access or host-home
access is needed for ordinary fixtures; capabilities required by a test are
explicit. Tests cannot escape their disposable roots or leave background children.

Reference and candidate executions each start from the same state. Stateful tests
maintain independent copies across the same mutation sequence. Bound time, output
and child counts; timeout is a separately reported observation, not an ordinary
success/error code. Save enough state to reproduce every discrepancy.

The fixture runner also tests real invocation names and mixed GNU/rmake recursive
trees. A recipe helper records which executable recursion actually reached. A
candidate that accidentally delegates to GNU must fail the test.

## Observations and comparators

| Observation | Comparison |
| --- | --- |
| stdout/stderr | Exact bytes in deterministic cases, including whitespace, warnings and source locations; capture separate streams or shared-descriptor output according to the fixture |
| termination | Exit code or terminating signal, with timeout classified separately |
| filesystem | New/changed/deleted entries, bytes, permissions, symlinks and hard-link relationships, timestamp values or specified relationships |
| command context | Recipe-observed arguments, environment, cwd, umask, stdin and inherited-descriptor behavior |
| scheduling | Start/finish/failure event traces, dependency order, invocation counts, slot bounds and cleanup |
| protocols | Jobserver token ownership/return, recursion state, output synchronization and signal propagation |

Internal syscall counts need not match: reducing overhead is a goal. Shell and
process behavior that a recipe can observe still belongs in conformance.
Preserve raw captures alongside comparator reports. Do not let the candidate's
own internal model produce expected results for black-box comparisons.

For parallel cases, use helper-controlled barriers and acknowledgements instead
of sleep-based timing. Assert exact results under controlled schedules, and
specified partial-order constraints for legitimately unconstrained schedules.
Check each output-sync mode separately; globally sorting output would conceal
interleaving defects. GNU describes the modes in
[parallel output](https://www.gnu.org/software/make/manual/html_node/Parallel-Output.html).

Time/PID/descriptor fields receive narrowly named validators with relationship
checks, never broad regular-expression scrubbing. For debug/database output,
control the clock when practical and validate only intrinsically variable fields
separately; keep deterministic ordering and statistics as compatibility work.
Every comparator rule gets a test proving that an actual semantic mismatch still
fails. TTY-specific cases use a PTY, not the ordinary pipe-capture path.

Include shared-descriptor invocations such as `make >log 2>&1`: two separately
captured streams cannot establish their merged ordering. Compare controlled
merged output exactly and exercise output synchronization, closed descriptors,
broken pipes, and the resulting signal/error behavior under each topology.

## Requirement ledger and corpus

Create a stable requirement ID for every option, directive, assignment form,
built-in function, special variable/target, expansion rule, diagnostic class,
execution protocol and relevant documented ordering guarantee. Map each ID to
positive, negative, boundary and interaction cases. A test count cannot substitute
for this inventory.

| Family | Required original cases |
| --- | --- |
| Source selection | `GNUmakefile`/`makefile`/`Makefile`, repeated `-f`, stdin, include paths, `MAKEFILES`, absent/default files |
| Lexical boundaries | Continuations, comments, escaping, CRLF, missing final newline, whitespace, computed separators, non-UTF-8 bytes, `.RECIPEPREFIX` changes |
| Variables | All assignment forms and modifiers, empty versus undefined, computed names, recursion, `define`, `undefine`, precedence and append semantics |
| Scope and exports | Environment/CLI/global/target/pattern scopes, `private`, `override`, inheritance, first-updater context, export/unexport and recursive export effects |
| Functions | Every built-in, nested/missing/extra arguments, whitespace boundaries, lazy arguments, `call`, `eval`, `foreach`, `let`, integer and string edge cases |
| Expansion effects | `shell`, `!=`, `.SHELLSTATUS`, file reads/writes, info/warning/error, repeated evaluations and lazy branches |
| Automatic variables | Ordering and duplicates, directory/file variants, archive context, multiple rule types, missing values and secondary expansion |
| Explicit rules | Multi-target independent/grouped rules, double-colon, static patterns, default goals, missing rules, cycles, duplicate/order-only prerequisites |
| Implicit search | Pattern specificity/order, chains, suffix transformations, cancellation, match-anything rules, failed candidates, built-in rules and variable overrides |
| Filesystem | Missing/equal/older/newer files, timestamp resolution/skew, directories, symlinks, hard links, permissions, search paths, wildcard order and negative matches |
| Archives | Members, automatic variables, member timestamps, implicit rules, missing archives and controlled parallel archive updates |
| Special targets | Every 4.4.1 special target: phony/intermediate/secondary/precious/delete behavior, low-resolution times, POSIX, one-shell, wait and serialization |
| Remaking | Generated/missing includes, optional includes, multiple restarts, parallel remakes, restart suppression, `MAKEFILE_LIST`, `MAKE_RESTARTS`, loaded-object remakes |
| Recipes | `@`, `-`, `+`, shell selection/flags, per-line shells, `.ONESHELL`, escaped newlines, empty recipes, direct exec boundaries, command-not-found and ignored errors |
| CLI | Every short/long option, abbreviations where accepted, repetition/conflicts, operands/assignments, `-n/-q/-t`, `-B/-k/-r/-R/-o/-W`, shuffle, invalid inputs |
| Recursion | `MAKE`, `MAKELEVEL`, `MAKEFLAGS`, `MFLAGS`, `GNUMAKEFLAGS`, `MAKEOVERRIDES`, direct/indirect recursive detection and mixed-engine trees |
| Parallelism | Fixed/unlimited jobs, load limit, `.WAIT`, `.NOTPARALLEL`, shared prerequisites, grouped updates, readiness order, output sync and stdin ownership |
| Jobserver | FIFO/pipe, token bytes, implicit slots, invalid/missing descriptors, incompatible auth, interrupts, child failure and cooperating tools |
| Diagnostics | Syntax failures, missing prerequisites, recursion, cycles, errors/warnings, trace/debug/database output, locations, exit codes and locale profiles |
| Extensions | `load`/`-load`, `.LOADED`, independently written ABI probes, callback expansion/evaluation, ownership, errors and enabled/disabled Guile profiles |
| Host failures | Signals, terminal modes, exhausted descriptors, disk full, EACCES, spawn errors, long paths, temporary directory errors and child cleanup |
| Caching | Off/cold/warm/stale/corrupt/concurrent cases, every external observation and every effectful expansion boundary |

Prioritize 4.4.1-specific cases: immediate `MAKEFLAGS` visibility, command-line
assignments present during parsing, private-target export inheritance, and custom
parallel archive rules. Write new fixtures from the behavior described in the
[4.4.1 announcement](https://lists.gnu.org/archive/html/info-gnu/2023-02/msg00011.html).

Proposed release-scale targets are at least 2,500 reviewed named cases, 100,000
bounded generated cases per nightly run, and 1,000,000 seeds across a release soak.
Measure runtime in milestone 0 and shard/rebudget transparently. These are coverage
goals, not evidence of compatibility by themselves. Pairwise generation broadens
interactions; deliberate higher-order cases cover secondary expansion + target
scope + implicit search, restarts + exports + shell effects, and recursion +
jobserver + failure.

## Generation, reduction and mutation

Maintain three project-owned generators:

1. Grammar generation for valid/invalid syntax, delimiters, escaping and bounded
   recursive expansion; differential comparison, not only crash detection.
2. Semantic generation for bounded graphs, rule conflicts, variable scopes,
   search paths and timestamp states, with useful independent invariants.
3. Stateful generation for repeated builds, file/environment edits, include
   creation, interruptions and cache transitions.

Save the seed, complete fixture, reference digest, environment and observations
on failure. Minimize files, definitions, graph edges and build steps while
preserving the discrepancy; promote the reduced result to a reviewed permanent
fixture. Non-reproducible failures stay visible with captured evidence.

Use metamorphic tests only with explicit preconditions. Renaming ordinary targets
can preserve behavior, but default-goal selection, implicit patterns or special
names can invalidate that assumption. Likewise, adding an unreachable rule is not
automatically harmless. Differential behavior remains the primary oracle.

Mutation-test the harness by altering one output byte, status, file, permission,
or event. Mutation-test the implementation by reversing precedence, skipping a
secondary expansion, dropping an order-only edge, leaking a token, or omitting an
invalidation dependency. Surviving mutants expose gaps that coverage percentages
alone cannot find. Fuzz parsers, evaluators, cache decoders and ABI boundaries with
resource bounds; run applicable unsafe-code and sanitizer checks.

## Cache state matrix

For each applicable stateful sequence, compare the oracle against rmake with:

1. Persistent caching disabled.
2. An empty cache.
3. A valid warm cache.
4. A cache warmed before input mutation.
5. Corrupt, truncated, incompatible-version or concurrently written entries.

Mutations include content changes with preserved mtime and size; new/deleted
includes; wildcard misses becoming hits; changed environment or CLI assignments;
shell/PATH changes; symlink retargeting; directory renames; permission changes;
generated Makefiles; new rule candidates; extension changes; and mutations during
validation. Compare sequences, not just isolated invocations.

Count and order expansion effects. A warm cache that suppresses a shell command,
file write, warning, or restart is wrong even when final target bytes match.
Exercise interrupted writers, concurrent readers, cache eviction, resource limits
and full-disk/read-only fallback. Test fixture and cache roots are disjoint, with
the implementation cache namespace explicitly identified in reports.

## Integration and continuous gates

Use original synthetic projects and pinned real-project builds covering generated
dependencies, recursive Makefiles, C/C++ builds, generated configuration, archives
and large parallel graphs. Review the licenses and provenance of external corpus
material; any upstream-derived tests form a separately attributed supplementary
suite. Pin toolchains and inputs, disable network downloads, and compare both
clean and incremental builds; distinguish nondeterministic compiler output from
make errors.

| Gate | Required execution |
| --- | --- |
| Feature change | New original fixtures, unit tests, requirement mapping, differential tests and minimized bug regressions |
| Pull request | Full curated suite, fixed generated seeds, relevant cache/stateful cases, harness checks and self-host smoke; grow from initial seed counts |
| Nightly | Broad generators, interaction matrices, signal/jobserver stress, resource failures, mutation checks, integration corpus and repeated schedules |
| Release | Complete in-scope ledger, zero unresolved in-scope differential failures, no outstanding required-feature skips, supported-profile matrix, reproducible soak and performance report |

The current `make test-modes` gate executes 512 fixed original combinations of
ordinary/one-shell recipes, eight command shapes, and all subsets of `-n`, `-q`,
`-t`, `-i`, and `-s`. It performs oracle self-comparison before candidate
comparison, asserts independent simple anchors and preserved input files, and
retains exact failure reproductions. `MODE_CASE` replays a recorded cell. The
matrix has no pending classification. It complements the curated corpus and the
separate stateful timestamp gate; it does not cover every option interaction.

`make test-scopes` extends the seeded DAG generator with explicit target variable
declarations. Its default 100 graphs combine assignment flavors, inherited
appends, private/override/export/unexport modifiers and shared prerequisites.
Each node records expansion, raw value, origin, flavor and the actual child
environment. Comparisons retain exact output, status and filesystem checks plus
independent graph anchors; they do not predict GNU's scope resolution internally.
`GRAPH_SEED`, `GRAPH_CASES`, `GRAPH_NODES` and `START_INDEX` control replay, and
failure metadata retains the `--scopes` flag. `make test-pattern-scopes` adds
ordered matching pattern declarations to the same graphs and runs a separate
100-program CI campaign; `--patterns` is retained for exact replay. Effectful
functions remain outside these generators' current scope.

`make test-filesystem` adds eleven original Linux/glibc fault scenarios. A
temporary C interposer injects interrupted opens/reads/path observations, partial
reads, write errors and errors after actual close. The runner compares exact
streams, status, filesystem snapshots and injection traces with authored
expectations, then compares both engines. `FILESYSTEM_CASE` selects a case for
replay; reports retain the compiler, helper source and binary identities.
Curated cases separately cover normal file/realpath behavior, byte filenames,
symlinks, NUL expansion consumers, newline treatment and effect timing.
See the [filesystem effect notes](filesystem-effects.md) for safety boundaries.

Six additional interposer cases perform deterministic external mutations during
a read without advancing make's command generation. Their authored observations
check stale wildcard results, fresh literal existence tests, cached missing
directories, inode aliases, path replacement and subsequent write invalidation.
The injection trace and complete filesystem are anchored for both engines. An
additional interrupted literal lookup verifies GNU's one-shot lstat boundary.
A separate archive-open interruption verifies member-pattern fallback without
silently retrying the failed open.

`make test-wildcards` adds 100 seeded query programs by default. A separate Python
model predicts ASCII glob matches, dot rules, per-pattern sorting and duplicates
across files and directories. A write phase verifies cache invalidation, and
both engines must match exact stdout/status and complete filesystem expectations.
Reports retain seed, program index, expression count, input digest and executable
identities for replay. Curated Makefiles supply escapes, raw bytes, tilde,
archive formats, errors and recipe timing outside the generated model's scope.

`make test-whitespace` checks pure-function tokenization, lazy arguments and
local binding scopes across C whitespace and non-whitespace control bytes.
Every case has authored stream/status/filesystem anchors, including selected
and skipped effects; `WHITESPACE_CASE` supports individual replay. The
[function whitespace notes](function-whitespace.md) describe the distinct
preservation rules and the explicitly excluded upstream `let` crash family.

`make test-word-index` checks `word` and `wordlist` numeric arguments against
independently authored byte/status/filesystem expectations. Cases cover signed
64-bit boundaries, signs, whitespace, invalid bytes, validation precedence and
effects before failures, including missing arguments. `WORD_INDEX_CASE` replays
one case. The [word index notes](word-indices.md) describe the numeric grammar
and its distinction from arbitrary-length `intcmp` values.

The current `make test-intcmp` gate executes 6,400 integer expressions across
200 seeded programs. An independent Python integer model predicts normalization,
branch selection and GNU 4.4.1's observed negative-number ordering quirk. The
oracle must satisfy those predictions before candidate comparison. Original
curated errors complement the generated valid-input campaign, including raw-byte
diagnostics asserted through `stdout_hex`/`stderr_hex`. Reports preserve the
source, seed, index, process bounds and exact observations for replay.

The current `make test-diagnostics` gate checks 206 independently authored
`info`/`warning` scenarios, including raw bytes, argument evaluation, caller
locations, modes, exports, output before fatal errors and merged-stream order.
Every cell has its own expected streams, status and complete filesystem snapshot;
agreement between two wrong engines cannot pass these expectations. Reports and
failure inputs support `DIAGNOSTIC_CASE` replay. The matrix has no pending
classification; the curated corpus separately retains observed export-order and
make-flag evaluation gaps that are outside its supported scope.

Known missing features may be tracked during development, but reports must show
their count and requirement IDs. CI must fail new differences and unexpected
passes. Release conformance cannot pass with those missing features waived away.
Tests that need unavailable host capabilities are reported as not run; a required
release job must provide those capabilities.

Self-hosting is a milestone gate: both GNU Make 4.4.1 and rmake must drive the same
top-level developer flows, including recursive invocations. A passing build of
rmake itself is necessary but far from sufficient evidence of full compatibility.
