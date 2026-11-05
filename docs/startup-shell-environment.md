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
