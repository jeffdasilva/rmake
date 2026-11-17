# Directory reporting

Use `-w` or `--print-directory` to see the working directory of a build and its
recursive Make processes. For example, run the default goal in an existing
`build` directory.

```sh
rmake -C build -w
```

Use `--no-print-directory` to suppress those messages. The order and origin of
the options determine the active setting. An explicit command-line choice takes
precedence over later Makefile flag assignments. A recursive build or `-C` can
enable automatic reporting. Silence can disable that automatic entry, while an
entry already printed still gets a corresponding leave message. See GNU Make's
[option summary](https://www.gnu.org/software/make/manual/html_node/Options-Summary.html)
and [recursive use](https://www.gnu.org/software/make/manual/html_node/Options_002fRecursion.html).

## When a directory message appears

A process-level message appears when Make publishes output or starts a child
that needs reporting. A quiet recipe reduced to `@:` can finish without a
child and without a directory pair. A real quiet child can still trigger the
pair.

With line or target synchronization, directory lines frame nonempty buffered
output. A group containing only stderr still receives its frame. The header,
payload, and footer are written under the same coordination lock. An empty
group emits no pair. A live switch to unsynchronized output can open the
process trace while earlier buffered output still takes its own frame. See
[output during parallel execution](https://www.gnu.org/software/make/manual/html_node/Parallel-Output.html).

## Restarts and process exits

A Makefile remake can replace the process image. When a trace has already
entered, rmake carries that state in its restart environment. The final image
prints the leave message before restoring the original working directory.
Early output can use the available directory and recursion level before both
are known. Native finalizer output after the trace closes leaves it closed.
See the [restart guide](restart-state.md) and the
[imported directory trace tests](imported-directory-trace.md).

## Qualification and remaining work

Run the full comparison or replay one stored case from the repository root.

```sh
make test-directory-reporting
make test-directory-reporting DIRECTORY_REPORT_CASE=directory-explicit-w
```

The development report is `artifacts/directory-reporting/development.json`.
The gate runs the pinned GNU Make twice and compares the candidate's exact
streams, status, complete expected filesystem, ordered effects, and process
bounds. Predetermined paths come from the reset workspace. The single explicit
version slot is calibrated separately and keeps surrounding directory lines
in place. Two original native finalizer profiles use a C compiler.

The initial checkpoint passed 40 independent directory models in both builds.
The preceding rmake passed four. The
[test corpus](../tests/directory-reporting) retains the original predictions,
input sources, and observations. Later tests cover inherited option policy,
recursive invocations, and output synchronization. The
[historical performance reports](benchmarks/directory-reporting.md) retain the
measured allocation and timing costs.

The top-level developer flows include development and optimized comparisons and
a focused replay under rmake. See the [M3/M6 audit](m3-m6-audit.md) for remaining
directory, locale, resource, and diagnostic interactions.
