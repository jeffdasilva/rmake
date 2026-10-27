# Owned children and Linux signal shutdown

The interpreter now owns one Linux child handle and one reaper per command.
It polls child readiness, jobserver token availability and a signal wake pipe
together. Ordinary pidfd-capable execution creates no waiter thread per command
and imposes no fixed delay on returned external tokens. Older or restricted
kernels retain unique Child ownership and use a five millisecond readiness
fallback. A reaped handle never signals a reused PID. Dropping a live owned child
kills and reaps it before releasing ownership; ordinary completion is already
reaped. Private wake/pidfd descriptors are close-on-exec and stay above stderr.

The handler for HUP, INT, QUIT, PIPE, TERM, XCPU and XFSZ records the signal,
restores its default disposition and wakes the interpreter using an errno-safe
nonblocking pipe write. No allocator, mutex, diagnostic, filesystem traversal,
child wait or permit destructor runs in a handler. A different subsequent fatal
signal is handled as a new shutdown phase; a repeated copy takes its default
action. Explicit inherited ignore dispositions remain ignored. A small ELF
startup hook remembers inherited SIGPIPE before Rust's runtime changes it.
Child setup restores an explicit inherited SIGPIPE ignore after Rust's spawn
setup, including recipes, shell capture and executable-format fallback.

TERM is forwarded to local children. HUP/INT/QUIT rely on group delivery; sending
one of those to make alone does not arbitrarily kill its children. Changed
nonprecious regular targets, grouped peers and archive warnings follow GNU's
cleanup policy. Stopped recipes do not start another command, even with ignored
errors or keep-going. Ordinary child-only failure still lets an already-running
peer finish its continuation. Signal names come from libc and core-dump status
retains its diagnostic suffix. Invocation QUIT returns 1; other caught fatal
signals are re-raised after normal resource cleanup, preserving wait status.
Signal intermediate removal uses stderr and its distinct diagnostic. Every wait
keeps the wake pipe visible, including after an earlier interrupt. The interpreter
drains and resamples pending signals, avoiding a lost subsequent signal between
state inspection and entry into poll.

## Original regression gates

`make test-runtime-signals` runs GNU self-comparison and candidate comparison.
337 controlled profiles and 36 borrowed-pool safety profiles each repeat three
times. There is also an optimized candidate gate. These cover all seven signals;
parent/group/child delivery; serial and parallel execution; FIFO and pipe pools;
precious, phony, unchanged/same-mtime, directory, symlink, grouped and archive
targets; ignored failures; inherited ignore; final commands; one-shell exec;
forced dry-run/query/touch execution; interrupted load queues; token-blocked
preparation; and different subsequent fatal signals. Two live children use
explicit release/reap barriers, distinguishing fatal cancellation from ordinary
peer continuation. Existing mixed-engine and forty-repeat graph campaigns stay
mandatory. Forty-nine curated Makefiles separately check real child-only signal
failures, ignored/silent diagnostics and output cleanup. No GNU tests were used.

Helpers explicitly disable kernel core dumping for stable native inputs. A
separate Rust status test checks diagnostic rendering with and without the core
flag; original local native probes also observed GNU's core-dump suffixes. The
controller waits for readiness and shutdown acknowledgments before releasing a
live child. Staged group delivery still sends the signal to the entire group;
the helper records it and re-raises it after the cleanup/wait acknowledgment.
Exact streams, status, file contents/types/modes/links, protected outputs, fixed
timestamps, process containment and temporary-resource removal are checked.
No signal output is sorted, stripped or normalized. Six tooling tests corrupt
these anchors and verify borrowed resource identity and distinctive token bytes.

Two GNU behavior boundaries are explicit:

- GNU 4.4.1 clears borrowed jobserver descriptors before fatal reaping and loses
  acquired token bytes. The safety profiles record that control observation;
  rmake must instead return every original byte and retain the borrowed resource.
  This difference does not exempt stream, status or filesystem comparisons.
- An unstaged simultaneous group-exit probe observed GNU reenter a wait after
  consuming a child's status, report ECHILD twice and exit 2 instead of by INT.
  The local reproducer/report is retained in ignored artifacts
  (`signals-be7lfksc`); it is not a passing equivalence result. The original
  input remains reproducible with `make test-runtime-signals
  SIGNAL_CASE=sigint-group-j1-pipe-changed SIGNAL_REPEAT=100 SIGNAL_UNSTAGED=1`
  (the command is expected to fail if the reference race occurs). Required tests
  stage termination after signal delivery to avoid that reference race. rmake
  keeps its reaper outside handlers rather than reproducing reentrant wait state.

These trusted process fixtures are not a security sandbox. This is an M5 slice,
not its exit claim. Output synchronization, broader shell/reader interruption,
synthetic fatal continuations and final runtime/mixed-tree stress remain open.
The second-signal profiles also passed 100 repetitions each after the lost-wake
fix. A separate load profile changes the observed load exactly at entry into an
unavailable-token wait, checking GNU's one-second queued-load retry; the prior
reactor fails that profile.

SIGKILL and a repeated fatal signal taking its default action cannot promise
normal resource cleanup.

## Source references

Consulted the verified GNU Make 4.4.1 archive, SHA-256
`dd16fb1d67bfab79a72f5e8390735c49e3e8e70b4945a15ab1f81ddb78658fb3`:
`src/main.c` (fatal signal setup), `src/commands.c` (`fatal_error_signal`,
`delete_child_targets`, `delete_target`), `src/job.c` (`reap_children`,
`child_error`, `free_child`), `src/file.c` (`remove_intermediates`) and
`src/posixos.c` (`jobserver_clear`). Linux pidfd/poll and POSIX signal effects
are confined to owned adapter APIs; application and core retain unsafe-code
forbid. The reactor, handler bridge, ownership model, fixtures and controller are
independently written; no GNU implementation or tests were copied or translated.

## Performance observation

Final paired optimized measurements used 101 alternating samples per engine and
two warmups, with no local functional campaign running concurrently. Median
milliseconds (GNU / current / preceding load/stdin slice): tiny no-op
0.550 / 0.639 / 0.635; 10,000-rule no-op 11.482 / 31.126 / 31.558; expansion
2.898 / 4.385 / 4.398; parallel depth one 88.045 / 88.855 / 89.579; depth four
88.025 / 89.032 / 89.671. These selected costs are approximately unchanged or
slightly lower; they do not establish general parity. No CPU pinning/isolation,
CPU/RSS/I/O measurements or acceptance threshold was used. Raw samples and
binary identities are retained in ignored
`artifacts/bench/m5-signals-final-{core,parallel}-101.json`.
