# Trace output and rebuild reasons

rmake now accepts and propagates `--trace` on the pinned Linux profile. Rebuild
messages preserve recipe filenames/lines, raw target names, `.PHONY` and missing
target reasons, changed prerequisite names, and GNU's missing-prerequisite or
unknown-reason fallback. Trace prints recipe commands even when `@`, `-s` or
`.SILENT` would ordinarily suppress them. Empty recipes and nonexecuting modes
retain their separate behavior.

Trace reasons are emitted after all recipe bodies expand and a recipe slot is
reserved, before shell configuration/command execution and load waiting. An
expansion failure precedes trace; a later configuration failure follows it.
Existing output buffers retain synchronization boundaries. Includes, built-in
recipes, VPATH/GPATH identities, recursive invocations and `.ONESHELL` keep their
ordinary locations and execution semantics.

## Requested flags and active diagnostics

GNU suspends these diagnostics while remaking Makefiles unless Makefile debug
output was separately enabled. The requested `--trace` flag still belongs in
recursive flag metadata. A flag assignment during a remake can decode the
requested flag again and reactivate diagnostics; leaving the remake restores
the entry diagnostic state. A dedicated live policy cell represents that
boundary independently of the retained option spelling. Broader `--debug`
categories remain unimplemented and receive no acceptance credit here.

Fallback reasons inspect the interpreter's existing completion observations.
A non-phony prerequisite assumed updated by dry-run is not still logically
missing merely because its physical timestamp is absent. A missing phony
prerequisite retains its distinct state, including when a physical file exists.
No new timestamp cache, mutable semantic cache, filesystem probe, unsafe code
or GNU implementation fallback is introduced.

## Evidence and remaining work

165 original required fixtures have explicit byte/status models checked against
two pinned GNU Make 4.4.1 observations and the final candidate. They cover normal,
dry-run, query, touch, silent, parallel and synchronized modes; empty/expanded
recipes; ordinary, hidden and order-only prerequisites; `.PHONY`, double-colon
and cyclic dependencies; raw names; implicit and built-in recipes; includes,
restarts, recursion, option sources and live remake reactivation. Negative
anchors reject missing echo, changed locations/identities and reordered events.
GPATH controls must reach a real rebuild; phony fallback controls must reach an
existing parent target rather than pass through the missing-target shortcut.

The corpus is 9,796 required ordinary cases with no ordinary pending entries,
plus 17 separately gated safe-error exceptions. Totals are 453 Rust / 381 tooling
tests. Full development/optimized acceptance and exact-head CI remain merge
gates. M3/M6 are open: full startup, option/debug/profile/locale interactions,
native loading, enabled Guile and the complete compatibility inventory remain
separate acceptance work.

## Performance observations

Output-checked common workloads run with trace disabled. Frozen comparisons use
101 alternating samples, retain executable identities and every sample, and run
without concurrent builds or acceptance campaigns. Host activity is not
controlled. These are workload observations, not statistical proof or a claim
of general performance parity. GNU remains faster on standard workloads.

| Workload | GNU ms | Current ms | Preceding main release ms |
| --- | ---: | ---: | ---: |
| Tiny no-op | 0.628 | 0.744 | 0.751 |
| 10,000 explicit targets | 12.636 | 32.459 | 32.623 |
| Expansion | 2.950 | 4.896 | 4.860 |
| Parallel branches, depth 1 | 87.926 | 90.266 | 90.310 |
| Parallel branches, depth 4 | 87.953 | 90.341 | 90.467 |

The medians are mixed; expansion is about 0.7% higher in this run. These samples
establish neither a statistical regression nor a speedup.

Raw [core](benchmarks/trace-core.json) and
[parallel](benchmarks/trace-parallel.json) reports accompany this slice.

## Localized trace output

`make test-trace-nls` and `make test-trace-nls-release` compare C and
verified French streams, exit status, complete filesystem effects and
fixed timestamp inputs. The profiles cover builtin and included source
names, changed prerequisite names, VPATH, recursive builds, a real restart,
nonexecuting modes and output synchronization. Two original byte-oriented
fixtures cover non-UTF-8 source, target and prerequisite names and a user
line that looks like a trace diagnostic.

The renderer passes a typed reason and raw bytes to the active gettext
domain at the existing trace publication boundary. The French catalog
translates the generic prerequisite template and the builtin source label.
Its fixed reason strings use gettext fallback. Recipe echoes and recipe
output retain their original bytes. The message IDs come from GNU Make
4.4.1's trace producer in `src/job.c`. Catalog provenance and translation
notices are recorded in [catalogs/provenance.json](../catalogs/provenance.json)
and [catalogs/COPYING](../catalogs/COPYING).

`make test-trace-job-languages` also checks 18 trace and JOBS comparisons
in German, Japanese, and Brazilian Portuguese. The gate verifies each
catalog and preserves the existing live PID and address checks.
