# Raw implicit prerequisite spelling

Immediate implicit prerequisites now substitute their first literal `%` while
retaining preceding backslashes. Target-pattern percent quoting remains separate.
For example, `%.out: \%.in` searches for a prerequisite whose name begins with
a backslash followed by the selected stem; it does not select a literal `%.in`.
Directory reconstruction, initial leading-dot removal and later filename parsing
retain their own phases.

The implementation borrows unchanged literal prerequisite bytes and allocates
only when substitution or normalization changes the name. It reuses an owned
substitution result when no subsequent prefix removal is needed. This changes
one candidate-preparation loop, without a persistent cache, new dependency,
public Rust field type, unsafe code or GNU runtime fallback. Existing output
budgets and candidate-failure handling remain bounded.

## Independent evidence

144 original required models agree with two bounded pinned GNU Make 4.4.1
observations and frozen development/optimized builds. Forty fail in preceding
rmake. Coverage comprises 96 filename/directory/backslash/raw-byte models,
32 original/refined substitution controls, fourteen shared-graph/automatic-variable
controls and two exact rejected-search scaling fixtures. Status, streams and
filesystem types/modes/links/bytes match; ordered file witnesses verify effects
and completion. No invocation reaches a bound or leaves a background child.

The [proof manifest](../tests/research/implicit-prerequisite-spelling.json)
identifies [all compressed raw observations](../tests/research/implicit-prerequisite-spelling.json.gz).
Four original quote models incorrectly expected successful selection; independently
named rejection models derive the missing-target diagnostic from their input.
Eighteen original inputs lacked a parent directory before their parse-time file
write; separately named repaired fixtures add that directory. Two models applied
leading-dot removal after directory reconstruction instead of during the initial
read. Both their original observations and separately named refinements remain
unchanged in the [experiment archive](../tests/research/prerequisite-sharing-experiments.json.gz)
and its [hash manifest](../tests/research/prerequisite-sharing-experiments.json).

Four proof tests guard the inventory, original inputs, all observations, wrong
models, scaling contracts, mutation anchors and rejected optimization evidence.
The allocation tool now profiles the two required rejected-search fixtures,
checking exact arguments, environment, source and output. Its existing negative
tests cover both controls and reject changed arguments/output.

Formatting, Clippy, 525 Rust and 573 tooling tests pass. The corpus becomes
25,030 required / zero pending across 151 families. The same 17 approved safe-error
exceptions / 68 candidate-only cells remain separately gated, with zero exact
conformance credit. Full frozen rmake-driven `m5-acceptance` and exact-head
development/optimized/aggregate GitHub CI remain merge gates; the PR records
their final results. The development gate for the preceding change took nearly
its 60-minute budget; CI now allows 90 minutes for the growing full gates while
retaining every required check. Complete M3/M6, resource/decoder/lifecycle, native loading,
enabled Guile, locales, real exec/stdin, integration and general performance
parity remain open.

## Optimization selection

An earlier copy-on-write prerequisite-list experiment passed its fourteen
original graph controls and core checks. It reduced large-noop request counts
but made rejected implicit searches about 8–12% slower in two 101-sample batches.
That implementation is unselected. Its source patch, new module, complete
fourteen-case proof, eight elapsed reports and twelve request profiles remain
in the experiment archive. A later mutable-access draft was compiled but has no
qualified performance or complete compatibility evidence. Neither experiment
earns conformance, safety-exception or performance-win credit.

The selected smaller change closes the spelling mismatches and has eight
output-checked elapsed reports plus twenty repeated request profiles. Rejected
search medians are roughly 4–5.5% lower in the two batches; core and other inputs
are mixed, including 0.8–1.3% higher large-noop medians. Rejected-search requests
fall about 28–32%; these are allocation requests, not live/peak memory. GNU
remains faster overall. The [measurement index](benchmarks/implicit-spelling-index.md)
retains every selected report and its limitations.

The implementation and all models/probes are original. No GNU code or tests were
copied or translated. The verified GNU Make 4.4.1 `src/implicit.c`, `pattern_search`
immediate-prerequisite path (lines 558–580), informed the raw-percent behavior.
The pinned x86-64 Linux/glibc, C-locale, disabled-Guile profile is
`92525c9784f4dd471cceb12ce66a9d367e267b638e1384cdc29962b738d16de7`.
Oracle SHA256: `b8806e8eb7452ed9b7ed5e18cb7e76ff9abd488c9d62a266a398d8044fade974`.
Selected debug SHA256: `23395e0c3ec6c90b17bd271528550192cda739ae3000b51077279073fd079791`.
Selected release SHA256: `f2cce2a73e25a19d2d112e95e7427e33d0d298205548083ee993f5dfabde69b4`.
