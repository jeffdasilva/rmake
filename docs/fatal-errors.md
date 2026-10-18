# Explicit errors and fatal expansion control flow

This slice implements GNU Make 4.4.1's `error` function and global abort behavior
for recipe-time expansion failures. It was independently authored from original
executable probes and the public [Make control functions documentation](https://www.gnu.org/software/make/manual/html_node/Make-Control-Functions.html).
No GNU implementation source or upstream test code was consulted for this slice.

## Expansion and message bytes

`$(error text)` expands its argument and stops evaluation with that result as the
message. Commas remain part of a direct function's single argument. Leading
syntax whitespace is consumed, while trailing message whitespace and non-UTF-8
bytes are preserved. An error encountered while expanding the argument takes
precedence over the outer message.

`$(error)` has no separating whitespace and is an ordinary variable reference.
`$(error )` invokes the function with an empty message. `$(call error)` supplies
no argument and returns an empty value; `$(call error,)` supplies an empty
argument and fails. Like other built-ins invoked through `call`, all supplied
arguments are expanded eagerly, including arguments beyond the function's
accepted maximum. The first resulting argument becomes the message without a
second expansion. An error in a surplus argument therefore wins over the first
argument's otherwise fatal message.

Unselected direct `if`, `and`, `or` and `intcmp` branches remain lazy. Merely
storing a recursive variable containing `error` does not evaluate it. Diagnostic
source precedence is described in [the location notes](diagnostics.md); explicit
errors and numeric validation errors intentionally follow different rules.

## Aborting the build

A fatal expansion result is distinct from a failed recipe process. It stops
remaining prerequisites and top-level goals even when `-k` requests continued
work after ordinary recipe failures. `-i`, a recipe's `-` prefix and `.IGNORE`
do not suppress it. Already completed prerequisites and their files remain;
later work does not run. Fatal expansion does not manufacture child-process
failure or request `.DELETE_ON_ERROR` cleanup.

Recipe bodies are expanded before any of that target's commands run. Shell
configuration and exports have their own later evaluation points. Imported
environment values are passed back to children as raw bytes, even though make
variable references expand those values recursively. Replacing such a binding
with a file assignment changes its origin and can make its exported value expand.
Automatic command-line and export-all selection requires an ASCII letter or
underscore first, followed by ASCII letters, digits or underscores. Explicit
export and inherited environment membership can export other names as well.
Command-line exports have no Makefile definition location. Dry-run and
question modes can still reach expansion errors. An unforced touch-mode shortcut
can skip body expansion; a forced recipe can expose the same error. This is why
fatality alone does not determine whether a stored error is evaluated.

## Boundaries

Original fixtures and independent generated probes cover message bytes, argument
timing, source contexts, modes and suppression of later filesystem side effects.
Existing tests continue to check ordinary child failures, ignored errors and
keep-going behavior separately. The broader implementation still lacks `info`,
`warning`, other expansion effects, includes and scoped variables. GNU's special
`MAKELEVEL` initialization/export behavior also remains incomplete; recorded
pending cases preserve those mismatches. This slice is not a full compatibility
claim.
