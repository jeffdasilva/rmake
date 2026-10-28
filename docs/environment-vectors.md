# Owned Linux child environment vectors

Recipes and shell expansions now launch with a complete owned vector in the
interpreter's selected export order. Rust's ordinary `Command` environment map
sorts entries; that loses observable ordering and can reject logical names that
GNU passes through `execve`. The Linux adapter owns the byte strings, argument
vector and pointer arrays until execution. It preserves non-UTF-8 values/names,
spaces and equals signs, including distinct logical names that serialize to
entries with the same apparent key. No implicit parent-environment merge can
restore undefined or unexported bindings.

All allocation and validation occur in the parent. The final child hook calls
`execve` with immutable owned storage after signal, descriptor, stdin, output and
jobserver setup. It allocates nothing, acquires no locks and never mutates the
parent environment. Linux unsafe pointer ownership is isolated in the platform
crate; the interpreter and core retain their unsafe-code prohibition. The parent
launcher retains PATH search, EACCES precedence and ENOEXEC fallback with the
actual attempted executable. Undefined PATH uses the system default search path.

The selected environment keeps synthesized `MAKELEVEL` in its GNU table slot
when eligible, or adds its numeric child value at the end. GNU's saved original
`SHELL` is invocation metadata distinct from the Make variable: it can remain
forwarded after `unexport` or `undefine`. An active recursive export falls back
to the process value, or an empty value when absent. Evaluation and its effects
remain ordered; cached recipe environments retain their selected vector across
residual commands and `.ONESHELL` boundaries.

`.EXPORT_ALL_VARIABLES` activates when dependencies are frozen after reading,
including activation through `.PHONY`; merely naming it as a prerequisite does
not activate it. Bare `export` still takes immediate effect. Runtime eval of
`export` or `unexport` changes subsequent selection without reactivating an old
special target. Original phase comparisons include secondary expansion and
makefile restarts.

## Evidence and limitations

The slice adds 278 required original fixtures, each observed twice with GNU Make
4.4.1 and compared with the final candidate. Original C observers inspect the
actual native `envp`, without sorting or normalizing output. Cases cover built-in
profiles, imported and synthesized bindings, target/pattern policy, raw bytes,
invalid shell identifiers, undefinition, recursive exports, eval effects,
`.ONESHELL`, pipe/FIFO jobservers, output synchronization, recursion and executable
fallback. Raw stream/status and generated-file comparisons remain enabled. The
corpus now has 6,967 cases: 6,966 required and one pending.

Three additional original inputs delete a currently expanding export and crash
GNU with SIGSEGV in both observations. Their raw inputs and outcomes are retained
in [the safety record](../tests/research/gnu-441-environment-deletion.json).
The historical candidate terminated safely. The owner subsequently approved
[documented fatal errors](safety-exceptions.md); a separate candidate-only gate
now enforces S003 without conformance credit. M3/M6 remain open; see the [remaining audit](m3-m6-audit.md).

Post-merge CI also exposed a pre-existing clock race in two archive touch
fixtures: the second header write observes a live file modification second.
The fixtures now use an original narrow stat/fstat/statx adapter that fixes only
`archive.a`'s observed modification time. Header bytes, payload, outputs and file
operations still undergo exact comparison. No output filter or timestamp-field
mask is introduced. The controlled archive campaign continues to check ordinary
unmodified adapter behavior and timestamp relations.

## Performance observations

An alternating 101-sample comparison against GNU and the preceding metadata
implementation produced these unisolated medians. No build or acceptance run
was concurrent with timing.

| Workload | GNU | Current | Previous |
| --- | ---: | ---: | ---: |
| Tiny no-op | 1.328 ms | 1.741 ms | 1.712 ms |
| 10,000-target no-op | 16.161 ms | 54.674 ms | 55.367 ms |
| 5,000 expansions | 4.018 ms | 6.284 ms | 6.508 ms |
| Parallel branches, depth 1 | 87.176 ms | 89.311 ms | 88.049 ms |
| Parallel branches, depth 4 | 87.707 ms | 90.356 ms | 89.101 ms |

The owned post-fork launch adds about 1.4 percent to these parallel medians.
This cost preserves observable environment order and ownership safety. These
measurements establish neither a statistical performance result nor general
parity; GNU remains faster on every workload. Raw reports are
`artifacts/bench/envp-final-{core,parallel}.json`. Candidate SHA256:
`7aa347b45de3612bbd28517c86138967cd2b9ce1d417982d65d7216258208268`;
preceding candidate:
`ebb0af892041ad8f9d52cdf1197e5bbf82020b66d44693563089b02ed474f49b`.
Full sequential `make m5-acceptance` and exact-head CI remain merge gates.

## Reference provenance

The verified GNU 4.4.1 archive, SHA256
`dd16fb1d67bfab79a72f5e8390735c49e3e8e70b4945a15ab1f81ddb78658fb3`,
was consulted at `src/variable.c` (`target_environment`), `src/file.c`
(`snap_deps`) and `src/job.c` (launch/path behavior). The Rust ownership design,
C observers/clock adapters, Makefiles and anchors are original; no GNU
implementation or upstream tests were copied or translated. GNU is exclusively
the test reference and never an implementation fallback.
