# Measure rmake performance

Use the optimized build and the same workload when comparing rmake with GNU
Make. The benchmark runner checks output, exit status, and file effects before
reporting a timing. Run the commands from the repository root after `make
bootstrap`.

## Check and time a workload

1. Check that both tools produce the expected results for the three core
   workloads. This command skips timing.

   ```sh
   make bench-compare BENCH_VALIDATE=1
   ```

2. Run the paired timing comparison.

   ```sh
   make bench-compare BENCH_SUITE=core BENCH_REPEAT=25
   ```

   The core suite runs a tiny build, a no-op build with 10,000 prerequisites, and an
   expansion workload. `make help` lists the other suites. Use `BENCH_BASELINE`
   with the absolute path to an earlier optimized rmake binary to check a change
   against that version in the same run.

3. Read the report under `artifacts/bench/`. It contains the inputs, sample
   timings, and executable identities. Paths in committed reports identify the
   binaries used for those measurements. Use the commands above to build a new
   comparison on your machine.

## Interpret benchmark results

Read timings in the context of the workload and measurement environment. For
example, the startup and restart studies found cases where rmake took less time
with large argument lists and cases where GNU Make took less time on the core
workloads. Both findings depend on the specific inputs and builds. See the
[startup argument measurements](../startup-arguments.md) and the
[restart directory measurements](../restart-directories.md).

When reporting an optimization, include the workload, versions, and machine.
Run paired samples with other builds idle when possible. Keep the sample data
and report whether you measured elapsed time, CPU time, memory, or allocations.
The [performance criteria](../implementation-plan.md#performance-gates) describe
how the project evaluates an optimization.
