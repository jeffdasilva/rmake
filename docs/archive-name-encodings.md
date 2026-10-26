# Archive member-name encodings and borrowed scans

GNU and BSD long-name fields have a different numeric contract from an archive's
mode, size, date, uid and gid. On the pinned 64-bit Linux GNU Make 4.4.1 profile,
name-table indexes and inline-name lengths accept leading C whitespace and one
sign, then narrow the decimal result to an unsigned 32-bit value. For example,
`/4294967296` and `/+0` both select the first table entry; `#1/4294967297` consumes
one inline byte. Trailing control whitespace is invalid. Trailing spaces in the
fixed field have already been removed before this conversion.

The conversion rejects missing digits and other suffixes. Bounds apply after
narrowing: a name-table index must refer to a nonempty string inside the current
table; an inline length must be in 1..4096 and its bytes must actually exist.
Malformed name encodings stop the scan before numeric metadata fields are parsed.
Numeric metadata fields retain their separate strict grammar and fatal diagnostic
order. A leading plus accepted in a length remains invalid in a date or mode.

Name-table marker recognition uses the string through its first NUL, before
removing a final slash. Thus `//` and `ARFILENAMES/` still identify mapping members
when followed by NUL and raw tail bytes inside the fixed-width field. Table
replacement, inline names, embedded NULs, slash removal and payload alignment
retain their existing scan boundaries.

## Allocation and lifetime

A scan now passes a borrowed member-name view to its visitor. Short names borrow
the current header; GNU long names borrow the invocation's table; BSD inline
names use one reusable scan-local buffer. The visitor cannot retain a borrowed
name beyond its call. Metadata and touch searches need only compare the name or
retain an integer offset. Rule wildcard expansion copies only names it actually
retains. Wildcard counting retains no member names.

The higher-ranked visitor lifetime enforces that boundary in safe Rust. No unsafe
code, global member index or persistent cache is added. Header read-ahead remains
adaptive; file contents and buffers remain local to one scan. A new scan still
observes replacement/truncation, and malformed later headers still raise the
same errors when a visitor has not stopped early. All numeric fields are checked
before visiting a member; allocation reduction does not skip error checks.

## Original coverage

The slice adds 176 required Makefiles under `archives.name-encodings`. The
preceding `c5045a6` release passes 83 and fails 93. Independently authored stream
and status anchors cover signed and wrapped indexes/lengths, all six leading C
whitespace bytes, trailing whitespace rejection, table markers with NUL tails,
space-marker indexes, empty tables, range checks, short inline data, 4,095-byte
inline scans, early stops, rule wildcards, metadata diagnostics and touch.
The long inline boundary cases exercise the scanner through wildcard expansion;
oversized standalone-member path diagnostics remain an open filesystem gate.

A new twelve-invocation stateful scenario alternates GNU and BSD archives, query,
touch, malformed names, replacement and restoration. Verification steps compare
every byte against independently authored expected archives. They prove that
touch selects the first duplicate, changes only its twelve-byte date field and
preserves the other member, table, name encoding, NUL padding and payload.
Unchanged-file anchors protect query and verification steps.

The curated corpus now contains 3,834 cases: 3,822 required matches and 12
unchanged pending mismatches. Stateful coverage is 58 scenarios / 210 invocations;
internal coverage remains 378 Rust tests, with 271 tooling tests. Existing top
level `test-oracle`, `test-diff`, `test-stateful`, `check` and `ci` flows include
these required cases. Both GNU and rmake drive the flows.

This completes the member-name conversion and allocation slice on the pinned
profile. M4 remains open for wider file-record/timestamp interactions, descendant
scope audits, remake/lifecycle combinations and archive I/O/format/profile
coverage. Broader runtime and compatibility profiles remain open too. A green
corpus does not establish full GNU compatibility or complete M4/M8.

## Measurements

`make bench-compare BENCH_SUITE=archive-names` adds three original no-child
workloads. Fixed-date GNU and BSD archives are prepared outside timing. They use
canonical encodings accepted by both builds; the old build's convergence here
does not validate the unusual encodings in the new compatibility cases.
Existing short-name, dense/sparse payload and core controls are unchanged.

Local alternating paired comparisons against `c5045a6` use 101 samples per engine:

| Workload | Current rmake (ms) | Previous rmake (ms) | GNU 4.4.1 (ms) |
| --- | ---: | ---: | ---: |
| 128 GNU long-name members | 2.523 | 2.726 | 2.482 |
| 128 BSD inline-name members | 1.959 | 2.264 | 2.874 |
| 64 wildcard scans of 256 long names | 2.282 | 2.448 | 5.787 |
| 64 short-name members | 1.149 | 1.164 | 1.019 |
| 256 short-name members | 4.390 | 4.652 | 6.330 |
| 64 members with 4 KiB payloads | 1.466 | 1.489 | 1.013 |
| 64 members with 64 KiB payloads | 1.468 | 1.490 | 1.001 |
| Startup | 0.698 | 0.717 | 0.639 |
| 10,000 ordinary no-op targets | 29.217 | 29.656 | 11.348 |
| 5,000 expansion assignments | 4.783 | 4.618 | 2.876 |

GNU long-name member time is about 7% lower, BSD inline-member time about 13%
lower, and repeated archive wildcard time about 7% lower. Dense 256-member scans
use about 6% less time; smaller and sparse-payload differences are close. Core
results are mixed, including about 4% higher expansion time. Selected archive
workloads beat GNU, while GNU remains faster in other archive and core controls.
These runs do not establish general speed parity or complete performance gates.

Reports retain source/input hashes, executable identities and all raw samples
under ignored `artifacts/bench/archive-names-101.json`,
`archive-names-archives-101.json` and `archive-names-core-101.json`.
Runs include process startup, two warmups per engine, warm OS caches, alternating
engine order and exact stream/status checks. They ran after functional checks
with no concurrent local validation workload. There is no CPU isolation,
confidence interval, CPU/RSS/I/O profile or performance acceptance threshold.

Current release SHA256: `ee47d802f3fb42628f443629ebecb70e75394ee685cd6169fe3ab3404fd6f29d`.
Previous release SHA256: `4676cac3873a7ebcf7bf18744f118feddf38addcbaf79fcbb029f892b90b9387`.

## Reference use

The verified GNU Make 4.4.1 source archive was consulted for `src/arscan.c`
(`ar_scan`, name-marker recognition, numeric conversion order and name bounds),
`src/misc.c` (`make_toui`, the decimal conversion and return width), and
`src/ar.c` (`ar_name_equal`, member lookup and touch behavior). The source was
used to explain independently observed behavior. The Rust implementation,
archive bytes, Makefiles, stateful anchors and benchmarks are original; no
upstream implementation or tests were copied.
