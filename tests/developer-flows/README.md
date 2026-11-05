# Developer entry points

`make test-developer-flows` runs the actual top-level Makefile and scripts
with GNU Make 4.4.1 twice and with rmake. Both engines are invoked under
one `make` basename; executable naming is otherwise observable in diagnostics.
The gate copies the Makefile and its scripts into an owned disposable fixture.
It uses the pinned compiler as an input and a private offline Cargo cache.

The fixture runs warm oracle bootstrap, six real Cargo clean modes (including
parallel pipe jobserver and `-n`, `-q`, `-t` guards), and three benchmark
preconditions. `clean` checks a real compiled target, preserves unselected
and neighboring targets, and never runs against the original checkout. The
benchmark cases require the real oracle and release prerequisites, exact
failures, and no result file. The positioned Cargo completion time is the
only variable transcript field. No latency is collected by this gate.

This gate needs a provisioned pinned Rust toolchain and verified GNU oracle.
It is separate from cold downloading/installing and timing benchmarks.
Use `DEVELOPER_FLOW_CASE=clean-parallel-pipe` for one case.
