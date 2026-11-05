# Detached Guile callbacks during parallel recipe waits

Ten original Makefiles cover pipe and FIFO jobservers under `-j2`.
Two directly owned recipe children hold both slots until a detached Scheme
worker publishes its result. In the token-wait cases a third recipe is
expanded while those slots are occupied. The worker expands, evaluates, or
reenters Scheme through the make API. Two models exercise successive requests
during the same blocking wait. Plain Scheme workers provide a control for
each transport.

The observer requires both live direct children, their readiness files and a
blocking syscall on the process-leading thread before releasing the worker.
It verifies the kernel pipe inode and endpoint directions named in the
advertised authentication, or the live PID-named FIFO. Both GNU runs and the
enabled candidate are checked independently for exact streams, exit status,
effects and complete filesystem state. The only variable file fields are the
verified transport identities.

```sh
python3 tools/guile_detached_parallel.py \
    --profile .tools/guile/reference-profile.json \
    --candidate /absolute/path/to/enabled/rmake \
    --report artifacts/guile-parallel-detached/comparison.json
```

The gate uses the enabled Linux x86-64 profile. Fatal reference races,
arbitrary detached callbacks between scheduler waits and descriptor
exhaustion remain separate compatibility cases.
