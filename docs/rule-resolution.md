# Rule resolution slice

The database now retains pattern declarations independently of explicit rules.
Competing prerequisites remain alternatives; a later identical declaration
replaces or cancels its earlier recipe. Candidates are ordered by stem length
and declaration order. The runtime accepts candidates whose prerequisites exist
locally, through VPATH, or as explicit targets. Directory stripping and stem
reconstruction follow target-pattern slash semantics. Pattern declarations do
not select the default goal. No failed resolution result is persistently cached.

Static pattern rules substitute each target's stem, retain order-only inputs,
and warn for mismatched target names without discarding their recipes.
Double-colon branches retain separate dependency lists, recipes and source
locations, and update independently. Mixing colon forms is an error. Unconditional
double-colon makefile recipes are excluded from automatic remakes to avoid loops.

SECONDEXPANSION retains deferred references until the target is considered, with
target variables and automatic prerequisite variables. Recipe-bearing clauses
expand after earlier clauses; their expanded inputs are then placed first for
execution. VPATH and selective vpath declarations preserve search ordering and
feed resolved names to prerequisite automatic variables. Rebuilt targets use the
local name and no longer retain the searched file's timestamp.

Validation adds 38 original differential fixtures and promotes seven existing
fixtures. The corpus has 1,943 cases: 1,928 required passes and 15 pending
mismatches. The pending count is not an inventory of everything left to implement.
Chained implicit intermediates, default built-in rules, full suffix conversion,
grouped peers, GPATH and the complete deferred/implicit search algorithm remain
follow-up work. This slice does not establish full GNU rule compatibility.
