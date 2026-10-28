# Ordered explicit startup arguments

Explicit CLI definitions and filename operands now execute in the shared early
database as the decoder consumes them. This precedes `-C`, final recursion depth,
late automatic/default metadata and `-E`. `MAKECMDGOALS` updates at each goal,
preserving stronger imported/command bindings and their raw values. Each effect
uses the warning and environment policy active at its argument position.

The parser has a typed argument receiver; the pure decoder uses the same loop
without an effect receiver. Expansion errors retain real included-file locations
and byte messages. Arguments execute once, and prepared state transfers into the
constructor without evaluating definitions a second time. Original invocation and
environment inputs reconstruct this phase iteratively on Makefile restart.

Tilde operands use the full database evaluator for HOME, including nested eval,
includes and shell/file effects. Undefined-variable warnings are temporarily
suppressed for this lookup, while explicit warnings still emit. A filename
operand is compared with previously expanded operands before expansion; two raw
`~/...` operands can therefore both expand. Makefile operands permit duplicates.
Plain `-` and empty non-definition arguments are ignored. No general make-dollar
expansion is applied to filename/goal operands.

Inherited output synchronization is activated before explicit CLI effects;
explicit synchronization changes take effect when decoding ends. The owned reader
buffer transfers into construction. Buffered fatal diagnostics remain with that
buffer through exit so an already-consumed version flag appears at its GNU phase.
Help/version text retains rmake branding. Both flags select version then help,
with exact buffering, effect and status controls around the cosmetic text.

## Evidence and limits

126 original semantic cases and two large ordered-argv controls have independent
status/stream and intended-effect models, checked against two pinned GNU runs and
the candidate. Controls cover metadata, goal visibility, imported raw values,
option positions, tilde operands, duplicate paths, real-file errors, byte values,
merged output and serial/parallel restarts. The earlier 10,360 required corpus
passed during development. The curated inventory becomes 10,488 ordinary required
cases, zero pending, with the same 17 separate safe-error exceptions.

The new `make test-startup-exits` gate has 128 cells. It captures each engine's
isolated help/version banner, then requires the exact declared position and
multiplicity of that known block. All other output bytes, warning/error ordering,
exit status and filesystem effects remain anchored and compared. Two oracle runs
must agree. Negative controls reject missing/moved/extra banners or effects,
changed diagnostics, wrong signals/status, and execution after fatal/early exit.
No regular-expression stripping or broad output normalization is used.

42 new staged interruption cells cover inherited synchronization during CLI shell
capture and shell capture reached through tilde HOME. The runtime inventory is
378 cells; these have zero curated-conformance credit. The candidate must reap
its capture child and preserve exact wait status with no later argument effects.

Initial model disagreements about tilde duplicate operands, HOME warning policy,
repeated Makefile recipe diagnostics and combined/banner fatal-exit ordering were
investigated against the fixed source and retained locally before refinement.
They were not converted into safety exceptions or accepted output differences.

This does not close M3/M6. Reentrant initial flag resets, full malformed/abbreviated
option behavior, directory/debug/database output, old/new-file assumptions,
shuffle, native/Guile interfaces, real process replacement, locales and broader
profile/integration acceptance remain in [the audit](m3-m6-audit.md). In-process
startup reconstruction is not actual self-exec. General performance parity and
persistent caching remain separate work.

GNU Make 4.4.1 `src/main.c` startup/decoder/exit phases and `src/read.c` tilde and
include-path policies were consulted from the verified archive, SHA256
`dd16fb1d67bfab79a72f5e8390735c49e3e8e70b4945a15ab1f81ddb78658fb3`.
GNU source is copyright Free Software Foundation, Inc., GPL-3.0-or-later. Rust
implementation, controls and fixtures are independently authored; no upstream
implementation or tests were copied or translated. No unsafe Rust, semantic cache
or GNU runtime fallback is added.

## Frozen validation and performance observations

Formatting, warnings-denied lint, 457 Rust and 413 tooling tests passed. All 241
combined inherited/ordered controls match two GNU runs and independent models
under both frozen builds. The new 128-cell exit gate passed under both builds;
63 focused CLI interruption cells passed, including the 42 new cells.
Debug SHA256:
`c047c4514c0aa772abed9f7fdf5d8294f7627f3ed241185064a7e847511daf06`.
Release SHA256:
`b3ec2ed13f0fcb844a1a46478dd704fdd0cda099ef6ed660cd072ea3e7761069`.
Full final rmake-driven development/optimized acceptance and exact-head CI remain
merge gates.

Each report contains 101 alternating samples per implementation, exact output,
workload inputs and binary/profile identities. No build or test campaign ran
concurrently with measurement; host activity was not controlled. Results remain
mixed. The 1,000-effect CLI workload median is about 3 percent higher than
preceding rmake, while large no-op and large literal argv medians are slightly
lower. Expansion is slightly higher. GNU remains faster on several common
workloads. These observations establish neither a statistical speedup nor general
performance parity.

| Workload | GNU ms | Current ms | Preceding rmake ms |
| --- | ---: | ---: | ---: |
| tiny-noop | 0.661 | 0.800 | 0.804 |
| large-noop | 11.813 | 31.832 | 32.304 |
| expansion | 2.823 | 5.028 | 4.967 |
| startup-arguments-0 | 0.634 | 0.767 | 0.770 |
| startup-arguments-1000 | 2.169 | 2.026 | 2.057 |
| startup-arguments-5000 | 26.051 | 8.392 | 8.491 |
| parallel-branches-depth-1 | 87.410 | 89.567 | 89.764 |
| parallel-branches-depth-4 | 87.459 | 89.774 | 89.799 |
| eval-arguments-0 | 0.731 | 0.880 | 0.901 |
| eval-arguments-1000 | 3.121 | 1.998 | 2.009 |
| startup-effects-0 | 0.591 | 0.687 | 0.687 |
| startup-effects-1000 | 2.550 | 3.030 | 2.939 |
| initial-flags-0 | 0.585 | 0.731 | 0.705 |
| initial-flags-1000 | 2.504 | 2.905 | 2.923 |

Raw reports: [core](benchmarks/ordered-cli-core.json), [startup](benchmarks/ordered-cli-startup.json), [parallel](benchmarks/ordered-cli-parallel.json), [eval](benchmarks/ordered-cli-eval.json), [effects](benchmarks/ordered-cli-effects.json), [inherited](benchmarks/ordered-cli-inherited.json).
