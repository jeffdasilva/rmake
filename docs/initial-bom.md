# Initial UTF-8 byte-order mark

The parser skips one complete UTF-8 BOM (`EF BB BF`) at the beginning of the first
observed logical line of each input file. This happens after the reader consumes
and counts stream bytes. Removing the bytes from the original source before
reading would change NUL-buffer boundaries, warnings and retained input.

Later BOMs, BOMs after leading whitespace, a second consecutive BOM and partial
prefixes remain ordinary bytes. Values, definition bodies and recipes retain
them too. Line positions are unchanged, including first-line warning locations,
continuations and CRLF. Each separately read Makefile applies its own first-line
handling. This does not introduce Unicode decoding or alter arbitrary byte names.

Twenty-six original fixtures cover these boundaries, including first-line
directives, explicit goals, multiple input files and four NUL-buffer edges.
A separate 400-program NUL-stream campaign with initial BOMs matched exact
streams and statuses against the pinned GNU Make 4.4.1 oracle.

GNU Make 4.4.1 `src/read.c`, the initial-BOM handling in `eval`, was consulted for
the read/parse phase boundary. The Rust implementation and fixtures are
independently authored; no upstream source text or tests were copied. General
debug messages and future generated-input readers remain separate work.
