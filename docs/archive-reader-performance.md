# Adaptive archive scans

The archive reader now enables an 8 KiB buffer after a member with fewer than
256 payload bytes. Dense small-member scans can seek inside that buffer, reducing
header read/seek system calls. Larger members use direct header reads and payload
seeks. The scanner tracks its logical cursor, including inline names and GNU name
tables, and restores that cursor before dropping unread buffered bytes on a mode
change. Absolute offsets and cursor arithmetic remain checked.

The buffer belongs to one scan and is discarded on return, including early
success or errors. There is no persistent archive content/index cache, graph
cache or memory mapping. Touch still seeks to the selected header's absolute date
field through the descriptor it scanned. Archive open attempt counts and atomic
close-on-exec remain unchanged. This is an I/O policy change, without changing
which headers are validated or which metadata reaches the engine.

The first fixed-buffer experiment reduced dense-archive cost but regressed larger
payload workloads: 64 members with 4 KiB/64 KiB payloads measured 2.417/3.326 ms
against 1.538/1.559 ms for the prior build. That implementation was replaced by
the adaptive policy before publication. The archive benchmark suite now includes
both payload sizes alongside its 64/256 one-byte-member workloads, so those costs
remain visible through `make bench-compare BENCH_SUITE=archives`.

Twenty-six original required fixtures cover offsets around buffer boundaries,
straddling headers, odd and large payloads, GNU tables larger than the buffer,
BSD inline names, early matches before malformed later fields, errors before
matches, partial tails, header-only touches, dense/sparse transitions and fresh
reads after replacement/truncation. They also pass on the preceding scanner;
they protect behavior while the I/O policy changes. Corpus: 2,740 cases, with
2,728 required matches and 12 existing pending mismatches. Stateful coverage
remains 48 scenarios / 122 invocations, with 21 filesystem fault cases. The
existing descriptor-inheritance regression and parallel/self-hosting gates remain
part of CI. No upstream implementation or tests were copied.

These selected workloads do not establish broad performance parity or complete
M4/M8. GPATH/permanent renaming, broader implicit secondary expansion, per-clause
static stems, remaining formats/faults, and the full compatibility/performance
acceptance gates remain open. Mixed member distributions and cold-cache resource
profiles need broader measurement; these tests do not certify arbitrary concurrent
external archive rewrites.

Final 101-sample alternating paired measurements against the preceding
`db3665c` build:

| Workload | rmake (ms) | Previous rmake (ms) | GNU 4.4.1 (ms) |
| --- | ---: | ---: | ---: |
| Dense archive, 64 members | 1.107 | 1.454 | 0.979 |
| Dense archive, 256 members | 4.792 | 11.877 | 6.276 |
| 64 members, 4 KiB payloads | 1.580 | 1.569 | 1.095 |
| 64 members, 64 KiB payloads | 1.587 | 1.577 | 1.097 |
| Tiny no-op | 0.685 | 0.695 | 0.617 |
| 10,000 no-op targets | 28.891 | 29.109 | 11.618 |
| Expansion | 4.736 | 4.744 | 2.901 |

The dense 256-member workload improves by about 2.5x over the previous build and
beats GNU in this selected workload. GNU remains faster on the other six; the
small core/payload differences do not establish a performance change. These runs
include process startup and two warmups per engine, with identical paths,
environments, arguments and exact output checks. They ran sequentially after
validation, with warm OS caches, no CPU isolation, resource profile or confidence
intervals. All raw samples and binary/profile identities are retained in ignored
`artifacts/bench/archive-adaptive-final-101.json` and
`artifacts/bench/archive-adaptive-core-101.json`. The initial fixed-buffer
experiment is retained in `artifacts/bench/archive-buffered-8192-101.json`.

[Archive-first VPATH resolution](archive-vpath.md) is now implemented as a
separate slice; GPATH/permanent renaming and the other remaining M4 gates are open.
