# Quoted semicolons in rules

The rule reader removes one layer of backslash quoting before a source
semicolon, outside make references. It keeps the original physical recipe
offset and leaves the recipe text available with its original bytes. Without a
physical recipe delimiter, the expanded prerequisite text has its own
semicolon scan and quote reduction. A semicolon exposed by source dequoting
can begin a recipe, while one still quoted during expansion remains part of
a prerequisite. Backslash parity determines which case applies.

The source pass also prepares target-specific assignment text. Top-level
assignments and recipes retain their own quoting rules. Thirty-six original
GNU2 fixtures cover backslash counts zero through six, generated and literal
colons, original recipe boundaries, lazy values, raw bytes, and ordered file
and diagnostic effects. The GNU Make 4.4.1 reader was consulted for the two
scan phases. The Rust code and fixture sources were written independently.

An inline separator exposed in an expanded source word cuts the generated
assignment value. The reader expands the remaining recipe text before
recording that assignment, even when the target is unused. A still-quoted
semicolon loses its quoting byte and remains part of the variable name or
value. The following raw text retains the assignment's usual expansion
timing. A physical inline separator bounds syntax lookahead, and a recognized
assignment keeps the original separator and following bytes in its value.

Twenty additional GNU2 fixtures cover simple and recursive values, protected
variable names, file effects, `eval`, multiple targets, and fatal expansion
before recipe selection.

When prefix expansion has already exposed an ordinary inline recipe, the
remaining source expands before target filename observation. The reader keeps
that expansion for the recipe, so file effects run once. A physical inline
delimiter, a still-quoted semicolon, or a delimiter found only in the later
prerequisite expansion retains the usual target-first order. Fourteen GNU2
fixtures check created and removed glob matches, grouped and double-colon
rules, recipe arguments, and file and diagnostic effects.

When the expanded prefix contains a quoted semicolon and source text remains,
the reader retains the first dequoting result for the scan after prerequisite
expansion. The raw remainder can be whitespace or a reference that expands to
empty text. Target filename observation still precedes that expansion.
Thirty-two GNU2 fixtures cover both scan counts, backslash parity, comments,
continued lines, `eval`, recipe overrides, and ordered file effects.

Quoted semicolons in expanded target names lose one quoting level before
filename preparation. The reader keeps the resulting spelling separately so
the recognized rule delimiter and recipe offsets still refer to the expanded
line. With a literal inline recipe, filename preparation uses the spelling
produced by source parsing and expansion. Thirty-one GNU2 fixtures cover
generated and literal colons, even and odd parity, raw target bytes, scoped
assignments, globs, pattern rules, and grouped and double-colon targets.

The physical inline delimiter also governs prefix scanning. With that
delimiter present, generated semicolons in the target spelling remain
filename data, and the reader can recognize the rule colon before expanding
the prerequisite tail. Fifteen GNU2 fixtures cover generated and literal
colons, target assignments, grouped and double-colon rules, and files created
or removed after globbing.
