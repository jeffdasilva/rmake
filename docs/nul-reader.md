# NUL bytes in Makefile input

Literal NUL bytes in a file follow GNU Make 4.4.1's file-stream behavior. They
are distinct from NUL bytes produced during expansion or from a future `eval`
string reader. Files without NUL bytes retain the existing reader path.

The stream model keeps the reference reader's initial 200-byte capacity and its
growth across logical lines. A read consumes its complete chunk, but only bytes
before its first NUL remain visible. The next read can append to that prefix,
including text from a later physical line. A NUL at the start of a chunk instead
produces a warning and a synthetic newline. Observed line numbers count retained
newlines, so diagnostics need not use the physical line number in the file.

Continuation and recipe-prefix handling applies to the retained text. Warnings
occur before interpreting that text. Define-body reads use the definition
reader's preceding source position, including continued headers. CRLF and final
unterminated input retain the same boundaries.

The Linux reference passes remaining capacity through a signed 32-bit `fgets`
count. A nonpositive converted count ends the read. Repeated NUL-bearing lines
can reach that boundary with a small input. rmake models capacity as an integer
and stores only visible bytes; it does not allocate the reference's large virtual
buffer. All Rust indexing and ownership remain safe. General huge-file/resource
and alternate-profile behavior remain release-validation work.

Original fixtures cover chunk edges, earlier capacity growth, warning and fatal
positions, define bodies, deferred effects, recipe comments, CRLF, EOF and the
signed read-size boundary. Two reader tests cover retained text, line counts,
warning consumption and capacity lifetime. A separate seeded differential
campaign exercises mixed chunk sizes, ignored tails and continuations.

GNU Make 4.4.1 `src/read.c` (`readline`, `eval`, `do_define`) was consulted to
understand the stream and diagnostic phases. The Rust implementation and fixture
inputs are independently authored; upstream source text and tests are not copied.
