# Target context allocations

Targets without matching pattern-variable declarations no longer allocate an
empty pattern scope. Matching declarations still activate in the same order and
retain their conditional/global effects. Empty local variable maps share an
immutable Arc; definitions use existing copy-on-write mutation. The invocation
working-directory bytes are immutable shared storage instead of a vector copied
by every variable-store clone. Replacing a store's directory remains local to
that store. No filesystem results or evaluations are cached.

The existing target-context isolation tests and the full conformance, generated
scope, stateful, runtime and self-hosting suites cover the changed storage paths.
All 2,478 required differential cases also pass with the optimized binary; the
same 12 known cases remain pending.

Two local 101-sample alternating comparisons used the preceding include-search
binary as their baseline. The target-context suite has 3,000 missing targets
whose recipes expand automatic values and abspath, and then become empty without
launching subprocesses. Separate workloads exercise no local declarations,
explicit declarations and pattern declarations. Both engines' output is checked.

| Workload | Previous rmake | Current rmake | GNU 4.4.1 | Median paired previous/current |
| --- | ---: | ---: | ---: | ---: |
| Tiny no-op | 0.586 ms | 0.583 ms | 0.521 ms | 1.008x |
| 10,000-target no-op | 28.413 ms | 28.200 ms | 11.656 ms | 1.007x |
| Expansion | 4.742 ms | 4.637 ms | 2.760 ms | 1.024x |
| Target contexts, no declarations | 17.307 ms | 16.889 ms | 12.176 ms | 1.025x |
| Target contexts, explicit declarations | 24.685 ms | 23.906 ms | 15.974 ms | 1.033x |
| Target contexts, pattern declarations | 22.557 ms | 22.045 ms | 14.951 ms | 1.028x |

These measurements suggest small gains of about 2–3% in expansion and the
focused context workloads; the core no-op differences are too small to establish
a useful speedup. GNU remains faster. Samples include startup, warm OS caches,
and background load, without CPU pinning, confidence intervals or resource
measurements. They do not establish a release performance gate.

Reproduce through `make bench-compare BENCH_SUITE=target-contexts BENCH_REPEAT=101
BENCH_BASELINE=/path/to/previous/rmake` on one line; use `BENCH_SUITE=core` for the
core control. Local raw samples and identities are in the ignored
`artifacts/bench/lazy-target-scopes-contexts-101.json` and
`artifacts/bench/lazy-target-scopes-and-cwd-101.json`.

- Previous source: `2721dd743c91314c62daae2d0a1f0cbbb9943e5e`
- Previous binary: `37a77e639e1d0bdcb692a4e0211063c59d84e433b380b5d1765d9317e3da9f7a`
- Current binary: `e39ac997682ce8b0e23ea0f7a272daea6af42f29ecdcf5396b238feb402ddd34`
- GNU binary: `b8806e8eb7452ed9b7ed5e18cb7e76ff9abd488c9d62a266a398d8044fade974`
