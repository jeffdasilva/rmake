# Second-stage include search

GNU Make 4.4.1 expands a tilde operand at its argument position, stores that
operand, and performs another tilde lookup while constructing active include
search. The second lookup can observe later definitions and execute HOME effects
again. Its physical directory must not replace the stored MAKEFLAGS operand.
Post-read GNU flag sources still do not construct a new active search.

Search preparation now owns its traversal and selected directories. It runs after
argument decoding restores its parent's depth, terminal and following-definition
state. Each step copies one name and drops the option-state borrow before HOME,
eval or nested reset effects. Nested additions remain visible to later search
steps. Metadata and search publish only after preparation and metadata effects
succeed; errors retain completed variable effects and the prior active search.
No HOME effects or filesystem observations are cached.

A resolved directory named `-` remains a directory; only the original `-I-`
operand clears search and disables defaults. Search expansion preserves dot
prefixes, repeated/trailing slashes and raw bytes until directory selection.
Stored tilde operands select the owned decoder on later resets as well, retaining
literal end markers. Empty streams preserve the shared argument cursor.

474 original required models agree with two bounded GNU observations and frozen
development/optimized builds, closing 240 preceding mismatches. They include a
384-case phase/kind/history/output/warning/byte matrix, sixteen initial probes,
sixteen directory/control cases, forty nested post-decode resets, twelve enclosing
cursor controls, four fatal effects and two invalid output-sync cases. The
[proof](../tests/research/secondary-include-search.json.gz) retains raw streams,
statuses, bounds, ordered filesystem witnesses, input bytes and identities.
[Discovery](../tests/research/secondary-include-discovery.json.gz) retains original
warning assumptions, ten failed prototype marker controls and self-referential
hypotheses that crash or time out in GNU. None receives ordinary-conformance or
new safety-exception credit. Separate refinements are independently qualified;
the existing 32 approved exceptions remain unchanged.

Reference study used the pinned GNU Make 4.4.1 search construction and reset
ordering in `src/read.c` and `src/main.c`. No GNU implementation or tests were
copied or translated. GNU remains an external test oracle.

Full frozen rmake-driven acceptance and exact-head CI remain merge gates. Full
M3/M6, broader malformed decoding and nested resets, native/Guile profiles,
directory/debug/database output, exec/stdin, integration, persistent cache
acceptance and general performance parity remain open.

[Four paired elapsed reports and twelve request profiles](benchmarks/secondary-search-index.md) show mixed timings and unchanged allocation/free counts. GNU remains faster; general speedup and parity are not claimed.
