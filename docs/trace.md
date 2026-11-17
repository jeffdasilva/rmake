# Trace why a target rebuilds

Add `--trace` to a build command to see which target rmake is about to rebuild
and why. The output includes the recipe's origin when known and identifies
changed prerequisites when they are available.

```sh
rmake --trace
rmake --trace greeting.txt
```

Trace prints recipe commands even when `@`, `-s`, or `.SILENT` would
ordinarily hide them. It still obeys the selected execution mode. For example,
`rmake --trace -n` uses dry-run rules. See the
[GNU Make option summary](https://www.gnu.org/software/make/manual/html_node/Options-Summary.html)
for the behavior of those options and their exceptions.

## Requested flags and active diagnostics

rmake prints a rebuild reason after expanding a recipe and reserving a job
slot. An expansion error interrupts the recipe before the reason appears.
A later shell configuration failure follows the reason. Output synchronization
keeps the same recipe and recursion boundaries. See
[output during parallel execution](https://www.gnu.org/software/make/manual/html_node/Parallel-Output.html)
and [options for recursion](https://www.gnu.org/software/make/manual/html_node/Options_002fRecursion.html)
in the GNU Make manual.

During Makefile remaking, GNU Make suspends trace diagnostics unless Makefile
debug output is enabled. The recursive flags still carry the requested
`--trace` option. A live flag assignment during a remake can reactivate trace.
When the remake phase ends, rmake restores its previous diagnostic policy.

Reasons use the target and prerequisite results available when the recipe is
ready. For example, a regular prerequisite hypothetically updated by dry-run
can count as updated even while its file is missing. A phony prerequisite
keeps separate freshness rules.

## Evidence and remaining work

The original trace comparison added 165 fixtures with independently specified
output and status. The checks cover command echo, raw file names, recipe
locations, missing and changed prerequisites, phony targets, include and
restart paths, `VPATH` and `GPATH`, implicit and built-in recipes, recursive
builds and live option changes. They also cover parallel and synchronized
output. The fixtures require the intended rebuild to occur before checking its
reason.

Use the [testing guide](testing.md) to run or replay comparisons. The
[M3/M6 audit](m3-m6-audit.md) tracks remaining interactions among traces,
diagnostic settings, execution profiles and locales.

## Localized trace output

From the repository root, run the trace-specific comparison gates.

```sh
make test-trace-nls
make test-trace-job-languages
```

Add `-release` to a target name to test an optimized build. The trace NLS
reports are in `artifacts/trace`. The language reports are under
`artifacts/trace-job-languages`.

The NLS gate compares raw streams, exit status, and filesystem effects under C
and a verified French locale. It covers built-in and included recipes, changed
prerequisites, directory search, recursion, restarts, nonexecuting modes and
synchronized output. Two byte-oriented cases preserve non-UTF-8 names and a user
line that resembles a trace diagnostic.

The trace and JOBS language gate covers 18 German, Japanese and Brazilian
Portuguese comparisons, including live PID and address checks. The French
catalog translates the generic prerequisite template and built-in source
label. Fixed reasons use gettext fallback. Recipe commands and their output
retain their original bytes. See the [localization guide](nls-diagnostics.md)
for catalog setup and [catalog provenance](../catalogs/provenance.json) for
message sources and notices. The source reference for the trace message IDs
is GNU Make 4.4.1's `src/job.c`.

## Performance observations

The historical trace change was measured with trace disabled. These medians
come from 101 alternating, output-checked samples on one host. The measurements
cover the listed workloads. Host activity was uncontrolled.

| Workload | GNU ms | Current ms | Preceding main release ms |
| --- | ---: | ---: | ---: |
| Tiny no-op | 0.628 | 0.744 | 0.751 |
| 10,000 explicit targets | 12.636 | 32.459 | 32.623 |
| Expansion | 2.950 | 4.896 | 4.860 |
| Parallel branches, depth 1 | 87.926 | 90.266 | 90.310 |
| Parallel branches, depth 4 | 87.953 | 90.341 | 90.467 |

Raw [core](benchmarks/trace-core.json) and
[parallel](benchmarks/trace-parallel.json) reports accompany this slice.
