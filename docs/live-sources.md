# Live makefile streams and parser continuations

Named makefiles and includes now retain their open source stream through
interpretation. GNU's source reader observes later writes through that stream,
subject to libc buffering and sticky EOF/error state. Reading a whole file
before interpreting its first line loses those effects. The new host-owned
`Source` interface keeps filesystem I/O in the Linux adapter and exposes each
physical C-string fragment at the interpreter's actual logical-read boundary.
Memory-backed eval buffers keep their independent byte reader. Stdin retains
its preceding cached-byte replay path; complete GNU stdin temporary-file and
self-exec lifecycle acceptance remains open.

The Linux adapter uses the existing reviewed stdio module for `fopen`, `fgets`,
`ferror` and `fclose`, marks source descriptors close-on-exec and owns each FILE
exactly once. Source `fgets` errors are sticky and are not the retrying `$(file)`
read operation. Errors are selected before the interrupted logical line runs.
Normal source-close errors are ignored, matching the reference, while fatal
cancellation prevents further source effects. Directory/read errors remain
unlocated through nested eval/call transport. Replacement paths do not replace
an already-open inode; truncation, no-final-newline EOF, BOM/NUL handling,
continuations, physical line numbers and child descriptor inheritance retain
separate controls. A reusable scratch buffer requests at most 64 KiB per libc
call; larger logical fragments assemble before interpretation, preserving NUL
visibility and the existing retained virtual buffer-count boundary.

Includes now use heap reader frames with per-file declaration, condition,
location and pending filename state. The entire include RHS is expanded and
its filenames prepared before reading the first child. Opening files,
MAKEFILE_LIST updates and interpretation still occur one child at a time.
The parent resumes after the child's declaration and condition checks complete.
Beyond the former 128-frame window, additional live reader control storage is
limited to 16 MiB per reader driver. Completed frames release reservations.
This is additional control-storage accounting, not a process peak-memory claim.

Nested eval still needs same-thread native reentry into the mutable interpreter.
A core host interface permits a bounded stack adapter without putting platform
dependencies in core. The qualified x86-64 Linux executable uses the safe
[`stacker` API](https://docs.rs/stacker/0.1.25/stacker/): it checks remaining
stack before eval entry, switches to a 4 MiB segment below a 1 MiB red zone and
limits simultaneously reserved extra segments to 64 MiB. Resource rejection
precedes the callback; RAII releases reservations on return, error and unwind.
The platform-free API without an adapter and unqualified architectures retain
the preceding 128-level native guard. This does not establish native extension,
Guile GC/stack scanning, non-x86 Linux or broader resource parity. Those profiles
need their own adapters and qualification. Internal limits earn no approved
GNU crash/hang exception credit.

Active recursive-variable identities survive eval reentry as shared immutable
parent contexts and local keys. Suspending a new eval copies only its local
keys, rather than all ancestor keys. Membership checks span the inherited
contexts; simple rebinding bypasses an older recursive key, recursive rebinding
retains cycle identity, and errors restore caller scope, bindings and active
state. Expanded values and filesystem observations are never cached. The first
nonempty physical source chunk becomes the logical buffer directly, avoiding
an extra ordinary-line copy.

## Original models and qualification

269 new required cases comprise 32 historical live-write cells, 120 include
frame cells, 12 child-EOF condition errors, 24 prepared HOME/glob controls,
40 nested eval/cycle/rebinding cells, 38 source identity/buffering/descriptor/
directory cells and three output-checked scaling controls. Include depths are
3, 96, 128, 129 and 192; eval depths extend to 512. Both output-sync none/target
and direct/eval include entry points are retained. File effects check exact
before/return order, not just successful exit. Empty directory-error fixtures
retain a tracked sentinel for Git packaging.

Two bounded pinned GNU Make 4.4.1 observations agree with each independent model.
Both frozen builds match all 269. Preceding rmake fails 158 and passes 111
controls, including all three timed depths. The ordinary corpus is now
15,393 required / zero pending, with the same 17 separately gated approved
safe-error exceptions. Zero curated pending cases does not establish M3/M6.

[Raw evidence](../tests/research/live-sources.json) includes byte/mode inputs,
arguments, environments, executable identities and all raw output/status/
filesystem observations and bounds. It also retains 27 rejected original
models with zero credit: 24 incorrectly predicted that HOME/glob preparation
would occur after reading the first child; three scaling models omitted GNU's
empty-recipe status message. Separate corrected prepared-name models select
the before/old files. Separate quiet scaling inputs explicitly run a silent
recipe. The original wrong expectations and observations remain unchanged.
No model was silently rewritten to make a candidate pass.

The separate `make test-source-streams` gate adds 36 original Linux/glibc
profiles: main file, include, eval, call, 512-deep eval and MAKEFILES preload,
each with no fault, interrupted open, EIO/EINTR read, close error and fatal
SIGTERM. The observer changes only selected I/O outcomes; it supplies no Make
semantics. Every non-normal trace must prove exactly one reached injection.
Prior/later file markers, an unchanged anchor, raw diagnostics/status and full
filesystem comparison establish effect ordering and cleanup. Both frozen builds
match two GNU observations and each model. These profiles receive no additional
ordinary or safety-exception credit. Raw [debug](../tests/research/live-source-io-debug.json)
and [release](../tests/research/live-source-io-release.json) observations preserve
observer hashes and fault causality. Normal, optimized and self-hosted top-level
flows include the gate.

Eleven additional Rust tests cover stream equivalence, post-read cancellation,
late read errors, 2,048 heap include frames, storage rejection, cycle cleanup,
live appended tails, descriptor inheritance, same-thread stack growth and
budget rejection/unwind cleanup. Thirteen tooling tests protect inventory,
models, refinements, effects, every timed read, fault anchors and the complete
locked dependency notice set. Formatting, lint, 509 Rust and 507 tooling tests
are required, followed by frozen rmake-driven development/optimized acceptance
and exact-head CI before merging. Results belong to the PR.

The oracle profile remains
`92525c9784f4dd471cceb12ce66a9d367e267b638e1384cdc29962b738d16de7`,
GNU binary SHA256
`b8806e8eb7452ed9b7ed5e18cb7e76ff9abd488c9d62a266a398d8044fade974`,
verified GNU source archive SHA256
`dd16fb1d67bfab79a72f5e8390735c49e3e8e70b4945a15ab1f81ddb78658fb3`.
GNU source was consulted for externally observable source lifetime, filename
preparation, stdio error/close phases and evaluation context. Rust, C observers,
fixtures and test tooling are original; no GNU implementation/tests were copied
or translated and no GNU runtime fallback was added. Additional unsafe FFI
calls stay inside the existing stdio boundary. The pinned stack adapter and
transitive runtime/build packages retain original license files, package
checksums and hashes in [third-party notices](../THIRD_PARTY_NOTICES.md).

## Remaining acceptance

This closes the recorded live-source and finite deep include/eval slices.
[Complete M3/M6](m3-m6-audit.md), native modules, enabled Guile, runtime HOME/tilde
flag decoding, real self-exec and stdin temporary lifecycle, remaining CLI/debug/
database/directory presentation, broader Linux/locales/integrations and resource
acceptance remain open. General performance parity and persistent cache
correctness remain separate acceptance work.

Frozen debug SHA256: `464dbbdfb05df62f68650ba32e819dc12d5c2a3ba95c662f577f1dadc2f735d3`.
Frozen release SHA256: `e2c13983352801f21fff4f28a38794ec5f2aa76a0eb1fb76ef3084fd4ed0d11a`.

## Measurements

Six final reports retain two sequential batches of 101 alternating samples per
engine/workload, two warmups and exact output agreement. Pinned GNU drives A;
frozen rmake drives B through the top-level Makefile. No unrelated local build,
test or probe campaign runs during timing. All timed inputs are correct on GNU,
preceding rmake and the candidate. Newly accepted deep cases earn no timing
credit. Host activity is uncontrolled; there are no confidence intervals,
CPU/RSS/I/O measurements or performance thresholds.

| Workload | GNU / candidate / preceding median ms A | GNU / candidate / preceding median ms B | Paired preceding/candidate A / B |
|---|---:|---:|---:|
| tiny-noop | 0.679 / 0.837 / 0.820 | 0.538 / 0.621 / 0.615 | 0.994 / 0.997 |
| large-noop | 10.982 / 29.580 / 28.754 | 11.280 / 29.855 / 29.287 | 0.975 / 0.976 |
| expansion | 2.886 / 5.084 / 4.995 | 3.019 / 5.292 / 5.170 | 0.968 / 0.978 |
| function-bodies-0 | 0.949 / 1.422 / 1.383 | 0.936 / 1.418 / 1.387 | 0.972 / 0.972 |
| function-bodies-16 | 3.825 / 7.690 / 7.514 | 3.915 / 7.851 / 7.742 | 0.975 / 0.981 |
| function-bodies-48 | 10.145 / 24.288 / 23.873 | 10.372 / 24.492 / 23.859 | 0.979 / 0.975 |
| nested-evals-0 | 0.573 / 0.736 / 0.722 | 0.638 / 0.816 / 0.821 | 0.971 / 1.003 |
| nested-evals-32 | 2.443 / 5.647 / 4.763 | 2.444 / 5.478 / 4.775 | 0.849 / 0.863 |
| nested-evals-96 | 5.401 / 12.328 / 9.792 | 5.316 / 12.155 / 9.639 | 0.796 / 0.795 |

Ratios above one favor the candidate. Core and function-body results remain
mixed; nested eval is slower than preceding rmake, especially at depth 96.
Source lifetime, stack preparation and active identities add costs that require
further profiling. GNU remains faster. No general speedup, performance parity,
allocation reduction or process-memory improvement is claimed.

The first otherwise-correct buffer prototype repeatedly cleared/scanned an
entire retained 64 KiB scratch buffer after a long first line. Its large-noop
median was about 190 ms versus 28 ms for preceding rmake. The corrected adapter
clears only bytes consumed by each read and takes the visible-newline path
without a full-buffer scan. A library test covers short and embedded-NUL reads
after a large buffer. All semantic and I/O qualification was repeated after
that change and immediate errno capture. Both batches of rejected prototype
measurements remain available with their actual executable identities; they
receive no final-candidate performance credit.

Final raw reports: [core A](benchmarks/live-sources-core-a.json),
[core B](benchmarks/live-sources-core-b.json),
[function A](benchmarks/live-sources-function-bodies-a.json),
[function B](benchmarks/live-sources-function-bodies-b.json),
[eval A](benchmarks/live-sources-nested-evals-a.json),
[eval B](benchmarks/live-sources-nested-evals-b.json).
Prototype reports: [core A](benchmarks/live-sources-buffer-prototype-core-a.json),
[core B](benchmarks/live-sources-buffer-prototype-core-b.json),
[function A](benchmarks/live-sources-buffer-prototype-function-bodies-a.json),
[function B](benchmarks/live-sources-buffer-prototype-function-bodies-b.json),
[eval A](benchmarks/live-sources-buffer-prototype-nested-evals-a.json),
[eval B](benchmarks/live-sources-buffer-prototype-nested-evals-b.json).
