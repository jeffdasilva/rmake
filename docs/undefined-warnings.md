# Undefined-variable warning emission

rmake now accepts and propagates `--warn-undefined-variables` for the implemented
Linux reader/runtime profile. Missing references and missing nonempty `call`
function names produce byte-preserving warnings. Empty defined values stay
defined; `origin`, `flavor` and `value` do not become ordinary references.
Lazy branches and empty iteration bodies retain their evaluation order.

The policy updates during an expression that eval-assigns `MAKEFLAGS`. Ordinary
file assignments respect priority, including `-e`; `GNUMAKEFLAGS` enables policy
when its post-read decode occurs. Recursive `MAKEFLAGS`/`MFLAGS` retain the long
option in GNU's observed order. Includes, secondary expansion, recursive exports,
recipe configuration, restarts, recursion and directory changes share the same
live policy and output boundary.

## Diagnostic boundaries

Reader/recipe locations, the first binding context when no caller exists, and
unlocated decoder warnings remain distinct. In particular:

- With initial `-R`, absent `GNUMAKEFLAGS` can itself produce an unlocated
  post-read warning. It is not a generally silent internal lookup.
- Enabling warnings through inherited `GNUMAKEFLAGS` can diagnose missing
  `MAKEFLAGS` before directory changes and Makefile reading.
- Internal shell/HOME selection stays silent for its missing outer binding;
  explicit missing references inside a selected value still warn.
- GNU's ten warning-exempt metadata/search names are checked across ordinary,
  substitution and `call` references. Automatic variables warn when missing
  outside their context, but defined empty automatic values do not warn.

The output receiver exposes live policy through a narrow trait. Ordinary closure
receivers remain quiet by default; `WarningOutput` pairs a receiver with the
runtime policy cell. This keeps policy out of the expansion-context payload and
avoids formatting/allocation for disabled warnings. Named internal binding
selection and explicit user references have separate entry points. Empty lexical
binding snapshots have an explicit fast path; mutable values are not cached.

Numeric `let`/`foreach` bindings also exposed an existing `call` masking defect.
Temporary frames now identify function-parameter frames explicitly. Inner calls
mask missing outer call parameters without erasing unrelated numeric lexical
bindings. Eval preserves frame identity through its ordinary snapshot/restore
path. No unsafe code or GNU runtime fallback is introduced.

## Acceptance and open work

358 original required fixtures have explicit byte/status models checked against
two pinned GNU Make observations and the candidate. They cover enabled/disabled
controls, raw names, missing/computed/substitution references, lazy functions,
numeric lexical/call scopes, metadata exceptions, automatic variables, option
sources and phases, export/reader/recipe contexts, includes, restarts, recursion,
directory changes, output mode and late built-in cleanup. Negative controls
protect exact warning order, locations, raw bytes, phase reachability and silent
behavior. The curated corpus is 9,631 ordinary required cases with no ordinary
pending entries, plus 17 separately gated safety exceptions. Unit totals are
452 Rust / 377 tooling. Full acceptance and exact-head CI remain merge gates.

This is a verified slice, not full M3/M6 acceptance. The existing startup-order
architecture still creates defaults before command-line assignments expand.
Two original `INPUT:=$(CC)` / `INPUT:=$(.DEFAULT_GOAL)` probes retain their raw
GNU/prototype observations in
[`gnu-441-warning-startup.json`](../tests/research/gnu-441-warning-startup.json).
They are ordinary open compatibility gaps, receive no passing credit, and have
no safety exemption. Broader startup/flag expansion, malformed/early-exit option
interactions, enabled Guile/native profiles, locales and the complete
[compatibility inventory](m3-m6-audit.md) remain open.

## Performance observations

The common workloads run with warnings disabled and validate exact output.
Core and parallel reports use 101 alternating samples; binding reports use 31.
No builds or acceptance campaigns run concurrently with timing; host activity
is not controlled. This does not measure warning-flood throughput or establish
general performance parity. GNU remains faster on the standard workloads.

| Workload | GNU ms | Current ms | Preceding main release ms |
| --- | ---: | ---: | ---: |
| Tiny no-op | 0.556 | 0.652 | 0.640 |
| 10,000 explicit targets | 12.396 | 31.954 | 31.675 |
| Expansion | 2.923 | 4.775 | 4.676 |
| Parallel branches, depth 1 | 88.453 | 90.763 | 90.546 |
| Parallel branches, depth 4 | 88.252 | 90.643 | 90.589 |
| Global 256 KiB values | 4.454 | 6.019 | 5.931 |
| Local 256 KiB values | 7.607 | 7.732 | 7.502 |
| Global 1 MiB values | 12.876 | 16.961 | 16.929 |
| Local 1 MiB values | 30.280 | 27.462 | 27.724 |

Several medians are higher, including expansion by about 2.1% in this run;
these observations do not establish a statistical regression or improvement.
They are not presented as performance parity or a speedup. Raw
[core](benchmarks/undefined-core.json), [parallel](benchmarks/undefined-parallel.json)
and [bindings](benchmarks/undefined-bindings.json) reports retain every sample
and executable identity.
