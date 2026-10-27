# Fatal configuration after child completion

This M5 follow-up implements a retained-recipe-owner shutdown phase, separate
from ordinary child failure and keep-going. GNU Make 4.4.1 evaluates SHELL,
.SHELLFLAGS and IFS when preparing each command, after expanding all recipe
bodies. A configuration error after a completed child can leave the GNU recipe
owner registered while its original process has already been reaped.

The interpreter now records that ownership independently of query cleanup and
of the latest launch attempt. An initial failed launch does not register an
owner, even if a later command starts. Optimized `:` commands do not register
one. A later failed launch retains an existing owner. Existing cleanup policy
still distinguishes successful children from failed launches.

The first fatal report begins shutdown. Running sibling recipes finish their
queued commands, then the retained owner produces the pinned terminal wait
error. A later fatal report while already draining interrupts that final wait;
it does not add another waiting message or an ECHILD diagnostic. Ordinary
unignored failures start draining only when keep-going is disabled. These paths
preserve output order and do not start unrelated graph work after abort.

A retained owner's synthetic failed-launch completion uses the observed current
recipe location and ignore policy. The covered cases preserve GNU's terminal
Error 127 rather than reporting ECHILD. Terminal recursive fatal shutdown skips
intermediate removal, which GNU never reaches on that path. Initial failures
without an owner keep ordinary cleanup. Fatal makefile-remake errors stop before
another restart, even if an earlier prerequisite was successfully rewritten.
Archive touch/cleanup errors share this shutdown mechanism; their existing
required metadata and I/O tests stay strict.

## Evidence and scope

Promote `continuation-configuration-after-child` from pending to required and add
69 original required Makefiles: SHELL/flags/IFS; child/no-child/eager preparation;
serial/parallel/dry/query/touch modes; keep-going; surviving and later-fatal peers;
first/later failed launches; intermediate cleanup; and required/optional makefile
remaking. Barriers have bounded waits; merged output cases anchor exact ordering.
No GNU tests were copied. `runtime-fatal-transitions` adds twelve stateful runs
with fixed timestamps, protected output, query deletion, precious files and
recovery across mode/configuration changes. Four core tests protect shutdown and
owner transitions independently of the process runner.

The fixture uses GNU's pipe jobserver for fatal-owner probes because the pinned
GNU default-FIFO path can leak a PID-named FIFO. rmake continues to release owned
resources and reap its workers. No comparator normalization or new pending
exemption was introduced. Product Rust still forbids unsafe code.

The [runtime completion follow-up](runtime-cancellation.md) adds the active-peer,
failed-launch, all-output-mode and unbuilt-intermediate/barrier regressions that
were open at this tranche. Its required cases close those tracked runtime gaps.
M5's remaining position is recorded in the implementation plan and progress log.

## Reference study

Consulted the verified GNU Make 4.4.1 source archive, SHA-256
`dd16fb1d67bfab79a72f5e8390735c49e3e8e70b4945a15ab1f81ddb78658fb3`:
`src/job.c` (`start_job_command`, `start_waiting_job`, `reap_children`, failure
handling) and `src/main.c` (`die`). The Rust state model, helpers, fixture sources
and all anchors are independently written; no upstream implementation or tests
were copied or translated.
