# Initial shell environment

GNU expands inherited MAKEFLAGS when constructing the environment for
`$(shell ...)`. Ordinary inherited variables keep their literal values.
When shell environment construction encounters the same active binding,
the child receives the original process environment value for that name.

The seven original required fixtures cover the initial inherited and explicit
argument sources, a command-line MAKEFLAGS binding, environment override
promotion, a consumed GNUMAKEFLAGS binding, and an unrelated inherited
variable. The nested controls record which commands run and the MAKEFLAGS
bytes the children receive. The active-binding control checks the original
bytes in the nested child and the expanded bytes in the outer child.

These cases form the `shell.initial-environment` requirement family. The
standard development and optimized conformance gates compare literal
streams, exit status, and the full fixture filesystem.

## Recursive bindings in a shell function environment

GNU uses the original process value when environment preparation encounters
an active recursive binding through another export. The fallback also applies
when the active variable itself is unexported. An absent original value expands
as an empty string. Verbose mode identifies the variable that recurred, using
`(null):0` when its definition came from the command line. A recursive binding
in an ordinary expansion or a recipe process environment remains fatal.

When an export expands an `eval` that reads another target's variables, the
recursion check follows the binding's table across scopes. A suspended global
binding can remain active at a different layer in the destination, and an
active variable local to the caller can be absent from the destination.

A private pattern binding remains visible to Make expansion on the matching
file. When shell environment preparation skips that binding, the eligible
ancestor or global binding supplies both the value and its export policy.

The original fixtures in `functions.shell` cover direct and indirect paths,
original bytes, computed names, nested shell execution, target-local scope and
the fatal-cycle boundary. GNU Make 4.4.1's [release archive](https://ftp.gnu.org/gnu/make/make-4.4.1.tar.gz)
was consulted in `src/expand.c` and `src/variable.c` for the shell environment
recursion boundary. The Rust implementation and fixtures were written
independently.
