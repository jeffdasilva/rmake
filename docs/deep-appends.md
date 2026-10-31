# Deep inherited append layers

Inherited target-variable appends no longer consume one Rust stack frame per
outer binding. The direct evaluator first captures the same ordered outer
binding snapshots as its former recursive descent, then evaluates bodies from
outermost to innermost. The selected innermost body is already owned by the
caller and needs no extra clone. Ordinary non-append bodies retain a direct
path. Heap evaluation already had resumptions for append layers; those now use
storage accounting in place of the separate 128-layer rejection.

Literal-reference eligibility now uses a short ASCII shape scan, while eager
function preparation rejects the inapplicable pure-reference path from a short
function-name scan. Both avoid duplicate balanced-expression walks. Eligibility
performs no effects and retains the full reader for complex/malformed syntax;
later syntax cannot preempt an earlier effect. No syntax or value survives its
current evaluation.

Every body still expands in the selected file context. Private inherited
bindings are skipped by the existing lookup policy, exposing earlier ancestors
again. Empty suffixes retain their separators. Recursive and simple flavors,
raw values, metadata snapshots, active variable identities and diagnostic
contexts use the existing operations. All outer snapshots precede any body
effect, so an effect cannot invalidate suspended bytes or change which outer
body is evaluated. Fresh lookups inside each body remain fresh. Effects run
outer first, exactly once; no expanded value is cached.

A separator is checked before the following body, including before that body's
file or shell effects. Cancellation prevents further body effects and restores
active state. The native expansion stack guard remains unchanged. Beyond the
old 128-layer acceptance window, direct append snapshots have a 16 MiB extra
metadata/source budget. Heap append depth participates in the existing live
continuation budget while body logical depth retains its original meaning.
Completed resumptions release reservations. These are control-storage limits,
not measured process peak memory, and do not establish broad resource parity.
Internal limits receive no approved GNU crash/hang exception credit.

495 original required models comprise 492 semantic cells and three scaling
controls. Semantic cells cross depths 0, 3, 32, 96, 128 and 192; literal, file,
automatic and empty suffixes; recipe, secondary-expansion and export sites;
private inheritance; output-sync none/target; and direct/192-level function-body
entry points. Every independent model agrees with two bounded pinned GNU Make
4.4.1 observations. Both frozen builds match all 495; preceding rmake fails 144
and passes 351 controls. No expectation refinement or safety-exception promotion
was needed. File order, omitted private effects, automatic leaf context and
empty separators are checked independently of mere successful exit.

[Raw evidence](../tests/research/deep-appends.json) preserves original files and
modes, arguments, environments, all byte/status/filesystem observations, bounds
and executable identities. Historical observations earn no separate conformance
credit. The ordinary corpus is 15,124 required / zero pending, plus the same
17 separately gated approved safe errors.

Five additional Rust tests cover 2,048 append layers in both dispatch paths,
exact effect order, cancellation and active-state cleanup, extra storage
exhaustion before the outermost effect, and separator overflow before the next
body effect, plus an eager first-argument effect across a 1,024-level dispatch
transition without replay. They receive no GNU conformance credit. Seven tooling tests protect
inventory/evidence, paired entry points, effects/private inheritance, empty
separators, automatic context, site/mode coverage and every scaling read with a
wrong-value negative control. Formatting, lint, 498 Rust and 494 tooling tests
pass. Full frozen rmake-driven development/optimized acceptance and final-commit
CI remain required merge gates; outcomes are recorded in the PR.

Debug SHA256:
`76dad9c9b581a3f4bff2844e1ecc20314650f339726f281805767855246cd2e4`.
Release SHA256:
`09c4bb14aaca7cebe104e709ad561534a759e0a2446c32f98c62b9b920b52911`.
The [pinned oracle profile](../tests/research/deep-appends.json) remains
`92525c9784f4dd471cceb12ce66a9d367e267b638e1384cdc29962b738d16de7`,
GNU binary SHA256
`b8806e8eb7452ed9b7ed5e18cb7e76ff9abd488c9d62a266a398d8044fade974`,
and verified GNU source archive SHA256
`dd16fb1d67bfab79a72f5e8390735c49e3e8e70b4945a15ab1f81ddb78658fb3`.
Rust, fixtures and tooling are original. No GNU implementation/tests were copied
or translated, unsafe Rust added, GNU runtime fallback used or new approved
safe-error exception introduced.

This closes the separately recorded inherited-append depth gap, not M3/M6.
Parser eval/include and native callback recursion, broader output/resource
acceptance, native modules, enabled Guile, real self-exec, runtime HOME/tilde,
remaining CLI/debug/database and Linux/locale/integration profiles remain in the
[M3/M6 audit](m3-m6-audit.md). General performance parity and persistent cache
correctness remain open.

## Measurements

Six sequential reports contain two batches of 101 alternating samples per engine/workload, two warmups and exact output agreement. Pinned GNU Make drives A; frozen rmake drives B through the top-level Makefile. No unrelated local build/test/probe campaign runs during timing. Every timed input is correct on GNU, preceding rmake and the candidate; the newly corrected deep cases receive no timing credit. Host activity is uncontrolled, with no confidence intervals or performance thresholds.

| Workload | GNU / candidate / preceding median ms (A) | GNU / candidate / preceding median ms (B) | Paired preceding/candidate A / B |
|---|---:|---:|---:|
| tiny-noop | 0.626 / 0.743 / 0.743 | 0.492 / 0.598 / 0.614 | 1.002 / 1.017 |
| large-noop | 10.875 / 28.245 / 28.492 | 10.788 / 28.227 / 28.651 | 1.011 / 1.015 |
| expansion | 2.669 / 4.563 / 4.771 | 2.635 / 4.492 / 4.690 | 1.043 / 1.042 |
| function-bodies-0 | 0.890 / 1.308 / 1.332 | 0.888 / 1.292 / 1.330 | 1.018 / 1.029 |
| function-bodies-16 | 3.544 / 7.073 / 7.847 | 3.482 / 7.022 / 7.731 | 1.109 / 1.108 |
| function-bodies-48 | 10.192 / 22.638 / 24.884 | 10.131 / 22.653 / 24.735 | 1.096 / 1.092 |
| deep-appends-0 | 0.580 / 0.697 / 0.688 | 0.546 / 0.648 / 0.653 | 0.988 / 1.015 |
| deep-appends-32 | 0.804 / 1.238 / 1.227 | 0.815 / 1.266 / 1.248 | 0.995 / 0.993 |
| deep-appends-96 | 1.472 / 2.469 / 2.486 | 1.459 / 2.473 / 2.472 | 1.008 / 1.004 |

Ratios above one favor the candidate. The core expansion workload improves about 4%; valid function-body workloads improve about 9–11% over the preceding heap evaluator. Append controls are mixed, retaining small 32-layer regressions. The earlier function-body regression is reduced, not erased relative to the pre-heap version. Newly accepted deep inputs receive no timing credit. GNU remains faster overall in these workloads; no general performance parity, allocation reduction or peak-memory improvement is claimed.

Raw reports: [core A](benchmarks/appends-core-a.json), [core B](benchmarks/appends-core-b.json), [function-bodies A](benchmarks/appends-function-bodies-a.json), [function-bodies B](benchmarks/appends-function-bodies-b.json), [deep-appends A](benchmarks/appends-deep-appends-a.json), [deep-appends B](benchmarks/appends-deep-appends-b.json).
