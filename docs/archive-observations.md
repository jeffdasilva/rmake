# Archive observations and temporary file names

This M4 follow-up separates initial archive observations, implicit-rule viability,
recipe completion and continuation cleanup. Member stat precedes container stat
and header validation. A standalone member newer in whole seconds makes the
archived member absent. Implicit viability checks its archive date separately.
Rejected candidates may enter observed container names without installing rules.

A missing container leaves the member timestamp unknown; an existing container
with a missing member produces a known absent timestamp. Revisit unknown values
when the parent consumes the completed prerequisite. Preserve GNU's serial and
parallel differences, including a serial rebuild whose final `$?` is empty.
Any declared recipe, including an empty or expansion-only recipe, invalidates
archive metadata after its prerequisites finish. A rule without a recipe keeps
its earlier observation. Existing completion values remain invocation-local.

Container and member VPATH record changes happen before header scanning. A
temporary searched hash key retains its original recipe and cleanup name;
GPATH or a merge with a distinct existing record may make the name permanent.
A later original-container lookup can merge into its own temporary record
without renaming the member. Safe owned hash keys, tombstones and explicit
logical names preserve traversal order through collisions and table growth.
When there are no created intermediates, cleanup skips the file-table scan.

The scanner retries interrupted reads and seeks. Retrying an interrupted
buffered seek uses an absolute position to avoid applying a relative displacement
twice. Initial archive open retains GNU's single-attempt behavior. A header I/O
failure is invalid structure; clean EOF at a header boundary is absence.
Fatal numeric metadata errors remain fatal during touch and failed-recipe
cleanup, with the pinned continuation-owner diagnostics and exit status.
Simulated completion after a forced query/touch command retains the archive
warning even if the header date did not change. Precious-member policy applies.

Touch scans through a read descriptor before acquiring write access. Before
writing the twelve date bytes, compare the scanned and writable descriptors'
device/inode identities. A replaced pathname returns an error and leaves the
replacement untouched. This deliberately preserves safety during a race rather
than reproducing GNU's possible write to the wrong archive. The native race
regression tests the guard directly; no ordinary compatibility expectation is
relaxed for this guard.

## Evidence

- 38 original required Makefiles cover temporary/GPATH hash collisions and growth,
  empty versus absent recipes, cached observations and fatal continuation cleanup.
  Cleanup expectations use an independently written table/phase model.
- `make test-archive-times` checks **2,600** independently anchored fault cells:
  1,280 initial stat, 64 open, 896 read/seek, 56 forced touch/query continuation,
  64 visible/hidden intermediate and 240 post-recipe serial/parallel observations.
  Local, VPATH, GPATH and previously declared records retain exact streams,
  status, bytes, modes and protected files. Faults target fixed byte offsets,
  not a particular engine's read-call count. Replay with `ARCHIVE_TIME_CASE`.
- The 14-step `archive-observation-transitions` scenario changes remote/local
  archive priority, standalone timestamps, bad headers, GPATH and hidden
  secondary edges with fixed input and output timestamps.
- Native tests cover archive descriptor inheritance and replacement during touch.

Inventory after this follow-up: **4,639 curated / 4,628 required / 11 historical
pending**, **67 stateful scenarios / 317 invocations**, **390 Rust / 293 tooling
tests**. Existing pending classifications and comparison boundaries are unchanged.
The exact combined development and self-hosted gates must pass before merging.
These counts do not prove full GNU compatibility or performance parity.

## Reference study and scope

Consulted the verified GNU Make 4.4.1 archive, SHA-256
`dd16fb1d67bfab79a72f5e8390735c49e3e8e70b4945a15ab1f81ddb78658fb3`:
`src/remake.c` (`f_mtime`, update/completion), `src/ar.c` (member date/touch),
`src/arscan.c` (header I/O), `src/implicit.c` (viability), `src/file.c`,
`src/vpath.c`, `src/hash.c`, `src/hash.h`, `src/commands.c` (cleanup), and
`src/filedef.h` (unknown timestamp). The Rust, C fault helper, Makefiles, scripts
and anchors are independently written; no upstream implementation/tests were
copied or translated.

No persistent archive index or timestamp survives an invocation. The bounded
scanner/resource policy remains documented; arbitrary allocator behavior and
concurrent external mutation are not promised deterministic GNU parity. M4's
recursive HOME behavior and combined acceptance audit remain open at this commit;
general speed parity and persistent caching remain M8 work.
