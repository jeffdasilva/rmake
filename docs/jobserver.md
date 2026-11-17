# Share parallel jobs across recursive builds

Run `rmake -j4` to allow up to four concurrent jobs. When a recipe invokes
another Make process using `$(MAKE)`, the parent and child can share that slot
limit. rmake and GNU Make can take either role in the build.

```sh
rmake -j4
rmake -j4 --jobserver-style=pipe
```

The FIFO jobserver is the default on Linux. Use the pipe option when a
cooperating process expects the pipe transport. Unlimited `-j` has no slot
limit. See [parallel execution](https://www.gnu.org/software/make/manual/html_node/Parallel.html)
and [the POSIX jobserver protocol](https://www.gnu.org/software/make/manual/html_node/POSIX-Jobserver.html)
in the GNU Make manual for the public options and protocol.

## Recursive invocation

`MAKEFLAGS` passes job slots and authentication to children. A child with an
explicit job count chooses its own pool and prints the GNU-compatible warning.
If a child cannot use its inherited pool, it falls back to one job.
With the pipe transport, Make passes the live descriptors to recursive or
forced recipes. The GNU manual explains
[how to invoke Make recursively](https://www.gnu.org/software/make/manual/html_node/MAKE-Variable.html)
and [how tools share job slots](https://www.gnu.org/software/make/manual/html_node/Job-Slots.html).

## Run the jobserver tests

From the repository root, run the mixed-engine comparisons.

```sh
make bootstrap
make test-jobserver
```

The campaign uses both transports and all four parent and child engine pairings.
A counter spans six recipe workers, checks that three can run together, rejects
oversubscription, and must finish at zero. The campaign also covers recursive
failure with keep-going, borrowing a pool, and returning tokens.

The gate compares exact streams, statuses, and final filesystem observations.
It checks child cleanup and temporary FIFO cleanup. Its 61 recorded checks
include an independent rmake control for preserving distinctly valued tokens.
GNU 4.4.1 returns `+` for those tokens, so the control records rmake's token
preservation policy separately. See the [testing guide](testing.md) for
replay and report details.

## Linux resource ownership

A process has one implicit slot. Additional running targets acquire one byte
each from the shared server. The owned permit stays with a recipe through
continuations and recursive commands. The adapter returns the acquired byte
after completion or a handled failure. Borrowed pools keep their existing
identity and are never unlinked. The Linux adapter creates a private FIFO
and removes it only if the path still names that FIFO.

Pipe mode uses procfs to open a nonblocking reader while preserving the blocking
flags shared with GNU clients. It verifies that both inherited descriptors
refer to the same pipe before using them. Child completion and token arrival
both wake the event loop. Linux systems without pidfd support use bounded
readiness polling, and very large pools can exceed pipe capacity during
initialization.

[Signal shutdown](runtime-signals.md) documents cancellation, changed-target
cleanup, and token behavior on interruption. GNU 4.4.1 can lose acquired
borrowed tokens on fatal exit. rmake returns those tokens during its handled
shutdown path. The [compatibility audit](m3-m6-audit.md) tracks remaining
resource and recursive jobserver combinations.
