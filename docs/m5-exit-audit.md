# M5 Linux runtime acceptance

Audit date: 2026-10-27. The M5 runtime implementation is complete for the pinned
Linux/glibc, C-locale GNU Make 4.4.1 development profile. Its exit requires the
combined development and optimized gates to pass on the exact proposed commit.
An incomplete or failed run does not satisfy this requirement. This is a feature
tranche acceptance; complete GNU compatibility and general speed parity remain
open.

## Requirement coverage

The ledger maps twelve runtime families to **737 required Makefiles**, with no
pending exemptions. A tooling test enforces complete mapping and rejects pending
runtime, parallel or recursion cases.

| Family | Required cases |
| --- | ---: |
| Parallel execution | 132 |
| Jobserver protocols | 16 |
| Recursive environment | 17 |
| Recursive make level | 38 |
| Fatal continuations | 69 |
| Standard input | 4 |
| Load limits | 29 |
| Signals | 49 |
| Output synchronization | 107 |
| Synthetic completion and peers | 96 |
| Normal capture and cancellation ownership | 12 |
| Runtime flag interpretation | 168 |

The implementation includes graph scheduling, local FIFO/pipe jobservers,
GNU/rmake mixed recursive trees, token return, load admission, per-child stdin,
`.WAIT` and `.NOTPARALLEL`, all four output-sync modes, interoperable POSIX output
mutexes, inherited terminal metadata, cooperative interruption, owned capture and
recipe shutdown, intermediate cleanup, and immediate/post-read/restart flag
phases. The notes on [load](load-limits.md), [signals](runtime-signals.md),
[output](output-sync.md), [cancellation](runtime-cancellation.md) and
[flags](runtime-flags.md) specify observation and ownership boundaries.

The whole curated corpus has **5,426 cases: 5,418 required and eight pending**.
The pending cases cover export ordering, shell quoting/direct-tail/newline
behavior and an upstream `let` signal edge, outside M5. They remain observed
compatibility gaps. No broad normalization or newly pending runtime exemption
is used to obtain acceptance.

## Combined gate

Run **`make m5-acceptance`** with frozen candidate executables when comparing
revisions. It sequentially runs `make ci` and `make test-m5-release`.
The optimized target retains the complete M4 release gate and adds the current
runtime campaigns. GitHub CI runs both phases; merge only the exact green head,
then synchronize main and verify its post-merge checks.

The gate includes 417 Rust and 324 tooling tests, pinned-oracle self-comparison
and independent anchors, all required curated fixtures, generated language and
graph/scope/secondary suites, stateful scenarios, all 2,600 archive fault cells,
mixed jobserver trees, forty repetitions of eighteen parallel programs, and
self-hosted top-level Makefile flows. Both development and optimized candidates
also run the controlled native runtime campaigns:

- 337 ordinary signal profiles and 36 borrowed-pool safety profiles, each with
  three paired repetitions.
- 76 output-sync and 21 load profiles, each with three paired repetitions.
- 294 effect/capture/I/O interruption, 76 output-lock and 64 inherited-PTY
  profiles, each with three paired repetitions.

Campaigns retain raw paired observations and reproducers. Output grouping tests
admit either complete target first only where scheduler order is unspecified;
partial interleaving, missing data, wrong stream/status and target effects fail.
Native descriptor, token, live-child, lock and terminal checks complement the
ordinary filesystem/stream comparator. Matching engines must also satisfy
independent expected anchors.

## Safety and remaining scope

Signal handlers only publish state and wake the interpreter. Owned process
identities, pidfds and close-on-exec descriptors prevent signalling reused PIDs
or leaking private handles. Tokens and mutexes have explicit ownership; borrowed
resources are not unlinked. Deferred effects and output preserve GNU's observed
phase order without unsafe pointers or running the oracle as an implementation.

Documented safety differences remain deliberate: rmake returns borrowed tokens
where GNU's fatal path can lose them; rmake reaps its owned capture child where
GNU can orphan an unregistered shell-function child; rmake leaves no owned stdin
temporary where GNU can leak a random temporary. Campaigns record these
differences explicitly, and still compare target files, streams and status.
The signal notes describe excluded GNU wait-state races. Terminal tests use
inherited PTYs, not interactive controlling-terminal job-control sessions.

M3 still owns remaining dynamic language/export behavior, including assignments
newly introduced inside changed flag payloads. M6 owns remaining CLI ordering,
special-variable metadata, debug/database/locale, native loading and Guile
profiles. M7 owns release-scale real projects, fuzzing and the broader supported
Linux profile matrix. This audit does not establish arbitrary Linux architecture
or libc coverage. Existing resource bounds remain explicit.

M8 owns general performance parity and persistent cache validation. Runtime
switch receivers avoid cloning unchanged options; disabled output buffers create
no spool, and literal-dollar quoting has a common-path fast return. These are
implementation choices, not evidence of general speed superiority. Quiescent
paired benchmarks must compare frozen GNU, candidate and preceding-main binaries;
their raw reports are retained locally. The [runtime measurements](m5-runtime-performance.md)
record the final 101-sample comparison and binary identities. Cache work remains
behind its separate correctness gate.
