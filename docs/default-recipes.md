# Default recipe fallback and special flags

When no explicit rule or viable implicit rule exists, `.DEFAULT` can now supply
the target's commands. Only its commands are reused; its prerequisites and
target-specific bindings are not inherited. The real target keeps its ordinary
parent/target/pattern context, recipe locations and suffix-derived stem. `$<`
is the target itself, including when `.DEFAULT` is explicitly requested; the
remaining prerequisite variables describe the real target's prerequisites.

Explicit empty rules suppress fallback, and viable implicit rules take precedence.
Fallback is not a way to satisfy otherwise-impossible implicit-chain inputs.
An empty `.DEFAULT` declaration clears the prior recipe at declaration completion;
a following recipe belongs to that declaration and retains override diagnostics.
Clearing also resets the previous recipe location, so a later redefinition does
not warn about a recipe that has been removed. Includes and eval use the same
incremental reader lifecycle. Missing required/optional included makefiles can
be remade through a default recipe, with normal restart behavior.

The new cases also exposed overly broad special-flag matching. A `.PRECIOUS` or
`.NOTINTERMEDIATE` pattern flag now comes from the exact selected implicit rule
pattern. Arbitrary patterns do not mark matching explicit, static or default
rules. Concrete named flags still apply, including to defaulted targets. The
selected implicit pattern is explicit rule metadata, shared by error cleanup and
intermediate lifecycle handling. Suffix-converted rules use the same mechanism.

Forty-two new original differential fixtures cover fallback priority, explicit
empty rules, cancellation/redefinition, override locations, default/parent/target
scopes, automatic variables, suffix stems, requested/existing/duplicate goals,
ignored failures, deletion, precious flags, selected-pattern distinctions,
parallel multi-command defaults, order-only prerequisites, execution modes and
generated includes. The full corpus has 2,187 cases: 2,173 required passes and
14 pending mismatches. This is not a complete GNU feature inventory.

Full ready-graph scheduling, grouped outputs, archive-member rule semantics,
implicit secondary-expansion search and complete signal/load/output-sync behavior
remain unfinished, along with other tracked compatibility gaps. No persistent
cache or GNU runtime fallback is introduced.

Reference: GNU Make 4.4.1 `src/implicit.c` was consulted for the use of the selected
implicit pattern when inheriting precious/not-intermediate flags. The fallback
and cancellation behavior was established by independently authored executable
probes. No upstream implementation code or tests were copied.
