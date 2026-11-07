# Invocation-only old and what-if files

The executable implements `-o`, `--old-file`, `--assume-old`, `-W`,
`--what-if`, `--new-file` and `--assume-new`. Attached and separate short
operands and exact long spellings retain their filename bytes and ordinary
initial path preparation. Operands do not become goals or recursive flags.

An old-file declaration bypasses that ordinary file's rules, secondary
expansion and prerequisite traversal. It behaves as an existing, logically
old file even when absent on disk. A what-if declaration supplies a logical
newest timestamp: prerequisites are still visited and errors still matter,
but a later physical update cannot overtake the assumed timestamp. A phony
file's initial missing timestamp can also be overridden. Combined old/new
operands suppress its own rules and make dependents see the newest age,
regardless of operand order. Forced builds retain GNU's `$?` behavior.

These are invocation inputs. Initial inherited MAKEFLAGS/GNUMAKEFLAGS values
participate, whereas additions while reading, command-line `--eval`, ordinary
command-variable MAKEFLAGS bindings and recipe-time flag changes do not add
assumed files. Neither file switch is exported automatically to a recursive
make. A restart reconstructs the original invocation. It applies old-file
assumptions during makefile remaking and publishes what-if records for normal
goals afterward. The initial invocation, including an imported restart counter
of zero, applies both during remaking. Late publication updates a completed
file's logical timestamp and retains its cached update status. A cached
double-colon chain still visits its newly published input record in the
ordinary goal phase. With keep-going, a same-pass visit can see the declared
head's success and the tail's logical time. A later pass observes the tail
failure.

The restart cases cover normal goals, non-executing modes, imported counters
and ordered double-colon visits. A synthetic double-colon entry can select a
pattern rule or fall back to `.DEFAULT`. The selected recipe has its own
automatic variables and prerequisites. Its inferred children enter the
ordinary graph, and the declared rules retain their individual state. The
models cover a fresh invocation, publication after a completed remake, and
the first ordinary visit after publication. GNU compares each raw `-W`
operand to the already prepared spellings before resolving it. Thus
`-W cached -W ./cached` adds two synthetic branches, while the reverse
order adds one. Each branch performs its own implicit search and uses the
same initial comparison time. A stopping query with one ordinary goal can
search multiple synthetic branches. The first out-of-date result stops that
chain. Stopping queries during makefile remakes or with multiple requested
goals still use at most one synthetic branch.
GNU can hang when an earlier goal has returned an out-of-date query result
and a later double-colon chain has additional unvisited inputs.

The optional owned table exists only when at least one operand is present.
Its logical old/new values use checked SystemTime values outside the pinned
GNU filesystem timestamp range. No assumed value is written to the filesystem.
Normal completed updates release the new assumption; hypothetical ordinary
updates do not outrank a newest parent. Phony hypothetical completion retains
its separate missing-file semantics. Physical timestamp witnesses are required
in addition to the normal byte/type/mode filesystem comparison.

An eligible VPATH record supplies its logical age during directory search.
When an explicit source target finds a physical file whose qualified record
is only an input, the search observes its physical age. GPATH retains the
newer age when it merges that observation into the qualified record. An
old-file assumption can still suppress the merged target's recipe while
dependents use the observed physical age. Nineteen original fixtures cover
these distinctions, including selective search, query, dry-run, symlink times
and an aliased archive member.

Makefile remaking uses the active invocation assumptions. Existing double-colon
branches keep their physical times and declared rules: `-o` does not suppress
them. A what-if double-colon operand adds a trailing input-only record. Its
missing-rule error occurs in queue order, after preceding updates; a non-keep
query can stop before reaching it. This behavior is an ordinary finite GNU
compatibility requirement, with no safe-error exception credit.

## Qualification and limits

631 independently modeled required cases cover 405 ordinary graph/mode cells,
123 phony/delayed/double-colon/physical-time/recursive/missing-rule cells,
25 ordered double-colon and status-message controls, 23 activation/cleanup/
secondary-expansion cells, and 55 initial-phase, VPATH/GPATH, remake and repeated
operand controls. Modes include normal, query, dry-run, touch, parallel,
output synchronization, always-build and keep-going query. Inputs create explicit
historical nanosecond timestamps; the delayed control crosses one wall-clock
second. They do not infer correctness from status alone.

Each final model agrees with two bounded pinned GNU Make 4.4.1 observations.
Preceding rmake has 560 mismatches and 71 controls across this family.
Both frozen development and optimized executables match output bytes,
status, file bytes/types/modes and link relationships without leftover children
or a reached execution limit. [Raw evidence](../tests/research/assumed-files.json)
retains all inputs, arguments, environments, identities, models and observations.
41 rejected original models remain unchanged with zero credit; independently
named refinements correct their documented timestamp, phony, double-colon,
phase, VPATH and output-escape assumptions.

Five new Rust tests protect the absent-table path, byte identities, independent
old/new properties, logical timestamp range, real-update release, double-colon
records and recursive/live flag ownership. Tooling tests protect qualification,
physical witnesses, phase distinctions and discarded-model history. Standard
Makefile conformance flows discover all new cases; the frozen rmake-driven
`m5-acceptance` and exact-head development/optimized CI must pass before merge.

The ordinary corpus becomes 16,027 required / zero pending. The same 17 approved
safe-error exceptions remain separately gated. This is an implemented option
slice, not complete M3/M6 acceptance. Abbreviations and malformed usage, complete
option/resource/profile interactions, native loading, enabled Guile, remaining
CLI/debug/database/directory output, real self-exec/stdin lifecycle, locales,
integrations and general performance parity remain open in the
[M3/M6 audit](m3-m6-audit.md).

The qualified profile is Linux x86-64/glibc, C locale and disabled Guile.
Oracle profile: `92525c9784f4dd471cceb12ce66a9d367e267b638e1384cdc29962b738d16de7`.
Oracle binary SHA256: `b8806e8eb7452ed9b7ed5e18cb7e76ff9abd488c9d62a266a398d8044fade974`.
Verified source archive SHA256: `dd16fb1d67bfab79a72f5e8390735c49e3e8e70b4945a15ab1f81ddb78658fb3`.
GNU's manual and timestamp domain declarations were consulted as references.
Rust, Makefiles and tooling are original; no GNU implementation or tests were
copied or translated, and GNU is never an execution fallback.

Frozen debug SHA256: `c3983be56a41cc8e0d9c2374a5c0e051fb4c7ee708e1cf09f8428d4b2ea76c52`.
Frozen release SHA256: `8d5f31806ce4a4b59e39b16aef179d27341ffc59423109dc35b1887fd4033540`.

## Measurements

Four retained reports contain two sequential batches of 101 alternating samples
per engine/workload, with two warmups and exact output checks. Pinned GNU drives
A and frozen rmake drives B through the top-level Makefile. No unrelated local
build, test or probe campaign runs during these measurements. Baseline is the
preceding published runtime rebuilt in this same worktree; all measured inputs
already behave correctly on it. Newly accepted option cases earn no timing
credit. Host activity is uncontrolled; there are no confidence intervals,
CPU/RSS/I/O measurements or performance thresholds.

| Workload | GNU / candidate / preceding median ms A | GNU / candidate / preceding median ms B | Paired preceding/candidate A / B |
|---|---:|---:|---:|
| tiny-noop | 0.633 / 0.763 / 0.753 | 0.622 / 0.707 / 0.696 | 0.987 / 0.993 |
| large-noop | 11.059 / 30.218 / 29.806 | 11.043 / 30.051 / 29.645 | 0.987 / 0.987 |
| expansion | 2.741 / 4.921 / 4.943 | 2.865 / 5.101 / 5.109 | 1.000 / 0.998 |
| function-bodies-0 | 0.850 / 1.337 / 1.324 | 0.958 / 1.419 / 1.417 | 0.993 / 0.998 |
| function-bodies-16 | 3.622 / 7.138 / 7.152 | 3.839 / 7.490 / 7.467 | 0.995 / 0.993 |
| function-bodies-48 | 9.887 / 22.126 / 22.307 | 10.315 / 23.313 / 23.494 | 1.008 / 1.010 |

Ratios above one favor the candidate. These controls show mixed small changes,
with no general speedup or GNU performance parity claim. The option table is
absent for ordinary invocations; compatibility work still changes execution and
comparison paths. Broader workload and allocation/cache acceptance remains open.
Raw reports: [core A](benchmarks/assumed-files-core-a.json),
[core B](benchmarks/assumed-files-core-b.json),
[functions A](benchmarks/assumed-files-functions-a.json),
[functions B](benchmarks/assumed-files-functions-b.json).
