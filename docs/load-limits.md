# Linux load limits and child stdin leases

The M5 runtime implements `-l`, `--load-average` and `--max-load`, including
recursive propagation, explicit child overrides, bare-option reset, C floating
prefix parsing and `%g` flag spelling. Negative values disable gating without
losing their spelling (except the default -1); NaN also disables the comparison.
These observations are invocation-local, never persisted or used as graph facts.

The interpreter expands all recipe bodies before waiting for a local slot or
jobserver token. Shell configuration and exports remain deferred until dispatch.
A load-delayed recipe retains its token and expanded continuation, but owns no
process or stdin lease. Delayed recipes resume in LIFO order on scheduling
activity or child completion. The first active command always runs, allowing
progress even with a zero limit. Ordinary abort discards queued work and returns
its permits; keep-going and ignored errors let that work continue. Running recipes
retain their existing ordinary-failure continuation policy.

On Linux the primary observation is the instantaneous runnable count in
`/proc/loadavg`, compared strictly greater than the limit. The cached descriptor
is rewound and reread for each observation. Interrupted operations retry; invalid
or failed observations select libc's load average with recent-launch pressure.
Repeated fallback errors report once per error number. No load observation is
reused after a child or scheduling decision as a supposedly permanent fact.
Load changes alone do not wake an ordinary child wait. A blocked jobserver wait
with load-delayed recipes has GNU's one-second retry; token arrival still wakes
immediately. A controlled wait-entry load transition proves this retry rather
than relying on a host load change or a completion sleep.

Stdin belongs to one child command at a time. On completion another target's
continuation can acquire it. Other commands receive an owned read pipe with its
writer closed, preserving both EOF and the FIFO descriptor type. Descriptor
creation is atomically close-on-exec, including concurrent worker launches.

## Evidence

33 original required fixtures cover numeric/recursive flags, serial load queues,
expansion-before-token acquisition, stdin handoff, EOF and descriptor type.
The four stdin fixtures passed 20 repeated oracle and candidate executions each;
the previous release fails the handoff/type observation. Load fixtures use bounded
expansion barriers rather than a completion sleep.

`make test-load` runs 21 native controlled profiles three times against both GNU
and rmake: above/equal/below load, integer boundaries, disabled limits, load changes
after a peer completion, EINTR, malformed/read/open fallbacks, unsupported/error
sampling, and queued ordinary/ignored/keep-going failures. A test-only original
LD_PRELOAD helper supplies system observations, never Make behavior. Explicit
concurrency/zero-active-worker, exact stream/status, queue/membership and journal
anchors reject two engines agreeing on an incorrect result. Only the inherently
unordered parallel completion journal uses a named membership validator; its raw
order is retained. All other file observations remain exact. Four tooling tests
exercise these anchors with deliberately corrupted results.

The campaign is in development CI and the optimized regression flow. This is an
M5 slice, not M5 completion. Signals/cancellation, output synchronization, deeper
synthetic-fatal completion and broader mixed-tree/runtime stress remain open.

## Reference study

Consulted the verified GNU Make 4.4.1 source archive, SHA-256
`dd16fb1d67bfab79a72f5e8390735c49e3e8e70b4945a15ab1f81ddb78658fb3`:
`src/job.c` (`new_job`, `start_waiting_job`, `start_waiting_jobs`, `reap_children`,
`load_too_high`), `src/main.c` (floating option parsing and flag rendering),
`src/misc.c` (`make_toui`) and `src/posixos.c` (`get_bad_stdin`). The Rust state
transitions, owned descriptor adapter, fault helper and fixtures are original;
no GNU implementation or test material was copied or translated.

## Performance observation

Paired, alternating optimized runs used two warmups, 101 core samples and 31
parallel-branch samples per engine. No other local functional gate ran during
measurement. Median milliseconds (GNU / current / prior main): tiny no-op
0.677 / 0.805 / 0.804; 10,000 no-op rules 11.567 / 32.329 / 31.737; expansion
2.930 / 4.414 / 4.466; parallel depth one 88.187 / 89.582 / 89.575; depth four
88.310 / 89.395 / 89.341. These selected observations show roughly unchanged
runtime cost, not general parity or an acceleration claim. There was no CPU
isolation or CPU/RSS/I/O acceptance threshold. Raw observations and executable
identities remain in ignored `artifacts/bench/m5-load-{core-101,parallel-31}.json`.
