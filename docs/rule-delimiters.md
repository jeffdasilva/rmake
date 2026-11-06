# Rule delimiter provenance

The reader keeps the target boundary, prerequisite start, grouping, and colon
width together once it recognizes a rule delimiter. The same boundaries drive
target-specific assignments, early filename expansion, and ordinary rules.

A literal `&:` groups targets. If the target expression supplies a trailing
ampersand and the source supplies a separate `:`, the ampersand belongs to the
target name. A colon generated before a literal colon also leaves two separate
delimiters, so the second introduces static-pattern parsing. Adjacent references
can form a single double-colon delimiter when they expand within one source
word. Prefix effects run once, and a recursive target assignment retains its
unexpanded value.

Twenty original required Makefiles cover ordinary and grouped rules,
target-specific and pattern-specific assignments, `eval`, invalid static
patterns, and effect ordering. Their exact exit status and output anchors are
qualified against two verified GNU Make 4.4.1 runs, including filesystem and
process checks. The pinned GNU `src/read.c` was consulted for rule token and
delimiter phases. The Rust implementation and fixtures were written independently.

An ordinary, static-pattern, or grouped declaration with no target names
consumes its attached recipe lines without recording a rule. The reader
keeps effects already evaluated while scanning the target and skips the
unexpanded prerequisite tail. A new statement closes the declaration,
and include and eval frames keep their own recipe attachment state.
Thirty-one required fixtures cover these boundaries and true orphan recipes.

Wider reader and profile combinations remain part of the M3/M6 audit.
