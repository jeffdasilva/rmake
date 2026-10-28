# Late disabling of built-in rules and variables

GNU Make snapshots `-r`/`-R` before reading Makefiles and reconciles newly
disabled built-ins after the complete read. Setting a flag while reading does
not immediately erase default variables. Initial `-R` and late `-R` also differ
when `.POSIX` introduces defaults. The fixed reference source in `main.c` and
`default.c` informed this phase model; no implementation or upstream tests were
copied or translated.

rmake now takes the same entry snapshot and performs cleanup at that boundary:

- New `-R` removes only the pinned default-variable names at default priority.
  Stronger file, environment, command and override bindings survive. Deletion
  also destroys that binding's export policy. Default/automatic runtime metadata
  outside the compiler-variable table stays intact.
- New `-r` clears untouched built-in `.SUFFIXES` dependencies and resets the
  `SUFFIXES` variable at default priority. A user's explicit suffix declarations
  and stronger `SUFFIXES` binding survive independently.
- Still-built-in suffix recipes/records installed by the current startup model
  are removed before conversion. Preserve user recipes and explicit declarations;
  an empty user suffix recipe retains its observed cancellation/completion behavior.
- Recipe/secondary-expansion flag changes do not repeat post-read cleanup.
  A Makefile restart restores the original invocation before rereading. File
  assignments are reapplied during that read; flags introduced only by a retired
  remake recipe are not silently promoted into original startup options.

Under `-e`, a higher-priority `MAKEFLAGS` binding can prevent an ordinary file
append from changing flags. The fixtures preserve that origin behavior.

## Acceptance

204 original cases cover three startup profiles, both flag variables, seven
assignment/include/preload forms, variable origins, POSIX defaults, suffix
declarations, built-in/user/empty suffix recipes, later evaluation, and reader
state across restarts, plus 24 child-environment controls for deleted/retained
export policies and recreated values. Two GNU observations agree and match the
candidate's raw status, streams and filesystem. The preceding release fails 93
of these cases.
Independent status/phase/origin/effect assertions were required before promotion.
Successful secondary/recipe probes must actually reach those phases; an early
syntax error is not accepted as equivalent behavior.

Three further required cases observe startup defaults and successful late
cleanup around 0, 1,000 and 10,000 unused declarations. These are also the
output-checked `builtin-transitions` benchmark. It deliberately omits initial
`-rR`; negative controls reject disabling defaults at startup or retaining them
after reading. The top-level flow is:

```sh
make bench-compare BENCH_SUITE=builtin-transitions BENCH_REPEAT=101
```

GNU and rmake support this flow. A baseline must implement the cleanup semantics
correctly: the previous release's wrong output is rejected before timing it.

The corpus now has 9,273 ordinary required cases, no ordinary pending entries,
and 17 separately gated [safety exceptions](safety-exceptions.md). Unit totals are
447 Rust / 372 tooling. Full development/optimized acceptance and exact-head CI
remain merge gates. M3/M6 and general speed parity remain open. In particular,
this phase correction does not implement `--eval`, the full startup/self-exec
model, native loading, enabled Guile, presentation/locale support or the complete
integration inventory.

## Bounded cleanup work

The first correct prototype scanned the complete rule table to find built-in
recipes. The final implementation looks up only the 49 pinned suffix-record
names, plus `.SUFFIXES`. An existing set-equality test protects agreement with
the declarative compatibility data. Default-variable removal likewise uses the
fixed default table. Neither path scans the arbitrary user graph or caches
mutable observations.

The cleanup benchmark compares the final implementation with the correct
scanning prototype; both produce the same independently checked result. Common
workloads compare against the preceding main release. Raw reports retain all
samples, workload bytes/arguments and executable identities. Measurements include
startup, use 101 alternating samples, and run without concurrent builds or
acceptance campaigns. Host activity is not controlled.

| Cleanup workload | GNU ms | Bounded cleanup ms | Correct scanning prototype ms |
| --- | ---: | ---: | ---: |
| 0 unused declarations | 0.698 | 0.976 | 0.991 |
| 1,000 unused declarations | 1.226 | 2.296 | 2.310 |
| 10,000 unused declarations | 6.639 | 14.491 | 15.016 |

The last median is about 3.5% lower than the scanning prototype in this run.
The structural change removes a graph-wide scan; the samples alone do not
establish a statistical speedup or general parity. GNU remains faster here.

| Common workload | GNU ms | Current ms | Preceding main release ms |
| --- | ---: | ---: | ---: |
| Tiny no-op | 0.610 | 0.750 | 0.748 |
| 10,000 explicit targets | 13.138 | 33.889 | 34.651 |
| Expansion | 3.099 | 4.984 | 5.141 |
| Parallel branches, depth 1 | 86.925 | 89.238 | 89.215 |
| Parallel branches, depth 4 | 87.488 | 89.758 | 89.688 |

The mixed differences establish neither a statistical improvement/regression nor
general parity. Raw [cleanup](benchmarks/late-builtins-cleanup.json),
[core](benchmarks/late-builtins-core.json) and
[parallel](benchmarks/late-builtins-parallel.json) reports retain all samples.
