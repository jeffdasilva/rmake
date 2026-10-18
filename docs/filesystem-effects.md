# Filesystem expansion effects

`file` reads, writes and appends, and `realpath` resolves existing paths through
an explicit `ExpansionContext` callback. The core owns argument expansion,
validation and byte tokenization; the Linux runtime owns I/O. A pure context
rejects a requested effect instead of silently performing filesystem access.
Callbacks are invocation-local, never retained in variable stores or caches.
All observations run at their required evaluation point.

## Semantics and diagnostic context

File arguments expand eagerly before validation. Omitted write text creates or
truncates an empty file; explicitly empty text writes a newline. Present text
receives one final LF unless it already ends with LF. Reads remove one final LF
and an immediately preceding CR. A missing read returns empty; other open,
read, write and close failures remain distinct fatal errors. Names use the
first token after the operator, including all six C whitespace separators.
No UTF-8 conversion occurs in filenames or returned data.

Usage errors preserve recursive binding provenance; I/O errors use the active
reading, recipe or export context, including explicit unlocated pattern
initialization. Data containing NUL remains in the active expansion until the
consumer boundary: later source effects still run, then that expansion result
is truncated at the first NUL. Nested expansion consumers have separate
boundaries. Literal NULs in Makefile source are a separate pending reader issue.

Realpath resolves symlinks, preserves argument order and omits unresolved paths.
It performs no memoization. File writes follow existing symlinks and preserve
their topology. Replacing a file atomically would change this behavior and is
not an interchangeable optimization.

An ordinary recipe can acquire new command boundaries during expansion. Each
fragment executes with the original recipe's source location and raw modifiers;
expanded `@` and `-` apply to their fragment, while expanded `+` persists through
later fragments of that logical recipe. Direct and shell preparation preserve
their different escaped-newline rules. The existing `.ONESHELL` quoted-newline
direct-command replay gap remains explicit in the ledger.

## Linux safety boundary

`rmake-linux` exposes a safe filesystem callback. Its private `stdio` module is
the only module permitted to use unsafe FFI. Core and application crates retain
their existing `unsafe_code = "forbid"` lint. The adapter uses the host C
library's standard I/O buffering so immediate write failures and deferred close
failures retain their observed stage; it contains no GNU implementation code.

Each stream has one owner, checked terminated strings and initialized read
buffers. A consuming close removes ownership before `fclose`, including on
failure; neither retry nor Drop can close that pointer again. Early failure
cleanup preserves the captured error. Interrupted opens, reads and path
observations retry. Close is never retried. No stream pointer escapes the safe
API, and streams do not acquire unsafe Send or Sync implementations.

Reads retain the interpreter's existing 16 MiB expansion resource bound. This
is an existing compatibility limitation, not an assertion of GNU's unbounded
behavior. Timestamp mutation, signal stress, other libc profiles and complete
filesystem compatibility remain broader milestone work.

## Tests and provenance

Curated cases compare exact output, status, contents and symlink topology.
`make test-filesystem` compiles an original test-only Linux/glibc interposer and
checks eleven controlled faults against independently authored expectations and
both executables. Injection traces prove the faults occurred and that close
happened once. Cases cover interrupted open/read/path operations, partial reads,
write failures and errors returned after an actual close. Prefix effects must
persist and later effects must not run after a fatal error. The harness uses
bounded processes and disposable roots for trusted inputs, not a security sandbox.

Reference study: GNU Make 4.4.1 `src/function.c` (`func_file`, `func_realpath`),
`src/job.c` (`start_job_command`, `construct_command_argv_internal`,
`job_next_command`), and the host `stdio.h` interface declarations. Rust code,
fault injectors and Makefile fixtures were independently written; no upstream
implementation or test code was copied or translated. See the
[licensing policy](licensing.md).
