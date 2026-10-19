# Dynamic makefiles

Required `include`, optional `-include` / `sinclude`, ordered nested reads,
expanded names, escaped spaces, and wildcard matches use an explicit runtime
reader. Core parsing still performs no operating system I/O. Included files
share the current database, but have independent conditional and recipe state.
`MAKEFILE_LIST` is updated immediately before each successful read.

After all files are read, the engine updates makefiles as build goals. Missing
required include diagnostics are deferred until a remake fails; successful
regeneration has no missing-file warning. A changed mtime causes a new database
to be read from the startup bindings, with `MAKE_RESTARTS` incremented. The
restart clears graph traversal and target context observations. Stdin is saved
for reuse rather than consumed twice. Phony makefiles are excluded from the
remake loop. Dry-run, query and touch modes do not suppress makefile generation.

Safety limits bound include nesting and restarts. There is no persistent cache.
Search directories (`-I`), `MAKEFILES`, double-colon remake exceptions, and the
complete GNU error recovery rules remain follow-up compatibility work. These
limits and remaining features are not claims of complete include compatibility.

Validation adds 25 original differential fixtures and promotes seven existing
include/remake/error fixtures. Cases cover nested lists and conditionals,
optional parse errors, duplicate reads, escaped names, sorted glob matches,
missing files with empty rules, multi-pass generation, command-line binding
preservation, mode flags, default goals, and phony restart avoidance.
GNU Make 4.4.1 `main.c` and `remake.c` were consulted for remake staging and
deferred errors; no upstream implementation or test text was incorporated.
