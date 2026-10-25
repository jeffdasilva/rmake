# Intermediate-file policy

An empty `.SECONDARY:` makes files eligible for lazy intermediate inspection,
in addition to preserving intermediates from cleanup. When an output is already
current, a missing intermediate need not be recreated. Even a missing leaf with
no rule can stay absent when it is itself intermediate and nothing requires
rebuilding the existing output. A newer normal input, a forced rebuild or a
request for the missing file still requires normal materialization and reports
missing-rule errors. Order and query mode do not bypass those checks.

A named `.INTERMEDIATE` or `.SECONDARY` setting takes precedence over an empty
`.NOTINTERMEDIATE:` for lazy inspection. Global `.NOTINTERMEDIATE:` still keeps
created intermediate files. A `.NOTINTERMEDIATE` flag inherited from the selected
implicit pattern also protects cleanup, even when the literal file is explicitly
marked intermediate. Literal pattern names and selected pattern flags are separate.

The interpreter compiles the three settings after each complete Makefile read.
It indexes literal names, includes every double-colon branch, and rejects named
`.NOTINTERMEDIATE`/`.INTERMEDIATE` and `.NOTINTERMEDIATE`/`.SECONDARY` conflicts
in GNU's order. Empty `.NOTINTERMEDIATE` and `.SECONDARY` settings conflict too.
Validation follows read-time effects and parse errors, before VPATH/GPATH policy
expansion and makefile remaking. Includes and restarts get the same pass.

GNU checks filename records, not just explicit build rules: a prerequisite
mention or a target-specific assignment can enter an empty special-name record
and enable its global setting. Original fixtures cover these forms and their
conflicts. Deferred special-target prerequisites retain GNU's read-time flag
behavior without executing skipped secondary-expansion effects.

The policy module owns invocation-local name indexes and booleans. It does not
cache graph freshness, expansion results or filesystem observations. Lazy visits
leave unmaterialized files available for a later required build. Existing graph
continuations, side-effect invalidation and permanent filename aliases remain
in force. The top-level Makefile drives validation and measurement under GNU
Make and rmake.

## Validation and remaining work

There are 155 new original required cases under `rules.intermediate-policy`.
The preceding `5440bff` release fails 51 and passes 104. Fixtures cover old/new/
missing inputs and outputs, no-rule leaves, named/global precedence, normal/dry/
query/touch/always/parallel modes, errors and effect order, double-colon branches,
includes/restarts, pattern and GPATH controls, and special-name records.

The twelve-step `intermediate-policy-transitions` stateful scenario independently
checks bytes and exact timestamps across no-op/query runs, missing leaves, forced
failure, new input, materialization, removal and changes of policy between runs.
It is part of `make test-stateful` and CI. Corpus: 3,271 cases, with 3,259 required
candidate matches and 12 pre-existing pending mismatches. Stateful coverage:
53 scenarios / 160 invocations. No upstream implementation or tests were copied;
GNU 4.4.1 source was consulted as a behavior reference.

This is another M4 slice. Wider file-record aliases and metadata, directory-cache
interactions, descendant scope refresh after late merges, intermediate lifecycle
combinations, archive format/fault coverage and remaining runtime/compatibility
profiles still need work. These test counts do not enumerate every remaining gap.
Persistent graph caching remains deferred until validation and invalidation are
proved for its proposed scope.

## Measurements

`make bench-compare BENCH_SUITE=intermediates BENCH_REPEAT=101` uses 1,000
current physical outputs and missing explicit middles. One workload uses global
`.SECONDARY`; another combines global `.NOTINTERMEDIATE` with named
`.INTERMEDIATE`. Literal empty recipes launch no children. Inputs are prepared
with fixed historical timestamps outside timing. The previous implementation's
extra attempted work produces the same streams/status and leaves these files
unchanged, so the three engines can be compared; this convergence does not imply
that the previous implementation had correct policy semantics.

A local 101-sample alternating paired comparison against `5440bff` measured:

| Workload | Current rmake (ms) | Previous rmake (ms) | GNU 4.4.1 (ms) |
| --- | ---: | ---: | ---: |
| 1,000 missing secondary middles | 8.477 | 10.602 | 3.753 |
| 1,000 named middles with global not-intermediate | 8.742 | 10.602 | 3.796 |
| Startup | 0.718 | 0.724 | 0.666 |
| 10,000 ordinary no-op targets | 33.876 | 33.791 | 12.190 |
| 5,000 expansion assignments | 5.859 | 5.806 | 3.725 |

The selected lazy workloads have roughly 18-20% lower elapsed medians; ordinary
core differences are small. Current release SHA256:
`a5ddded3ca1f1bda9f56c206a920629c9246c0b8fc7c987ec61916db8eda343d`. Previous release SHA256:
`c2494df18874ddb25fc8a4eab4bf567ec89b2298980951459ddb6064a30383b2`.

Reports retain raw samples, source hashes, arguments, profile and executable
identities. They are ignored artifacts `artifacts/bench/intermediate-policy-101.json`
and `artifacts/bench/intermediate-policy-core-101.json`. Measurements include
startup, two warmups per engine, warm OS caches and alternating engine order,
with exact output/status checks. There is no CPU isolation, confidence interval,
CPU/RSS/I/O profile or performance acceptance threshold. GNU remains faster on
these workloads; the selected improvements do not establish general speed parity.
