# How rmake orders intermediate-file cleanup

An implicit rule chain can create a temporary file on the way to a requested
target. When the build completes, rmake removes eligible intermediates and
prints their names. For example, the [two-chain fixture](../tests/cases/chain-two-roots/)
expects `rm b.mid a.mid` in that order. To learn when files count as
intermediates, see the GNU Make manual's
[chains of implicit rules](https://www.gnu.org/software/make/manual/html_node/Chained-Rules.html).

## How the order is determined

The engine records a filename as it encounters it. For example, a goal from
the command line enters before built-in suffix names. Immediate prerequisites
enter before their target. Deferred prerequisites enter when expanded, and
accepted implicit rules enter their selected inputs. The order depends on when
each filename is encountered, including names from includes and target
variable declarations.

The table uses the pinned Linux filename fingerprint and linear probing.
A deleted slot stays marked so lookups can still reach keys beyond it. When
fewer than one in sixteen slots remain unused, the table reinserts live names
in their previous slot order. A merge keeps the existing destination slot. A
tentative VPATH lookup can change the lookup key while keeping the original
name for recipes and cleanup.

Cleanup walks the file table after active jobs finish. An expansion-only
recipe can create an eligible intermediate. A failed removal can leave a
partial `rm` line and emit an error. The engine preserves the mode-specific
behavior for dry runs, question mode, touch mode, quiet builds, and parallel
builds. See [rule resolution](rule-resolution.md).

## Check a filename order difference

From the rmake checkout, prepare the reference and run the curated comparisons.

```sh
make bootstrap
make test-diff
```

If the difference needs an implicit chain, include every input and rule needed
to choose it. Compare the cleanup line, any errors, and which files remain.
The curated cases cover collisions, table growth, scope-only declarations,
aliases, implicit chains, and files an attempted cleanup cannot unlink. See the
[testing guide](testing.md).

## Cost of storing filenames

A slot now stores names up to 15 bytes inline. Longer names use boxed slices.
The test for the boundary includes non-UTF-8 bytes, a cloned table, and renames
in both directions. Table growth and tombstones preserve the same lookup and
traversal order.

On the 10,000-target no-op benchmark, the inline slots removed about 10,000
allocator calls and increased requested bytes by about 175 KB because the
slots are larger. Alternating samples on two CPUs measured about 1.7% to 2.2%
less wall time for that workload. The distinct-declaration workload also
improved. The expansion workload took about 0.5% to 1% longer, and the small
no-op changes were within the range of the same-binary control. These
measurements include startup and reflect the selected workloads.

Repeat the core comparison with a previous rmake executable.

```sh
make bench-compare BENCH_SUITE=core BENCH_REPEAT=101 \
  BENCH_BASELINE=/absolute/path/to/previous-rmake
```

## Source references and original measurements

GNU Make 4.4.1's `src/hash.c`, `src/file.c`, `src/read.c`, `src/implicit.c`,
`src/main.c`, and `src/default.c` informed the table, registration, and cleanup
behavior. The Rust implementation and tests were authored independently.
Original GNU-only instrumentation recorded the numeric hash observations.
See [licensing and provenance](licensing.md).

The original filename index passed `make CANDIDATE=/tmp/rmake-record-final-debug ci` passed on this
source. All 4,456 oracle cases and 4,445 required candidate cases passed; eleven
fixtures were pending at the time. The 65 stateful scenarios / 287 invocations,
112 timestamp-fault programs, 61 jobserver cells, repeated 720-cell graph progress
campaigns for each engine and top-level self-host gates passed. Core checks passed
388 Rust tests and 285 tooling tests. Debug SHA-256:
`207b7291dfcfcf7095756cc552b4fe8e0be1c0b18915196a8270fb82a8129f73`.
Release SHA-256:
`04f6e172292bcbdae29912bb107ce1ad0c8bb0672198ae7242daf94f7c5a7656`.

A paired 101-sample core benchmark (two warmups, alternating engine order, warm
OS cache, exact status/stream checks) measured median milliseconds:

| Workload | GNU | This release | Previous main |
| --- | ---: | ---: | ---: |
| startup | 0.625 | 0.706 | 0.706 |
| 10,000 target no-op | 11.472 | 34.491 | 31.459 |
| expansion | 2.811 | 4.755 | 4.755 |

The ignored report `artifacts/bench/file-record-core-101.json` recorded the
samples and binary identities from the original comparison. The measurements
included startup and did not isolate the CPU.
