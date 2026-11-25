# Limit parallel jobs by system load

Use `-l` together with `-j` to cap how much work a build starts at once.
For example, allow up to eight jobs and wait to start additional jobs while
the measured load exceeds four.

```sh
rmake -j8 -l4
```

The first active command can start even at a zero limit. A bare `-l` clears an
inherited limit. Recursive builds inherit the setting through `MAKEFLAGS` and
can override it. See GNU Make's
[parallel execution guide](https://www.gnu.org/software/make/manual/html_node/Parallel.html)
for `-j` and its [option summary](https://www.gnu.org/software/make/manual/html_node/Options-Summary.html)
for `-l`, `--load-average`, and `--max-load`.

## Job startup and standard input

rmake expands the recipe before waiting for a slot or a jobserver token. A
recipe delayed by load holds its token and expanded commands. It starts its
shell and acquires standard input when the scheduler dispatches the job.
Queued work resumes in last-in, first-out order after relevant scheduling
events. In a blocked jobserver wait with load-delayed work, the scheduler also
retries after one second.

On Linux, rmake rereads the instantaneous runnable count from `/proc/loadavg`.
If the read fails or its contents are invalid, rmake uses libc's load average
and accounts for recent launches. A repeated fallback error is reported once
per error number. Load observations apply to the current scheduling decision.

One active command owns standard input. Other commands receive the read end
of a pipe whose writer is closed, so reads return EOF. The next command may
use standard input when the owner finishes. Read how recursive jobs
[share slots](jobserver.md) for process and token ownership.

## Evidence

Run the load and Makefile-remake comparisons from the repository root.

```sh
make test-load
make test-remake-parallel
```

`test-load` tests 21 controlled load profiles, replayed three times under
both engines, along with the localized fallback cases. The runner uses an
original test-only shim to supply load observations, including failed,
interrupted and malformed reads. The tests check actual recipe overlap,
queue order, exit status, output, token behavior, and filesystem effects.
When a queued executable fails to launch, a stopping pass returns the other
queued permits before those recipes dispatch. A keep-going pass can still
run the remaining queued recipes. The controlled pipe and FIFO cases check
the completion journal and filesystem for both outcomes.
When parallel completion order is inherently variable, a specific membership
check compares that journal and the report retains its original order.

The tracked load and stdin Makefiles also cover numeric option spellings,
recursive overrides, expansion before token acquisition, stdin handoff, EOF,
and the descriptor type. The stdin fixtures require the next real command to
receive the input after the first command finishes. See the
[M5 execution audit](m5-exit-audit.md) for how these checks fit with cancellation,
output synchronization and recursive runs.

## Verbose named includes

When a Makefile is also a command-line goal, dry-run, question, and touch can
affect its remake visit. The [held remake gate](../tests/remake-parallel/README.md)
starts real workers, waits for the specified Make process state, and compares
the exact output visible before each release. It also checks final output,
status, file contents, mode and deletion.

| Inputs | What the cases check |
| --- | --- |
| Required, optional, and directory-qualified includes | Which source is visited first, which recipe runs in the requested mode, and when a completed pass can restart. |
| High, low, falling, and absent load limits | When a queued source can start, whether an active primary worker can complete, and where an ordinary goal observes the result. |
| Built-in rules and stock primary Makefiles | Explicit source recipes, a primary with an empty recipe, a primary whose implicit search settles unchanged, and qualified implicit primary recipes. |
| Named tails and unnamed earlier sources | Required and optional positions, several named-source counts, pipe and FIFO transport, and two, three, four, or unbounded job counts. |
| Basic and verbose debug | The first and later visit, cached results, current worker identity, release order and restart output. |
| Phony or unconditional double-colon sources | When a missing required source prevents a restart and when another physically changed include still causes a reread. |

A simulated recipe can finish without reaping a child. A later pass then has
a new set of considered files, and a cached question result can surface again.
An optional source can complete its pass before a queued sibling starts.
The [M3/M6 audit](m3-m6-audit.md) tracks broader scheduling and debug
combinations. One open group includes unheld high-load races where GNU's
final files varied across runs with the same primary release.

## Fallback diagnostics

The load probe warns using the active message catalog and libc error text.
Four French profiles cover an unsupported probe and an I/O error through the
pipe and FIFO jobservers. The checks retain exact probe and queue order, and
compare the full streams against the C and French expectations.

## Reference study

The reference is the verified GNU Make 4.4.1 archive, SHA-256
`dd16fb1d67bfab79a72f5e8390735c49e3e8e70b4945a15ab1f81ddb78658fb3`.
Source references include `src/job.c` for job dispatch, reaping and load
checks, `src/main.c` for option parsing and skipped-source restarts,
`src/misc.c` for integer conversion, and `src/posixos.c` for the EOF
descriptor. The Rust implementation, shim and fixtures are original.

## Performance observation

The original load change used alternating optimized runs with two warmups,
101 samples per core workload, and 31 per parallel workload.

| Workload | GNU Make median (ms) | rmake with change (ms) | Previous rmake (ms) |
| --- | ---: | ---: | ---: |
| Tiny no-op | 0.677 | 0.805 | 0.804 |
| 10,000 no-op rules | 11.567 | 32.329 | 31.737 |
| Expansion | 2.930 | 4.414 | 4.466 |
| Parallel branches, depth 1 | 88.187 | 89.582 | 89.575 |
| Parallel branches, depth 4 | 88.310 | 89.395 | 89.341 |

These are historical wall times on a shared host. See the
[benchmark guide](benchmarks/README.md) to repeat a workload.
