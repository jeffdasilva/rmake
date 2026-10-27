# Cooperative Linux interruption and completion

Core evaluation accepts an optional atomic cancellation flag through its explicit
expansion context. Reborrowing preserves it. Reader lines, nested expansion and
effect boundaries stop before later definitions, exports or filesystem effects.
The platform handler only stores a flag and wakes the interpreter; allocation,
cleanup, token return, diagnostics and owned-child reaping happen outside it.

Shell capture now owns its child and stdout descriptor through the same Linux
process adapter as recipes. The reactor watches both output readiness and child
exit, including a child that closes stdout before it exits. Cancellation or a
capture limit kills and reaps that owned child. Pidfds prevent signalling a
reused PID; the existing unreaped-child fallback retains unique PID ownership.
Captured and private output descriptors remain close-on-exec.

Source, include, stdin and `file` effects stop retrying interrupted I/O after a
fatal signal. Pipe/FIFO writes use bounded libc chunks so a positive partial
write cannot hide cancellation inside an unbounded libc retry. Regular-file
writes retain their original libc call and deferred-close error behavior.
Cancelled streams discard userspace buffers before closing, rather than retrying
a blocked flush after consuming its signal.

Output-lock interruption distinguishes reader output from a completed recipe
owner. Reader interruption retains signal exit. GNU reenters ordinary fatal
shutdown when a completed owner is interrupted while dumping; rmake reproduces
its diagnostic order, changed-target deletion, lost buffered output and explicit
exit 2 through interpreter state. It does not signal or wait on an unowned PID.
The synthetic failed-launch owner also accounts for active peers. Deferred launch
failure diagnostics retain their recipe buffer until that logical completion.

`make test-runtime-effects` runs three original native campaigns, each with three
paired repetitions: 294 expansion/capture/I/O interruption profiles, 76 actual
fcntl contention/release/interruption profiles and 64 inherited-PTY profiles.
The latter cover stdin terminal identity, stdout/stderr diversion, forced lines,
serial versus parallel modes, default tty spellings and explicit empty imported
`MAKE_TERMOUT`/`MAKE_TERMERR`. Both engines use the same PTY; raw CRLF bytes and
paths are compared exactly. These are inherited terminals, not interactive
controlling-terminal job-control sessions.

The required corpus adds 96 exact-byte completion regressions: all pairs of
real/no-child/missing commands with active peers, keep-going and all four output
modes, plus unbuilt intermediate/barrier cases at one, two and four jobs. Twelve
normal-capture cases anchor large reads, EOF-before-exit status and descriptor
ownership. Six Rust tests and six tooling tests protect cancellation propagation,
no later effects, capture lifetime and independent campaign anchors.

Two safety differences are explicit. GNU can orphan an unregistered shell
function child when interrupted; the controller records that live child and
terminates its exact pidfd. rmake must reap its owned capture before exit. GNU
can also leave a randomly named stdin-Makefile temporary on early interruption;
the campaign records those observer temporaries separately, while rmake keeps
its existing memory-backed input and must leave no owned temporary. Neither
exception normalizes target files, streams or status.

Development and optimized CI run these campaigns alongside the existing corpus,
stateful, syscall-fault, signal, load, mixed-jobserver and graph-progress gates.
The [M5 exit audit](m5-exit-audit.md) combines these with recursive flag
interpretation and the other runtime gates. Full GNU compatibility and general
speed parity remain later acceptance goals.

Reference study: the verified GNU Make 4.4.1 archive, SHA-256
`dd16fb1d67bfab79a72f5e8390735c49e3e8e70b4945a15ab1f81ddb78658fb3`,
`src/job.c`, `src/main.c`, `src/commands.c`, `src/output.c`, `src/posixos.c`,
`src/function.c` and `src/read.c`; Linux/glibc file, pipe, poll, pidfd and stream
interfaces. The state model, Rust implementation, helpers and fixtures were
independently authored. No GNU implementation or upstream test code was copied
or translated.
