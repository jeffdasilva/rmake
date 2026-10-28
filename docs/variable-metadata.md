# Variable metadata on the pinned Linux profile

Implement GNU Make 4.4.1's `.VARIABLES`, `.FEATURES`, `.LOADED`, default
`.RECIPEPREFIX`, `MAKE_HOST`, initial `.DEFAULT_GOAL` and legacy `MFLAGS`
values on the pinned x86-64/glibc, Guile-disabled profile. This closes the
previous occupancy-only placeholders. Broader native/profile and presentation
work remains in the [M3/M6 audit](m3-m6-audit.md).

## Ordered names and lookup lifetime

`.VARIABLES` enumerates the actual global bindings in GNU's table order. It
excludes target, pattern, automatic and temporary function locals. Preserve
initial metadata/default-variable ordering, including `MFLAGS` before default
variables and the default `GNUMAKEFLAGS` binding when built-in variables are
enabled. Post-read clearing also creates `GNUMAKEFLAGS` when initially absent.
This supersedes the earlier export follow-up's absent-binding description.

GNU caches the raw names value until global membership changes. Assigning to
`.VARIABLES` retains its special nature, origin and ordinary flavor. A lookup
may replace its raw value after membership changes; otherwise user-assigned
recursive values retain their expansions and effects. `origin`, `flavor` and
`value` lookups participate in that lifetime. Undefining and recreating the name
produces an ordinary variable. Outer target/pattern appends use the stored
global value at the observed phase without an extra special lookup.

The Rust representation shares immutable bindings and lazily allocates an owned
membership cache only when queried. Membership mutations isolate initialized
caches across copy-on-write snapshots. A lock protects owned cached bytes;
expansion runs after releasing it. Only raw introspection bytes are cached:
recursive expansion, output and nested eval effects still execute. Names remain
byte-preserving, and the standard expansion limit bounds the result.

## Legacy flags and other bindings

`MFLAGS` has environment origin, recursive flavor and option bytes without the
command-variable suffix. Environment override mode and stronger user bindings
retain GNU precedence. Recipe-time flag evaluation can introduce a local
`MFLAGS` binding over stronger global bindings; it must not rewrite unrelated
global state. Startup, changed flags, post-read replay, jobserver setup and
restarts update the appropriate binding.

`.FEATURES` reports the pinned GNU profile's words, while `.LOADED` starts empty.
Their metadata values do not establish implementation of native loading or
Guile-enabled profiles: those adapters remain explicitly open. `MAKE_HOST` here
matches `x86_64-pc-linux-gnu`; broader architecture/libc profile acceptance remains
open. Default `.RECIPEPREFIX` and `.DEFAULT_GOAL` bindings retain their ordinary
assignment and rule-selection behavior.

## Acceptance and performance

Add 268 original required cases: four built-in profiles, assignment and lookup
lifetime, scope and effect cases, legacy flag precedence, metadata queries, and
name-table growth through 2,048 generated bindings. Every case has two exact
GNU observations and a final candidate comparison, with normal stream, status
and filesystem comparison. Three Rust tests protect snapshot isolation,
recursive effects and exclusion of automatic locals; three tooling tests protect
coverage, independent effect anchors and research classification. The corpus is
6,689 total / 6,688 required / one pending. Unit totals: 430 Rust / 335 tooling.

An additional GNU default-goal undefinition probe produces SIGSEGV. Its original
input and two observations remain in [the safety record](../tests/research/gnu-441-metadata-signal.json).
The historical candidate completed safely. The owner subsequently approved the
[S002 fatal safety error](safety-exceptions.md), tested separately without passing
conformance credit or deliberate memory corruption.

The first eager implementation regressed the expansion benchmark about 20%.
A single-search insertion path and allocation only on metadata lookup reduced
that cost. The final 101-sample alternating observations were:

| Workload | GNU | Current | Preceding implementation |
| --- | ---: | ---: | ---: |
| Tiny no-op | 0.576 ms | 0.694 ms | 0.676 ms |
| 10,000-target no-op | 11.875 ms | 33.113 ms | 33.292 ms |
| 5,000 expansions | 2.879 ms | 4.985 ms | 4.836 ms |
| Parallel branches, depth 1 | 87.219 ms | 87.892 ms | 88.129 ms |
| Parallel branches, depth 4 | 87.388 ms | 88.523 ms | 88.624 ms |

The expansion median remains about 3.1% higher; these unisolated synthetic
observations provide no statistical or general performance-parity claim. GNU
remains faster on all five workloads. No builds or local acceptance runs were
concurrent with timing. Raw reports:
`artifacts/bench/metadata-final2-{core,parallel}.json`. Candidate SHA256:
`ebb0af892041ad8f9d52cdf1197e5bbf82020b66d44693563089b02ed474f49b`;
preceding implementation:
`7505a44f569c60ab3da2408ee93e4c2ce58074ed91b0971df4873a84d20f647b`.

Full sequential `make m5-acceptance` and exact-head CI remain merge gates.
No M3/M6 completion or general speed parity is claimed by this slice.

## Reference provenance

Source-reference study used `src/variable.c` (`lookup_special_var`, automatic
bindings and target/environment lookup), `src/main.c` (startup phases,
`define_makeflags` and post-read replay), and `src/default.c` (default variable
membership) from the verified GNU Make 4.4.1 archive, SHA256
`dd16fb1d67bfab79a72f5e8390735c49e3e8e70b4945a15ab1f81ddb78658fb3`.
The reference is copyright 1988–2023 Free Software Foundation, Inc.,
GPL-3.0-or-later. The Rust representation and tests are independently authored;
no GNU implementation or upstream tests were copied or translated. Observable
metadata literals describe the pinned interface. Existing attributed built-in
rule/variable data remains unchanged.
