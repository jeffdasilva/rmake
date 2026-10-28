# M3/M6 compatibility work still open

This audit prevents the curated pending count from being mistaken for a complete
GNU Make inventory. Both milestones remain open. Their exit criteria in the
[implementation plan](implementation-plan.md) are unchanged, including native
loading, Guile profiles, diagnostics and locale behavior. M4/M5 acceptance on the
pinned development profile does not establish those surfaces.

| Area | Current evidence and next acceptance work |
| --- | --- |
| Changed flag assignments | [Owned payload evaluation](flag-payloads.md) covers ordinary assignment forms, origins and effects. Inherited file/directory switches, broader option ordering and decoder safety conflicts remain open. |
| Special metadata | [Variable metadata](variable-metadata.md) now implements ordered `.VARIABLES`, pinned `.FEATURES`, initial `.LOADED`, default `.RECIPEPREFIX`, `MAKE_HOST`, `.DEFAULT_GOAL` and `MFLAGS`, including lookup lifetime and scope tests. Module-updated metadata and enabled/broader Linux profiles remain tied to their adapters and acceptance gates. |
| POSIX mode | [POSIX mode](posix-mode.md) now implements sticky reader activation, default values, folding, per-command shell flags, suffix eligibility and original command-variable propagation, with 390 required cases across profiles and phases. Broader option/profile and cross-family acceptance remains in the complete audit. |
| CLI and presentation | [Negative silent/keep-going switches](switch-origins.md), their origin precedence and scoped flag metadata now have 468 required cases. Complete the full option inventory, debug/database/trace, directory reporting, shuffle and remaining option interactions. Required raw output or narrow semantic validators must establish ordering, identities and nondeterministic fields. |
| Restart namespaces | [Directory replay and recursive identity](restart-directories.md) have 348 required cases. Complete self-exec lifecycle, winning command-variable replay and startup assignment ordering remain open. Empty `-C` usage output is recorded explicitly in research. |
| Child environment vectors | [Owned raw vectors](environment-vectors.md) now preserve the selected byte/order representation across recipe and shell launches, with 278 required cases. Broader extension/profile changes must retain the same selection and ownership gates. |
| Existing GNU native loading | Implement `load`, GNU-compatible function registration and allocation/expansion/eval interfaces, module lifecycle/errors and `.LOADED`; add original C module integration tests. GNU may not be an implementation fallback. |
| Guile profiles | Preserve disabled-profile behavior and add a pinned enabled reference with Scheme evaluation/conversion, registration, lifecycle and error tests. Do not declare the disabled baseline to be full-profile acceptance. |
| Locales and Linux profiles | Add required non-C diagnostic/catalog behavior and profile-specific path/build metadata; keep glibc/x86-64 observations distinct from broader Linux acceptance. |
| Integration and self-hosting | Expand pinned original/external projects and all top-level developer flows. Existing selected-leaf self-host checks do not close M6's complete-flow criterion. |
| Unsafe oracle behavior | Keep raw `let` signal, allocator abort and nested decoder nontermination/option-read observations visible without deliberate crashes or conformance credit. Resolve compatibility/safety conflicts explicitly before milestone completion. |

The implementation order is language and metadata correctness, complete option
and presentation behavior, native extension/profile adapters, then the expanded
integration/profile audit. Continue adding cross-family cases rather than
limiting acceptance to named reproductions. Persistent caching and new rmake
plugin APIs retain their separate correctness and compatibility gates.
