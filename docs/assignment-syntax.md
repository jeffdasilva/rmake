# Assignment syntax boundaries

These notes describe the implemented subset against the pinned GNU Make 4.4.1
oracle. The original fixtures in the `variables.assignment-whitespace`,
`variables.assignment-recognition`, and `variables.assignment-operators`
requirement families retain exact byte expectations and pending boundaries.

Assignment recognition is lexical. It does not expand a name, evaluate a value,
or apply shell quoting. A single scan returns borrowed name and value slices and
the assignment kind. Name expansion and value evaluation happen later, with the
appropriate origin, precedence and recursive/simple timing.

## Names and whitespace

All six C whitespace bytes are skipped at the beginning of a raw name and value.
Only unconsumed space and tab separate a raw name from its operator. Once that
trailing separator starts, further ordinary name text rejects the assignment.
Quotes and backslashes are literal name bytes and cannot protect a raw blank.
Whitespace produced by expanding a valid name remains part of that name.

A short `$x` reference consumes its following byte, including a blank, dollar,
operator character or hash. Long `$(...)` and `${...}` references consume their
balanced contents before operator recognition. The name span must retain a blank
consumed by a reference: `X$ =value` defines `X`, whereas trimming that blank first
would incorrectly define `X$`.

## Operator tokens

The scanner recognizes `=`, `:=`, `::=`, `:::=`, `+=`, `?=` and `!=` only outside
consumed references. Identifying `!=` does not implement its execution; actual
shell assignments remain an explicit unsupported feature.

| Input | Observed assignment |
| --- | --- |
| `X$+=value` | Recursive `X`; `$+` consumed the plus. |
| `X$!=value` | Recursive `X`; the value is deferred data, not a command. |
| `X$:=value` | Recursive `X`; `$:` consumed the colon. |
| `X$::=value` | Simple `X`; the remaining operator is `:=`. |
| `X$::::=value` | Immediate recursive `X`; the remaining operator is `:::=`. |
| `X\=Y=value` | Recursive name ending in backslash, with value `Y=value`. |
| `$(subst =,_,A=B)=value` | Recursive `A_B`; the inner equal signs are reference contents. |

An unconsumed colon that is not part of an adjacent valid operator rejects
assignment recognition. An unconsumed hash also rejects it, including in CLI
arguments and after a backslash. A hash inside a make reference is ordinary
reference content. Physical-line comment processing remains a separate phase.

The rule-separator helper has different backslash rules and is deliberately not
used to tokenize assignments. File lines that are not assignments retain their
normal rule/directive handling; CLI arguments become goals. Effects produced by
those later paths are observable and covered independently.

## Directive-shaped variable names

Assignment recognition precedes conditional and directive dispatch. Thus
`ifdef = value`, `define := value` and `undefine += value` assign variables rather
than opening a conditional/definition or deleting a binding. This also applies
inside ignored branches: recognized assignments are skipped without expanding
names or values and without changing the conditional stack. Active assignments
close the preceding rule before their expansion effects.

Forty-two original fixtures cover keyword names, recursive/simple metadata,
ignored and active conditional stacks, effects and later genuine directives.
A separate 324-program matrix crosses eighteen keywords, six operators and
three spacings; all comparisons match the pinned oracle after the priority fix.
GNU Make 4.4.1 `src/read.c` (`parse_var_assignment`, the assignment-first path in
`eval`) informed this independently authored dispatch correction. This later
source-informed change is separate from the original scanner investigation below.

## Remaining boundaries and provenance

Broader colon-rule grammar, recursive option transport and actual shell
assignments remain separate compatibility work. The
[export-binding notes](export-bindings.md) describe name-list materialization and
its environment boundaries. The later [command replay slice](command-replay.md)
implements post-read binding replay within its documented scope. Passing token fixtures does not imply
complete support for these features or all `define` and directive combinations.

The scanner and tests were independently authored from original executable probes
and observed GNU 4.4.1 behavior. No GNU implementation source or upstream tests
were consulted for these assignment slices. The implementation avoids allocation
and repeated scans during recognition, but whole-program performance claims
require measurements; see the [progress record](progress.md).
