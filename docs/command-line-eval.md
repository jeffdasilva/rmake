# Command-line evaluation and startup phases

rmake accepts `-E STRING`, `-ESTRING`, `--eval STRING` and `--eval=STRING` on the
pinned GNU Make 4.4.1 Linux profile. Ordered buffers are deduplicated by their
exact bytes and evaluated after startup defaults, before `MAKEFILES` preloads
and ordinary Makefiles. They can define variables/rules, choose the default
goal, include files, run expansion effects and append new eval buffers through
live `MAKEFLAGS` decoding. Newly appended buffers join the active initial loop.

This uses the reader's buffer API, not a synthesized `$(eval ...)` expression:
command-line buffers receive ordinary Makefile parsing rather than an extra
argument-expansion pass. Memory buffers retain their fixed caller line number.
BOM handling occurs before recipe/continuation classification when that line is
one; real included files retain their own filename and physical locations.

## Metadata, recursive flags and restarts

`MAKEFILE_LIST` is initialized after the initial eval pass. An include can create
and expose the list during eval, but ordinary-file initialization resets its
file-priority value afterward. Imported `-e` and command-line bindings retain
their normal priority. Eval buffers themselves do not add a filename.

`MAKEFLAGS` holds a reference to the automatic/simple `-*-eval-flags-*-` snapshot,
before command-variable overrides. `MFLAGS` excludes eval payloads. The snapshot
is installed after the complete initial loop and quotes blanks, tabs,
backslashes and dollars for recursive transport. Later file/recipe `--eval`
options neither execute immediately nor rebuild that snapshot. Original
invocation buffers run again through the existing restart mechanism and see the
current `MAKE_RESTARTS`. Complete self-exec/startup behavior remains separate
acceptance work.

Anonymous warnings and fatal messages use the program prefix. Anonymous recipe
failures and trace output use `<builtin>`; included-file failures retain their
real filename/line. An existing reader-prefix defect now also reports an
incomplete recognized function as an unterminated function rather than a generic
variable reference, without changing successful structural balancing.

## Eval-selected switches

GNU's `-R` implies `-r` at startup and after post-read decoding, not at every live
assignment. Runtime decoding now preserves that phase distinction. If eval
selects `-r`, preinstalled built-in recipes are removed without clearing the
already-created suffix metadata or default variables. If eval selects only
`-R`, the later implied rule flag can still trigger post-read suffix cleanup.
User suffix recipes and pattern recipes consequently have different outcomes;
both are exercised. Existing bounded late-rule cleanup is reused.

An eval-selected job count becomes the post-eval entry value. An original
explicit CLI job setting retains priority against later file settings using
that entry value, rather than incorrectly restoring its pre-eval count.

## Evidence and open work

194 original required cases have independent exact status/stream models checked
against two pinned GNU observations and the final candidate. They cover four
startup built-in profiles; assignment flavors and origins; raw values, quoting,
metadata, BOMs and diagnostics; includes/preloads, recursion, restarts, generated
main files, job priority and runtime modes. File-effect anchors distinguish exact
duplicate buffers from distinct trailing bytes and require generated recipe
expansion to remain deferred.

Two output-checked scaling cases observe every separately evaluated binding at
0 and 1,000 buffers. A middle-value mutation must fail the expected output.
Run `make bench-compare BENCH_SUITE=eval-arguments` to compare the supported
implementations; the previous release does not support the nonempty workload.
Existing core, startup-argument and parallel suites retain before/after checks.

The curated corpus is 9,992 required ordinary cases, with no ordinary pending
entries, plus 17 separately gated safe-error exceptions. Totals are 457 Rust /
387 tooling tests. Full local development/optimized acceptance and exact-head
CI remain merge gates. This is not M3/M6 completion: broader startup command
assignment/environment decoding, malformed-option usage and early exits, true
self-exec, debug/database/directory reporting, native/Guile integrations,
locales and complete project/performance acceptance remain open in the
[compatibility inventory](m3-m6-audit.md). Existing startup research retains its
ordinary-gap status and receives no safety waiver or passing credit.

The fixed GNU source was consulted for option/eval phases, memory-buffer line
handling and post-read built-in installation. No GNU implementation or upstream
test code was copied or translated.

## Performance observations

The frozen final build uses 101 alternating, output-checked samples per workload,
without concurrent builds or acceptance campaigns. Host activity is not controlled.
The empty-eval path retains its previous metadata snapshot shape; ordinary file
reading keeps its existing reader path and startup/runtime decoding is specialized
at compile time. These observations do not establish statistical improvement or
regression, and GNU remains faster on standard workloads.

| Workload | GNU ms | Current ms | Preceding rmake ms |
| --- | ---: | ---: | ---: |
| Tiny no-op | 0.642 | 0.794 | 0.755 |
| 10,000 explicit targets | 12.231 | 32.345 | 31.922 |
| Expansion | 2.938 | 5.306 | 5.151 |
| Startup, zero overrides | 0.599 | 0.711 | 0.707 |
| Startup, 1,000 overrides | 2.338 | 2.215 | 2.206 |
| Startup, 5,000 overrides | 27.115 | 8.467 | 8.632 |
| Parallel branches, depth 1 | 87.094 | 89.228 | 89.129 |
| Parallel branches, depth 4 | 87.576 | 89.980 | 89.525 |
| Eval, zero buffers | 0.733 | 0.886 | unsupported suite baseline |
| Eval, 1,000 buffers | 3.475 | 2.179 | unsupported |

Several common medians are higher, including expansion by about 3.0% in this run;
5,000-override startup is lower. The new 1,000-buffer eval workload measures about
37% below GNU's median in this run. Every buffer's binding appears in output.
These are workload observations, not general parity or a universal speedup. Raw
[core](benchmarks/cli-eval-core.json),
[startup](benchmarks/cli-eval-startup-arguments.json),
[parallel](benchmarks/cli-eval-parallel-branches.json), and
[eval](benchmarks/cli-eval-eval-arguments.json) reports retain samples and identities.
