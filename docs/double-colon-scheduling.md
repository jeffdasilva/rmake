# Double-colon graph continuations

An unfinished double-colon branch now yields to independent graph branches.
An outer interpreter-owned continuation retains the original timestamp, first
parent, remaining declarations and aggregate status. It advances one declaration
at a time using the existing rule and recipe continuations. Recipes for the same
target remain sequential; the next branch starts after the current branch and
all of its commands finish. Other targets can use available jobserver slots.

Pending ownership covers both the outer continuation and its active rule/recipe.
Shared dependents therefore wait for all declarations, not just the current
recipe's completion. Errors, keep-going, ignored commands, local `.WAIT`, automatic
variables, target scope and secondary expansion keep their per-branch boundaries.
Fatal invocation stops suppress unstarted branches while draining started jobs.
Serial, dry-run, touch, query and makefile-remaking paths share the same state
machine and retain synchronous execution where required.

Twelve new original required differential cases cover declaration ordering,
shared targets and prerequisites, two concurrent double-colon targets, barriers,
secondary expansion once per declaration, scopes/automatic variables, empty
branches, failures, keep-going, ignored commands and recursive children. The
previous double-colon sibling case is promoted to required. Exact output, exit
status and filesystem observations are checked against GNU Make 4.4.1. No
upstream implementation or tests were copied.

The corpus has 2,233 cases: 2,219 required and 14 pending. Missing-intermediate
inspection remains a synchronous graph boundary; its sibling probe remains
pending. This is not a complete inventory of remaining compatibility work.
