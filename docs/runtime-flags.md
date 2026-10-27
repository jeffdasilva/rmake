# Recursive runtime flags

GNU Make 4.4.1 interprets a global `MAKEFLAGS` assignment immediately. Changing
`GNUMAKEFLAGS` instead takes effect during post-read option replay. rmake now
models both phases explicitly. The reader expands the assigned value, including
nested `eval` effects, before invoking the host option receiver. The receiver
updates switches, include search, output mode and variable precedence without
holding a mutable host borrow across expansion. Reborrowed expansion contexts
preserve both this receiver and cooperative cancellation.

Canonical `MAKEFLAGS` retains argument spelling such as `-j2`; jobserver capacity
is a separate scheduler property. Explicit command-line job counts win during
final replay, while immediate assignment effects remain visible during reading.
Appending flags reconstructs `MAKEOVERRIDES` once. Recursive option data quotes
literal dollar signs, so include paths survive subsequent expansion. Repeated
makefile reads restore startup options before interpreting the new database.
File-added switches cannot leak backward into a restarted reader.

Borrowed jobservers are joined before reader shell functions. Owned pools are
created after reading. Inherited pipe descriptors become close-on-exec, and
nonrecursive commands advertise unavailable pipe authentication before the
encoded command-variable suffix. Recursive commands retain usable authentication.
FIFO identity and pipe descriptor identity are validated by original helpers.
Output buffers remain lazy when synchronization is disabled; runtime `eval`
can enable a buffer during recipe or shell-configuration expansion without
introducing a default-path spool or an automatic drop-time output flush.

The requirement ledger adds 168 strict original Makefiles: 84 switch cases,
24 assignment/precedence phases, 24 recipe-eval cases, four immediate include
search cases, eight restart cases and 24 pipe/FIFO protocol cases. Two historical
warning-expansion pending cases are now required. Cancellation campaigns also
interrupt shell functions inside immediate `MAKEFLAGS` interpretation and
post-read `GNUMAKEFLAGS` replay. Raw streams, status and target-file effects stay
under the normal exact comparator; opaque descriptor identities are validated
by the helper rather than normalized in captured output.

This closes the M5 runtime switch tranche. Assignments newly introduced inside
a changed flag payload, broader command-line ordering, and remaining special
variable metadata are M3/M6 work. They are not claimed as complete compatibility.
See the [combined audit](m5-exit-audit.md).

Reference study used `src/main.c`, `src/variable.c`, `src/read.c`, `src/job.c`,
`src/output.c` and `src/posixos.c` from the verified GNU Make 4.4.1 archive,
SHA-256 `dd16fb1d67bfab79a72f5e8390735c49e3e8e70b4945a15ab1f81ddb78658fb3`.
The Rust state model, implementation, protocol helpers and fixtures were
independently authored. No GNU implementation or upstream test code was copied
or translated.
