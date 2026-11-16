# Verbose debug decisions

These 73 original Makefiles exercise VPATH and GPATH spelling, observed
prerequisite timestamp changes, serial pruning, declaration order-only
roles, always-make, empty double-colon branches, existing targets without
recipes, stopping and keep-going prerequisite failures, and missing and
successful file reads with non-ASCII paths, makefile loop prevention, and
directory cache invalidation during parsing and after optional makefile
remaking.

The runner compares two GNU Make 4.4.1 invocations and one candidate to the
literal C and French streams. Each invocation starts from the same files and
fixed nanosecond timestamps. The runner checks the exit status, complete
filesystem, and final timestamps independently. The changed-prerequisite
cases set a timestamp backwards and check the resulting diagnostic and the
parent's freshness decision.
The cache controls check first access, repeated access, a rebound directory
alias, and a shell launch failure alongside a stale lookup. The C and French
streams retain the same cache diagnostic.

Dry-run and query controls verify the reported age of a simulated prerequisite
with no physical file. They include explicit dependencies, an implicit chain,
explicitly newest parents with shared and repeated dependencies, empty
recipes, phony targets, explicit groups, actual forced commands, and order-only
prerequisites. Repeated dry-run prerequisites also exercise both jobservers.
Explicit grouped dry-run cases check physical owner and peer ages, a missing
prerequisite of a newest parent, and physical rechecks of a newest peer.
Always-make fixtures distinguish an existing recipe-less leaf from missing,
phony, and double-colon targets. Prerequisite controls cover older and newer
normal inputs, missing normal inputs, newer order-only inputs, and an
explicitly newest recipe-less target. Assumed-new phony cases cover
repeated visits, normal and order-only inputs, and equal newest times
when a missing prerequisite requires updating the phony target.
Synthetic assumed-new double-colon cases cover consideration before
failed resolution in serial and parallel and a successful default recipe.

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
