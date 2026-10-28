# M3/M6 compatibility work still open

This audit prevents the curated pending count from being mistaken for a complete
GNU Make inventory. Both milestones remain open. Their exit criteria in the
[implementation plan](implementation-plan.md) are unchanged, including native
loading, Guile profiles, diagnostics and locale behavior. M4/M5 acceptance on the
pinned development profile does not establish those surfaces.

| Area | Current evidence and next acceptance work |
| --- | --- |
| Changed flag assignments | [Owned payload evaluation](flag-payloads.md) covers ordinary assignment forms, origins and effects. [Well-formed inherited path switches](profile-inputs.md) now distinguish startup from later decoding. Broader startup/option ordering, malformed decoding and safety conflicts remain open. |
| Special metadata | [Variable metadata](variable-metadata.md) now implements ordered `.VARIABLES`, pinned `.FEATURES`, initial `.LOADED`, default `.RECIPEPREFIX`, `MAKE_HOST`, `.DEFAULT_GOAL` and `MFLAGS`, including lookup lifetime and scope tests. Module-updated metadata and enabled/broader Linux profiles remain tied to their adapters and acceptance gates. |
| POSIX mode | [POSIX mode](posix-mode.md) now implements sticky reader activation, default values, folding, per-command shell flags, suffix eligibility and original command-variable propagation, with 390 required cases across profiles and phases. Broader option/profile and cross-family acceptance remains in the complete audit. |
| Late built-in disabling | [Post-read -r/-R cleanup](late-builtins.md) preserves stronger origins, metadata, export policies, user suffix declarations/recipes and original restart phases, with 204 semantic comparisons and three output-checked scaling cases. Full startup/self-exec and option interactions remain open. |
| CLI and presentation | [Negative silent/keep-going switches](switch-origins.md), their origin precedence and scoped flag metadata now have 468 required cases. The [source option inventory](m3-option-inventory.md) lists the remaining debug/database/trace, directory reporting, eval, old/new-file, shuffle, undefined-variable and decoder interactions. Required raw output or narrow semantic validators must establish ordering, identities and nondeterministic fields. |
| Restart namespaces | [Directory replay and recursive identity](restart-directories.md) have 348 required cases. Complete self-exec lifecycle, winning command-variable replay and startup assignment ordering remain open. Empty `-C` usage output is recorded explicitly in research. |
| Reference boundaries | [Simple/computed/fallback names and function diagnostics](profile-inputs.md) now have 120 required cases. Broader reader, option and native registry interactions retain their own acceptance requirements. |
| Undefined-variable warnings | [Warning emission](undefined-warnings.md) has 358 required cases across scopes, phases, automatic variables and option sources. Numeric lexical/call-frame masking is corrected. Two startup default-order probes remain ordinary open research; full startup, option/profile and locale acceptance remain open. |
| Command-line eval | [Startup buffers](command-line-eval.md) have 194 required semantic cases plus two scaling controls, including anonymous/real-file diagnostics, live additions, metadata, switch phases, recursion and restart replay. Broader startup/decoder/self-exec/profile acceptance remains open. |
| Trace output | [Rebuild reasons and command echo](trace.md) have 165 required cases across modes, identities, synchronization, restarts and live remake reactivation. Requested flags remain separate from active remake diagnostics. Broader debug/startup/profile/locale interactions remain open. |
| Child environment vectors | [Owned raw vectors](environment-vectors.md) now preserve the selected byte/order representation across recipe and shell launches, with 278 required cases. Broader extension/profile changes must retain the same selection and ownership gates. |
| Existing GNU native loading | Implement `load`, GNU-compatible function registration and allocation/expansion/eval interfaces, module lifecycle/errors and `.LOADED`; add original C module integration tests. GNU may not be an implementation fallback. |
| Guile profiles | [Disabled-profile lookup, effects and scopes](profile-inputs.md) now have 504 required cases. Add a pinned enabled reference with Scheme evaluation/conversion, registration, lifecycle and error tests. Do not declare the disabled baseline to be full-profile acceptance. |
| Locales and Linux profiles | Add required non-C diagnostic/catalog behavior and profile-specific path/build metadata; keep glibc/x86-64 observations distinct from broader Linux acceptance. |
| Integration and self-hosting | Expand pinned original/external projects and all top-level developer flows. The complete rmake-driven m5-acceptance flow is now validated; bootstrap, cleanup, benchmark and external-project acceptance still need the complete-flow audit. |
| Performance | [Automatic snapshots](automatic-snapshots.md) avoid unused owned preparation with raw glibc-request and paired timing evidence. General performance parity, persistent cache correctness and complete workload/profile acceptance remain open. |
| Unsafe oracle behavior | The owner approved [documented safe errors](safety-exceptions.md). A separate candidate-only gate covers 17 recorded crash/hang inputs with zero exact-conformance credit. The finite nested decoder mismatch and startup CLI eval probe remain ordinary compatibility gaps. |

The implementation order is language and metadata correctness, complete option
and presentation behavior, native extension/profile adapters, then the expanded
integration/profile audit. Continue adding cross-family cases rather than
limiting acceptance to named reproductions. Persistent caching and new rmake
plugin APIs retain their separate correctness and compatibility gates.
