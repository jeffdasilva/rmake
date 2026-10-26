# Linux symlink timestamps and shared archive matchers

`-L` and `--check-symlink-times` include the timestamps of final-component
symbolic links in build decisions. The timestamp is the maximum of the followed
referent and each link encountered by the walk. A link can trigger a rebuild
even when its referent is older. Links used only as parent directories do not
independently contribute timestamps.

An initial ENOENT or ENOTDIR still permits the link walk, so a dangling link can
satisfy a prerequisite under `-L`. Other initial stat errors are diagnosed and
return an absent timestamp. Interrupted stat/lstat/readlink calls are retried.
A later lstat failure retains the timestamp already observed; missing/nondirectory
paths are silent, while other errors are diagnosed. A readlink failure emits its
diagnostic and retains the latest timestamp, including the current link.

## Paths, phases and flags

Link targets remain raw bytes. Relative targets are joined to the directory of
the current spelling without canonicalizing dot components. Absolute targets
replace that spelling. This preserves Linux lookup through parent symlinks.
The pinned profile uses GNU's 4,096-byte path checks. A 128-link development bound
also prevents an unbounded walk if another process repeatedly changes topology;
a stable successful Linux lookup follows at most 40 links. Race/profile coverage
still needs release-scale validation.

The Linux observer returns an owned timestamp/diagnostic result, with no global
state or persistent metadata cache. Ordinary files return after lstat, before an
owned walk pathname is allocated. A real link walk owns one spelling and reuses
it for relative targets. The implementation uses safe Rust and standard Linux
filesystem interfaces.

The engine applies the option to its build, grouped/double-colon completion and
Makefile timestamp observations. Local dangling links are considered before
VPATH. A non-declared VPATH candidate still needs a successful physical stat;
link-aware timestamps do not make a dangling searched entry a physical match.
Eligible declared records retain their existing bypass. A dangling local archive
container can prevent VPATH fallback under `-L`, while still containing no member.

For archive members, the standalone member's link-aware timestamp is compared to
the member date in integer seconds. A timestamp at date+0.999999999 does not
invalidate that member; the next integer second does. The archive header remains
the source of the member's own timestamp.

The option is accepted on the command line and through startup MAKEFLAGS and
GNUMAKEFLAGS. Canonical MAKEFLAGS uses GNU's `L` ordering and propagates through
recursive invocations, including parallel recursion. Existing restrictions on
changing make flags inside a Makefile remain a separate M3 gate.

## Archive matching cost

Archive scans compile their member pattern once per opened archive. The same
matcher accepts borrowed member names; simple single-star patterns avoid
per-member pattern/name C-string allocations. Complex patterns retain libc
fnmatch behavior. Shared simple-star predicates are inlined so ordinary directory
matching does not incur a separate helper call per entry.

Every header is validated before a match is counted or retained. An invalid
pattern produces no matches while the scan still observes numeric-header errors.
Only retained rule-wildcard results copy names. The native parity test exercises
both C-string and byte entry points against libc for raw bytes, dot/slash rules,
complex/escaped patterns, and explicit NUL rejection.

## Original validation and remaining work

The slice adds 77 required Makefiles under `filesystem.symlink-times`. The
preceding `17a827f` release passes ten default-mode controls and fails 67.
Cases cover nanosecond equality/newer boundaries, live/dangling chains, Linux's
40-link limit, loops and diagnostics, parent-directory links, lexical dot paths,
absolute/raw/space-containing names, VPATH/GPATH records, archives, implicit/static/
double-colon/grouped/order-only rules, execution modes, flag entry points,
recursive builds and included Makefiles.

`make test-symlink-times` adds 52 bounded original stat/lstat/readlink fault
programs across normal, dry, query and touch modes. Independent anchors check
exact streams/status and complete file bytes/types/modes/links, a protected file
and a one-fault journal. The test-only interposer reports real syscall faults;
it supplies no timestamps or Make semantics. The runner rejects equal wrong
observations, missing journals, changed links/data and resource-limit failures.
`SYMLINK_CASE=<id>` selects a replay. CI and a self-host smoke case use this flow.
This trusted-fixture runner is not a filesystem or network security sandbox.

Three new stateful scenarios add 31 invocations. They independently anchor link,
hop, referent and output nanoseconds across changes, mode switches, dangling and
restored links, deletion and rebuild. Archive steps distinguish fractional from
integer-second changes and compare rebuilt archive bytes against authored files.
The inventory is now 61 scenarios / 241 invocations, all required.

The curated corpus contains 3,911 cases: 3,899 required matches and 12 unchanged
pending mismatches. Internal checks include 380 Rust and 278 tooling tests.
The full Makefile-driven gates retain the existing generated suites, mixed
jobserver trees and repeated parallel comparisons. Both GNU and rmake drive the
new flow, and the optimized candidate also drives top-level checks under `-L`.

This completes the implemented link-time slice, not M4 or full GNU compatibility.
Wider file-record/timestamp caching and error-phase combinations, archive I/O
profiles, filename parsing/escaping, Makefile option replay and runtime/locale
profiles remain open. Default-mode stat diagnostics also need further coverage.
Persistent graph caching still requires its separate validation design.

## Measurements

`make bench-compare BENCH_SUITE=symlink-times` prepares fixed-date ordinary files,
live chains and dangling links outside timing. Empty recipes launch no children.
The old main does not implement `-L`, so it cannot be used as a conforming baseline
for these workloads. The prototype column below is an unreleased development
build from this slice that allocated a walk pathname eagerly. It is not the
preceding release, nor proof that allocation changes alone explain timing.

Local paired runs use 101 samples per engine:

| New -L workload | Current rmake (ms) | Development prototype (ms) | GNU 4.4.1 (ms) |
| --- | ---: | ---: | ---: |
| 2,000 ordinary files with -L | 4.394 | 4.435 | 2.912 |
| 1,000 link prerequisites with -L | 6.696 | 6.682 | 3.452 |
| 500 two-link prerequisites with -L | 3.771 | 3.827 | 2.087 |
| 1,000 dangling prerequisites with -L | 6.320 | 6.367 | 3.226 |

The prototype differences are small; these runs do not establish an elapsed-time
gain from pathname allocation avoidance. GNU remains faster in all four new
link-time workloads.

Separate default-mode controls compare against the preceding `17a827f` main.
They use 101 samples per engine, with 201 for ordinary wildcard controls:

| Default-mode workload | Current rmake (ms) | Previous main (ms) | GNU 4.4.1 (ms) |
| --- | ---: | ---: | ---: |
| Startup | 0.684 | 0.682 | 0.626 |
| 10,000 ordinary no-op targets | 29.376 | 29.779 | 11.491 |
| 5,000 expansion assignments | 4.670 | 4.725 | 2.816 |
| 128 GNU long-name members | 2.364 | 2.406 | 2.434 |
| 128 BSD inline-name members | 1.971 | 2.033 | 2.855 |
| 64 archive wildcard scans over 256 members | 1.911 | 2.324 | 5.857 |
| Cold wildcard over 1,000 files | 0.976 | 0.966 | 1.204 |
| 200 repeated wildcard queries | 11.133 | 11.099 | 37.273 |
| 200 complex wildcard queries | 14.478 | 14.394 | 38.296 |
| 1,000 literal wildcard names | 1.063 | 1.071 | 0.960 |

Repeated archive wildcard time is about 18% lower than the preceding main.
Core and ordinary wildcard controls are close in these runs. Selected wildcard
workloads beat GNU; other controls, including the new `-L` workloads, remain
slower. These results do not establish general speed parity or complete M8.

Reports retain all samples, workload/input parameters, command arguments and
executable/profile identities under ignored `artifacts/bench/symlink-times-101.json`,
`symlink-times-core-101.json`, `symlink-times-archives-101.json` and
`symlink-times-wildcard-201.json`. Measurements include startup, two warmups per
engine, warm OS caches, alternating engine order and exact stream/status checks.
They ran with no concurrent local validation workload. There is no CPU isolation,
confidence interval, CPU/RSS/I/O profile or performance acceptance threshold.

Current release SHA256: `a1d20621627c0c18f43303fe5b9639d7231826f53a34f5ef011ae5a7214db305`.
Previous main SHA256: `ee47d802f3fb42628f443629ebecb70e75394ee685cd6169fe3ab3404fd6f29d`.
Development prototype SHA256: `8380486a172f906116437048169f9e38f7557201fe4db14d211427b2295cd71a`.

## Reference use

The verified GNU Make 4.4.1 `src/remake.c` (`name_mtime`, `f_mtime`) and
`src/main.c` (check-symlink option table and recursive flags) were consulted for
walk boundaries, timestamp/error behavior, archive comparisons and flag ordering.
Linux/glibc stat/lstat/readlink and fnmatch interfaces were checked against the
local headers and native parity tests. The Rust implementation, interposer
extensions, Makefiles, stateful anchors and benchmarks are original; no GNU
implementation or tests were copied.
