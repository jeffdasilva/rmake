# Initial pinned real-project compatibility profiles

The separate integration gate builds [xxHash v0.8.3](https://github.com/Cyan4973/xxHash/releases/tag/v0.8.3)
at its full commit identity and [zlib 1.3.2](https://zlib.net/). The
[manifest](../tests/integration/projects.json) pins the archive URL, SHA256,
top-level path, license digest, build target, header dependency, watched outputs
and independently expected executable output. Only verified source archives are
cached; extraction and all builds start from fresh disposable inputs.

GNU 4.4.1 executes twice before each selected candidate. Full streams, status,
process bounds and filesystem bytes/types/modes/link topology must match at
each build, no-op, header-triggered rebuild and executable smoke. Independent
checks require real ELF/archive outputs, unchanged no-op output timestamps,
strictly newer rebuilt output timestamps, xxHash's known empty-input XXH32
value and a zlib compress/decompress round trip. The common zlib configure
invocation is retained as frozen input, including its log; it is not compared
after stripping date or PID fields. The C locale and explicit compiler/archive/
linker flags make the selected inputs reproducible without masking make output.

`make bootstrap` includes source verification/download. `make test-integration`
executes both projects; `INTEGRATION_CASE` selects an exact manifest ID.
`check`, `ci`, optimized acceptance and a selected rmake-driven self-host flow
include the new gate. Existing GitHub development/optimized jobs execute it and
retain its complete artifacts. No additional workflow with weaker criteria is
introduced.

The immutable [history](../tests/integration/history/index.json) retains the
original pre-execution plan, pinned-source discovery, prototype, development
and optimized observations and harness sources. Lossless byte deduplication
reduces storage; every original JSON digest is verified after reconstruction.
Comparisons use complete observations before storage. A failed archive-metadata
decoder experiment and its correction remain recorded as tooling research and
earn no compatibility credit. Later qualification supplements preserve earlier
records.

This is an initial serial Linux/GCC profile for two upstream build graphs,
counted separately from ordinary curated Makefiles. It establishes neither
arbitrary-project compatibility nor general performance parity. Recursive,
parallel, broader toolchain and project profiles remain open, alongside the
other M3/M6 exit criteria. Persistent caching and its safety proof remain M8
work; the test harness source-archive cache is not an rmake dependency-graph
cache.
