# Resumable missing-intermediate checks

Inspecting the real inputs of a missing intermediate now yields when an input
is still running. The parent can explore independent branches instead of waiting
inside that inspection. Inspections retain per-prerequisite progress in the
parent's rule continuation and are reconsidered after completion or a new outer
pass. They do not mark the absent intermediate updated or install a persistent
negative cache: another parent can ask against a different timestamp, and a later
trigger can still require materialization.

A running input's provisional absent timestamp cannot make the intermediate
stale. Only finalized input timestamps decide whether the intermediate must be
created. Inputs rebuilt with an old preserved timestamp therefore leave an
otherwise up-to-date target and its missing intermediate untouched. If another
prerequisite makes the parent stale, required intermediate creation also yields
through ordinary rule/recipe continuations. Finalization waits for complete
materialization before expanding the parent's commands.

`.WAIT` and named `.NOTPARALLEL` are honored during inspection. First-parent
scope ownership, shared-target execution, error propagation, secondary retention,
preexisting intermediate preservation and normal cleanup keep their established
boundaries. Deferred rules use the normal expansion/update path. Completed real
inputs remain governed by the invocation's existing target state; inspection
never replays their commands or variable effects.

Twelve new original required differential cases cover newer inputs, nested
parents, old timestamp preservation, late materialization, local barriers,
nested chains, shared scopes, failures, preexisting files, secondary files and
duplicate prerequisites. The previous missing-intermediate sibling probe is
promoted to required. Timestamp setup is explicit and deterministic; comparisons
retain exact output, status and filesystem contents. Existing incremental and
nanosecond stateful campaigns remain part of the full CI target. No upstream
implementation or tests were copied.

The corpus has 2,245 cases: 2,232 required and 13 pending. Full GNU compatibility
remains incomplete, including grouped/archive rules, implicit secondary expansion,
remaining diagnostics/launch edges, signals, load limits and output synchronization.
These counts are not a complete inventory of missing features. This work adds
transient scheduling progress, not a persistent dependency-graph cache.
