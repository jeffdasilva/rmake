# Restart directories and recursive invocation identity

A remade Makefile now replays the original `-C` sequence from the original
startup directory. Retargeting a symlink or replacing a directory during the
first read therefore changes the namespace of the restarted read, as it does
in GNU Make 4.4.1. Without `-C`, a renamed current directory remains current.
The filesystem working directory and file-origin `CURDIR` are refreshed after
reentry. Stronger user bindings can still hide `CURDIR`; `abspath`, include
search, wildcards and implicit prerequisites use the actual new namespace.
Late changes to flags do not replace the original directory arguments.

An unavailable original directory produces the observed `chdir` and fatal
messages. A failure while replaying `-C` reports the failing argument. GNU
retires an owned jobserver before restoring its directory; if that restoration
fails, its second cleanup emits an internal token-count warning. rmake retains
only the retired pool's creator metadata to reproduce that diagnostic. Its RAII
ownership still closes descriptors and returns live permits. Joined pools carry
no creator size, and ordinary pool cleanup is unchanged.

Default `MAKE` is now the recursive value `$(MAKE_COMMAND)`, with default origin.
`MAKE_COMMAND` remains simple. A relative invocation containing a slash is
prefixed with the startup working directory before any `-C`, retaining lexical
path components. Recursive recipes can therefore invoke the same executable
from the changed directory. Stronger environment, file and CLI values retain
their normal priority. Initial directory errors retain non-UTF-8 argument bytes.

## Acceptance

The 348 original required fixtures include 288 namespace cases: six namespace
operations, three Makefile/flag/CURDIR styles, two recursion levels, serial and
three local jobserver settings, and separate/merged output. The remaining cases
cover 48 default/override metadata profiles, eight relative recursive invocation
profiles and four non-UTF-8 directory success/failure inputs. Each case was
observed twice with the pinned GNU reference and compared with the candidate.
Generated files, link targets, status and streams are compared without output
normalization. All renames and replacements remain within the owned test root.

One Rust test checks creator metadata, joining and token conservation on both
transports. Four tooling tests protect coverage, recursive flavor and child
execution, namespace distinctions, raw diagnostic bytes and the explicit open
CLI case. The corpus is 8,173 total / 8,172 required / one pending, with 438 Rust
and 349 tooling tests. Full development/optimized acceptance and exact-head CI
remain merge gates.

The implementation retains the existing in-process restart loop. A complete
self-exec lifecycle for native/Guile process state, startup assignment ordering,
winning command-variable replay, directory reporting and broader CLI behavior
remain in the [M3/M6 audit](m3-m6-audit.md). In particular, empty `-C` produces a
GNU option diagnostic plus usage text; the candidate's current startup error
differs. Its raw two-reference observations and input are retained in
`tests/research/cli-empty-directory.json`, without passing conformance credit.
This slice does not close M3 or M6.

## Reference provenance

The verified GNU 4.4.1 archive, SHA256
`dd16fb1d67bfab79a72f5e8390735c49e3e8e70b4945a15ab1f81ddb78658fb3`,
was consulted at `src/main.c` for invocation initialization, restart directory
restoration and retired-master cleanup, with variable origin behavior checked
against the pinned reference. All Rust code, fixtures, setup scripts and tooling
checks were independently authored. No GNU implementation or upstream test code
was copied or translated. GNU is never an implementation fallback.

## Performance observations

An alternating 101-sample comparison against GNU and the preceding switch-origin
candidate recorded these unisolated medians. No builds or acceptance runs were
concurrent with timing.

| Workload | GNU | Current | Previous |
| --- | ---: | ---: | ---: |
| Tiny no-op | 0.873 ms | 1.049 ms | 1.092 ms |
| 10,000-target no-op | 13.938 ms | 39.345 ms | 39.815 ms |
| 5,000 expansions | 4.187 ms | 7.444 ms | 7.502 ms |
| Parallel branches, depth 1 | 87.909 ms | 89.974 ms | 89.961 ms |
| Parallel branches, depth 4 | 87.748 ms | 89.612 ms | 89.766 ms |

These observations remain close to the previous candidate and do not establish
statistical improvement or general performance parity. GNU is faster in all five
workloads. Raw reports: `artifacts/bench/restart-final-{core,parallel}.json`.
Current candidate SHA256:
`7e9fe304bb3d2f529e7d73cf90a17df7e2c47ff06b73dac68243b080023336dd`;
previous candidate:
`d15be250ba9f0c95efb92967d1b9332ead74ac01cffce22556eb00bc5706a58e`.
