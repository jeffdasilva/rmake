# Imported restart counters and export policy

Make reads its makefiles, then checks whether rules can update any of them. If a
makefile changes, Make starts over with the updated inputs before it builds the
requested goals. A required `include` can be missing during the first read. Make
tries to build it before reporting a failure. Use `-include` for inputs the build
can proceed without. [Remaking makefiles](https://www.gnu.org/software/make/manual/make.html#Remaking-Makefiles),
[Including other makefiles](https://www.gnu.org/software/make/manual/html_node/Include.html).

### Observe a restart

Save this `Makefile` in a new directory. Use tabs to indent the recipes.

```makefile
.DEFAULT_GOAL := all
$(info read=$(or $(MAKE_RESTARTS),0))
include generated.mk

all:
	@printf '%s\n' '$(VALUE)'

generated.mk:
	@printf '%s\n' 'VALUE := ready' > $@
```

Run rmake with a clean startup environment for the flags and restart counter.
Replace `rmake` with the pinned GNU binary to replay the same example.

```sh
env -u MAKE_RESTARTS -u MAKELEVEL -u MAKEFLAGS -u GNUMAKEFLAGS \
  -u MFLAGS -u MAKEFILES rmake -rR --no-print-directory
```

The first read prints `read=0`. The remake creates `generated.mk` and triggers a
second read, which prints `read=1`. The goal then prints `ready`. Run the same
command again and expect `read=0` followed by `ready`. The existing include is
current. Add `--debug=b,m` to see the remake decisions and the `Re-executing[1]`
message on a run that creates the include. [Debug options](https://www.gnu.org/software/make/manual/html_node/Options-Summary.html).

Give generated makefiles real inputs so their rules can settle after a restart.
Make skips remaking makefiles marked `.PHONY` and makefiles with a double-colon
recipe without prerequisites. `-B` forces the usual target decisions. After a
nonzero restart count, Make suppresses `-B` while considering makefile remakes.
[Remaking makefiles](https://www.gnu.org/software/make/manual/make.html#Remaking-Makefiles),
[`-B` and `--always-make`](https://www.gnu.org/software/make/manual/html_node/Options-Summary.html).

### Interpret `MAKE_RESTARTS`

Read `MAKE_RESTARTS` to see the count for this invocation. On ordinary startup
without an imported value, it is unset until a restart. Use `MAKELEVEL` to
inspect recursive sub-make depth. Leave `MAKE_RESTARTS` under Make's control.
[Special variables](https://www.gnu.org/software/make/manual/html_node/Special-Variables.html).

rmake imports the process counter from the startup environment. Assignments on
the command line or in a makefile can change the visible value independently of
that counter. The imported binding retains its original whitespace, signs, and
suffixes after removing a leading directory-trace marker when present. Numeric
conversion follows the pinned Linux unsigned conversion. `MAKELEVEL` has its
own first-byte minus check. On a restart, rmake increments the counter with
unsigned 32-bit wraparound and passes the decimal value in the replacement
image’s environment. A wrapped zero leaves an absent counter entry absent.

The imported binding has a no-export policy for recipe, shell function, and
recursive environments, including when export-all is active. Explicit export
can enable it. `undefine` removes the binding and its policy. A fresh
command-line definition without an imported binding follows normal export
rules.

### Understand the rmake restart path

A changed makefile makes the current rmake process replace its program image.
The new image uses the retained invocation and arguments and the live process
environment after updating the restart counters. Loader and startup hooks run
again. If the original invocation used `-C`, rmake restores the startup
directory before replaying the original directory arguments. An inherited pipe
can survive the replacement. [Restart directories and recursive invocation identity](restart-directories.md),
[Inherited pipe across process replacement](../tests/restart-pipe/README.md).

The 128-reread limit belonged to an earlier in-process experiment. The current
process-image path continues restarting beyond that count when the makefiles
keep changing.

## Evidence and limits

### Replay the pinned restart tests

From the repository root, verify GNU Make 4.4.1 and run the restart gates. If the
reference is missing, run `make bootstrap-oracle` first. The test targets build
the candidate and write the reports under `artifacts/`.

```sh
make oracle-verify
make test-process-restarts test-restart-environment test-restart-pipe
```

- `test-process-restarts` compares two GNU runs and one rmake run. It checks
  successive startup hooks, replacement of the executable alias, and exact
  streams, status, and file states. Use `PROCESS_RESTART_CASE` to select one
  case. [Process restart tests](../tests/process-restarts/README.md).
- `test-restart-environment` compares the ordered process environment at each
  entry, including duplicate keys and counter wrap. Use
  `RESTART_ENVIRONMENT_CASE` to select one case. [Restart environment tests](../tests/restart-environment/README.md).
- `test-restart-pipe` checks that the same incoming pipe descriptors and pipe
  identity survive both images. [Inherited pipe tests](../tests/restart-pipe/README.md).

To replay a single case with an existing binary, use the corresponding driver.
The following command runs the loader-constructor witness without rebuilding the
candidate.

```sh
python3 tools/process_restarts.py \
  --oracle "$PWD/.tools/oracle/bin/make" \
  --candidate /absolute/path/to/rmake \
  --case process-restart-constructor \
  --report artifacts/process-restarts/focused.json
```

### Historical restart-counter qualification

The earlier in-process implementation separated the imported GNU counter from
its own reread count. It transferred the owned original arguments and
environment into the next engine, retaining an argument copy for decoding and
reconstruction and reapplying startup effects. That version used its own
reread count for reconstruction, initial mutex handling, and the development
iteration budget. Imported numbers above 128 did not spend that budget.

The original 214 required controls cover 21 numeric input profiles across
startup, effect-prepared reads, snapshots, serial builds, pipes, and synchronized
output. They cover export-policy lifetime and visible assignment independence.
A fixed-nanosecond producer makes unsigned wrap with `-B` reproducible across
two actual remakes. Eight separate controls verify that very early `MAKEFLAGS`
bindings become active special variables only later. Two GNU observations
corrected the initial model, and source review confirmed the `v->special`
guard. The 222 controls compare exact models and file effects with two pinned
GNU runs and the candidate. At that checkpoint, 10,488 preceding required
cases passed and the ordinary corpus grew to 10,710 required cases with zero
pending. Seventeen approved safe-error exceptions remained separate. Negative
tooling tests check wraparound, raw bytes, policy lifetime, activation, and
research classification.

Two imported directory-marker probes originally recorded an owed
leaving-directory message under `--no-print-directory`. Their incomplete
models, inputs, binary identities, and before/after observations remain research
evidence. The later imported-directory gate covers both original reproducers
among 482 profiles. [Original directory-marker research](../tests/research/imported-restart-directory-markers.json),
[Imported directory trace lifetime](imported-directory-trace.md).

The initial snapshot-warning inputs also produced an unrelated undefined `A`
read. The revised fixture defines `A` and preserves the two missing
`MAKEFLAGS` warnings and the snapshot path. Earlier observations are retained.
For current compatibility scope, consult the [M3/M6 audit](m3-m6-audit.md).

The implementation work consulted GNU Make 4.4.1 `src/main.c` for environment
import and re-exec/remake behavior, `src/misc.c` for unsigned conversion, and
`src/variable.c` for the special-binding guard. The verified archive SHA-256 is
`dd16fb1d67bfab79a72f5e8390735c49e3e8e70b4945a15ab1f81ddb78658fb3`.
GNU source is copyright the Free Software Foundation, Inc., GPL-3.0-or-later.
The historical change uses independently authored safe Rust, fixtures, and
tooling. GNU runs as the comparison oracle. [Pinned archive manifest](../oracle/manifest.json).

## Frozen validation and performance observations

The following results describe the historical restart-counter builds. Formatting,
lint with warnings denied, 457 Rust tests, and 418 tooling tests passed at that
checkpoint. Both frozen builds passed all 463 inherited, ordered, and restart
controls and the 128-cell exit gate. Full rmake-driven development and optimized
acceptance and exact-commit CI were separate merge gates.

- Debug SHA-256 `c11e29abf2da4d3770901af93b09268fcb7733b733b8dc29bac9c5b22a4bf67e`
- Release SHA-256 `58cd282afd36baeb8ecfa52d7157a9166ad85cb5883a60d60a3a766e750cb526`

The six historical reports record 101 alternating samples per engine with exact
output checks. Builds and test campaigns were idle during timing, while host
activity remained unisolated. Large no-op, eager CLI effects, and inherited
effects had medians roughly 2–3 percent above those of preceding rmake. Other
workloads show a mixture of lower and close medians. These results describe the
recorded runs and establish no statistical speedup or general parity.

| Workload | GNU ms | Restart-counter build ms | Preceding rmake ms |
| --- | ---: | ---: | ---: |
| tiny-noop | 0.717 | 0.867 | 0.871 |
| large-noop | 11.548 | 30.353 | 29.700 |
| expansion | 2.876 | 5.050 | 5.075 |
| startup-arguments-0 | 0.649 | 0.762 | 0.760 |
| startup-arguments-1000 | 2.201 | 2.100 | 2.074 |
| startup-arguments-5000 | 25.907 | 8.411 | 8.373 |
| parallel-branches-depth-1 | 87.972 | 90.552 | 90.475 |
| parallel-branches-depth-4 | 88.161 | 90.521 | 90.891 |
| eval-arguments-0 | 0.716 | 0.886 | 0.891 |
| eval-arguments-1000 | 3.057 | 1.950 | 1.945 |
| startup-effects-0 | 0.644 | 0.766 | 0.765 |
| startup-effects-1000 | 2.563 | 3.101 | 3.040 |
| initial-flags-0 | 0.587 | 0.685 | 0.695 |
| initial-flags-1000 | 2.537 | 2.902 | 2.828 |

The original timing reports retain the measurements and executable identities.
[Core](benchmarks/restart-state-core.json), [startup](benchmarks/restart-state-startup.json),
[parallel](benchmarks/restart-state-parallel.json), [eval](benchmarks/restart-state-eval.json),
[effects](benchmarks/restart-state-effects.json), [inherited](benchmarks/restart-state-inherited.json).

The allocation campaign checked the recipe-free CLI and inherited-effect inputs
against the required fixtures before measuring. Both runs per engine and workload
recorded identical counters with equal-length trace and helper paths.

| glibc request metric | Preceding rmake | Restart-counter build |
| --- | ---: | ---: |
| effects calls | 29,771 | 28,747 |
| effects requested bytes | 2,111,797 | 2,050,273 |
| inherited calls | 28,850 | 28,824 |
| inherited requested bytes | 2,342,666 | 2,307,072 |

The CLI-effect workload used 1,024 fewer requests (about 3.4 percent) and
61,524 fewer requested bytes (about 2.9 percent). The inherited-effect workload
used 26 fewer requests and 35,594 fewer requested bytes. The profiler records
`malloc`, `calloc`, `realloc`, and `free` after initialization. Its byte count
covers requested memory, excluding aligned allocation entry points. Use sampled
stacks to locate callers and elapsed measurements to evaluate runtime.

The original allocation records include both repeats for both binaries.
[effects/priorxx/0](benchmarks/restart-state-alloc-effects-priorxx-0.json),
[effects/priorxx/1](benchmarks/restart-state-alloc-effects-priorxx-1.json),
[effects/current/0](benchmarks/restart-state-alloc-effects-current-0.json),
[effects/current/1](benchmarks/restart-state-alloc-effects-current-1.json),
[inherited/priorxx/0](benchmarks/restart-state-alloc-inherited-priorxx-0.json),
[inherited/priorxx/1](benchmarks/restart-state-alloc-inherited-priorxx-1.json),
[inherited/current/0](benchmarks/restart-state-alloc-inherited-current-0.json),
[inherited/current/1](benchmarks/restart-state-alloc-inherited-current-1.json).

Run the profiler against an existing GNU or rmake binary. The
`profile-allocations` target builds an isolated profile first.

```sh
make profile-allocations-frozen \
  PROFILE_CANDIDATE=/absolute/path/to/binary \
  PROFILE_WORKLOAD=startup-effects-1000 \
  PROFILE_OUTPUT=/absolute/path/to/report
```
