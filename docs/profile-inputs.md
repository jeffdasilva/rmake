# Startup flag paths and expression profiles

Well-formed file and directory options in startup `MAKEFLAGS` and `GNUMAKEFLAGS`
now participate in the original invocation. Their ordering with explicit CLI
options is retained. Later file/environment-origin decoding consumes those
arguments without changing the active Makefiles or directory. Ordinary runtime
normalization and recursive propagation omit the non-propagating path switches.
This removes two explicit unsupported errors, without claiming complete startup
assignment ordering or malformed-option behavior.

The pinned reference is built without Guile. In that profile `guile` is an
ordinary variable/function-variable name, rather than a reserved unsupported
builtin. Direct references, computed names and `call` use normal lookup, scope,
argument and effect rules. The feature list remains Guile-free. An enabled
Guile reference, Scheme adapter and native registration lifecycle remain open.

## Reference boundaries

Expression expansion now distinguishes ordinary variable names from recognized
function calls. A simple name without a dollar before the first closing delimiter
ends there, even if its text contains a literal opening delimiter. Computed names
with nested dollar references follow balanced expansion. GNU's unmatched nested
name fallback looks up the raw prefix and consumes the remainder of that
expansion buffer. Function calls retain structural balancing and now report the
specific function and missing delimiter when unterminated. Reader and argument
scans keep their separate structural rules.

The scanner only searches the current reference prefix before inspecting its
function name, avoiding a scan across the entire remaining expression for every
short reference. All operations use byte slices; no UTF-8 decoding determines
reference boundaries or variable identities.

## Acceptance

Add 792 original required fixtures, each observed twice with pinned GNU Make
4.4.1 and compared with the candidate and preceding release:

- 504 Guile-free cases: twenty expression/effect forms across six phases and
  four built-in profiles, plus target, inherited, private, pattern, foreach and
  let scopes. Named variables containing spaces use valid computed-name
  declarations. Success and explicit fatal outcomes have independent anchors;
  unexpected parse failures cannot count as lookup coverage.
- 168 flag-path cases: startup MAKEFLAGS/GNUMAKEFLAGS, split sources, four
  built-in profiles, short/long/space/chained directory and file arguments;
  reader, override, eval, recipe, secondary and restart phases for later flags.
- 120 reference-boundary cases: simple/deferred variables, ordinary/builtin
  names, literal/nested/mismatched delimiters, raw dollar names, discarded tails,
  byte names, substitution references and specific unterminated diagnostics.

The preceding release disagrees on 648 cases and passes 144 controls. Status,
streams and generated filesystem observations are compared without normalization.
Two Rust tests protect ordinary Guile-name calls and boundary/fallback behavior;
six tooling tests protect coverage, successful lookups, effect ordering, scopes,
phase-specific flags and distinct diagnostic anchors. An obsolete unit assertion
rejecting inherited `-f` now checks the accepted parsed filename.

The corpus is 8,965 total / 8,964 required / one pending; unit totals are 440 Rust
and 355 tooling tests. Full development/optimized acceptance, driven by rmake,
and exact-head CI remain merge gates. The [option inventory](m3-option-inventory.md)
and [M3/M6 audit](m3-m6-audit.md) retain the broader exit criteria. This slice does
not implement GNU native loading, enabled Guile, full startup/self-exec lifecycle,
all CLI/presentation behavior, locales or complete project integration.

## Reference provenance

The verified GNU 4.4.1 archive, SHA256
`dd16fb1d67bfab79a72f5e8390735c49e3e8e70b4945a15ab1f81ddb78658fb3`,
was consulted at `src/main.c` for source-dependent switches, `src/guile.c` and
`src/function.c` for profile registration, and `src/expand.c` for reference
boundaries. All Rust code, Makefiles and tooling checks were independently
authored. No GNU implementation or upstream test code was copied or translated;
GNU is never an implementation fallback.

## Performance observations

The alternating 101-sample medians below compare GNU, this candidate and the
preceding restart candidate. No builds or acceptance runs were concurrent with
timing; the host was not isolated.

| Workload | GNU | Current | Previous |
| --- | ---: | ---: | ---: |
| Tiny no-op | 0.525 ms | 0.612 ms | 0.614 ms |
| 10,000-target no-op | 12.374 ms | 32.495 ms | 31.815 ms |
| 5,000 expansions | 2.774 ms | 4.728 ms | 4.674 ms |
| Parallel branches, depth 1 | 88.058 ms | 90.101 ms | 90.325 ms |
| Parallel branches, depth 4 | 88.192 ms | 90.594 ms | 90.677 ms |

The current large/expansion medians are about 2.1%/1.2% higher than the previous
candidate; parallel medians are close. These unisolated observations do not
establish a statistical regression or improvement. GNU is faster throughout;
general parity remains open. The original [core](benchmarks/profile-input-core.json)
and [parallel](benchmarks/profile-input-parallel.json) reports retain every sample
and binary identity. Current SHA256:
`7036659e3319b763f484abe0fdda2dca52c0aa760a4816d37f0d7cd46c3c914c`;
previous:
`7e9fe304bb3d2f529e7d73cf90a17df7e2c47ff06b73dac68243b080023336dd`.
