# Guile worker admission under descriptor pressure

The three original Linux x86-64 profiles lower the interpreter's soft and hard
descriptor limits after Guile initialization. A real recipe checks the inherited
limit and atomically publishes proof of successful exec. For the two pressure
profiles, a native witness fills the remaining slots with private descriptors
only during the eventfd call, invokes the actual kernel syscall and closes only
its own descriptors. Its separate trace requires `EMFILE` after the exec marker.
GNU does not make that eventfd call.

Two profiles release a detached worker only after the recipe marker and a
blocking interpreter wait are observed. The worker performs successive
`gmk-eval` and `gmk-expand` calls, and the recipe observes the worker result
before it can finish. The runner checks the original expected status, streams,
files, observer trace and process bounds independently for both enabled GNU
Make 4.4.1 runs and the enabled rmake build. The ordinary eventfd path remains
a control.

Run `tools/guile_eventfd_pressure.py` with a verified enabled `--profile`,
optional `--candidate`, and a fresh `--report` path.
