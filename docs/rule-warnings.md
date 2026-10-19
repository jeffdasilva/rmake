# Rule diagnostic timing

Rule warnings belong to a completed declaration, not to its target list or the
first recipe line being read. The reader now keeps a declaration object with
targets, prerequisite offsets, header location, recipe state and pending
diagnostics. This state is local to each input read.

Repeated targets warn only when that declaration has a recipe. An explicitly
empty recipe and a recipe comment count; an absent or ignored recipe does not.
Warnings use the header location, while replacement warnings use the new and old
recipe locations. Duplicate and replacement warnings retain target occurrence
order, including interleaved repeated names. Additional recipe lines do not
repeat the declaration's warnings.

Blank lines, comments and conditionals keep a declaration open. Conditional
expansion effects therefore precede pending rule warnings. Assignments,
undefines, defines, export lists and a following rule close it before their
name/value effects. Successful EOF also closes it. A fatal conditional or
missing `endif` stops before the pending rule is recorded; its warnings do not
leak into that fatal path. Read-time NUL warnings retain their own earlier phase.

The graph still uses the existing explicit-rule representation. Prerequisite
occurrences, recipe replacement and default-goal behavior are not a complete
implementation of GNU's deferred file database. This change establishes warning
timing for the currently supported declaration forms; includes, generated input,
implicit and other unsupported rule families remain separate work.

Twenty-nine original fixtures and one promoted case cover these boundaries,
including line continuations, source bytes, effects and failures. A separate
440-program matrix crosses target lists, recipe forms, prior recipes and closing
statements. All comparisons match exact streams and statuses. The previous
2,380-program modifier matrix also now matches without warning normalization.

GNU Make 4.4.1 `src/read.c` (`eval`, `record_waiting_files`, `record_files`) was
consulted for declaration barriers, EOF/fatal ordering and warning predicates.
The Rust implementation and fixtures are independently authored; no upstream
source text or tests were copied.
