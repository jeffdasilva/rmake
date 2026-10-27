# Shell argument construction and one-shell residual execution

This M3 follow-up closes five curated shell mismatches on the pinned GNU Make
4.4.1 Linux/glibc profile. It does not establish complete GNU compatibility.
The Rust implementation, Makefiles and argument observers are independently
authored. No GNU implementation or upstream tests were copied or translated.

## Configuration parsing

Ordinary recipes tokenize expanded `SHELL` and `.SHELLFLAGS` separately from the
protected recipe argument. An unescaped LF in configuration is literal argument
data on the fast path: `SHELL = ./capture A<LF>B` with flags `-c` passes one
`A<LF>B` argument. Double quotes in flags require an outer interpreter. At that
boundary unescaped configuration LFs fold, so flags `"-c"` instead produce `AB`.
Existing backslash-LF pairs remain for the interpreter; the recipe is protected
after configuration folding and retains its own newlines.

`.ONESHELL` keeps the entire expanded `SHELL` as an executable path. Its flags
use the ordinary preparer, but its resulting invocation is argument data for
that executable. For example flags `one "two words"` pass `/bin/sh`, `-c`,
`one "two words"`, then the entire script. The argument observer records these
bytes without executing them as a second configuration-parsing command.

## Residual execution

GNU's direct-command scan can encounter a quoted LF before any reason to use
the shell. In `.ONESHELL` that boundary leaves a residual recipe cursor even
though the first shell execution receives the whole script. A later execution
therefore receives the remaining tail. Shell syntax after that boundary does
not undo the earlier decision; earlier syntax, nonstandard flags, shell or IFS
can prevent it. An unquoted LF follows the ordinary whole-script path instead.

The implementation returns an owned invocation and an optional tail offset.
It tests the fast-prefix eligibility without mutating source bytes or exposing
borrowed pointers across child execution. The engine queues already-expanded
bytes, preserving the recipe's location, modifiers, cached child environment,
output ownership and ordinary failure/cleanup behavior. Make expansions and
export effects run once, even if the child script has multiple residual passes.

Actual execution echoes only the prefix up to the retained boundary before
launching the first whole script. Unforced `-n` echoes the whole script once
without constructing an execution plan. Forced `-n` performs execution and uses
the prefix echo. Original cases also check `-q`, `-t`, ignored failures,
`.DELETE_ON_ERROR`, custom recipe prefixes and each output synchronization mode.

## Acceptance and limits

Add 278 original required cases: 46 in `process.invocation` and 232 in
`recipes.oneshell`. Promote the five historical cases. These two ledger families
now have 95 and 298 required cases, respectively, with no pending exemptions.
The complete curated inventory has 5,967 cases: 5,966 required and one pending.
The remaining `let` empty-list case records an upstream signal; rmake continues
to return safely rather than deliberately crashing. The broader M3/M6 inventory
is incomplete, including recursive flag payloads, metadata, CLI and environment
vector serialization. Neither milestone is declared complete here.

Every new case was compared with two exact GNU observations and the candidate
before recording raw stdout/stderr/status anchors. The harness also compares
filesystem effects. Three tooling regressions protect promoted coverage,
independent normal/dry/forced echo anchors, and research classification. The
unit inventory is 425 Rust and 329 tooling tests. `make m5-acceptance` remains
the full sequential development and optimized gate, including controlled
runtime, stateful, graph, filesystem fault and self-hosted campaigns.

Five additional original flag probes produced GNU allocator aborts. Their raw
inputs, two oracle observations and bounded candidate outcomes are retained in
[the research record](../tests/research/gnu-441-shell-argv-aborts.json).
They are not passing conformance cases and are not executed by normal CI.
The two earlier allocator reproductions remain separate research. There is no
claim that rmake reproduces unsafe oracle memory corruption.

## Performance observation

Run `make bench-compare` with 101 alternating samples per engine, separately
for `BENCH_SUITE=core` and `BENCH_SUITE=parallel-branches`, against frozen
candidate and preceding-main binaries. No local builds or acceptance campaigns
ran during these measurements. Median elapsed milliseconds were:

| Workload | GNU 4.4.1 | Current | Preceding main |
| --- | ---: | ---: | ---: |
| Tiny no-op | 0.608 | 0.750 | 0.748 |
| 10,000-target no-op | 11.347 | 31.582 | 31.106 |
| 5,000 expansions | 2.813 | 4.612 | 4.671 |
| Parallel branches, depth 1 | 87.062 | 87.982 | 88.068 |
| Parallel branches, depth 4 | 87.941 | 89.023 | 88.924 |

Current measurements remain close to preceding main, with the large no-op
median about 1.5% higher. This unisolated synthetic run provides no statistical
or general performance-parity claim. GNU remains faster on all five workloads.
Raw reports are retained locally as
`artifacts/bench/shell-argv-final-{core,parallel}.json`. Candidate SHA256:
`76d053048b62cb607881c99589188db73c2feec41fcb3e748775f83f8fbdfda0`;
preceding main SHA256:
`4708224ec00aff3bc427af21dac7be8dd1a2a7949ef63103cddd759b2ac842e5`.

## Reference provenance

Source-reference study used the verified
[GNU Make 4.4.1 archive](https://ftp.gnu.org/gnu/make/make-4.4.1.tar.gz), SHA256
`dd16fb1d67bfab79a72f5e8390735c49e3e8e70b4945a15ab1f81ddb78658fb3`.
`src/job.c` command construction and job continuation, and `src/commands.c`
recipe chopping, explained executable observations. The reference is copyright
1988–2023 Free Software Foundation, Inc., GPL-3.0-or-later. Sources remain
outside the repository. The pinned local oracle binary SHA256 is
`b8806e8eb7452ed9b7ed5e18cb7e76ff9abd488c9d62a266a398d8044fade974`.
