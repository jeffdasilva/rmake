# Source words with generated rule delimiters

When an expansion introduces a rule delimiter, the reader expands the rest of
that source word before classifying the declaration. A later reference in the
same word can supply part of the variable name, the assignment operator, or
the value. Literal bytes can occur between those references. Expansion still
stops at the next raw token boundary, so separate words keep their prerequisite
or deferred value semantics.

The source-word suffix is consumed once. Its effects are visible during early
target globbing. A raw inline recipe separator retains the established recipe
or target-assignment handling, including deferred text in recursive values.

Sixteen original required Makefiles cover adjacent references, grouped and
double-colon forms, raw punctuation, recursive values, `eval`, glob effects,
and separate-word controls. The GNU 4.4.1 reader was consulted for source-word
and declaration phase ordering. The Rust implementation and fixtures were
written independently.
