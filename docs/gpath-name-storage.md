# Permanent filename storage

Permanent GPATH names are now stored once in the alias table. Goal reporting and
prerequisite projection resolve that identity before consulting tentative VPATH
names. A rename no longer copies its original/resolved filename pair into both
tables. This removes two filename copies and one tentative-map entry per alias;
rebuilds can discard tentative names while retaining permanent identity.

The existing 118 original GPATH fixtures exercise goals, aliases, automatic
values, rule merging, scopes, archives, modes and overlapping jobs. Their expected
bytes and required status are unchanged. Full `make ci` and optimized-candidate
corpus/stateful/self-host checks validate the change; no new output exemptions,
filesystem observation cache or persistent graph cache are introduced. Corpus
coverage remains 3,104 required matches and 12 existing pending cases, with
52 stateful scenarios / 148 invocations.

The `gpath` benchmark suite now includes a mixed control: 1,000 permanently named
files and 1,000 files found through another VPATH directory outside GPATH. Pure
recipe assertions check the automatic prerequisite count and both path prefixes,
without launching children. This exercises both name tables in the same invocation.
Preparation writes physical input files with fixed equal historical timestamps
outside the timed runs. The benchmark tooling tests check this preparation.

A local 101-sample alternating paired run compares the deduplicated build with
the preceding GPATH implementation (`5491cb1`):

| Workload | Current rmake (ms) | Previous rmake (ms) | GNU 4.4.1 (ms) |
| --- | ---: | ---: | ---: |
| 1,000 explicit searched files | 5.105 | 5.291 | 2.780 |
| 1,000 original/canonical alias pairs | 5.884 | 6.023 | 3.179 |
| 400 implicit searched files | 3.194 | 3.212 | 2.182 |
| 1,000 permanent plus 1,000 tentative files | 8.246 | 8.347 | 5.361 |
| Startup | 0.606 | 0.609 | 0.552 |
| 10,000 ordinary no-op targets | 29.597 | 29.594 | 11.515 |
| 5,000 expansion assignments | 4.566 | 4.639 | 2.806 |

Median paired previous/current ratios are 1.035, 1.030, 0.998 and 1.009 for the
four GPATH rows. These are small local differences; the selected explicit/alias
workloads improve around 3%, with implicit, mixed and core results close. GNU
remains faster. There is no general speed-parity claim, CPU isolation, confidence
interval, resource profile or performance acceptance threshold. Runs include
startup, two warmups per engine, warm OS caches and exact output/status checks.

Reports with raw samples, source hashes and executable identities are ignored
`artifacts/bench/gpath-name-storage-101.json` and
`artifacts/bench/gpath-name-storage-core-101.json`. Current release SHA256:
`c2494df18874ddb25fc8a4eab4bf567ec89b2298980951459ddb6064a30383b2`.
Previous release SHA256:
`2d8ee0fc457659730c23c3bbda7e47615923bb595fec76d32db0767730f241f1`.

The [GPATH slice](gpath.md) records introduction behavior and remaining M4 work.
This storage cleanup does not close a milestone gate or justify persistent caching.
