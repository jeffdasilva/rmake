# Imported restart-directory trace lifetime

GNU's imported `MAKE_RESTARTS` value can begin with a marker saying that the
process already printed its entering-directory message. That state is distinct
from the current directory-printing option. Even `--no-print-directory` does not
cancel the eventual leaving message. The two finite historical reproducers in
[restart-state research](../tests/research/imported-restart-directory-markers.json)
now match, including counter wrapping and eager assignment replay across remakes.
Their original incomplete models and mismatches remain unchanged as history.

The executable gives marked invocations an outer output owner. It remains alive
through constructor errors and successive interpreters, remembers selected
program/level and logical directory state, and closes the trace exactly once.
Before directory selection, a CLI failure reports an unknown directory with
initial identity; after selection, recipe/reader failures and successful builds
retain the selected identity and directory. Failed `-C` retains the preceding
directory. The owner finishes before signal teardown and releases synchronization
resources. Early exits retain normal local-variable destruction order so buffered
constructor output is emitted before final output.

Only invocations whose imported raw value starts with `-` allocate the shared
process owner. Ordinary invocations retain separate interpreter controllers and
avoid copying directory strings. Whitespace before `-` is not a trace marker.
The public counter still strips exactly one marker; source/CLI assignments do not
invent a new imported marker. Logical rereads reset current output identity,
mode and directory selection while retaining the process's trace lifetime.
No unsafe code, external package or GNU runtime fallback was introduced.

## Original gate and evidence

`make test-imported-trace` runs 482 bounded profiles: six raw marker forms,
ten success/failure/effect/remake contexts, initial levels 0/2, output modes
none/target, explicit/inherited synchronization, plus the two exact historical
reproducers. Contexts include silent success, reader info, visible bindings,
eager CLI info/error, reader/recipe error, successful/failed directory changes
and generated includes. Each profile has an independent exact status/output
model and source/generated file byte, mode and topology checks.

The observer supplies its actual workspace path as a model input before running
either binary. It never rewrites or normalizes observed output. Both streams are
merged to check cross-stream ordering. Source files remain unchanged; only the
modeled include producer may create `generated.mk`. Wrong identities, directories,
missing or duplicate messages, altered modes, lost writes and later extra effects
have negative tooling controls.

Two bounded GNU Make 4.4.1 observations agree with every original model, without
refinements or discarded expectations. Preceding rmake fails 242 and passes 240
controls. Both frozen builds match all 482 without reached execution limits or
leftover children. Models and raw before/development/optimized observations are
retained in [the model file](../tests/research/imported-trace-models.json),
[before evidence](../tests/research/imported-trace-before.json),
[debug evidence](../tests/research/imported-trace-debug.json) and
[release evidence](../tests/research/imported-trace-release.json).

The gate participates in normal development, optimized and focused self-hosted
Makefile flows. Five tooling tests protect inventory, original reproducers,
raw workspace inputs, output mutations and independent filesystem witnesses.
Formatting, Clippy, 514 Rust and 519 tooling tests pass before full frozen
rmake-driven `m5-acceptance` and exact-head development/optimized CI. Those full
acceptance results belong to the PR.

These controlled profiles receive zero additional ordinary or safety-exception
credit. The ordinary corpus remains 16,027 required / zero pending, with 17
unchanged approved safe-error exceptions. This closes the recorded imported-marker
slice; `-w`, implicit directory reporting, line/recurse grouping, the complete
startup/self-exec/temp-stdin lifecycle, remaining CLI/debug/database surfaces,
native loading, enabled Guile, Linux/locales/integrations and complete M3/M6
acceptance remain open. General speed parity and persistent cache correctness
also remain open.

The qualified profile is x86-64 Linux/glibc, C locale, disabled Guile.
Oracle profile: `92525c9784f4dd471cceb12ce66a9d367e267b638e1384cdc29962b738d16de7`.
Oracle SHA256: `b8806e8eb7452ed9b7ed5e18cb7e76ff9abd488c9d62a266a398d8044fade974`.
Verified source archive: `dd16fb1d67bfab79a72f5e8390735c49e3e8e70b4945a15ab1f81ddb78658fb3`.
GNU's manual and source were consulted for externally observable marker and
output phases. All Rust, models and observers are original; no GNU implementation
or tests were copied or translated.

Frozen debug SHA256: `7dcf1609924622385fc4ae46a3e2e7a9a5e4b917bd2808daf0a3b44dcce0ca1a`.
Frozen release SHA256: `858a5d2aa28fdef9dd2034e7af55e85e4a54a192d83a392b2b92c8d39b702b3d`.

## Measurements and rejected prototype

Two sequential batches per version retain 101 alternating samples per engine
and workload, two warmups and exact output agreement. GNU drives A; frozen rmake
drives B through the top-level Makefile. The preceding option implementation was
built in this same worktree. All timed inputs already behave correctly on it;
newly corrected marker cases receive no timing credit. No unrelated local build,
test or probe campaign runs during timing. Host activity is uncontrolled; there
are no confidence intervals, CPU/RSS/I/O measurements or performance thresholds.

### Selected optional owner

| Workload | GNU / candidate / preceding median ms A | GNU / candidate / preceding median ms B | Paired preceding/candidate A / B |
|---|---:|---:|---:|
| tiny-noop | 0.677 / 0.836 / 0.840 | 0.604 / 0.683 / 0.691 | 1.000 / 1.007 |
| large-noop | 10.909 / 29.760 / 29.711 | 10.884 / 30.067 / 30.048 | 0.998 / 1.000 |
| expansion | 2.746 / 4.867 / 4.941 | 2.745 / 4.909 / 4.984 | 1.011 / 1.011 |
| startup-effects-0 | 0.629 / 0.741 / 0.736 | 0.616 / 0.719 / 0.719 | 0.981 / 0.998 |
| startup-effects-1000 | 2.549 / 3.069 / 3.086 | 2.521 / 2.999 / 3.039 | 1.003 / 1.012 |

### Rejected unconditional owner

| Workload | GNU / candidate / preceding median ms A | GNU / candidate / preceding median ms B | Paired preceding/candidate A / B |
|---|---:|---:|---:|
| tiny-noop | 0.679 / 0.831 / 0.825 | 0.505 / 0.581 / 0.576 | 0.992 / 1.000 |
| large-noop | 10.950 / 29.927 / 29.920 | 11.044 / 29.783 / 30.064 | 0.999 / 1.011 |
| expansion | 2.612 / 4.922 / 4.880 | 2.794 / 5.053 / 5.082 | 0.993 / 0.998 |
| startup-effects-0 | 0.548 / 0.644 / 0.632 | 0.620 / 0.724 / 0.732 | 0.981 / 1.005 |
| startup-effects-1000 | 2.417 / 2.912 / 2.928 | 2.410 / 3.014 / 2.909 | 1.002 / 0.984 |

The unconditional prototype passed every new model, but its shared owner
changed the ordinary lifetime path and showed an eager-startup regression in one
batch, alongside mixed results in the other controls.
It is not shipped. Its full [debug](../tests/research/imported-trace-unconditional-debug.json)
and [release](../tests/research/imported-trace-unconditional-release.json) model
observations remain unchanged with zero extra credit. The selected version
allocates shared ownership only for an actual imported trace. Timing remains
mixed; no general speedup or GNU performance parity is claimed.

Raw selected reports: [core A](benchmarks/imported-trace-core-a.json),
[core B](benchmarks/imported-trace-core-b.json),
[startup A](benchmarks/imported-trace-startup-a.json),
[startup B](benchmarks/imported-trace-startup-b.json).
Rejected reports: [core A](benchmarks/imported-trace-unconditional-core-a.json),
[core B](benchmarks/imported-trace-unconditional-core-b.json),
[startup A](benchmarks/imported-trace-unconditional-startup-a.json),
[startup B](benchmarks/imported-trace-unconditional-startup-b.json).
