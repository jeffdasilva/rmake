# Informational output and warnings

This slice implements `info` and `warning` against the pinned GNU Make 4.4.1
profile. Both expand their argument and return an empty string. `info` writes to
standard output; `warning` writes a diagnostic to standard error and continues.
The public [Make control functions documentation](https://www.gnu.org/software/make/manual/html_node/Make-Control-Functions.html)
describes these functions. The detailed behavior below comes from independently
authored executable probes; no GNU implementation source or upstream tests were
consulted for this slice.

## Bytes, arguments and evaluation

Direct calls accept one argument, including any commas in its text. Function
syntax consumes leading whitespace, while trailing whitespace, embedded newlines
and non-UTF-8 message bytes survive. Each emission appends a newline; an empty
message still produces output. `$(info)` and `$(warning)` without separating
whitespace are ordinary variable references.

Calls through `call` evaluate every supplied argument before invoking the
built-in, including surplus arguments. Only the first resulting argument is
printed, without a second expansion. Calling either function with no argument
produces no output. Empty arguments, nested messages and a failure while computing
the outer message are tested separately: completed inner output survives, but a
partially computed outer message is never emitted.

Messages occur when an expression is expanded. Storing a recursive value does
not print it; each later expansion can print again. Immediate assignments print
at assignment time, even when origin precedence later discards their value.
Unselected lazy branches do not print. Command-line immediate assignments emit
before Makefile parsing and before the invocation-depth prefix is installed.

Recipe bodies expand before that target's commands execute. Shell configuration
and exported bindings have later evaluation points; they must not be eagerly
evaluated with the body or cached after their first use. Dry-run, question and
touch modes can take different paths through these phases. The curated and
matrix tests preserve those observed differences.

## Locations and output order

Warnings use the active parser, recipe or exported-variable context. With no
active caller, the first entered recursive binding establishes the context
through nested references and user calls, even when it has no source location.
Explicit `error` uses this same context; numeric validation failures instead
retain their innermost definition. A direct recursive
default-goal body has no location of its own at evaluation time; a nested binding
can still supply one. A warning without a location uses the
invocation name, including the startup `MAKELEVEL` suffix when applicable.
Informational messages have no location or invocation prefix. A conditional's
extraneous-text warning occurs after its left operand is expanded and before
its right operand is expanded.

Synthetic metadata updates must not evaluate expressions accidentally. Default
goal selection stores the already-expanded target name literally. Appending a
filename to `MAKEFILE_LIST` preserves the previous raw bytes and flavor, after
its distinct startup initialization; it does not execute a recursive body.

Parser warnings about duplicate recipes and duplicate targets now emit when
encountered. Their order relative to user messages is observable, and a later
fatal parse error must not erase them. Target names in these warnings retain
their original bytes. Separate-stream checks verify each stream; merged-output
fixtures also verify ordering between informational messages, warnings, command
echoes and child output.

## Internal effect interface

The core reports borrowed, typed output events through an explicit expansion
context. The executable renders and flushes those events synchronously. The
context carries the active caller; recursive definition fallback is local to
one expansion and restored after nested calls. Variable-store clones do not own
or share a mutable output buffer.

Pure convenience APIs reject an attempted informational/warning emission instead
of silently dropping it. Runtime callers supply an output receiver, while unit
tests can collect events without process or filesystem access. Completed output
is an effect even if the expansion later returns an error; it cannot be replayed
or eliminated by a future cache.

## Remaining work

Multiple exported effectful variables expose a general export-order mismatch:
the current ordered variable representation does not reproduce GNU's traversal
order. Dedicated pending fixtures retain this gap. Make-flag evaluation and
replay, includes, `eval`, target/pattern scopes and other unimplemented effects
also require their own interaction work. Passing this slice does not establish
complete output, diagnostic or language compatibility.

`make test-diagnostics` runs the permanent 206-cell independent matrix, first
against the oracle itself and then against the candidate. Each cell checks exact
streams, status and filesystem results. `DIAGNOSTIC_CASE=<case ID>` replays one
cell; failure artifacts retain the source and observations. Curated fixtures add
focused regressions and the explicitly pending boundaries.
