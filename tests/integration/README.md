# Real-project compatibility fixtures

The [integration guide](../../docs/project-integration.md) lists the current
profile IDs, expected behavior, and commands. The [manifest](projects.json)
pins release sources, their licenses, compiler arguments, watched build
outputs, and expected executable results. It includes GNU Make itself as one
external build input.

The runner uses the pinned GNU Make reference twice and then rmake at the same
reset path. Build profiles compare exact streams, status, process limits, and
filesystem contents. They also check that watched files remain current in a
no-op build and become newer after an input edit. Configuration-only profiles
check the generated files and independent digests. Runs use reviewed source
in disposable workspaces and keep installs inside those workspaces.

## Retained evidence

`history/index.json` retains the original plan, source provenance, first
prototype, both development and optimized production-harness campaigns, their
full raw observations, source and logs. Filesystem content uses a lossless
content-addressed representation: `tools/integration_records.py` restores the
original complete JSON, checks each byte blob and the whole original record
digest. Deduplication changes storage only; comparisons always use complete
in-memory observations. Failed archive-encoding research and its correction are
recorded without changing previous engine observations. Earlier raw files are
also left in the local research directory. Repeated campaigns do not multiply
the number of qualified project profiles.

`qualification/index.json` retains the final Makefile-driven development and
optimized campaigns, including the exact source/observer/record-code closure,
compiler/binutils/shell identities and both GNU runs before each candidate.
Its raw observations supplement the initial history without replacing it.

[`lua-qualification.json`](lua-qualification.json) describes the lossless Lua build and self-host evidence archive. File payloads are deduplicated by digest, and every reconstructed observation was checked against its original hash before packaging. Earlier xxHash and zlib history is unchanged.

The [semantic rebuild archive](lua-semantic-rebuild.json) retains complete
GNU2 and both candidate runs, including separate fresh and rebuilt smoke
results. The original comment-only qualification stays unchanged.
