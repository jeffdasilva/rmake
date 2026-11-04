# Pinned real-project integration

This separate required profile builds reviewed upstream projects using their
own Makefiles. It does not count source files as conformance models or establish
complete M3/M6 compatibility. No GNU implementation or test sources are used.

`projects.json` pins xxHash v0.8.3 to commit
`e626a72bc2321cd320e953a0ccf1584cad60f363` and zlib 1.3.2 to its official
release archive. Archive and license digests are checked before every fresh
extraction. Only verified compressed source archives are cached. Builds execute
reviewed trusted source; the harness is not a security sandbox. Nothing installs
into the host. The source projects retain their own licenses.

Each project runs GNU 4.4.1 twice and then the selected candidate at the same
reset path. Zlib configuration runs once and becomes common frozen input,
including its complete log. No date, PID, path, compiler, recipe, or filesystem
output is stripped. The four observations are a fresh build, a no-op, a rebuild
after an independently authored header mutation, and an executable smoke.
Every observation includes complete stdout/stderr, status, bounded-process
outcome, and the full file/type/mode/link/content snapshot. Watched output
timestamps must stay identical for the no-op and become strictly newer after
the header change; absolute wall-clock timestamps are not compared between
engines. Expected archive/ELF magic and exact smoke bytes provide independent
controls beyond GNU/candidate agreement.

These initial profiles use `-j1`, GCC, the C locale, deterministic static
archives and disabled linker build IDs. They cover xxHash's CLI and zlib's
static-library/example build. Other projects, recursive and parallel build
profiles, installation and broader compiler/toolchain environments remain
additional work. These functional checks provide no performance evidence.

The top-level `bootstrap-integration` verifies or downloads the pinned sources;
`test-integration` runs both projects. `INTEGRATION_CASE=xxhash-0.8.3` selects one
exact ID. Reports and lossless raw records go to `artifacts/integration/`.
Developer and optimized compatibility gates require both projects; a selected
top-level self-host flow also executes the integration harness.

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
