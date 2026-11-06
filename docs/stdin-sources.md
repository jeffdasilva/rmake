`rmake` uses named files for stdin Makefile sources and recognizes the internal `--temp-stdin` restart operand. The same process restart path serves named-file and stdin invocations. The required profiles describe the accepted behavior for these boundaries; M3/M6 remain open.

The implementation validates the process temporary-directory choice before inherited option effects. `MAKE_TMPDIR` takes precedence over `TMPDIR`; invalid values produce the selected GNU warnings. A relative spelling remains relative when a later temporary is created after directory changes. Output spools share the retained choice. The selected controls cover ordinary invocation, explicit help/version, inherited effect ordering and a native constructor changing the environment after selection.

A stdin source is a real named file with mode0600. Transfer uses public libc stdin buffering, so native constructor reads and `ungetc` state are respected. The input is not retained as a second Rust byte vector. Creation, buffered copying, publication and unlink are separate lifecycle stages. Published sources retain their name for MAKEFILE_LIST and source locations, are marked successfully updated, and are unlinked when makefile remaking stabilizes before goal expansion. Fatal completion removes the published source before draining children or running native finalizers. Adopted raw filenames also survive early-help and constructor/error paths through an explicit exit owner.

The actual CLI decode records the first stdin argument's original span. A restart builds argv from that span, retaining short-option prefixes and surrounding arguments, and passes the rewritten span when replacing the process image. It does not decode, expand or replay an argument to reconstruct the span. The internal operand bypasses tilde expansion. Repeated stdin is the observed safe GNU fatal error. A copied source is published into both invocation and live flag state before command evals; later MAKEFLAGS changes cannot restore the old `-` filename. Real exec inherits live process environment changes made by native code while overriding the existing restart counters.

Named `-f`, `--file`, and `--makefile` operands retain their selected paths in
the same original argument coordinates. A restart can then replay command
assignments with a new `MAKE_RESTARTS` while rereading the first selected
Makefile. Short clusters and separated operands retain their shape, and full
long forms become attached `-f` operands. GNU leaves abbreviated long forms
in the original argument vector. Eight ordinary fixtures cover these forms,
multiple files, and literal and no-restart controls.

Two finite GNU quirks remain observable and receive no safety-exception credit. Restarting an abbreviated `--fi=-` invocation loses its input and leaves its first source file; the initial process's native finalizers do not run across exec. An early signal during a blocked, unpublished stdin copy also leaves its empty source. The tests bind exactly one source before exit through an independent host open/fstat/full-byte observation, then require that same device/inode/mode/full content and the complete final filesystem. They do not discard wildcard filenames or normalize output. Every other selected filesystem is static. This matches observed GNU behavior rather than treating any candidate temporary as acceptable.

`tools/stdin_sources.py` exposes `--oracle`, `--candidate` or `--oracle-only`, `--report`, repeatable exact `--case` and `--cc`. Original modules compile once per case before identical GNU2/candidate resets. Forty-four profiles cover the initial four stdin/restart combinations, option forms and retained cluster policies, raw manual paths, help/version cleanup, temporary-directory priority, libc prefetch/consumption, reader failure, publication phases, and native environment transfer. Eight tooling controls reject shared wrong streams/status/effects, extra files, absent/late/replaced identities and incorrect containment; failing candidates always produce a nonzero separate report.

The first source-complete development checkpoint passed42/42,582 workspace Rust tests and strict all-target Clippy. The next development checkpoint and its optimized build both passed44/44, including two new independently prewritten controls that failed the first checkpoint. The final source passed582 workspace Rust tests, strict all-target Clippy and14 tooling controls (eight source-gate and six legacy-ownership controls). The original GNU2 and preceding-candidate records remain unchanged. These are isolated checkpoints; integrated-head, broader regression and performance evidence must be recorded separately.

The legacy cancellation gate has21 early-copy stdin cells across seven signals and three output modes. Its original memory-backed candidate policy forbade owned temporary files and recorded GNU names outside the ordinary target snapshot. That historical scope and observations are retained. A fresh independent GNU2 qualification and both44-profile development/optimized candidates pass all21 under stronger pre-signal physical-identity/full-filesystem requirements. The updated controller retains all other phase, stream, status and owned-child checks; arbitrary candidate leaks are still failures.

The [three process restart profiles](../tests/process-restarts/README.md) reproduce differences hidden by an in-process reread. The corrected runtime checks the temporary directory again, runs loader constructors before the next Makefile read, and follows a replacement of the invoked executable. Two GNU runs and both frozen rmake builds pass each case. The preceding candidate fails all three. The corrected optimized candidate also passes all 27,766 ordinary cases, and both builds pass the 44 stdin, 41 database, and 14 restart lifecycle profiles. The observer refinement requires both actual advertised private jobserver descriptors to retain their original pipe identity and close-on-exec flags at cleanup.

[Three raw environment controls](../tests/restart-environment/README.md) require
ordered duplicate and unkeyed entries, non-UTF-8 bytes, synchronous native edits,
and exact duplicate or wrapped restart-counter behavior. The Linux adapter
owns the vector passed to real exec, preserving live PATH search and signal
state. Both corrected builds pass; the preceding build fails each case.

[The inherited pipe profile](../tests/restart-pipe/README.md) requires both
process images to see the same incoming jobserver pipe, with the advertised
descriptors private again after startup. The exec boundary verifies their
physical identity and access roles. An independently rebound descriptor must
remain close-on-exec. The existing failed-exec observer still checks cleanup.

Further descriptor/read/write/close faults, internal-option combinations, pathname mutation, database/debug interactions, enabled Guile, and broader resource profiles still need qualification. General performance parity is unproven. The selected corpus and typed observers add no ordinary-ledger cells or safety exceptions.
