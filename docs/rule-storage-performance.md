# Cost of grouped-rule storage

Grouped output support added metadata to every rule and graph continuation.
Keep the peer list, implicit identities and peer pattern flags in an optional
owned `RuleGroup`, and the recipe view/cursor in an optional owned `GroupState`.
Ordinary rules and frames carry only an optional pointer. On this x86-64 build,
`Rule` is 208 bytes; before grouped support it was 200 bytes.

Serialization policy is calculated once after prerequisite expansion for each
visited file phase, including peer phases. Leaves skip it. This avoids a
`.NOTPARALLEL` table lookup for every incoming prerequisite while preserving the
peer-specific ordering decisions. All data remains owned, with ordinary safe
Rust access and no persistent cache.

The 123 grouped fixtures passed before the performance measurement. Full
conformance, stateful, mode, generated, repeated-progress, jobserver and
self-hosting gates validate the shared paths. A local comparison used two
warmups and 101 alternating samples per engine:

| Workload | Previous rmake median | Compact storage median | GNU median | Median paired previous/current |
| --- | ---: | ---: | ---: | ---: |
| Tiny no-op | 0.517 ms | 0.516 ms | 0.486 ms | 0.997x |
| 10,000-target no-op | 28.829 ms | 27.739 ms | 11.612 ms | 1.042x |
| Expansion | 4.720 ms | 4.843 ms | 2.775 ms | 0.979x |

The large no-op workload improves by about 4%. Tiny startup is unchanged;
expansion is slightly slower in this run and shows no improvement. GNU remains
faster. These are local wall-clock observations without CPU pinning or a
confidence interval, including startup and background load. They do not show
that every workload improved or that all cost of the new features was recovered.

Reproduce with `make bench-compare BENCH_SUITE=core BENCH_REPEAT=101
BENCH_BASELINE=/path/to/previous/rmake` on one shell line. Raw samples, workload
hashes, arguments and identities are retained locally in the ignored
`artifacts/bench/grouped-compact-frames-101.json`.

- Previous source: `be3942363b3fc62b9f44080c966a60b6990c30af`
- Previous binary: `eaa152eb85043d4df373636aa2699c3a70662b4ae8fd91e10937a377363e4db1`
- Candidate binary: `938a2020d3cd9111500bb140312baef6146c9c9c2aabb36b6184856b4788529a`
- GNU binary: `d129996c7f842a77375573eae5b188a6e24510a076658df5e0dd9ffd92571625`
