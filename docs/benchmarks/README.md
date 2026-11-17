# Measure rmake and GNU Make

Compare the engines on the same workload, with the same inputs and executable
name. Run the untimed checks first, then collect elapsed times on a quiet
machine.

## Validate the workload

From the repository root, run the following commands.

```sh
make bootstrap
make bench-compare BENCH_VALIDATE=1
```

The default `core` suite covers a tiny no-op, 10,000 explicit targets, and
variable expansion. The validator prepares fresh input for two GNU Make runs
and an optimized rmake run. It checks stdout, stderr, exit status, and final
file contents, types, and modes.

## Collect and save timings

```sh
make bench-compare BENCH_REPEAT=101 BENCH_REPORT=artifacts/bench/core-local.json
```

The benchmark runs two warmups per engine, then alternates execution order
and checks both output streams for every timed sample. Elapsed time includes
process startup. The standard report is `artifacts/bench/comparison.json`.
Set `BENCH_REPORT` to keep a different file on each run.

Use `BENCH_SUITE` to choose an area, such as `startup-arguments`,
`parallel-branches`, or `wildcard`. Use `BENCH_BASELINE` with the path to an
older optimized rmake binary to include it in the paired comparison. The
[benchmark runner](../../tools/benchmark.py) lists the available suites and
their inputs. See the [testing guide](../testing.md) for developer checks and
conformance replay.

## Read a report

The JSON contains executable hashes, the reference profile, arguments,
workload identities, and every timed sample. Ratios compare elapsed times in
each pair. An oracle-to-rmake ratio above one means rmake took less time
on that pair. Review more than one run before drawing a conclusion about a
small change. The standard runner uses a warm operating-system page cache
and measures wall time on a shared host.

The reports in this directory are historical measurements tied to the binaries
named in each file. GNU Make took less time on the standard workloads in the
[ordered startup change](ordered-cli-core.json), and rmake took less time on
selected [large command-line workloads](ordered-cli-startup.json). Use those
reports to inspect a particular workload, or run the same suite on your
current binaries.
