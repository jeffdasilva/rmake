# Integer comparison compatibility

The target is GNU Make 4.4.1's `intcmp` behavior on the pinned Linux profile.
The [conditional-function documentation](https://www.gnu.org/software/make/manual/html_node/Conditional-Functions.html)
and [call documentation](https://www.gnu.org/software/make/manual/html_node/Call-Function.html)
describe its public interface. The details below were checked with original
executable probes; GNU implementation source was not inspected for this feature.

## Numeric representation and ordering

Operands are decimal byte strings of arbitrary length within the engine's
existing expansion limits. Surrounding space, tab, newline, vertical tab, form
feed and carriage return are accepted. One leading sign is allowed, followed by
at least one ASCII digit. Leading zeroes are insignificant, and signed zero
normalizes to unsigned zero. Parsing does not use a fixed-width integer or
floating point.

The pinned release has a negative-number ordering quirk: two nonzero negative
operands compare by increasing absolute magnitude. Consequently,
`$(intcmp -19,-7,less,equal,greater)` returns `greater`. Opposite signs retain
their usual ordering. Original regressions explicitly preserve this observed
behavior; a mathematical comparison would break the requested compatibility.

The Rust representation borrows normalized digit slices from expanded operands.
Sign, digit count and lexicographic comparison avoid integer overflow and remain
linear in operand length. The independent generated-test model uses Python
integers, with an explicit adjustment for the observed negative-number quirk.

## Expansion and default branches

Both operands expand from left to right before either is validated. Validation
then checks the first operand before the second. Only the selected result branch
expands. A missing greater branch uses the equal branch; other missing selected
branches produce an empty result. With exactly two arguments, equal operands
return their normalized decimal spelling. An explicit third argument, including
an empty one, disables that two-argument result rule.

Direct invocation accepts at most five argument slots, with additional commas
retained in the fifth slot. Through `call`, all supplied parameters expand first,
including surplus parameters. The builtin then receives up to five parameters
and performs its own operand and selected-branch expansion. Tests distinguish
eager errors, escaped expressions, unselected branches and nested calls.

`call` selects the first whitespace-delimited word of its expanded function name.
For user macros, `$(0)` ends at that word, while retaining any leading whitespace
introduced by expansion. Temporary positional bindings restore the enclosing
call's parameters after nested calls return.

## Diagnostic bytes and coverage

An invalid operand's diagnostic includes its original expanded bytes, including
surrounding whitespace and invalid UTF-8. Whitespace-only operands have the
separate `empty value` diagnostic. Raw bytes must remain intact through the
variable core, parser and runtime; a human-readable `Display` rendering is not
the transport for compatibility output.

Curated fixtures cover numeric limits, normalization, branch defaults, laziness,
comma handling, `call`, error precedence and parse/recipe/command-line diagnostic
contexts. Generated programs independently predict outputs for bounded random
operands and expression shapes. Exact oracle comparison remains required for
streams, status and filesystem observations. These tests do not establish
completeness of unrelated functions, expansion effects or diagnostic locations.
The recursive user-macro location regression is now required coverage after the
[source-location implementation](diagnostics.md). Later expansion features will
need their own location and interaction tests.

`make test-intcmp` runs 200 programs with 32 expressions each against both the
oracle's independent expected results and the candidate. `INTCMP_SEED`,
`INTCMP_CASES`, `INTCMP_EXPRESSIONS` and `START_INDEX` reproduce or extend a
campaign. Reports retain input snapshots and identities, exact invocation and
environment details, and failing observations. Resource limits and incomplete
captures are failures, never matching outputs.
