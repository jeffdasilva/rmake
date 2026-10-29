# rmake implementation plan

Roadmap established 2026-10-18. Implementation is underway; the dated
[progress record](progress.md) distinguishes implemented slices from open gates.
See the [contract](compatibility.md), [architecture](architecture.md), and
[testing strategy](testing.md) for the detailed requirements.

Deliver vertical slices with original fixtures before implementation. Baseline
performance throughout, establish compatibility before persistent graph caching,
and keep extension work from changing the default GNU behavior.

The [M4 exit audit](m4-exit-audit.md) records the implemented rule families,
closed blockers, strict combined acceptance gate and remaining profile limits.

Current position (2026-10-27): M4's rule-resolution and M5's Linux runtime
implementations are complete on the pinned Linux development profile, with
mandatory development and optimized acceptance in GitHub CI. Acceptance requires
green checks on the exact proposed commit. The [M5 exit audit](m5-exit-audit.md)
records its coverage and limits. M3 retains language/effect gaps; complete GNU
compatibility, broad runtime/profile acceptance and general speed parity remain
future milestones.

The next active work is the remaining M3 language gaps and M6 compatibility
surfaces, tracked in the [gap audit](m3-m6-audit.md). [Flag payloads](flag-payloads.md)
now evaluate embedded assignments with origin, ordering and effect coverage.
[Variable metadata](variable-metadata.md) adds ordered global membership and
lookup lifetime, default bindings and legacy flag updates on the pinned profile.
[Export traversal](export-order.md) closes the two historical
export-order pending cases, with collision/growth, scope and live-eval evidence.
[Shell argv and residual execution](shell-argv-replay.md) closes five more
curated gaps. The former upstream-signal pending case is now a separately gated
[approved safe-error exception](safety-exceptions.md); the broader M3/M6 inventory
is still incomplete. [Fatal continuations](fatal-continuations.md) close the original
post-child configuration failure; [immutable source-name sharing](shared-source-names.md)
is a measured allocation reduction. [Load limits and stdin](load-limits.md) have
controlled development and optimized gates. [Cooperative interruption and synthetic completion](runtime-cancellation.md)
now have mandatory effect, lock, terminal and peer regressions. [Recursive flags](runtime-flags.md)
include immediate assignment effects, post-read replay and restart restoration.
[Output synchronization](output-sync.md) has controlled grouping, mutex and mixed-tree gates.
[Owned children and signal shutdown](runtime-signals.md) now have controlled
recipe/runtime gates, including safe borrowed-token recovery. Persistent caching remains deferred to its
correctness design and acceptance gate.

[Late built-in disabling](late-builtins.md) now covers the post-read `-r`/`-R`
transition, origin/suffix preservation and restart-phase controls. Its cleanup
visits fixed tables; full startup, self-exec and option inventory remain open.

[Unused automatic snapshots](automatic-snapshots.md) are now avoided on empty
and absent recipe paths, with raw allocation and timing evidence.

[Command-line evaluation](command-line-eval.md) adds startup buffers, recursive
transport and source/metadata/switch phase checks. Broader startup remains open.

[Trace output](trace.md) adds rebuild reasons and command echo across runtime
modes and Makefile remake phases. Full debug/profile acceptance remains open.

[Undefined-variable warning emission](undefined-warnings.md) adds the live
reader/runtime policy, diagnostic scopes and numeric call-frame correction.
Startup default ordering remains an ordinary compatibility gap; this does not
complete M3/M6.

M4 includes explicit/static/implicit/suffix/grouped/double-colon rules; built-ins;
secondary candidate phases; VPATH/GPATH and library resolution; archive formats,
timestamps and errors; intermediate/special-target policy; hidden prerequisites;
file-record phases/order; filename queries and safe recursive HOME byte effects.
The ledger has 32 rule/archive/resolution families and 2,599 required cases, with
no pending exemptions in those families. The full corpus and cross-family,
stateful, fault, parallel and self-hosted suites also gate the milestone.

Earlier progress records describe the slices and measurements at their original
points in time; the current [acceptance audit](m4-exit-audit.md) supersedes their
historical M4-open statements. The overall requirement inventory remains
incomplete, because the complete compatibility contract extends beyond M4.

## Milestones and exit criteria

| Milestone | Deliverables | Exit gate |
| --- | --- | --- |
| M0: reference and test foundation | Verified 4.4.1 oracle/profile manifests; documentation/source provenance; isolated runner design; requirement ledger; initial original fixtures; benchmark inputs; pinned Rust/Cargo bootstrap; GPL package metadata; top-level developer commands | The runner compares the oracle against itself, detects injected mismatches, reproduces timestamp/parallel tests, rejects the host's 4.3 as an oracle, and records complete reproducers |
| M1: reader and expansion core | Byte-preserving sources, incremental reader, spans, assignments, scopes, conditional reading, core expressions/functions and effect interfaces | Positive/negative/boundary tests for implemented forms; expansion timing and parse-vs-recipe contexts agree; no premature graph-only representation |
| M2: serial vertical slice | CLI bootstrap, explicit rules, default goals, timestamps, automatic variables, recipe shells, basic diagnostics, errors and cleanup | Useful original projects build identically under `-j1`; simple top-level `build`, `test-unit` and `check-core` flows run under the candidate |
| M3: dynamic semantics | Complete variables/functions; target/pattern scope; secondary expansion; `eval`; includes, makefile rebuilding/restarts; export behavior and recursive options | Scope/effect/restart interaction suites pass, including 4.4.1 regression cases; no evaluation side effects suppressed |
| M4: complete rule resolution | Implicit/static/suffix/grouped/double-colon rules; ordered candidate indexes; built-ins; VPATH/GPATH; archives; intermediate/special-target behavior | Every rule family has ledger coverage; search priority, automatic variables, lifecycle and filesystem edge cases match |
| M5: Linux parallel runtime | Parallel scheduler, jobserver FIFO/pipe, recursion, output-sync, stdin, load limits, `.WAIT`, `.NOTPARALLEL`, signals and cancellation | Controlled and stress suites pass, including GNU/rmake mixed trees, token recovery, failures and clean child shutdown |
| M6: complete compatibility surfaces | Remaining CLI/diagnostics/debug/database/locale behavior; GNU native loading; Guile profiles; real-project corpus; all developer flows self-host | Required profiles have no known unimplemented language/execution surfaces; curated and integration suites pass without broad output exemptions |
| M7: compatibility candidate | Coverage audit, higher-order cases, fuzzing/shrinking, stateful stress, full supported Linux profile matrix and release-scale corpus | Zero unresolved in-scope differential failures; no required skips; reproducible release soak; published limitations and measurement baseline |
| M8: safe acceleration | Profile-led allocation/search/process improvements; persistent source/expression cache; conservative graph-fragment cache only after its proof/validation design passes | All prior gates pass with cache off/cold/warm/invalidated/corrupt/concurrent; measured net gains including validation costs; cache-default decision made explicitly |
| M9: extension platform | Versioned opt-in rmake plugin protocol, capability model, namespacing, lifecycle/error policy, authoring examples and extension tests | Compatibility mode unchanged and fully tested; plugins have explicit cache/effect policy; public ABI/version promises documented |

M1-M6 are feature tranches, not permission to postpone testing until M7. Split
each into small independently reviewable changes: original fixture and behavior
note, failing candidate observation, implementation, regression comparison, and
relevant benchmark. Diagnostics and error paths travel with their feature rather
than waiting entirely for M6. Existing-GNU extension probes can be prepared early
without implementing the optional adapters ahead of the core.

M0-M7 form the critical compatibility path. Corpus authoring, oracle probes and
benchmark design can run in parallel with implementation. Runtime work can
proceed behind agreed interfaces, but interpreter semantics remain a single
coherent model. M8 may use benchmark findings gathered earlier; correctness gates
must precede each optimization. M9 is intentionally after compatibility, while
internal module boundaries are present from M1.

## First implementation batch

The initial implementation batch starts with M0, with this bounded output:

1. Pin the toolchain and create the small Cargo workspace, lockfile, and minimal
   entry points needed by the harness. Do not begin broad GNU semantics yet.
2. Acquire and verify the reference binary and any source used as reference;
   record build features, runtime/profile digests and source provenance. A pinned
   reproducible source build is acceptable. Keep test execution isolated and do
   not substitute an unverified `make` found on `PATH`. Record any adapted material
   and its applicable notices under the [licensing policy](licensing.md).
3. Add the requirement-ledger format and about 100 original fixtures spanning
   expansion timing, assignment precedence, simple builds, failures, includes,
   recursive invocation, and intentionally controlled parallel cases.
4. Implement same-path isolated execution, exact stream/status/filesystem capture,
   oracle-vs-oracle checks, harness mutation tests, and mismatch artifacts.
5. Add top-level build/check/test/oracle commands, CI jobs that invoke them, and
   startup/no-op/large-graph benchmark definitions. Record an oracle baseline.

Most candidate semantic cases will initially be marked unimplemented. Report that
honestly while keeping the harness's own correctness suite green. The first useful
implementation milestone is the serial slice in M2, not a false conformance badge
on a placeholder executable.

## Top-level Makefile interface

The following table defines the planned full interface. `make help` lists the
currently implemented subset. Do not add successful no-op targets that pretend
to build or test the project.

| Target | Flow |
| --- | --- |
| `help` | Discover commands, inputs and prerequisites |
| `doctor` | Check tools/profile prerequisites without modifying the host |
| `bootstrap` | Explicit tool/oracle provisioning with pinned identities |
| `build`, `release` | Debug/release compilation |
| `fmt`, `fmt-check`, `lint` | Formatting and static analysis |
| `check-core` | Formatting check, lint and fast internal tests; safe self-host leaf |
| `test`, `test-unit` | Standard suite and focused internal tests |
| `oracle-verify` | Verify exact oracle version, digest and features |
| `test-diff` | Curated black-box conformance cases |
| `test-generated` | Seeded generated cases with reproducible budgets |
| `test-cache` | Cold/warm/invalidation/corruption/concurrency matrix |
| `test-jobserver`, `test-signals` | Recursive protocol and process-lifecycle cases |
| `test-integration` | Pinned external/original project builds |
| `test-selfhost` | Run selected top-level leaf flows under GNU and rmake |
| `fuzz`, `stress` | Bounded campaigns and reproducible stress |
| `bench`, `bench-compare` | Candidate measurement and paired oracle comparison |
| `check`, `ci` | Required local and CI aggregations |
| `coverage`, `compat-report` | Internal coverage and requirement-ledger results |
| `package`, `install`, `uninstall` | Later release artifacts and explicit installation operations |
| `clean` | Remove only known local generated outputs |

Keep the bootstrap Makefile deliberately simple and GNU-compatible. Use `$(MAKE)`
for recursive invocations, with normal recursive-job semantics, never a hard-coded
`make` subprocess that changes engines. Share target recipes between CI and local
work; CI provides environment/matrix configuration instead of duplicating flows.
Use explicit prerequisites or separate output roots so `-j` does not race tools
that modify the same directory. Paths and tool overrides must be quoted correctly.

Planned runner controls include absolute `ORACLE` and `CANDIDATE` paths, `CASE`,
`SEED`, and `JOBS`. Forward only intended controls into fixtures; ordinary harness
settings must not leak into their Makefile variable/environment space. Benchmark
results record any environment differences explicitly.

Self-host tests invoke a finite set of leaf targets such as `build`, `check-core`
and conformance smoke in separate output directories. They must not invoke
`test-selfhost` or `ci` recursively. Bootstrap starts with installed GNU make;
once built, the candidate must drive the same file. Test both direct invocations
and mixed recursive trees. Do not assume a `gmake` alias selects version 4.4.1;
the oracle manifest controls that identity.

## Performance gates

Performance targets are engineering objectives, not claims about unbuilt software.
Proposed primary objectives after compatibility are at least 2x lower engine time
for representative large warm no-op builds and material reductions in expansion,
implicit search and memory overhead. Cold startup should stay competitive with
GNU; do not pay cache initialization costs that dominate tiny invocations.

Use fixed machines/profiles, paired repeated trials, confidence intervals, fixed
corpora and documented warm/cold OS-cache states. Separate recipe time from engine
time. Measure wall/CPU time, median/tail latency, peak memory and I/O, not just a
single favorable geometric mean. Do not require identical binaries from builds
whose compiler embeds timestamps without controlling that compiler input first.

Calibrate noise in M0. A repeatable slowdown beyond a proposed 5% threshold on a
core workload requires investigation and explicit acceptance; use absolute-time
floors for tiny runs. A good average cannot hide a major regression in cold start,
incremental builds or memory. Freeze the benchmark set before selecting an
optimization and publish new adversarial workloads when weaknesses are found.

Every cache optimization must beat uncached rmake after validation overhead and
preserve the entire conformance matrix. A speculative fast path that cannot
validate cheaply should fall back. Never substitute command-result or artifact
caching for GNU's timestamp/execution semantics to win a benchmark.

## Release decision and unresolved engineering choices

Before a compatibility release, require:

- A completed supported-profile ledger and zero unresolved in-scope differential
  failures. Acknowledging or explaining a mismatch does not waive it; only the
  contract's explicit branding differences and narrowly validated variable inputs
  are permitted exceptions.
- All required deterministic, parallel, signal, filesystem, extension and cache
  cases executed on capable runners; no stale expected-failure exemptions.
- Independent fixture/provenance review, reproducible oracle acquisition, license
  and notice review for original/adapted material, and matching Corresponding
  Source for distributed binaries under the [licensing policy](licensing.md).
- Self-hosted developer flows under GNU 4.4.1 and rmake, mixed recursion, and
  benchmark reports that include regressions and resource usage.

M0 resolves the exact oracle distribution/configuration, supported kernel floor,
initial architecture/libc matrix, sandbox backend and practical test budgets.
Those are measurable engineering choices; none authorizes dropping language or
local execution features. Later decisions cover cache enablement and public plugin
transport. Keep them in dated decision records once evidence exists, rather than
choosing a framework or serialization format prematurely.

Owned child environment vectors and export activation phases now have [278 required original comparisons](environment-vectors.md). This closes that specific representation gap, with full M3/M6 exit criteria unchanged.

[POSIX mode](posix-mode.md) now has 390 required original comparisons across reader/default, shell, suffix, recursion and startup-origin phases. Complete M3/M6 criteria remain unchanged.

[Switch origin and runtime metadata behavior](switch-origins.md) adds 468 required cases across CLI, import, file/eval, recursive and current-file boundaries. Complete M3/M6 exit criteria remain unchanged.

[Restart namespace acceptance](restart-directories.md) adds 348 required cases for directory reentry, recursive invocation metadata and raw path diagnostics. Full startup ordering, self-exec lifecycle and CLI/profile criteria remain open.

[Startup/profile and reference-boundary acceptance](profile-inputs.md) adds 792 required cases. The [option inventory](m3-option-inventory.md) makes remaining CLI surfaces explicit; enabled Guile, native loading, startup/self-exec, locales and integration gates remain open.

[Single-pass explicit startup arguments](startup-arguments.md) removes redundant decoding only when no inherited tokens need merging. Exact-value fixtures and the top-level benchmark suite retain correctness gates; general speed parity remains open.

[CLI assignment effects and default phases](cli-assignment-effects.md) add 186 required original comparisons and close the recorded early CC/default-goal warning controls and inactive-metadata eval control. The ordinary repeated CLI file-effect mismatch remains explicit restart work; full M3/M6 criteria are unchanged.

[Ordered startup replay](startup-replay.md) adds 67 required original controls and two output-checked scaling fixtures. It closes the exact repeated CLI file-effect control while retaining explicit process-replacement and broader startup/profile work. M3/M6 criteria are unchanged.

[Initial inherited flag expansion](initial-flag-expansion.md) adds 111 semantic and two output-checked scaling cases, plus 42 staged runtime interruption cells. Source ordering, owned transfer and iterative restart preparation are implemented without changing M3/M6 exit criteria.

[Ordered explicit CLI startup](ordered-cli-startup.md) adds typed decoder effects before directory/metadata initialization, with 126 semantic and two scaling controls. A 128-cell exit-phase gate and 42 staged interruption cells retain exact behavior around cosmetic help/version blocks. M3/M6 criteria remain unchanged.

[Imported restart state](restart-state.md) separates the GNU process counter from in-process rereads, retains binding export policy, and transfers original startup inputs without copying them a second time. It adds 214 counter/policy and eight activation controls. Directory-marker research and full M3/M6 criteria remain open.

[Prepared-startup retention](startup-retention.md) removes two unused copies, with repeated allocation and mixed paired timing evidence. Nineteen original include-metadata research cells retain 13 finite ordinary gaps in default visibility, effect context, publication order and passive late options. Fix these against the full database evaluator and owned search publication before declaring startup complete. M3/M6 criteria remain unchanged.

[Runtime include metadata](runtime-includes.md) now uses the full database evaluator and owned publication after effects, with final flags selected from current nested state. Passive late includes remain distinct from special resets; immediate RHS effects retain current binding value and priority. 155 required original controls and 42 staged interruption cells cover these phases. Twelve finite startup-metadata gaps remain ordinary work, with complete M3/M6 criteria unchanged.
