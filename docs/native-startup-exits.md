# Native finalization across startup exits

Native modules now retain the interpreter state from the phase that actually
exits. Decoder failures keep the partial database and warning policy; prepared
help/version exits keep their existing output guards; constructor failures keep
already-published include metadata and the current program/level. A new restart
pass supplies its own exit owner, including help/version, so finalizers cannot
accidentally inspect the preceding Engine's database. No effects are replayed and
no later defaults or option receivers are installed to construct an exit host.

Diagnostics remain separate from ownership. `Failure<E>` carries an optional
native exit owner, and the normal path leaves it empty. Actual Engine state is
retained for errors after construction. Ordinary resources are released before
signal cleanup, then libc finalizers run through the existing native request
bridge with their owner alive on the Rust stack. An interpreter error still
abandons its foreign stack; finalization never resumes that stack.

Native C stdout is flushed before shared help/version banners. Directory cleanup
restores the original OS working directory before finalizers while retaining
the database's constructed `CURDIR`. Include search and warning selection retain
their actual publication point, including a switch immediately before a decode
failure. These are distinct phase properties rather than reconstructed defaults.

## Qualification

31 original compiled-module profiles add inherited MAKEFLAGS/GNUMAKEFLAGS,
eager CLI and HOME-argument loads followed by help/version, decode errors,
directory failures, successful directory changes and eval errors. Additional
profiles cover partial include construction, fatal-finalizer suppression,
second-pass help/version/errors and C stdio buffering. Both frozen builds pass
all 215 native models against two GNU Make 4.4.1 runs. The preceding main build
passes 188 and differs on 27. The ordinary inventory remains 27,766 required
cases across 158 families; approved exceptions remain 32 inputs / 128 cells.

The trusted help/version fixtures redirect stdout to `/dev/null`, so branding
is not normalized into acceptance. Strict file/stderr witnesses check state and
effects, and `__fpending` checks that the banner flushed a known partial C buffer.
Existing startup-exit gates separately check banner ordering. Every model checks
exact status, streams, ordered effects, filesystem snapshots, repeated reference
agreement and process/resource bounds. Negative witness mutations reject shared
wrong state even when every engine agrees. Initial incorrect predictions and
complete observations remain immutable under `tests/native`.

565 Rust tests and 613 tooling tests pass. Five new real-module ownership tests
run in isolated subprocesses, preserving the production interpreter-thread
restriction. Formatting and workspace Clippy pass. Full frozen rmake-driven
acceptance and exact-head development/optimized/aggregate CI remain merge gates.

[Allocation and timing evidence](benchmarks/native-startup-exits.md) includes all
regressions. It does not establish general GNU performance parity. Broader
startup, descriptor/export/stdio, stdin/native exec, interruption, resource,
locale/Linux and enabled Guile/GC profiles remain open, alongside remaining
CLI/debug/database/directory-reporting surfaces, integration and persistent cache
correctness. These selected profiles do not complete M3/M6.
