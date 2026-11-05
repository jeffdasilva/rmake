# Process-limit launch errors

These sixteen original Linux cases apply a hard and soft `RLIMIT_NPROC`
of zero to a disposable make process. Before exec, the launcher must
observe a real fork failure with `EAGAIN`. The gate requires an
unprivileged Linux account subject to the process limit and fails if
that witness is unavailable.

The cases compare exact streams, status, complete filesystem state, and
process containment with two GNU Make 4.4.1 executions. They cover
recipes, shell expansion and assignment, output synchronization,
recursive setup, and lookup errors for missing, nonexecutable, and
directory candidates. A later executable candidate retains the actual
resource error.

Separate research combined the process limit with a tight descriptor
limit. GNU's failed shell captures exhausted its descriptors while
rmake released the failed captures. That difference remains open
outside this selected gate.
