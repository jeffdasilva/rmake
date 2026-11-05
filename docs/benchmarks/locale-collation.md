# Locale collation cost and rank optimization

The selected implementation fixes locale ordering and retains exact GNU tie
order. It is faster than the initial collation implementation on the measured
non-C glob controls. C.UTF8 still costs more than the preceding rmake executable.
These results do not establish general performance parity.

All three development campaigns explicitly acknowledged fully quiet. Commands
ran serially on shared CPU23, without frequency or exclusive-CPU controls,
confidence intervals, CPU time, RSS or I/O measurements. Six comparison reports
retain 31 alternating samples per engine/control. The new `wildcard-locale`
suite bounds warm/complex queries at 20 over 1,000 files; the historical
200-query suite remains unchanged. Each sample checks its complete count output,
status and stderr. The separate original 74-profile gate checks actual ordering,
raw bytes, equal keys, filesystem state and effects against two GNU executions.

The selected executable is `fbc6475b1fd3`; the initial implementation is
`8e2ec674fa57`; preceding mainline is `b3ccc5ecb1d6`. Medians are milliseconds.
Paired change is the reciprocal of median preceding/candidate ratio minus one;
it need not equal the ratio of independent medians.

| Locale / control | GNU | Preceding main | Selected | Paired change |
| --- | ---: | ---: | ---: | ---: |
| C cold | 2.132 | 1.961 | 1.991 | +0.55% |
| C warm | 5.382 | 3.008 | 3.044 | +0.99% |
| C complex | 5.572 | 3.352 | 3.339 | -0.11% |
| C literal | 1.954 | 2.160 | 2.161 | +0.58% |
| C.UTF8 cold | 2.255 | 2.102 | 2.228 | +6.30% |
| C.UTF8 warm | 5.999 | 3.134 | 3.746 | +20.88% |
| C.UTF8 complex | 6.535 | 4.272 | 4.898 | +15.32% |
| C.UTF8 literal | 2.056 | 2.295 | 2.306 | +0.76% |
| French cold | 2.970 | 2.133 | 2.363 | +9.69% |
| French warm | 20.610 | 3.193 | 5.432 | +69.51% |
| French complex | 21.225 | 4.366 | 6.669 | +51.69% |
| French literal | 2.101 | 2.323 | 2.354 | +0.87% |

Preceding mainline fails 33 of the 74 semantic ordering profiles, including
French. The count-only performance controls do not establish equivalent full
collation behavior. The C.UTF8 regression remains a real default cost, even
though these selected glob workloads still run faster than GNU.

Against the initial correct collation implementation, selected paired changes
are -1.71% / -18.47% / -14.50% for C.UTF8 cold/warm/complex, and
-24.98% / -75.26% / -71.67% for French. C controls range +0.43% to +3.37%; all
samples remain recorded. The optimization keeps byte-ordered directory emission
and carries original discovery ranks for equal collation keys. It neither
guesses locale behavior from an alias nor repeats directory/interpreter effects.

Two allocation repeats per executable cover five C-locale recipe-free controls:
tiny, large graph, expansion, 1,000 arguments and 512 arguments/100 live updates.
The reports retain calls, frees, requested bytes, raw traces and helper/source
identities. They do not measure actual locale-glob allocation, live/peak memory,
aligned/hidden allocator entries or native throughput.

The [initial screen](../../tests/locale-collation/performance-storage-collation-screen.json)
retains all 13 timing and 80 allocation reports, including rejected storage
variants and every initial regression. The
[ranked qualification](../../tests/locale-collation/ranked-qualification.json)
retains six follow-up timing and 20 allocation reports, both frozen builds,
74/183 complete observations in each build, authenticated locale provenance,
source/helper bytes, and the benchmark inventory test failure and correction.
Full milestone, wider locale/NLS, concurrent native mutation, persistent cache
and general performance acceptance remain open.
