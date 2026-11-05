# Controlled JOBS output and jobserver ownership

Nine original GNU2-qualified profiles now pass the isolated JOBS development and
optimized builds. They check real child identities, exact event sequences and
external FIFO/pipe token availability. The same frozen builds also pass all 64
deterministic debug, 215 native-module and 40 directory-reporting profiles.
Formatting, strict Clippy, 578 Rust tests and 643 tooling tests pass. These are
bounded results; complete JOBS behavior and M3/M6 remain open.

A recipe owns an optional stable `Rc` record while its individual commands obtain
new PIDs. Its record survives multiline continuations and map movement. An
explicit newest-first live chain determines debug order independently of HashMap
iteration. Launch, dispatch, logical reaping and removal produce raw JOBS events;
implementation-only idle nonblocking graph polls do not invent visible reaping
passes. Events retain GNU's exact protocol spacing and bypass ordinary output
spools and directory-start hooks. The no-JOBS path allocates no job records or
chain storage.

`Child::id` reads the PID from its already owned process handle. The worker
protocol independently verifies each recipe process's PID, parent PID, Linux
starttime and command line before acknowledging it. The report retains every
raw PID and pointer. The strict model checks stable pointer relationships and
active uniqueness rather than equating separately allocated addresses between
programs. No arbitrary Live-line suppression, PID removal or broad stdout
normalization is permitted.

The first parallel candidate exposed an additional idle live scan and a real
pipe-authentication difference. Signal wake-pipe descriptors now reserve space
above fd4 during initial installation, before publishing globals or installing
handlers. The live wake pipe is never relocated, avoiding an in-flight handler's
retained-old-FD race on native or Guile threads. This permits the selected owned
pipe jobserver to use descriptors 3,4. Reserving the private pair adds two
initial duplicate/close pairs. The [separate measurements](benchmarks/job-debug.md)
retain their whole-process cost without isolating those operations. Broader
descriptor/resource profiles remain unqualified.

A separate functional witness found a jobserver ownership difference: after the
implicit-slot recipe finished, a surviving paid recipe retained the only shared
byte. Both FIFO and pipe witnesses failed with worker exit96 before the fix.
After completion leaves no active implicit lease, rmake moves one remaining paid
lease into the inherited slot and returns that lease's original byte exactly
once. No permit is cloned and no token is fabricated. The independent worker can
then borrow and return the shared byte while its recipe remains alive. A Rust
unit also checks original-byte conservation and absence of a duplicate return.
Logical Released diagnostics remain attributed to the completing recipe rather
than naively to the particular `Permit` object that held the byte.

`make test-job-debug` runs the strict nine-case gate;
`JOB_DEBUG_CASE=jobs-serial-multiline` selects an exact profile. Development,
optimized and focused self-host flows include the gate. The full protocol,
worker and archive index are described in
[tests/job-debug](../tests/job-debug/README.md).

[History](../tests/job-debug/history.json) retains all original discovery,
observer failures, raw GNU2 results, 30 repeated two-case GNU2 controls and every
candidate refinement. The missing-runtime-root observer failure and worker
ESRCH fixture failure are not GNU or candidate compatibility credit. Pre-transfer
failures remain complete and unchanged. The
[qualification archive](../tests/job-debug/qualification.json) retains both frozen
binary identities, complete eight qualification reports, coherent source/input
hashes and final helper/Makefile bytes. It excludes separate database and Guile
work in progress.

The ordinary ledger remains 27,766 required / zero pending across 158 families;
32 documented safety inputs / 128 separately gated cells are unchanged. These
separate JOBS profiles add no ordinary or exception credit. Full frozen local
acceptance and exact-head CI remain merge gates. General GNU performance parity,
persistent-cache correctness and external integration are not established by
these results. The [measurement page](benchmarks/job-debug.md) and
[performance archive](../tests/job-debug/performance.json) retain three timing
reports, sixteen allocation reports/traces and twelve exact inputs. Selected
paired elapsed changes range from -1.20% to +0.80%; all request/free counts are
unchanged. Requested bytes decrease by 117 on tiny/large/startup controls and
increase by four on runtime. Build/profiler-path effects and the new descriptor
operations are not isolated. Shared inherited CPU23 affinity applies only to
elapsed campaigns; there are no confidence intervals or general parity claims.
The additional archive-integrity unit passes separately, checking all three
compressed/raw hashes and raw byte counts; the original 643-tool qualification
remains intact and the next complete gate includes 644 tooling tests.

Open JOBS work includes additional transport/scoped/recursive/restarted and
loaded-module combinations, live debug changes, load waits, cancellation,
interruption, descriptor/resource limits, and wider scheduler identities.
Broader asynchronous/implicit/default-reader/database diagnostics, enabled
Guile/GC integration and complete developer-flow acceptance also remain open.
The qualified queued BASIC behavior is unchanged: GNU omits individual
asynchronous Success records in those profiles, so none were invented here.

## Localized JOBS profiles

`make test-job-debug-nls` and `make test-job-debug-nls-release` run the same nine
cases in C and verified French. Each locale has literal expected event text.
The existing live process, FIFO path, job address and token checks apply in
both locales. The French gate also checks the localized ignored-error suffix.

The checked formatter treats job addresses as opaque numbers. It formats
`%p` with the glibc Linux spelling and passes the target, PID text and
jobserver authentication bytes as separate arguments. The renderer looks up
the active catalog at each translated event. The token request and final
drain messages are English literals in GNU Make 4.4.1.

The message keys come from GNU Make 4.4.1's JOBS producers in
`src/{job,main,posixos}.c`. The French expectations come from the pinned
compiled French catalog. See the translator and license records in
[catalogs/provenance.json](../catalogs/provenance.json) and
[catalogs/COPYING](../catalogs/COPYING). The existing workers and observers
supply the independently verified identities and effects.

## Controller and load observations

`make test-job-debug-load` and `make test-job-debug-load-release` compare
owned, inherited and disabled controllers under verbose debug in C and French.
The reader marker fixes the controller's publication phase independently of
when descriptors are attached.

The load cases use an original preload fixture to supply fixed Linux runnable
counts or a fixed fallback average. They cover interrupted reads, malformed
bytes, errno detail, the first fallback sample and a disabled limit. The
literal load lines occupy declared positions in the existing JOBS event
protocol. The live-child and full filesystem witnesses still validate every
other event and effect. Fallback pressure advances at the first sample in a
new libc wall-clock second and accounts for actual child launches and reaps.
The raw load messages follow GNU Make 4.4.1's Linux `load_too_high` producer.
The numerical fields use the active libc numeric locale.
