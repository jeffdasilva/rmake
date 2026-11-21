# Original deterministic debug-output profiles

Use these tests to compare GNU Make 4.4.1 and rmake diagnostics with the same
Makefile, process arguments, and starting files. Most cases compare text and
file effects. Held-worker cases also check the output visible before a real
recipe process can finish.

## Run or replay the gate

From the repository root, run the full debug gate or select one case.

```sh
make test-debug-output
make test-debug-output DEBUG_CASE=debug-remake-cache-include-current-existing-ordinary
```

The normal target writes `artifacts/debug-output/development.json`. To test an
existing optimized binary or select several cases, use the runner directly.

```sh
python3 tools/debug_output.py --candidate target/release/rmake \
  --case debug-bare-basic \
  --case debug-remake-cache-include-current-existing-ordinary \
  --report artifacts/debug-output/focused-release.json
```

Use `--oracle PATH` to select a verified reference, or `--oracle-only` to check
the expected models against GNU Make without evaluating rmake. See the
[testing guide](../../docs/testing.md) for reference setup and how to interpret
a replay.

## Read a case result

Each case runs GNU Make twice in the same reset workspace, followed by rmake.
The checks include stdout and stderr bytes, exit status, the complete expected
filesystem, and ordered effects in `seen`. Inputs specify timestamps when
timing affects a rebuild. A process bound, an unexpected file, or an uncollected
child fails the case.

The fixture knows the workspace and invocation paths before either engine runs.
`@WORK@` and `@BIN@` insert those paths into the expected streams.
GNU versions printed when debug starts must match exactly. The one explicit
`--version` case compares each engine's banner at its declared position, then
checks all other bytes and the surrounding effects.

## Find a case

| Case group | What it checks |
| --- | --- |
| Original and extension cases | Debug levels, option reset and serialization, trace, startup and live flags, source annotations, and output order. |
| Child and goal waits | Real workers held at declared boundaries, `.WAIT` and targeted `.NOTPARALLEL`, and the exact trace after a child can finish. |
| [Recursive question cases](recursive-question.toml) | Forced exit status one, nested and double-colon parents, keep-going and current children, and directory messages around a sub-make include. |
| [Remake cache cases](remake-cache-phase.toml) | Required and optional reads, restarts, named goals and cached results. Eight cases hold workers across Makefile and ordinary phases. Four more check verbose output enabled in the middle of ordinary traversal. |

These fixtures use ordinary Makefiles and shell recipes. The
[JOBS comparisons](../../docs/job-debug.md) use live process relationships to
check diagnostics that include process IDs or addresses. The
[compatibility audit](../../docs/m3-m6-audit.md) tracks remaining combinations.

## Historical reports

The early campaigns and their negative controls are retained with their source
and executable identities. The first 40 models pass the pinned GNU reference
twice. The early rmake binary passed one. A later revision added 14 reader and
option boundaries, then added child-lifecycle cases. A historical observer fix
treated `ESRCH` like `ENOENT` when an observed child exited, and retained a
control for other read errors.

| Evidence manifest | Contents |
| --- | --- |
| [history.json](history.json) | Original 40 models, raw and refined observations, and earlier wording and order hypotheses. |
| [extension-history.json](extension-history.json) | The 14 added input and option cases, raw observations, and earlier binary results. |
| [qualification-v2.json](qualification-v2.json) | Full 54-case debug comparison, native and directory checks, and preceding-binary results. |
| [qualification-children.json](qualification-children.json) | Both 64-case child-lifecycle runs and the two failures on the preceding debug binary. |
| [child-history.json](child-history.json) | Original child models, first predictions, corrected traces, and explicit worker completion boundaries. |
| [performance-v2.json](performance-v2.json) | Elapsed and allocation reports, helper sources, and workload inputs for the earlier debug version. |
| [performance-children.json](performance-children.json) | Paired timing and allocation reports for the child-lifecycle change, including regressions. |

Historical measurement notes preserve the workloads, compared revisions and
observed regressions. Read the [earlier debug measurements](../../docs/benchmarks/debug-output.md)
and [child-lifecycle measurements](../../docs/benchmarks/debug-children.md) with
their linked reports. The [JOBS measurements](../../docs/benchmarks/job-debug.md)
and [raw performance archive](../job-debug/performance.json) cover the separately
held process and token comparisons.
