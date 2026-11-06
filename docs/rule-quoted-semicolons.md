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

One recorded generated-assignment case still needs reader classification
work. If an expansion supplies `a:W=` and a quoted semicolon is exposed in
the same source word, GNU stops the assignment value at the expanded recipe
boundary. rmake currently retains the remainder of that word in the value.
