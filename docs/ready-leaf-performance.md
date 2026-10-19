# Avoiding graph-frame work for leaves

A rule without prerequisites cannot suspend dependency traversal. After scope
ownership and secondary expansion, it now proceeds directly to the existing
recipe finalization path. It avoids inserting/removing a pending graph frame;
recipe expansion, automatic variables, process/jobserver handling, cleanup and
outcome recording remain in the same code. Successful outcomes also short-circuit
the invocation-stop check before querying parallel policy.

The existing full CI, exact differential corpus, stateful and mode campaigns,
jobserver matrix and self-hosting validate this shared execution path. No semantic
cache or output normalization is introduced. Required/pending inventory counts are
unchanged: 2,232 required and 13 pending out of 2,245 cases.

A local comparison against the immediately preceding optimized scheduler build
used two warmups and 101 alternating measured samples per engine:

| Workload | Previous rmake median | Leaf fast path median | GNU median | Median paired previous/current |
| --- | ---: | ---: | ---: | ---: |
| Tiny no-op | 0.646 ms | 0.665 ms | 0.591 ms | 0.988x |
| 10,000-target no-op | 36.686 ms | 33.885 ms | 12.339 ms | 1.080x |
| Expansion | 5.121 ms | 5.079 ms | 2.963 ms | 1.010x |

Exact output and successful status matched throughout. The large no-op workload
improves; the other two provide no meaningful speedup evidence. GNU remains
faster. This is an unpinned local wall-time measurement with process startup,
background load, no confidence interval and no acceptance threshold. It does not
establish general performance parity or superiority.

Reproduce with `make bench-compare BENCH_SUITE=core BENCH_REPEAT=101
BENCH_BASELINE=/path/to/previous/rmake` (on one shell line). Raw samples, arguments,
Makefile hashes and identities are in the ignored local
`artifacts/bench/leaf-fastpath-101.json`.

- Previous binary: `b44a75fc594305fdbf0843980bb057f30a95ef7085255d9c246ba918639f19e2`
- Candidate binary: `4fa304fbc8a7b95ca394d66f1a4b04e633bf097aa138e7d9a68bed4893138500`
- Oracle: `d129996c7f842a77375573eae5b188a6e24510a076658df5e0dd9ffd92571625`
