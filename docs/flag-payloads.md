# Assignments in changed make flags

This M3 follow-up interprets assignments embedded in changed `MAKEFLAGS` and
`GNUMAKEFLAGS` values. It closes the previous explicit assignment rejection;
it does not close all M3/M6 gaps.

## Evaluation and origins

Reader assignments to `MAKEFLAGS` decode immediately. Their payload definitions
inherit the triggering assignment origin, including `override`; later ordinary
file definitions can replace file-origin payload values. Existing command-line
values retain their precedence. `GNUMAKEFLAGS` payloads decode after reading,
with environment origin and the then-applicable environment override policy.
The policy at each assignment's argument position matters: `X=value -e` differs
from `-e X=value` when an imported X already exists.

The host returns owned assignment tokens and their option positions/policies.
The ordinary database evaluator expands their names and evaluates recursive,
simple, immediate-recursive, append, conditional and shell forms. Definitions
have invocation provenance, and comments/semicolons inside a token remain data.
Nested `eval`, output, shell, filesystem and cancellation effects use the same
receivers as ordinary evaluation. Later definitions see earlier effects.

Expanded payloads do not consume make-dollar quoting a second time. The lexer
skips C whitespace at the beginning, then separates only spaces and tabs;
embedded CR, LF, VT and FF retain their observed byte behavior. Startup encoded
flags and canonical option storage retain their separate dollar-quoting layer.
The existing startup CLI override snapshot is not reconstructed from file-added
payload definitions.

An owned cursor retains reproducible nested-decoder argument position effects.
Empty nested flag values leave that cursor alone, matching GNU's early return.
Depth and replay budgets bound self-repeating definitions without dangling
argument storage or unbounded interpreter recursion. Reborrowed evaluation
contexts share the cursor only within that option interpretation.

## Coverage and safety conflicts

Add 454 required original Makefiles under `variables.flag-payloads`, including
all seven assignment forms across six interpretation phases and seven origin
profiles, plus raw bytes, effects, exports, restarts and recursive inheritance.
Each case has two GNU observations and an exact candidate comparison before
recording independent stdout/stderr/status anchors. Filesystem effects also
remain under the normal differential comparator. The curated inventory becomes
6,421 total / 6,420 required / one upstream-signal pending. The unit inventory
is 427 Rust and 332 tooling tests.

Six additional observations remain in
[the decoder research record](../tests/research/gnu-441-flag-decoder.json), with
original inputs, two raw oracle runs and bounded candidate outcomes. Five GNU
inputs timed out. One finite nested decode unexpectedly interpreted later word
bytes as options; an original bounded `getopt_long` interposer confirmed those
calls. Global getopt state retaining stack-local argument storage is a
source-informed explanation, rather than a passing equivalence claim. The
research is not run by normal CI or counted as passing conformance. These remain
explicit compatibility/safety conflicts under the compatibility contract.

Recursive inheritance fixtures pass `--no-print-directory` explicitly to isolate
binding behavior. GNU's implicit recursive directory messages remain an M6
presentation gap; these fixtures do not establish that surface. Inherited
file/directory switches and broader option ordering also remain open. See the
[M3/M6 gap audit](m3-m6-audit.md).

## Reference provenance

Reference study used `src/main.c` (`handle_non_switch_argument`,
`decode_switches`, `decode_env_switches`, `reset_makeflags`, startup override
snapshot construction) and `src/variable.c` (`do_variable_definition`,
`set_special_var`) from the verified GNU Make 4.4.1 archive, SHA256
`dd16fb1d67bfab79a72f5e8390735c49e3e8e70b4945a15ab1f81ddb78658fb3`.
The upstream source is copyright 1988–2023 Free Software Foundation, Inc.,
GPL-3.0-or-later. Rust code, probes, interposer and fixtures are independently
authored; no upstream implementation or tests were copied or translated.

`make m5-acceptance` remains the full sequential development/optimized gate.
Exact-head CI must pass before merging. Benchmark results are observations on
the local profile, not a claim of general speed parity.

With 101 alternating samples per engine and no concurrent local builds/tests,
median milliseconds were:

| Workload | GNU | Current | Preceding main |
| --- | ---: | ---: | ---: |
| Tiny no-op | 0.682 | 0.810 | 0.800 |
| 10,000-target no-op | 11.927 | 35.852 | 36.293 |
| 5,000 expansions | 3.141 | 5.300 | 5.277 |
| Parallel branches, depth 1 | 87.165 | 88.422 | 88.109 |
| Parallel branches, depth 4 | 87.547 | 89.102 | 88.872 |

These unisolated observations remain close to preceding main; GNU is faster on
all five workloads. No confidence interval or general parity claim follows.
Raw reports: `artifacts/bench/flag-payload-final-{core,parallel}.json`.
Candidate SHA256:
`7505a44f569c60ab3da2408ee93e4c2ce58074ed91b0971df4873a84d20f647b`;
preceding-main SHA256:
`76d053048b62cb607881c99589188db73c2feec41fcb3e748775f83f8fbdfda0`.
