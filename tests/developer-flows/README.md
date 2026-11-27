# Developer entry points

`make test-developer-flows` runs the actual top-level Makefile and scripts
with GNU Make 4.4.1 twice and with rmake. Both engines are invoked under
one `make` basename; executable naming is otherwise observable in diagnostics.
The gate copies the Makefile and its scripts into an owned disposable fixture.
It uses the pinned compiler as an input and a private offline Cargo cache.
The read-only flows compare the top-level help and verify that doctor invokes
the pinned Cargo and oracle in order without changing the worktree.
The catalog flows validate the owned files and install all 63 package assets
through a staging path containing a space. The gate checks the full installed
layout, bytes, modes and link topology and verifies the source tree is preserved.
A validation retry alters a private copy of `fr.po`, checks that the installer
rejects it before creating the stage, then restores its bytes and modification
time and repeats the install in the same workspace.
A destination retry checks the exact partial package left by a regular file
blocking a locale directory. It removes the obstruction, retries in the same
staging tree, and verifies the complete package and the pre-existing files.

The fixture runs warm oracle bootstrap and retries it after a real advisory
lock conflict in the same disposable workspace. It compares both transcripts
and checks that the installed oracle and lock file are preserved. The integrity
retry checks that warm bootstrap rejects a private executable with the wrong
digest before running it, then succeeds after the verified bytes are restored.
The gate also runs six real Cargo clean modes (including parallel pipe jobserver
and `-n`, `-q`, `-t` guards) and a manifest validation retry. Cargo rejects a
duplicate workspace resolver without changing either target tree. The fixture
restores the manifest bytes and modification time, then retries in the same
workspace and checks that only the selected compiled target is removed.

The three benchmark preconditions require the real oracle and release
prerequisites, exact failures, and no result file. The positioned Cargo
completion time is the only variable transcript field. No latency is collected
by this gate.

This gate needs a provisioned pinned Rust toolchain and verified GNU oracle.
It is separate from cold downloading/installing and timing benchmarks.
Use `DEVELOPER_FLOW_CASE=clean-parallel-pipe` for one case.
