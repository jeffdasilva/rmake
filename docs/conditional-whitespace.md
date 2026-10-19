# Conditional operand whitespace

Conditional syntax uses several whitespace rules rather than one common trim.
The implementation preserves their phase relative to argument expansion.

For `ifdef` and `ifndef`, raw whitespace after the directive is skipped before
expansion. The expanded result is then split at its first C-whitespace byte.
Only whitespace may follow that first token. A leading separator followed by a
name is therefore invalid syntax; it is not trimmed into a valid lookup. A
trailing separator is valid. An entirely blank result denotes the empty name.
The binding test still examines the selected variable's stored value without
expanding it. Non-ASCII bytes remain name data under the pinned C locale.

For parenthesized `ifeq` and `ifneq`, only literal space and tab are removed from
the end of the raw left operand. Leading C whitespace is skipped on the raw
right operand. Neither operand is trimmed again after expansion. Literal trailing
whitespace in the right operand remains significant. The gap between quoted
operands and whitespace after the closing delimiter use C whitespace too.

The left operand still expands before right-delimiter validation. Both operands
run before comparison on a valid condition. Name warnings occur before name
validation, while ignored outer conditions and ineligible `else if` branches
skip their name/value effects. The existing source and diagnostic context remain
unchanged.

One hundred thirty original fixtures cover all six C whitespace bytes where
expressible, literal versus expanded separators, both positive/negative
conditionals, non-ASCII name bytes and output/error ordering. The 122-program
operand matrix had 26 differences before the correction and none after; all
independently authored status/output anchors match the oracle.

GNU Make 4.4.1 `src/read.c` (`conditional_line`, its name-token check, asymmetric
operand trimming and expansion order) was consulted. The Rust implementation
and fixtures are independently authored; no upstream source text or tests were
copied. Broader definition, directive and generated-input grammar remain separate
compatibility work.
