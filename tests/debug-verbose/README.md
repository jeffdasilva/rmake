# Verbose debug decisions

These 23 original Makefiles exercise VPATH and GPATH spelling, observed
prerequisite timestamp changes, serial pruning, declaration order-only
roles, always-make, empty double-colon branches, existing targets without
recipes, and stopping and keep-going prerequisite failures.

The runner compares two GNU Make 4.4.1 invocations and one candidate to the
literal C and French streams. Each invocation starts from the same files and
fixed nanosecond timestamps. The runner checks the exit status, complete
filesystem, and final timestamps independently. The changed-prerequisite
cases set a timestamp backwards and check the resulting diagnostic and the
parent's freshness decision.

Run from a checkout with the pinned oracle and locale data.

```sh
python3 tools/debug_verbose.py \
    --candidate /absolute/path/to/rmake \
    --locale-root .tools/locale-data \
    --report artifacts/debug-verbose/comparison.json
```

The timestamp-change and pruning models cover serial visits. Parallel
reconsideration, live debug-mask changes during recipe execution, and broader
grouped and archive combinations remain separate compatibility cases.
