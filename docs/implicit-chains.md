# Implicit chains and intermediate files

The resolver searches direct candidates before trying chains. Stable pattern
identities prevent a rule from recurring within its own chain, even when it
produces different filenames. Searches have a depth and work budget. Candidate
plans are private until every prerequisite is viable, so rejected alternatives
cannot leave rules or intermediate markers in the live database. Double-colon
pattern rules are terminal. Nonterminal match-anything rules are suppressed by
specific patterns and cannot supply inferred intermediate files. A final
compatibility pass accounts for prerequisites mentioned on unrelated rules.

The reader records explicit names incrementally. Search and intermediate
classification use that index instead of rescanning every target's dependencies.
Existing file and VPATH observations are invocation-local; no persistent graph
or timestamp cache is introduced.

Inferred intermediate files are removed after outstanding jobs complete, including
on recipe failure, and before restarting after makefile remakes. Explicit names
and requested goals retain their files. `.INTERMEDIATE`, `.SECONDARY`,
`.NOTINTERMEDIATE` and `.PRECIOUS` control intermediate behavior; precious and
not-intermediate patterns are recognized. Preexisting files are retained. Dry
run, query, touch and silent modes have separate cleanup behavior. A missing
intermediate file does not itself invalidate an existing parent: its real inputs
are checked first, and the temporary chain is materialized only when required.
Optional missing includes can now select implicit rules too.

Validation adds 39 original differential cases and six stateful scenarios with
12 invocations. Of the new differential cases, 37 pass and two expose remaining
gaps: multiple-output implicit peer handling, and GNU's hash-table ordering of
multiple names in its final `rm` diagnostic. They remain explicitly pending;
outputs are not normalized. The full corpus contains 2,010 cases, with 1,998
required passes and 12 pending mismatches. Stateful tests cover missing, existing,
secondary and explicitly intermediate files, nanosecond timestamp comparisons,
a later prerequisite forcing a previously deferred chain, and repeated builds.

This is not the complete implicit engine. Built-in rules/default variables,
suffix conversions, grouped outputs, archive-member implicit rules and implicit
secondary-expansion search still need work. The bounded search is also not a
claim of equivalence for arbitrarily large implicit search trees.

Reference: GNU Make 4.4.1 `src/implicit.c` (`pattern_search`) and `src/file.c`
(`remove_intermediates`) were read to clarify two-pass selection, terminal rules,
compatibility fallback and cleanup stages. The implementation and fixtures were
independently authored; no GNU C implementation or upstream tests were copied.

Search-limit exhaustion is distinct from a proven failed search. The engine
terminates with an explicit resource-limit error, including during makefile
remaking; it cannot install or execute `.DEFAULT` as a substitute for an
unfinished search. The private resolver result is must-use at its call sites.
An original 131-rule regression in a private empty directory checks the fatal build outcome,
absence of an installed fallback, and absence of recipe execution. This is a
fail-safe implementation bound, not GNU equivalence for arbitrarily deep chains.
