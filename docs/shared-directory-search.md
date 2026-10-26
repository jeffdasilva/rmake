# Shared directory observations for rule search

Wildcard expansion, local implicit viability and VPATH/GPATH now use the same
invocation-owned directory observations. A new file created by an external
mutation is not automatically visible after a directory has been read. A deleted
cached name can still select a local implicit rule, whose actual prerequisite
build then sees the missing file. Replacing either decision with fresh stat
changes rule selection and diagnostics.

VPATH has an additional check: a positive directory name must still pass a fresh
stat. Eligible Makefile records bypass directory membership, including missing
declared outputs. Local build timestamps and literal wildcard/realpath queries
remain fresh. Archive-container VPATH selection shares directory membership;
archive-member metadata itself remains a fresh scan.

The historical repeated-trailing-separator quirk has separate record-lookup,
directory-membership and final stat/output spellings. The original basename is
used for membership even when the final path loses its first basename byte.
Original positive, negative and declared-record fixtures cover these phases.

## Incremental streams and refresh

Path keys share contents by device/inode. Missing directories and unsuccessful
opens are cached too. A membership probe reads only until it finds the requested
name or EOF, preserving actual dot entries and unread stream state. Full wildcard
queries consume the remainder and sort once. Membership-only scans retain their
hash index without sorting; full glob scans append directly without first
building an unnecessary hash table. Owned terminated names serve matching and
raw-byte lookup without a second filename allocation.

Completed commands and successful file-write opens advance the existing
filesystem generation. Stale directories refresh on their next access, including
negative observations and inode aliases. Reads, realpath, skipped commands and
optimized colon commands retain the previous generation. Failed implicit-search
observations continue to use this same boundary. Makefile restarts retain the
existing effect and reread behavior; no cache survives an invocation.

Opening a tenth directory drains and closes that new stream, leaving at most
nine retained partial streams. The libc stream has one Rust owner, preserves
EINTR retry/EOF/error distinctions, copies bounded dirent64 names, and closes
once on EOF, failure or drop. Directory descriptors have close-on-exec protection;
a recipe-child probe verifies that none of the nine partial streams are inherited.
An injected allocation-budget failure at the tenth stream verifies its release
on the exceptional path as well. Core and application code still forbid unsafe
Rust; the small Linux adapter owns the FFI boundary.

An opendir failure makes the directory unavailable for that generation. A
readdir failure is fatal with GNU's program-only diagnostic, including after a
prefix was read. Close failures remain consumed once. The existing estimated
128 MiB directory-data budget and 16 MiB expression limit remain development
limits requiring release-scale validation.

## Group and completion phases

Explicit grouped outputs inspect peer timestamps in also-make order only while
all examined outputs exist. A missing output stops further peer timestamp
lookups. Implicit groups are installed after this phase and do not inspect their
new peers there. Peer completion uses local metadata with VPATH search disabled.
These distinctions prevent unused peer searches from raising spurious I/O errors.
Secondary expansion and peer prerequisite traversal remain separate phases.

A completed recipe can invalidate the previous negative directory observation.
The parent's post-update timestamp phase can then discover a GPATH result and
retain its new identity. Ordinary tentative VPATH spelling does not itself
replace the target's command name. A per-outcome marker requests this observation
without replaying the recipe. Virtual dry/query/touch updates retain their
synthetic timestamp; entirely recursive commands request a real observation.
The recipe's recorded flags include a `+` produced by variable expansion, rather
than inferring recursion again from raw source text.

## Original coverage and limits

The slice adds 56 required Makefiles under `resolution.directory-search`. The
preceding `dc81c76` release passes 44 and fails 12. They cover file/shell/recipe
refresh, candidate effects, local directory and dangling-link inputs, modes,
late GPATH discovery and the separate trailing-separator spellings.

`make test-directory-search` adds 176 bounded original programs with independently
authored stdout, stderr, status, file bytes/types/modes/links and mutation-journal
anchors. The test-only interposer changes files, faults libc calls and records
real streams; it does not synthesize directory entries or implement Make rules.
The preceding release passes 105 and fails 71. Coverage includes stale/new/deleted
names, missing and replaced directories, inode aliases, declared-record bypass,
reverse search-to-wildcard sharing, stream limits, descriptor inheritance,
interrupted/denied opens, interrupted/failed reads, close errors, grouped phases,
and literal/expanded recursive prefixes in dry/query modes.

The runner rejects equal wrong observations, including nested filesystem or
journal mutations and omitted resource proofs, and retains complete reproducers.
`DIRECTORY_CASE=<id>` selects a replay. The target runs from GNU and rmake's top
level Makefile, CI and a self-host smoke case. The late-GPATH parallel fixture
joins the default 40-repeat graph-progress gate, which now runs 720 comparisons
per engine. The optimized-candidate check also runs it with twelve repetitions.

The curated corpus is now 3,658 cases: 3,646 required matches and 12 unchanged
pending mismatches. Internal checks include 378 Rust tests and 270 tooling tests;
stateful coverage remains 57 scenarios / 198 invocations. This completes the
shared-membership and stream-lifetime slice, not M4 or full GNU compatibility.
Wider entered-file/timestamp and failed-search metadata interactions, descendant
scope audits, further remake/lifecycle combinations, directory/kernel/libc/locale
profiles and the archive format/fault matrix remain open. Persistent graph
caching is still deferred until its separate validation design is proved.

## Measurements

`make bench-compare BENCH_SUITE=directory-search` prepares fixed-time inputs and
runs two original workloads with no child recipes. The implicit case uses 1,000
current outputs and inputs. The miss case probes eight directories with 64
unrelated files each. Exact streams/status also converge on the old build;
that convergence does not establish its compatibility.

Local alternating paired runs against `dc81c76` use 101 samples per engine,
with 201 for the wildcard controls:

| Workload | Current rmake (ms) | Previous rmake (ms) | GNU 4.4.1 (ms) |
| --- | ---: | ---: | ---: |
| 1,000 implicit input probes | 6.075 | 5.971 | 3.193 |
| 1,000 misses across eight VPATH directories | 4.544 | 5.276 | 3.129 |
| Startup | 0.700 | 0.700 | 0.621 |
| 10,000 ordinary no-op targets | 30.221 | 30.081 | 11.524 |
| 5,000 expansion assignments | 4.681 | 4.806 | 2.918 |
| Cold wildcard over 1,000 files | 0.957 | 0.958 | 1.186 |
| 200 repeated wildcard queries | 11.771 | 11.244 | 37.508 |
| 200 complex wildcard queries | 15.447 | 15.024 | 39.923 |
| 1,000 literal wildcard names | 1.193 | 1.176 | 1.061 |
| 1,000 declared searched records | 5.590 | 5.260 | 2.576 |
| 1,000 original/searched pairs | 6.415 | 6.040 | 2.812 |
| 400 implicit searched records | 4.204 | 3.821 | 2.138 |

Repeated VPATH misses use about 14% less elapsed time. The direct
implicit control costs about 2% more. Core controls are close in
this run. Warm wildcard time is about 5% higher; the declared-record
controls cost about 6% / 6% / 10% more.
Required negative searches now populate directories that the old build only
stat'd. Avoidable sorting/hash work was removed, but the remaining regressions
need further profiling. GNU is faster in the rule-search controls; rmake is
already faster in several wildcard controls. These results do not establish
general speed parity or complete the M8 performance gates.

Reports retain all samples, source/input hashes and executable identities in
ignored `artifacts/bench/directory-search-101.json`, `directory-search-core-101.json`,
`directory-search-wildcard-201.json` and `directory-search-records-101.json`.
They include startup, two warmups per engine and warm OS caches, with alternating
engine order and exact output/status checks. There is no CPU isolation,
confidence interval, CPU/RSS/I/O profile or performance acceptance threshold.
Current release SHA256: `82f185490b64ef80d7269e213f33bb7b6791a626f242074371bab02ba42ebf23`.
Previous release SHA256: `eaeea3a97b6b19c8d21fe5b6cf12bb078b7413bd6ea0979e8c4cc433576eb77e`.

## Reference use

GNU Make 4.4.1 `src/dir.c` (`find_directory`, `clear_directory_contents`,
`dir_contents_file_exists_p`, `file_exists_p`, `open_dirstream`, `read_dirstream`),
`src/vpath.c` (`selective_vpath_search`) and `src/remake.c` (`update_file_1`,
`notice_finished_file`, `f_mtime`) were consulted for sharing, lifetime, probes,
diagnostics and group/update phases. The dirent64 interface was checked against
the system Linux/glibc declarations and native ownership/byte tests. This slice's
Rust implementation, C test hooks and Makefiles are original; no GNU
implementation or tests were copied.
