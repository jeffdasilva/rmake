# Waitable child before the outer prerequisite wait

The inner target starts a held recipe child. The supervisor sizes an anonymous
stdout pipe to one Linux page and leaves room for the literal trace through
recipe dispatch. A full pipe blocks publication of the inner wait. The
supervisor then releases the worker and checks that its PID, parent, and start
time still identify the same recipe and that the process is a zombie before
stdout is drained. Both GNU runs and rmake receive the same barrier.

The stop case observes the outer wait before the child error ends the graph.
The success case also observes the post-reap parent and cached prerequisite
visits. The gate compares complete stdout, stderr, exit status, the filesystem
snapshot, and child cleanup. The supervisor removes the exact padding it put
in the pipe before execution. All bytes produced by either engine are compared.

The gate needs normal Linux anonymous pipes with `F_SETPIPE_SZ` and `FIONREAD`,
and procfs visibility of the test's owned processes. It runs without elevated
permissions, ptrace, or pidfds. A missing barrier or unavailable interface
fails the gate.

Run `make test-debug-reap-race` for development or
`make test-debug-reap-race-release` for optimized validation.
