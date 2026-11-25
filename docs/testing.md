# Test changes to rmake

The tests compare rmake with a pinned GNU Make 4.4.1 executable. If you change
how rmake reads a Makefile, chooses a recipe, or runs a command, add a small
Makefile that reproduces the behavior. For the supported platform, reference
builds, and permitted differences, see the [compatibility guide](compatibility.md).

## Run the checks

From the repository root, prepare the toolchain and reference executable.

```sh
make bootstrap
```

The bootstrap also prepares the sources for real-project tests. For a local
edit, choose the smallest check that exercises the change, then run `make check`
before sending the change for review.

| Command | What it checks |
| --- | --- |
| `make check-core` | Rust formatting, Clippy, unit tests, Python tooling, and translation catalogs. |
| `make test-oracle` | The GNU Make reference against itself and the expected outputs in the ordinary fixtures. |
| `make test-diff` | rmake and GNU Make on the ordinary fixtures. |
| `make test-stateful` | Repeated builds after file changes, including timestamp and include behavior. |
| `make test-remake-parallel` | Parallel Makefile remakes and restarts with controlled recipe completion. Use `make test-remake-parallel-release` for the optimized build. |
| `make test-jobserver` | Mixed GNU Make and rmake trees with pipe or FIFO jobservers. |
| `make test-integration` | Clean and incremental builds of pinned real projects. |
| `make test-selfhost` | Selected developer tasks run through rmake. |
| `make test-safety` | rmake's documented error behavior for the separate [safety cases](safety-exceptions.md). |

Use `make help` to find other suites and their replay settings. GitHub CI runs
the development and optimized checks, plus separate Guile checks. See the
[CI workflow](../.github/workflows/ci.yml).

The [native AArch64 job](../.github/workflows/native-aarch64.yml) uses an
AArch64 Linux runner to check host metadata, subprocess and restart behavior,
native standard streams, shared jobservers, developer workflows, and deep
evaluation. It builds a separate pinned GNU Make reference on the runner.
Selected C and C.UTF-8 locale cases and the pinned xxHash project compare the
optimized build with that reference.

The [native x86-64 musl job](../.github/workflows/native-musl.yml) builds
GNU Make and optimized rmake with a private musl 1.2.4 dynamic loader.
It compares selected C-locale wildcard, shell status, include, native
module, process restart, and inherited jobserver pipe cases. Recipes use
the runner's `/bin/sh`. To reproduce the smoke job on x86-64 Ubuntu 24.04
with GCC, binutils, Make, and dpkg installed, run

```sh
python3 tools/rust.py bootstrap
python3 tools/musl_smoke.py --report artifacts/native-musl/profile.json
```

The smoke script owns its temporary build and fixture directories. Its
report records the toolchain inputs, observed runtime libraries, and case
results. The private loader path is embedded in the smoke executables.

To replay one ordinary case, build the candidate and pass the case ID to the
runner. The runner uses `.tools/oracle/bin/make` by default.

```sh
make build
python3 tools/conformance.py --candidate target/debug/rmake \
  --case graph-ready-two-barriers
```

Repeat `--case` to select several cases. With explicit cases, add `--repeat 3`
to run each comparison three times in fresh directories. Outputs and failure
reproductions are written under `artifacts/`.

The runners execute Makefile recipes and helpers. Use trusted fixtures and run
untrusted programs only in an environment designed to contain them. The test
runner bounds processes and output and resets a temporary workspace between
engines.

## Add a regression case

1. Reduce the difference to a Makefile with as few rules and input files as
   possible. For a generated case, save the seed, full input, and exact command
   before reducing it.
2. Put a one-run case in `tests/cases/CASE_ID/` with its inputs and `case.toml`.
   Use the [manifest guide](../tests/README.md) to record arguments, environment,
   provenance, and the outcome you expect from GNU Make. For an incremental
   build, use the [stateful fixture format](../tests/stateful/README.md).
3. Add the case to [the requirements it covers](../tests/requirements.toml).
   Run it with GNU Make and reproduce the rmake difference before qualifying
   the fix. Record any upstream source or test references in the fixture's
   provenance.
4. Set the candidate status to `required` when the case passes against the
   pinned reference. Replay nearby cases to cover the shared behavior, including
   error paths, ordering, and boundary inputs where they apply.

Write the expected effects independently of the rmake implementation. A
fixture for a generated file should assert that the file exists with the right
contents or metadata as well as comparing what the programs print. Generated
programs should keep a replayable seed and check independent properties, such
as whether every required node in a dependency graph created its output.

## Compare results

For deterministic output, compare stdout and stderr byte for byte. Run the
engines with the same inputs, working directory, and executable name so the
comparison preserves details such as paths in restart diagnostics. Name and
recursion tests run with the real invocation names and check which engine
executes the child.

Compare exit status and filesystem effects even when both engines print the
same output. The ordinary runner compares file types, contents, permissions,
and links. The stateful and resource suites add checks for timestamps, process
termination, inherited descriptors, and other effects they exercise.

Use a controlled worker or a jobserver token to hold a parallel recipe until
the event you need to observe. Repeated GNU runs help establish whether the
schedule is stable. For a schedule with several valid outcomes, check the
required dependency order, job limits, and cleanup. Compare a shared stdout
and stderr descriptor directly when the order between the streams matters.
See the [parallel output documentation](https://www.gnu.org/software/make/manual/html_node/Parallel-Output.html)
and [jobserver tests](jobserver.md).

For fields that necessarily vary, such as a process ID or jobserver path, check
the field's format and relationship to the observed process or descriptor. Keep
the raw output alongside the comparison. Add a harness test that proves a real
difference still fails. When two GNU observations disagree, keep both and
identify the cause before making the test required.

## Choose a suite

The [requirements ledger](../tests/requirements.toml) records which ordinary
cases cover each behavior. Check the interaction you changed as well as the
individual features. For example, a secondary expansion change may affect
which target-specific variables an implicit rule sees.

| Behavior | Suites and replay settings |
| --- | --- |
| Expansion and variables | `make test-generated`, `make test-whitespace`, `make test-word-index`, and `make test-intcmp`. |
| Rule selection and prerequisites | `make test-graphs`, `make test-scopes`, `make test-pattern-scopes`, and `make test-secondary`. Use `GRAPH_SEED` and `START_INDEX` to replay a graph. |
| Filesystem observations | `make test-filesystem`, `make test-directory-search`, `make test-wildcards`, `make test-symlink-times`, and `make test-archive-times`. |
| Invocation and diagnostics | `make test-modes` and `make test-diagnostics`. Set `MODE_CASE` or `DIAGNOSTIC_CASE` to replay a case. |
| Process and environment behavior | `make test-runtime-signals`, `make test-runtime-effects`, `make test-startup-exits`, and `make test-output-sync`. |

Retain original test inputs and attribution for third-party inputs. For
milestone coverage and gaps beyond the current corpus, see the
[current compatibility audit](m3-m6-audit.md) and the
[implementation plan](implementation-plan.md).

## Measure performance

Validate the workload first. This command builds optimized rmake and checks
results against GNU Make without measuring elapsed time.

```sh
make bench-compare BENCH_VALIDATE=1
```

For timings, run `make bench-compare` and keep the report. Use `BENCH_SUITE`
to select a workload and `BENCH_BASELINE` to compare with a prior rmake binary.
The report includes the executable identities, inputs, and sample timings. Run
paired benchmarks without concurrent builds when you can, and describe the
workloads and machine when reporting a change. See the
[benchmark reports](benchmarks/README.md).

## Test future persistent caching

When persistent caching becomes available, compare runs with caching disabled,
with an empty cache, and with a populated cache. Change an input without
changing its size or timestamp. Also test changes to includes, wildcard
matches, environment, arguments, tools on `PATH`, symlinks, permissions, and
extension inputs. Check that shell calls, warnings, file writes, and restarts
still occur in the correct order as well as comparing the final files.

Exercise interrupted writes, corrupt and incompatible entries, concurrent
readers and writers, and read-only or full filesystems. Keep cache data outside
the fixture workspace and record its state for each run. See the
[caching design](architecture.md#safe-caching-progression).
