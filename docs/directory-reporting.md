# Directory reporting

`-w` / `--print-directory` and `--no-print-directory` now select ordered,
origin-aware directory policy. Explicit CLI selection survives later file flag
assignments. Automatic policy observes current silence, consumed `-C` operands
and the published startup recursion level; it does not add `w` to MAKEFLAGS.
Late `-s` can disable automatic entry while an already-emitted entry still earns
its leave message.

One process output owner spans decoding, construction, reading, restarts and
native finalization. It keeps selection separate from actual entry and terminal
closure. Managed output before directory/level publication can report an unknown
directory at level zero. Later publication updates the directory snapshot rather
than reconstructing it after cwd restoration. None and an explicitly empty
snapshot remain distinct, and repeated snapshots reuse storage.

Unsynchronized and recurse output enter lazily at managed output or actual child
launch. The qualified quiet-target boundary distinguishes `@:` (no child/pair)
from `@true` (a child/pair). Line/target synchronization brackets nonempty spool
dumps, including stderr-only groups. Header, payload and footer share the same
coordination lock; empty buffers stay silent. Serial reading retains its earlier
buffer mode until the reader dump finishes, before the global serial downgrade.

Restarts share the process owner or export an actually-entered trace through
negative MAKE_RESTARTS. Exec itself emits no leave. Process cleanup emits the
owed leave before restoring the original OS cwd; native finalizer output cannot
reopen the closed trace. A directory message also flushes pending native C stdout
before the raw descriptor write, including entry triggered by stderr output.

## Qualification and remaining work

`make test-directory-reporting` runs 40 independent models against GNU Make
4.4.1 twice and the candidate. Both frozen builds pass all 40; the preceding main
passes four and differs on 36. The models check exact streams/status, ordered
witnesses, repeated reference agreement, independent full filesystem expectations
and process/resource bounds. Path slots resolve from the predetermined reset
workspace, never observed output. The single version slot is calibrated in an
isolated invocation and cannot swallow directory boundaries. Only the two native
finalizer profiles invoke the compiler. Incorrect first predictions and complete
observations remain unchanged under `tests/directory-reporting`.

All 215 earlier native models pass in both builds and remain unchanged. Existing
imported-trace and startup-exit gates pass 482 and 128 profiles; approved safety
coverage remains 32 inputs / 128 cells. Rust has 571 passing tests, including
policy/origin, dynamic silence, terminal closure and independent-process lock
checks for both spool callbacks. All 623 tooling tests, formatting and Clippy
pass. The ordinary inventory stays 27,766 required / zero pending across 158
families. The separate directory gate adds no ordinary or exception credit.

The top-level Makefile requires directory acceptance in development, optimized
and focused self-host flows. `DIRECTORY_REPORT_CASE` selects a case. Full frozen
rmake-driven acceptance and exact-head development/optimized/aggregate CI remain
merge requirements.

[Both performance versions](benchmarks/directory-reporting.md) retain regressions
and the remaining allocation cost. They establish no general GNU parity.
Database/debug formatting interactions, broader early-output, path, descriptor,
stdio, restart, locale/Linux and resource profiles remain open. Enabled Guile/GC,
integration, persistent caching and complete M3/M6 retain their separate gates.
