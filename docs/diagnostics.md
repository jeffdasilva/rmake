# Expansion diagnostic locations

Compatibility diagnostics carry both byte-preserving messages and an optional
source location. The source location belongs to the expansion that fails, rather
than necessarily the recipe or parser statement that requested the value.
Original probes against the pinned GNU Make 4.4.1 profile establish the rules
below. This document describes the implemented variable-expansion slice, not a
complete specification of GNU's diagnostics.

The implementation and fixtures were independently authored from executable
observations and the public variable/function documentation. GNU implementation
source and upstream test code were not consulted for this slice.

## Definition and evaluation locations

A recursive variable's failing body uses that binding's definition location.
Nested variable references and user `call` bodies use the innermost failing
definition. Successful nested expansions restore the enclosing context; a later
failure in the enclosing body must not inherit a completed child's location.
Errors while expanding a call's arguments occur before entering its body.

Command-line, environment, automatic and temporary parameter bindings have no
Makefile definition location. Their failing bodies inherit the enclosing
expansion's location, or the calling parser/recipe location when there is no
enclosing definition. Simple values do not acquire a diagnostic context merely
because their bytes are referenced; they are already expanded.

Accepted redefinitions replace the stored location. A nonempty append moves the
whole resulting binding to the append's location, including errors originating
in its earlier text. An empty append preserves the existing binding's location
and origin. Skipped conditional assignments and assignments suppressed by origin
precedence preserve the existing metadata. Immediate right-hand sides still
expand at their required time even when precedence discards their result.

Multiline `define` distinguishes the definition header from the later evaluation
point. A recursive body stores the header location; an immediate body's direct
expansion failure uses the closing `endef` location. An error in a nested named
variable can override that fallback with its own definition location.
Expansion of the definition's name uses the header context. Ordinary continued
assignments and recipes retain the logical line's starting location. A detected
variable cycle reports the definition of the binding being re-entered.

## Runtime expansion boundaries

Most named runtime values, including `SHELL`, `IFS` and exported recursive
variables, use the normal variable-definition context. GNU's raw-body expansion
paths are distinct: `.SHELLFLAGS` expands its stored text without entering its
own definition context, including when its flavor is simple. A recursive
`.DEFAULT_GOAL` also expands its stored body without entering its own definition
context. Nested variable references within those bodies still establish their
own locations. Tests preserve these boundaries separately.

The core retains shared definition locations and attaches the innermost location
to structured expansion errors. Callers provide a fallback only when an error
has no location. This avoids leaking mutable diagnostic state between successful
expansions and keeps the parser and runtime on the same error representation.
Makefile paths, variable names and invalid integer operands remain byte strings;
lossy display formatting must not be used for compatibility output.

## Coverage and remaining work

The original curated corpus checks parse-time and recipe-time use, recursive and
call chains, multiline definitions, append/override/conditional behavior, runtime
configuration, multiple Makefiles and non-UTF-8 filenames. Separate generated
probes vary line padding, chain depth, source order and evaluation context.

Includes, `eval`, target/pattern scopes and other unimplemented expansion effects
will need additional context rules and interaction tests when implemented.
Existing resource bounds and unsupported-feature errors remain development
limitations. Passing this slice does not establish complete diagnostic or language
compatibility.
