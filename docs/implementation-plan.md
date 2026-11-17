# rmake implementation plan

rmake targets GNU Make 4.4.1 on Linux. The
[compatibility contract](compatibility.md) defines the required behavior and
approved exceptions. The
[architecture](architecture.md) describes the code and proposed designs.

**M4 rule resolution and M5 Linux runtime are complete on the pinned Linux
development profile.** Their acceptance requires development and optimized gates
on the exact proposed commit. The [M4 audit](m4-exit-audit.md) and
[M5 audit](m5-exit-audit.md) record that qualification. M3 dynamic semantics and
M6 complete compatibility surfaces remain open. The [current gap audit](m3-m6-audit.md)
tracks that work by area, and the [progress record](progress.md) retains the dated
implementation history.

A passing fixture inventory demonstrates the behavior it covers. Completing a
milestone also requires its broader exit criteria and profile coverage. General
performance parity, persistent caching and the public extension platform remain
future work.

## Milestones and exit criteria

The milestones define the dependency order. Work can overlap when its interfaces
and tests are ready. Local performance improvements can accompany any correct
feature. Persistent graph caching follows compatibility and an independently
validated invalidation design. Public extension interfaces follow the default
compatibility implementation.

| Milestone | Work | Exit criteria |
| --- | --- | --- |
| M0 — Foundation | Pin the Rust toolchain, reference builds and provenance. Establish the requirement ledger, execution isolation, original fixtures, benchmarks and top-level workflows. | Verify the reference profile and reject an incorrect reference. Pass oracle self-comparison, injected-mismatch checks, timestamp and parallel observations, and complete failure reproduction. |
| M1 — Reader and expansion | Preserve bytes and source locations through incremental reading, assignments, scopes, conditionals, functions and effects. | Cover positive, negative and boundary cases. Match evaluation phases while retaining dynamic interpretation where later state can affect behavior. |
| M2 — Serial execution | Implement CLI entry, default goals, explicit rules, timestamp decisions, automatic variables, shell launch, diagnostics and failure cleanup. | Match original project builds with one job. Run the top-level build, unit-test and core-check flows under rmake. |
| M3 — Dynamic semantics | Complete variable and function behavior, target and pattern scopes, secondary expansion, eval, includes, remakes, restarts, exports and recursive options. | Pass combined scope, effect and restart tests, including 4.4.1 behavior, with every required effect evaluated in its correct phase. |
| M4 — Rule resolution | Cover explicit, static-pattern, implicit, suffix, grouped and double-colon rules, built-ins, search paths, libraries, archives and intermediates. | Map every rule family to required cases. Match candidate priority, automatic variables, lifecycle and filesystem effects in development and optimized acceptance. Pinned-profile acceptance is recorded in the M4 audit. |
| M5 — Linux runtime | Cover scheduling, local FIFO and pipe jobservers, recursion, output synchronization, stdin, load limits, `.WAIT`, `.NOTPARALLEL`, signals and cancellation. | Pass controlled and repeated mixed GNU/rmake trees, token recovery, failure and shutdown checks in both builds. Pinned-profile acceptance is recorded in the M5 audit. |
| M6 — Complete surfaces | Finish CLI and presentation behavior, database/debug output, locales, native modules, optional Guile, project integration and developer flows. | Cover every required surface on its supported profiles. Pass strict curated and integration suites and complete self-hosted developer flows. |
| M7 — Compatibility candidate | Audit coverage, test higher-order interactions, add fuzzing with failure shrinking, expand stateful stress tests, and qualify the Linux profile matrix. | Resolve ordinary in-scope failures, run every required case, retain reproducible extended-run evidence, and publish profile limits and performance baselines. |
| M8 — Safe acceleration | Reduce allocation, search and process costs. Add persistent source/expression caching, then graph fragments only when validation is proven. | Pass all earlier gates and cache-off, cold, warm, invalidated, corrupt and concurrent tests. Demonstrate a net gain including validation cost and record a decision before changing defaults. |
| M9 — Extension platform | Define a versioned, opt-in protocol with capabilities, namespaces, lifecycle, errors, examples and tests. | Preserve default behavior. Test extension effects and cache policy, and document the supported API or ABI versioning commitment. |

### Finish M3 and M6

Use the [gap audit](m3-m6-audit.md) to select the next bounded change and its
technical evidence. The remaining acceptance work spans several areas.

- Complete dynamic language, metadata and option interactions across startup,
  reading, recipes and real process restarts.
- Extend rule and remake combinations beyond the accepted M4 profile, including
  grouped peers and source identity changes.
- Complete diagnostics, database/debug output and locale coverage across their
  producers and runtime phases.
- Qualify native and Guile callbacks, asynchronous work, cleanup and resource
  failures on the supported configurations.
- Expand Linux resource, signal, jobserver and output ownership combinations.
- Complete the project, compiler, profile and self-hosted developer-flow matrix.

A few recorded resource differences remain explicit work. The
[pipe-pressure gate](../tests/resource-fd-pipe/README.md) identifies noncanonical
token and parent-fatal recovery questions. The
[process-limit gate](../tests/resource-nproc/README.md) records a combined
process/descriptor-limit case in which GNU exhausts descriptors after failed
shell captures while rmake releases them. These observations require resolution
under the compatibility and safety policy.

Retain the existing targeted gates as this work expands. For example,
[inherited signal-mask tests](../tests/signal-mask/README.md) cover eager
expansion, reader entry, child launch and actual self-exec. The
[project corpus](project-integration.md) and
[developer-flow tests](../tests/developer-flows/README.md) provide broader
execution evidence. Each gate has a finite observation boundary that later work
must extend where needed.

### Maintain the foundation

The pinned toolchain, workspace, reference acquisition, original corpus and
Makefile entry points already exist. Maintain those foundations as the supported
profiles grow. Every reference build must retain its version, configuration,
executable identity and acquisition provenance.

The harness must execute both engines with controlled paths, invocation names
and environment. Keep independent oracle expectations, comparator mutation tests,
strict observations and complete failure reproducers. Unexpected candidate
errors and pending fixtures remain visible failures or recorded gaps. They earn
no passing compatibility credit. The [testing guide](testing.md) defines these
contracts and the current containment limits.

## Develop a change

Deliver each feature in a reviewable slice.

1. Write original fixtures and expected behavior before implementation. Verify
   them with the pinned GNU reference and retain the candidate's failing result.
2. Implement the behavior through the existing ownership boundaries. Cover
   diagnostics, invalid inputs and cleanup alongside successful execution.
3. Run the affected strict comparisons and applicable profile, stateful and
   runtime gates. Promote a pending case only after an exact match.
4. Measure performance where the change affects a relevant workload. Keep inputs,
   executable identities and observations sufficient to reproduce the result.
5. Validate the required combined suite on the proposed commit before acceptance.

Corpus, oracle and benchmark research can proceed in parallel. Runtime changes
need coherent interfaces and explicit ownership. Record any source reference or
adaptation under the [licensing policy](licensing.md). GNU Make remains an
external comparison reference and cannot provide a runtime implementation
fallback.

## Top-level Makefile interface

The top-level [Makefile](../Makefile) is the entry point for developer and CI
workflows. `make help` lists the available commands. The
[testing guide](testing.md#run-the-checks) explains how to choose a check.

| Current commands | Purpose |
| --- | --- |
| `bootstrap`, `doctor` | Prepare the pinned tools and reference inputs, and check the configured toolchain and oracle. |
| `build`, `release` | Build development and optimized executables. |
| `fmt`, `fmt-check`, `lint` | Format code or check formatting and lint rules. |
| `test-unit`, `test-tools`, `check-core` | Check Rust behavior, independent Python tooling and core source quality. |
| `oracle-verify`, `test-oracle`, `test-builtins`, `test-diff` | Verify the reference, check independent anchors and compare the required corpus. |
| `test-generated`, `test-graphs`, `test-stateful` and family targets | Run reproducible generated programs, graph cases, timestamp sequences and focused semantic suites. |
| `test-jobserver`, `test-runtime-signals`, `test-output-sync` and related runtime targets | Check controlled process, token, signal and output behavior. |
| `test-safety`, `test-native-modules`, `test-guile-components`, `test-guile-frontend` | Check the separate approved safety policy, native modules and configured Guile profiles. |
| `test-integration`, `test-selfhost`, `test-developer-flows` | Compare real projects and the repository's own workflows. |
| `m4-acceptance`, `m5-acceptance` | Run the documented combined development and optimized milestone gates. |
| `test`, `check`, `ci` | Run the current aggregate checks. CI also exercises self-hosting. |
| `bench`, `bench-compare`, `profile-allocations`, `profile-allocations-frozen` | Record reference baselines, paired timings and allocation evidence. |
| `clean` | Remove selected generated outputs. |

Reference and candidate paths use `ORACLE` and `CANDIDATE`. Generated suites
expose seeds and family-specific replay selectors. Use the relevant suite's
README for its exact controls and bounds.

Future fuzzing, stress, coverage, compatibility reporting, persistent-cache and
package/install workflows need working implementations before they become public
Makefile targets. A placeholder that reports success cannot satisfy a gate.

Use `$(MAKE)` for recursion and preserve jobserver semantics. Keep CI and local
commands on the same recipes, quote paths and overrides, and express prerequisites
or separate output directories where parallel flows could interfere. Fixture
environments must stay contained within their runs.

Self-host checks must run finite leaf flows in separate output directories.
They must avoid recursively invoking `ci` or `test-selfhost`. Bootstrap with an
installed GNU Make, then exercise the same Makefile with rmake, including real
executable names and mixed recursive trees. A `gmake` executable name alone does
not establish the required GNU version or configuration.

## Performance gates

The proposed objective is **at least 2× lower engine time for representative
large, warm no-op builds**. Other goals are lower expansion and implicit-search
costs, reduced memory use and competitive cold startup. Tiny projects must retain
low startup cost as caches and other features are added. These are acceptance
goals that require evidence.

Freeze representative workloads before optimizing them. Include large graphs,
expansion-heavy input, implicit search, cold startup, incremental rebuilds,
recursive execution and adversarial cases. Keep recipe execution time separate
from engine time. If a benchmark compares compiled artifacts, control compiler
inputs and timestamps that could otherwise change the output.

Use paired repeated trials on a fixed machine and reference profile. Record
executable identities, wall and CPU time, median and tail behavior, peak memory
and I/O. Document cold and warm operating-system cache conditions and report
uncertainty. Publish regressions alongside improvements. See the
[measurement workflow](testing.md#measure-performance) for current commands and
retained evidence.

A **repeatable slowdown above 5% on a core workload is a proposed investigation
and explicit-acceptance threshold**. Apply an absolute-time floor to tiny
workloads. Review cold, incremental and memory regressions individually even
when an aggregate result improves.

Persistent caches must improve end-to-end time after validation and preserve
all compatibility checks. Follow the
[safe caching progression](architecture.md#safe-caching-progression), beginning
with effect-free inputs and explicit invalidation dependencies. Keep caching
opt-in until its correctness and default-enablement decision are complete.
Fall back to ordinary evaluation whenever validity is uncertain. Recipe-result
or artifact caches cannot replace GNU's timestamp and execution semantics.

The [extension design](architecture.md#extensibility-without-changing-defaults)
follows compatibility and safe caching. Extensions must declare effects and
cache dependencies, remain opt-in and preserve the default GNU behavior.

## Release decision and unresolved engineering choices

A compatibility release requires all of the following.

- A complete supported-profile ledger and resolution of ordinary in-scope
  differential failures. Required cases must run, and expected failures must
  reflect current evidence.
- Deterministic, parallel, signal, filesystem, native and applicable Guile/cache
  gates on capable runners, including higher-order interactions and extended
  stateful runs.
- Independent fixture and provenance review, reproducible reference acquisition,
  license and notice review, and matching Corresponding Source for distributed
  binaries under the [licensing policy](licensing.md).
- Complete developer flows under the pinned GNU reference and rmake, mixed
  recursion, and performance reports that retain regressions and resource costs.

Apply the contract's branding allowance and narrowly validated variable inputs
only at their documented comparison boundaries. The separately approved
[safety exceptions](safety-exceptions.md), including the exact
[nested short-option decision](nested-short-options-proposal.md), remain bounded
candidate-only checks with zero exact-conformance credit. A new ordinary
mismatch needs compatibility work or an explicit new decision. Existing
exceptions do not expand automatically.

The reference development profile is pinned. Broader decisions still need
recorded evidence, including the supported kernel floor, architecture/libc
matrix, test isolation backend, runtime images and practical test budgets.
Record these decisions with dates and their acceptance evidence. Later records
must cover cache enablement and public extension transport. Each decision must
preserve the agreed language and local execution scope.
