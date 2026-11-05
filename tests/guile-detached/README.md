# Detached Guile callbacks during serial recipe waits

The six models use original Makefiles and a Guile-enabled GNU Make 4.4.1
reference. They check a detached Scheme-only worker, a callback released during a
later Scheme evaluation, and detached expansion, evaluation, and same-worker
nested Scheme entry while Make waits for a real serial recipe child. A final
model opens and closes two independent wait scopes in the same invocation.

The observer releases a callback only after the recipe has created its
complete readiness file and the process-leading thread is observed in its
blocking Linux x86-64 child-wait syscall. The recipe waits until the worker
atomically publishes its result file. The runner compares both GNU runs and
the candidate independently to the original output, status and complete
filesystem expectations, including the ordered readiness and result files.

Run from the repository root with a verified enabled profile and an enabled
candidate. Omit `--candidate` for an oracle-only repeat.

```sh
python3 tools/guile_detached.py \
    --profile .tools/guile/reference-profile.json \
    --candidate /absolute/path/to/enabled/rmake \
    --report /absolute/path/to/detached-report.json
```

The original fatal-during-wait probe had variable GNU output across its two
runs. Both drained the child and returned status 2. One printed an additional
wait error. That input remains separate from the six stable controls. Parallel
job waits, shell-function capture, arbitrary native threads and descriptor
resource limits require their own acceptance.
