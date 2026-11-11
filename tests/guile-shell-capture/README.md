# Detached Guile callbacks during shell capture

These original Makefiles release a Scheme worker after a real shell child has
started and make has reached the blocking capture wait. The observer confirms
the child PID and start time and matches its stdout pipe to a descriptor held by
make. Both GNU runs and the candidate must match the expected stdout, stderr,
exit status, complete filesystem, and process containment.

The cases cover ordinary Scheme, reader and target recipe expansion, shell
assignments, evaluation effects, lexical call scopes, consecutive requests,
nested Guile, and nested shell captures. The two-worker case holds both
captures and releases the second worker only after the inner wait is visible.
Pipe and FIFO profiles also verify a callback’s output during target and
recursive output synchronization. The pinned Guile gate runs this
comparison for both enabled build profiles.

```sh
python3 tools/guile_shell_capture.py \
    --profile .tools/guile/reference-profile.json \
    --candidate /absolute/path/to/enabled/rmake \
    --report artifacts/guile-shell-capture/comparison.json
```
