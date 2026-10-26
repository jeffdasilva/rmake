# Stateful timestamp fixtures

The current 65 original scenarios make 287 serial invocations per engine. Every case is required; there is no pending classification. They cover incremental rebuilds and no-ops, deletion, equal and adjacent nanosecond timestamps, order-only prerequisites, a rebuilt child restoring its old timestamp, dry-run propagation, question mode, always-make automatic variables, and touch mode.

Eight lifecycle scenarios add forced-command touch completion, dry-run timestamp preservation, query continuation deletion, optimized colon commands, precious/phony protection, and changed-content targets with restored timestamps. They were independently authored after the [source-informed behavior investigation](../../docs/forced-touch.md); no upstream tests or implementation text were copied. Exact timestamp and unchanged-file expectations distinguish normal failure cleanup from continuation cleanup. Later verification steps independently check preserved content, deletion, and absence of forbidden recipe output.

The runner is `python3 tools/stateful.py --self-test` for two oracle runs, or `python3 tools/stateful.py --candidate target/debug/rmake`. The top-level Makefile exposes these flows. The GNU Make 4.4.1 oracle must pass the same pinned profile verification used by the main differential suite. `--case ID` selects a scenario; failures are saved under `artifacts/stateful` by default. Every run also writes a JSON campaign summary under `artifacts/stateful/campaigns`, or to an explicit `--report PATH`, with executable identities, input hashes, step configuration and results.

Each scenario uses a `Makefile` and a small `case.toml`:

- `initial` and each step's `actions` accept `write(path, content, mtime_ns)`, `utime(path, mtime_ns)`, or `remove(path)`. Paths stay inside the work directory and actions reject symlinks and special files.
- Every step declares exact `stdout`, `stderr`, and `exit_code` expectations. GNU must satisfy these independently of the candidate comparison.
- `unchanged` requires a file's bytes, metadata and nanosecond mtime to remain unchanged during that invocation. `exact_mtime` requires a specified deterministic timestamp. `mtime_order` declares `[left_path, relation, right_path]`, where the relation is `<`, `<=`, `=`, `>=`, or `>`.

The same alias and workspace path are used for both engines. Steps preserve workspace state; resetting occurs only between engines. Initial regular-file and symlink timestamps are set to a fixed past value. Each requested `os.utime(..., ns=...)` value is checked by reading `st_mtime_ns` back, including a one-nanosecond calibration. Fixtures explicitly reset output timestamps before selected no-op and forced-rebuild steps so unintended writes cannot hide behind filesystem clock granularity.

Snapshots compare bytes, exit status, file types, modes, symlink destinations and hard-link topology exactly. File timestamps are compared against deterministic anchors, against the file's own pre-invocation timestamp, and through declared ordering predicates. Unrelated wall-clock writes are not required to have identical absolute timestamps or distinct clock ticks. Failure reports retain the exact input tree, mutations, arguments, environment, executable/profile digests, raw timestamp values, per-step output bytes, partial observations on execution errors, and a replay command.

This is trusted-fixture process containment, **not a filesystem or network security sandbox**. Do not run unreviewed Makefiles. Observations are taken at invocation boundaries; writes that restore both bytes and mtime are not visible. Directory times, atime, ctime, process-event traces and timestamps outside the controlled roots remain outside this suite's coverage.


The archive-name-encoding scenario adds twelve invocations across GNU/BSD name
encodings, duplicate members, query, touch, replacement and restoration. Its
verification recipes compare the complete archive bytes against independently
authored anchors, including NUL padding, metadata fields and payloads.


Three symlink-time scenarios add 31 invocations with exact link/hop/referent/output
nanoseconds, dangling and restored chains, mode changes, deletion/rebuild, and
archive-member second boundaries with complete-byte header expectations.

`filename-quoted-rebuilds` and `filename-glob-transitions` add twenty invocations
with space-containing target/input names, order-only changes, missing/restored
inputs, fresh wildcard membership, query/dry/parallel/forced builds, exact bytes
and fixed nanosecond output timestamps.

`tilde-home-transitions` adds twelve invocations changing HOME through an include,
with space-containing home directories, fresh glob membership, order-only changes
and exact output bytes/timestamps through query, dry, parallel and forced builds.
