# Avoid unused automatic-variable snapshots

Graph completion previously built owned canonical prerequisite-name snapshots
for rules with no recipe, and automatic-variable value arrays for explicit
empty recipe vectors. Those arrays are never read: recipe dispatch returns
before establishing automatic bindings for an empty vector.

Ungrouped rules without a nonempty recipe now retain borrowed normal-name
membership for freshness and skip the owned automatic snapshot. They still wait
for dependencies, propagate failures/query state, handle intermediates and
perform ordinary completion/cleanup. Grouped rules keep their existing canonical
view because peer freshness and recipe automatic dependencies have distinct
views. Nonempty recipe vectors keep the same preparation and expansion path.

The value array is constructed lazily on the stack. This adds no heap allocation
to the nonempty-recipe path. Raw recipe contents are not classified or cached;
nonempty bodies that expand to empty still run their effects and access automatic
variables. Touch and `.ONESHELL` shortcuts remain in the existing dispatcher.
No mutable semantic cache, new filesystem probe, unsafe code or GNU runtime
fallback is introduced.

## Allocation observation

The trusted, output-checked 10,000-target workload recorded:

| glibc request metric | Preceding release | Current |
| --- | ---: | ---: |
| Allocation requests | 274,775 | 244,722 |
| Requested bytes | 24,131,875 | 20,438,345 |

That is 30,053 fewer requests (about 10.9%) and about 15.3% fewer requested bytes
in this observation. The profiler observes glibc malloc/calloc/realloc/free after
helper initialization and excludes aligned entry points. Requested bytes are
not retained or peak memory, and sampled stacks are not CPU timings. Profile
output paths have equal lengths; both executions validate exact output. Raw
[baseline](benchmarks/automatic-baseline-allocations.json) and
[current](benchmarks/automatic-current-allocations.json) reports retain workload,
helper and executable identities, counters and sampled stacks.

## Runtime observations

Frozen comparisons use 101 alternating, exact-output-checked samples with no
concurrent build or acceptance campaign. Host activity is not controlled.

| Workload | GNU ms | Current ms | Preceding release ms |
| --- | ---: | ---: | ---: |
| Tiny no-op | 0.631 | 0.737 | 0.739 |
| 10,000 explicit targets | 13.926 | 36.454 | 38.557 |
| Expansion | 3.010 | 5.207 | 5.205 |
| Parallel branches, depth 1 | 88.096 | 90.134 | 90.136 |
| Parallel branches, depth 4 | 87.721 | 90.245 | 90.513 |

The large-graph median is about 5.5% lower in this run. Other medians are close
and mixed. These samples establish neither a statistical speedup/regression nor
general performance parity; GNU remains faster on standard workloads. Raw
[core](benchmarks/automatic-core.json) and
[parallel](benchmarks/automatic-parallel.json) reports retain every sample.

## Validation boundary

The prototype passed all 9,796 required ordinary GNU comparisons and 453 Rust
tests. The final source passed formatting, warnings-denied lint, 453 Rust and
381 tooling tests. This optimization adds no conformance credit or safety
exception; the existing corpus, generated graph/scope/filesystem campaigns,
runtime/interoperability gates, self-hosting, and 17 separate safe-error cases
remain mandatory. Full local development/optimized acceptance and exact-head CI
remain merge gates. M3/M6 and the complete compatibility/profile/performance
inventory remain open.
