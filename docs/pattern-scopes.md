# Pattern-variable activation

Pattern-specific variables are implemented separately from explicit target
bindings. The rules below are grounded in original Makefiles executed against
the pinned GNU Make 4.4.1 oracle. This implements variable scopes, not implicit
build-rule resolution.

## Reading and activation

Each declaration keeps its pattern, prepared variable name, assignment kind,
origin, qualifiers and source. Pattern names match complete target paths and
require a nonempty stem. Quoted percent characters are literal; the first
unquoted percent is the wildcard and later percent characters are literal.
Matching declarations are applied from broadest to most specific, preserving
original declaration order for equal specificity. Repeated pattern names do not
collapse into a single declaration or change this ordering.

Computed variable names use global read-time context. `:=` and `::=` expand
their RHS while reading and retain the resulting literal dollar bytes.
`:::=`, conditional assignment and appends are deferred until initialization.
They use global definitions and the accumulated pattern layer; immediate
effects run once for each initialized target. An append can therefore expand
later than its declaration while retaining an earlier binding's simple flavor.
Command-line values are captured at the observed declaration point, with the
pattern assignment's flavor semantics preserved.

Matching declarations accumulate into one pattern layer beneath explicit target
bindings and above inherited ancestor bindings. Initialization proceeds through
the first owning parent's context before the child's. Completed targets and
initialized contexts are separate: a prerequisite initializes each ancestor's
context when needed, even if the ancestor has no recipe. Contexts refresh global
bindings after a pattern conditional changes global privacy/export metadata.

Initialization is lazy. An up-to-date target, an empty recipe, a missing target,
or an ordinary recipe skipped by touch mode must not trigger deferred effects.
A nonempty raw recipe initializes its scopes before expansion, even if expansion
produces no command. Query and dry-run modes initialize those scopes; a forced
recipe under touch mode does too. Pattern expansion failures keep their observed
unlocated or binding-derived diagnostic context.

## Recipe environments

Environment binding selection skips private pattern bindings, but references
inside the selected value use the complete target context. For example, a global
exported `Y=$(X)` can see the private pattern value of `X`, while the selected
exported binding of `X` itself comes from an outer scope.

Inherited appends have an additional observed distinction: their environment
expansion restarts lookup by name through the complete target context. This is
covered separately from ordinary recursive binding expansion and recursion
detection.

Environment preparation remembers resolved inherited export policy on the
selected binding. Later global metadata changes do not undo that earlier
resolution. An optimized no-op command that launches no child does not prepare
an environment and must not resolve policy. Shared immutable binding maps carry
an atomic policy cell for this state; cloning an actual variable definition
copies its current policy value rather than aliasing independent snapshots.
This is invocation-local interpreter state, not persistent caching.

## Validation and reference provenance

`make test-pattern-scopes` combines explicit and pattern declarations across
seeded dependency graphs. It compares raw output, status and filesystem effects,
including each recipe's real environment, and retains deterministic reproductions.
Curated tests cover effect timing, declaration order, escaped percent characters,
global metadata aliases, initialization skips and diagnostic locations.

GNU Make 4.4.1 source was consulted as a behavioral reference while investigating
oracle differences: `src/read.c` (`record_target_var`); `src/variable.c`
(`create_pattern_var`, `lookup_pattern_var`, `initialize_file_variables`,
`do_variable_definition`, `define_variable_in_set`, `target_environment`,
`should_export`); `src/expand.c` (`recursively_expand_for_file`,
`variable_append`, `allocated_variable_append`); and `src/function.c`
(`func_flavor`). The source archive identity is pinned by the oracle tooling.
The Rust binding/activation implementation and regression tests were written
independently; no upstream source or test code was copied or translated.
See the [licensing policy](licensing.md) for the project's GPL terms.
