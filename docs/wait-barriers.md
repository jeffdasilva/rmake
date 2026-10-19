# Prerequisite phase barriers

GNU Make 4.4.1's `.WAIT` prerequisite markers now divide a target's prerequisite
list into phases. Each barrier waits for that target's earlier prerequisites,
including complete multi-command recipes, before starting the next phase. Jobs
within a phase can still overlap. An already-running unrelated job is not part
of the barrier. Normal and order-only lists, static and implicit rules, secondary
expansion, recursion and double-colon branches use the same execution boundary.

Markers do not create file dependencies, influence staleness, or enter automatic
variables. Leading, trailing and repeated markers are allowed. Unignored failures
before a barrier suppress the next phase unless keep-going is active. Recipe and
prerequisite declarations for the special `.WAIT` target retain their ordinary
requested-goal behavior and emit GNU's category-once warnings at declaration
completion. Header locations and output ordering are preserved.

Validation adds 28 required original differential fixtures. Two-stage barriers
prove parallel work within phases; the local-scope case would deadlock if a
barrier waited on all active jobs. Other cases exercise automatic lists,
order-only prerequisites, errors, keep-going, ignored failures, secondary/static/
implicit rules, recursion, double-colon branches, dry/query/touch/serial modes,
and special-target warnings (including merged output order).

A separate original pending fixture exposes a broader scheduler gap: the current
depth-first dependency traversal can wait inside one branch before it has visited
an independent sibling branch. GNU can schedule that sibling while the first
branch waits. A general ready-graph scheduler is still needed; local barrier
support does not claim complete graph scheduling parity. The corpus has 2,145
cases: 2,131 required passes and 14 pending mismatches. The pending count is not
an inventory of every missing GNU feature. Shared-pool signal, load and output
synchronization boundaries remain documented in [jobservers](jobserver.md).
