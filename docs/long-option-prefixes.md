# Unambiguous GNU long-option prefixes

GNU accepts an unambiguous prefix of a long option. rmake now resolves those
prefixes for its implemented options, including aliases and required/optional
operands. Exact spellings retain their existing dispatch. The fallback considers
the complete pinned GNU long-option namespace, including features not implemented
yet, so `--what-if` still makes `--w` ambiguous with undefined-variable warnings.
Exact `--jobs` wins over the longer `--jobserver-*` names.

Resolution owns the expanded option word, preserves its complete byte suffix
from the first `=`, and retries the same argument before any operand receiver or
assignment runs. The outer scan remains iterative; repeated prefixes do not add
native recursion. Separate operand consumption, argument positions, initial
filename preparation, definition effects and canonical recursive metadata use
the existing decoder. `--` still ends option interpretation. New callback tests
check exactly-once delivery and non-UTF-8 operand bytes. No global getopt state,
unsafe code or extra dependency is introduced.

## Original models and evidence

1,068 new required models comprise 552 boolean-prefix cases across direct CLI,
initial MAKEFLAGS and live reader origins, and 516 valued-option cases across
CLI, MAKEFLAGS and GNUMAKEFLAGS. They include canonical flag pairs, query status,
file/makefile/directory/include selection, eval, job/load/output modes, pipe
jobserver selection and old/new-file aliases. Valued cases retain attached and
applicable separate forms, literal `=` filename/value bytes, empty goal lists,
selected-file writes and real recipe-phase authentication publication.

Two bounded pinned GNU Make 4.4.1 observations agree with every final independent
model. Both frozen builds match status, exact stream bytes and full filesystem
bytes/types/modes/link relationships, with no reached limits or leftover children.
Preceding rmake fails 921 and passes 147 exact-spelling controls.
[Raw evidence](../tests/research/long-option-prefixes.json) retains inputs, modes,
arguments, environments, models, identities and every observation.

162 rejected original models remain unchanged with zero credit. Three `--w`
models omitted the what-if ambiguity and are excluded. 69 warning models left
GNUMAKEFLAGS undefined; separately named refinements explicitly bind it empty.
60 old-file models omitted GNU's normal up-to-date message. 30 jobserver models
expected active authentication while the makefile was still being read;
separate recipe-phase sources observe actual publication and write a witness.
No observed output was normalized or rewritten to make a candidate pass.

Two Rust tests protect raw operands, receiver count, positions, exact-name
priority, ambiguity, end markers and help/version selection. Five tooling tests
protect the inventory and frozen observations, refinements, actual selected
files, literal equality bytes, metadata and jobserver publication phase.
Formatting, lint, 516 Rust and 524 tooling tests pass before full frozen
rmake-driven `m5-acceptance` and exact-head development/optimized CI; results
belong to the PR. Existing Makefile conformance flows discover all new fixtures.

The ordinary corpus becomes 17,095 required / zero pending, with 17 unchanged,
separately gated approved safe-error exceptions. This is a valid-prefix slice,
not complete M3/M6. In particular GNU silently ignores the recorded ambiguous
reader `--w` while startup reports usage errors; that finite ordinary decoder
gap is retained without passing or safety-exception credit. The later [live invalid-option tranche](live-invalid-options.md) closes that exact reader form with 641 required cases while retaining the original evidence. Complete ambiguous/
invalid argument diagnostics, exact usage output, remaining option/profile/
lifecycle interactions, directory/debug/database output, native/Guile profiles,
locales and integration acceptance remain open in the [audit](m3-m6-audit.md).
General performance parity and persistent caching retain their separate gates.

The qualified profile is x86-64 Linux/glibc, C locale and disabled Guile.
Oracle profile: `92525c9784f4dd471cceb12ce66a9d367e267b638e1384cdc29962b738d16de7`.
Oracle SHA256: `b8806e8eb7452ed9b7ed5e18cb7e76ff9abd488c9d62a266a398d8044fade974`.
Verified source archive: `dd16fb1d67bfab79a72f5e8390735c49e3e8e70b4945a15ab1f81ddb78658fb3`.
GNU option names and interface descriptions were consulted as reference. All Rust,
Makefiles and tooling are original; no GNU implementation/tests were copied or
translated, and GNU is never a runtime fallback.

Frozen debug SHA256: `79b8f20b9dbb963124cf155cc1d77014ba78e41233d9fc7dd83bf54c41095e2f`.
Frozen release SHA256: `0229a4c57f66e408b636d63f7a21c183d713108b68278da7d8f68b7204af8a92`.

## Measurements

Four raw reports retain two sequential batches of 101 alternating samples per
engine/workload, two warmups and exact output checks. GNU drives A and frozen
rmake drives B through the top-level Makefile. Preceding rmake was built in this
same worktree; all timed inputs already match on it. Newly accepted prefix cases
earn no timing credit. No unrelated local build, test or probe campaign runs
during timing. Host activity is uncontrolled; there are no confidence intervals,
CPU/RSS/I/O measurements or performance acceptance thresholds.

| Workload | GNU / candidate / preceding median ms A | GNU / candidate / preceding median ms B | Paired preceding/candidate A / B |
|---|---:|---:|---:|
| tiny-noop | 0.629 / 0.757 / 0.754 | 0.612 / 0.730 / 0.706 | 1.008 / 0.986 |
| large-noop | 10.814 / 29.614 / 28.914 | 11.037 / 30.146 / 29.448 | 0.977 / 0.977 |
| expansion | 2.684 / 4.818 / 4.880 | 2.795 / 4.977 / 4.957 | 1.008 / 0.999 |
| startup-arguments-0 | 0.543 / 0.627 / 0.635 | 0.644 / 0.764 / 0.744 | 1.016 / 0.976 |
| startup-arguments-1000 | 2.173 / 1.972 / 1.961 | 2.198 / 2.031 / 2.000 | 0.985 / 0.986 |
| startup-arguments-5000 | 25.354 / 7.537 / 7.552 | 25.502 / 7.867 / 7.725 | 0.993 / 0.985 |

Ratios above one favor the candidate. These measurements are mixed. The large
no-op control is about 2.3% slower in both batches; argument controls also retain
small slowdowns. No general speedup or GNU parity is
claimed. Exact options do not scan the prefix namespace or copy their words;
future acceleration still needs measurements and all compatibility gates.
Raw reports: [core A](benchmarks/long-prefixes-core-a.json),
[core B](benchmarks/long-prefixes-core-b.json),
[arguments A](benchmarks/long-prefixes-arguments-a.json),
[arguments B](benchmarks/long-prefixes-arguments-b.json).
