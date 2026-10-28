# GNU 4.4.1 POSIX mode on Linux

An actual `.POSIX` rule now activates sticky POSIX behavior while reading. Names
produced by ordinary target expansion, included files and eval participate.
References through `.PHONY`, prerequisites or target-specific assignments alone
do not activate the mode. Repeated declarations do not reinstall deleted or
changed defaults; a restarted Makefile read begins with fresh invocation state.

The existing byte reader now receives that mode before each logical line.
Whitespace before a backslash-newline is preserved and adjacent joins remain
separate. Mode changes within includes or eval affect subsequent outer reads.
GNU expression, rule and assignment extensions retain their existing behavior.

Activation installs these simple default-origin values, respecting stronger
bindings: `.SHELLFLAGS=-ec`, `CC=c99`, `CFLAGS=-O1`, `FC=fort77`, `FFLAGS=-O1`,
`SCCSGETFLAGS=-s` and `ARFLAGS=-rv`. Default shell flags become `-c` only for a
recipe's per-command ignore prefix. Global `-i` and `.IGNORE` keep the POSIX
shell's `-e` behavior and separately ignore its failure. User shell-flag bindings
retain their raw expansion and origin behavior, including empty/undefined values,
`.ONESHELL`, shell functions, forced execution, dry-run, query and touch modes.

A two-suffix rule with prerequisites is retained as an ordinary rule rather than
converted to an implicit rule. Single-suffix behavior follows the reference's
separate conversion path. Suffix order, archive conversions and secondary
expansion continue through the existing resolver.

After reading, recursive `MAKEFLAGS` uses GNU's hidden original command-variable
snapshot in POSIX mode. User `MAKEOVERRIDES` changes still participate in the
post-read environment-origin replay. Ordinary mode also now removes a stale
command-variable separator when `MAKEOVERRIDES` becomes empty or undefined.
Dynamic flag assignment recognizes either encoded reference. Startup special
variables that GNU initializes before option decoding no longer acquire an
artificial environment-override origin from a later default initialization:
`.SHELLFLAGS`, `.RECIPEPREFIX`, `.LOADED`, `.FEATURES` and `.VARIABLES` retain the
observed phase, while later assignment conflicts still promote under `-e`.

## Acceptance

Add 390 original required fixtures, each with two exact GNU Make 4.4.1
observations and a final candidate comparison. Coverage spans four built-in
profiles, actual and non-activating declarations, defaults and all tested origins,
reader/include/eval/restart lifetime, line folding, shell and error modes,
suffix conversions, metadata and recursive command variables. Restart inputs
explicitly establish a historical included-file timestamp rather than depending
on its creation time. Stream, status and generated-file comparisons remain exact.
Three core tests cover sticky reader state, non-activation and suffix eligibility;
three tooling tests protect independent error-mode and propagation anchors.

The corpus is now 7,357 total / 7,356 required / one pending; unit totals are
436 Rust and 341 tooling tests. `make test-diff` and `make test-tools` include the
new coverage, with full sequential development/optimized acceptance and
exact-head CI remaining merge gates. This slice closes the explicit `.POSIX`
rejection. Broader [M3/M6 surfaces and safety conflicts](m3-m6-audit.md) remain
open; the curated pending count is not a complete inventory.

## Performance observations

An alternating 101-sample comparison against GNU and the previous environment
implementation produced these unisolated medians. No builds or acceptance runs
were concurrent with timing.

| Workload | GNU | Current | Previous |
| --- | ---: | ---: | ---: |
| Tiny no-op | 0.634 ms | 0.715 ms | 0.737 ms |
| 10,000-target no-op | 12.501 ms | 32.367 ms | 32.237 ms |
| 5,000 expansions | 2.896 ms | 4.933 ms | 4.917 ms |
| Parallel branches, depth 1 | 87.470 ms | 89.899 ms | 89.723 ms |
| Parallel branches, depth 4 | 87.680 ms | 89.871 ms | 90.176 ms |

The observations remain close to the preceding candidate. They do not establish
statistical performance or general parity, and GNU is faster on all workloads.
Raw reports: `artifacts/bench/posix-final-{core,parallel}.json`. Candidate SHA256:
`85b1508a291920d75153f4caa8a07769a8901387fe4ee3c5875cf499fe5b8a9b`;
preceding candidate:
`7aa347b45de3612bbd28517c86138967cd2b9ce1d417982d65d7216258208268`.

## Reference provenance

The verified GNU 4.4.1 archive, SHA256
`dd16fb1d67bfab79a72f5e8390735c49e3e8e70b4945a15ab1f81ddb78658fb3`,
was consulted at `src/read.c` (special target activation), `src/misc.c`
(continuation behavior), `src/rule.c` (suffix eligibility), `src/job.c` (default
shell flags), and `src/main.c` (startup origins and recursive flags). The Rust
implementation, Makefiles, tooling checks and expected byte anchors were
independently authored. No GNU implementation or upstream tests were copied or
translated, and GNU is never an implementation fallback.
