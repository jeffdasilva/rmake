# Target-variable scope design notes

Status: explicit target-specific assignments are implemented with layered bindings,
inheritance, privacy and export policies. Pattern-specific scopes use a separate
implemented [activation model](pattern-scopes.md). These notes record GNU
Make 4.4.1 observations from the pinned oracle on 2026-10-18; the
[progress record](progress.md) records the verified corpus and remaining gates.

The [GNU target-specific variable documentation](https://www.gnu.org/software/make/manual/html_node/Target_002dspecific.html) describes separate target definitions, evaluation in the target's context, inheritance to prerequisites, and first-parent behavior for shared prerequisites. The [inheritance documentation](https://www.gnu.org/software/make/manual/html_node/Suppressing-Inheritance.html) describes `private`. The details below were checked with original disposable Makefiles; no upstream tests were copied.

## Required representation

Keep target bindings separate from the explicit-rule table. A declaration such as `name: X = value` neither supplies a build rule nor chooses the default goal. Alone it produces `No targets`; requesting the missing `name` still produces `No rule to make target`. A scope entry must not make that target buildable.

The implementation uses byte-keyed binding layers rather than a flattened map.
Target bindings store their raw value, flavor, origin, source location,
privacy/export policy, and an explicit “append outer definition” marker.
Immutable maps are shared with `Arc`; recipe automatic variables use a small
overlay without copying the global map. The interfaces distinguish:

- `TargetScope`: local byte-keyed bindings, stored by target name separately from `Database.rules`.
- A parse context: that target's existing local bindings over the current global bindings.
- An execution context: current target bindings over inherited ancestor layers over globals, plus the recipe's automatic variables.
- Lookup that can start below a particular binding when expanding an inherited append, while retaining the full current context for references inside its text.

Origin precedence applies within a binding layer. Scope selection is a separate operation: a child's ordinary assignment can shadow an inherited `override` assignment or a global `override` assignment. Reusing a flat map's “lower origin cannot replace higher origin” check across layers gives the wrong result.

On descent, skip private bindings from ancestors; do not delete their names from the entire context. A private child `X=child` can expose the earlier parent's `X=parent` to a grandchild. Keep completed-target memoization: a shared prerequisite executes once with the first parent's context, including when command-line goal order chooses that parent.

## Assignment timing and stored values

| Form | Observed behavior |
| --- | --- |
| `target: X := RHS` or `::=` | Expand immediately using prior bindings on that target and current globals. Inherited parent values are unavailable while parsing. Preserve resulting dollar bytes literally. |
| Multiple targets with `:=` | Evaluate separately for each target; their earlier local values can differ. |
| Computed variable name | Expand in the target's parse context too. A previously assigned local name selector overrides its global counterpart. |
| `target: X = RHS` | Store recursive text; references, including automatic variables, use the context of the recipe where the value is expanded. |
| `target: X ?= RHS` | Decide while parsing. An existing local or global definition, even empty, suppresses creation. A future inherited parent value does not. |
| First local `X += RHS` | Store a recursive append layer even if global `X` is simple. Resolve the outer value later; `value X` exposes only the local suffix and `flavor X` is recursive. |
| `+=` with an existing local binding | Respect that local flavor. A simple local binding expands the suffix immediately; a recursive one defers it. Multiple inherited append layers retain their separate raw suffixes. |

For example, `K=old; X:=base-$(K); target:X+=tail-$(K); K=new` yields expanded `base-old tail-new`, raw `tail-$(K)`, and recursive flavor in the target. An outer recursive value also expands in the effective target context, so target-local names affect its references.

Do not trim append boundaries indiscriminately. An empty inherited append suffix over nonempty `X=global` expands as `global `, with a trailing space. Two such layers can preserve additional spaces. Ordinary global/local appends of empty text do not introduce that trailing space.

Recognize target assignments before ordinary prerequisite expansion and inline-recipe splitting: `target: X = a;b` stores `a;b`. Ordinary prerequisites still expand in the global read context; `X=global; target:X=local; target:$(X)` names prerequisite `global`, whose recipe can inherit `X=local`.

## Precedence and qualifier details

Non-override target assignments are replaced by the current command-line binding when applicable, preserving its flavor and origin. That creates a local snapshot: a later global `override X=later` does not change it. A suppressed target `?=` creates no local binding, so it observes later changes to the global definition instead.

With `-e`, GNU initially reports an untouched imported variable's origin as `environment`. A conflicting **global** assignment promotes it to `environment override`. An ordinary target assignment can win before that promotion; a preceding global collision makes the environment win. A collision after the target assignment does not retroactively replace the local value. Importing every environment variable with override origin at startup cannot reproduce these distinctions.

A later unqualified target assignment resets an earlier local `private` flag, even if `?=` or origin precedence prevents changing the value. Export/unexport policies likewise need explicit binding metadata rather than global export-name sets.

One additional 4.4.1 behavior needs an explicit model and regression fixture: `X=global; parent: private X ?= ignored` marks the existing global binding private without creating a local binding. `X` then disappears from every recipe, including `parent` and unrelated targets. A subsequent ordinary global assignment retains that privacy. Binding metadata can therefore be modified through a suppressed conditional assignment; silently replacing it with a target-local copy is wrong.

## Independent timing correction and validation

The current variable store now evaluates `:=` and `:::=` right-hand sides before discarding a lower-origin assignment. A losing `+=` also evaluates its suffix if the existing same-scope binding is simple. Recursive assignments/appends remain deferred, and `?=` still skips its RHS when any definition exists.

An original reproducer is:

```make
BROKEN = $(BROKEN)
X := $(BROKEN)
all: ; @printf '<%s>\n' '$(X)'
```

Running the oracle with `X=chosen` still reports recursive expansion. Replacing `:=` with `:::=` does too. With `+=`, command-line `X:=chosen` errors while `X=chosen` prints `<chosen>`. With `?=` or `=`, both retain the command-line value without expanding the discarded recursive text. Focused Rust tests cover these errors, preservation of the prior binding, and conditional laziness. A first target append is independently recursive, so a global command-line simple binding does not make that suffix immediate.

Required fixtures now cover inheritance, privacy, shared prerequisites, local
simple/recursive timing, literal dollars, automatic-variable context, append
layers, conditional assignments, CLI/environment precedence, metadata aliasing,
multiple-target/computed-name evaluation, variable-only declarations and exports.
`make test-scopes` also combines these operations across seeded dependency graphs.

A losing nonappend target assignment (`=`, `:=`, `::=`, `:::=`) clears an earlier
inherited-append marker even when its origin cannot replace the value. Losing
`?=` and `+=` preserve that marker. Unqualified assignments reset privacy in both
cases. Immediate RHS evaluation still precedes the metadata change.

Hidden private global bindings do not supply export/unexport policy to an
independent local binding. Explicit local policy and export-all mode still apply.
An unspecified child policy can inherit an ancestor policy even when a child
binding supplies the value. These distinctions are observable in real child
environments, not only through make variable expansion.

## Pattern scopes: separate activation model

The pattern fixtures establish a separate activation model. Preserve each
declaration in order instead of merging matching patterns into static target
maps. Computed names and simple RHS values use global context while reading;
conditional assignment and appending resolve when the target is activated.
Patterns with shorter stems are more specific; equal-specificity definitions
retain declaration order. A deferred append to a simple pattern binding expands
its suffix in the target activation context while retaining simple flavor.
This behavior is implemented separately from the explicit target scope API.
