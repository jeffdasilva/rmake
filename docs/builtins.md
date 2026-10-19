# Linux built-ins and suffix conversion

The product embeds 62 default program/command variables, 49 suffix recipe
records, and nine fixed pattern rules observed from the pinned Linux GNU Make
4.4.1 executable's public `-p` database output. These are declarative command
and variable bytes, with upstream attribution in each data file. No GNU C
implementation is embedded, translated, or invoked by the product. Executing a
built-in recipe launches the configured compiler/tool just like a user recipe.

Default variables have default origin and retain recursive formulas; environment,
file, target, pattern, and command-line overrides use the existing precedence
machinery. `-r` disables built-in rules while retaining command variables. `-R`
disables both. Independent automatic and suffix metadata retain their established
behavior. Runtime-specific metadata is not frozen from the oracle's build paths.

After reading makefiles, the final `.SUFFIXES` order supplies dummy patterns,
single-suffix transformations and double-suffix transformations. Explicit pattern
rules, including cancellations and empty recipes, precede converted alternatives.
Clearing suffixes disables their transformations while retaining fixed built-in
patterns. User-defined suffix transformations work with built-ins disabled.
Prerequisites on double-suffix rules produce GNU's warning and are ignored for
the transformation. Built-in failures use `<builtin>` locations without a line.
Makefile restarts reconstruct this state from the startup database.

`make test-builtins` verifies all embedded bytes against the verified oracle in a
clean environment. It is part of `make check`, `make test`, and CI. The optional
`make regenerate-builtins` refreshes the three data files. The extractor excludes
host/path/automatic/environment metadata, preserves command spacing and terminal
colons, and has three original tooling tests. Both targets also run under rmake.

The 66 new independent differential fixtures cover default variable values,
origins and flavors; compiler command and target/pattern overrides; transformations
for C, C++, assembly, Fortran, Pascal, Objective-C, Modula, yacc/lex and TeX/web;
real C object creation; single-suffix linking; suffix priority, cancellation,
empty recipes, warnings, directories, chains, default-goal exclusion and disabled
built-ins. The corpus has 2,076 cases: 2,064 required passes and 12 pending.

These data records do not imply full engine support for every built-in's domain.
Archive-member target parsing/search and grouped outputs still need work, as do
implicit secondary-expansion search and GNU's cleanup-name hash ordering.
The existing parallel and jobserver limitations also remain separate work.

Reference: GNU Make 4.4.1 `src/rule.c` (`convert_to_pattern`) was read to clarify
conversion staging and precedence. `src/default.c` was inspected for the division
between suffix and fixed rules; the embedded data was extracted from executable
output, not its C tables. The Rust conversion and extractor are independently
authored. The embedded GNU declarations carry their attribution separately.
