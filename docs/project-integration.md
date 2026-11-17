# Test real-project builds

The integration suite compares GNU Make 4.4.1 and rmake on **16 pinned profiles
across nine projects**. Each profile uses the project's own Makefiles and
checks a specific build configuration. The
[manifest](../tests/integration/projects.json) is the authority for source
identities, arguments, watched files and expected executable output.

## Run a profile

From the repository root, prepare the sources and select an exact profile ID.

```sh
make bootstrap
make test-integration INTEGRATION_CASE=xxhash-0.8.3
```

To refresh the pinned archives after setup, run `make bootstrap-integration`.
The test target builds the development
candidate and verifies the GNU reference before running the selected profile.

Run `make test-integration` without a selector for all profiles. Use `CANDIDATE`
and `ORACLE` to select other executable paths. The full optimized gate runs
through `make test-m4-release`, which is also included by `test-m5-release`.
The normal `test`, `check` and `ci` flows include integration checks. Selected
self-host flows run the xxHash and Lua cases through rmake's top-level Makefile.

For a focused reference-only check, use the runner without `--candidate`.

```sh
python3 tools/integration.py --case lua-5.4.9 \
  --report artifacts/integration/lua-oracle.json
```

This runs the reference twice and checks the independent expectations. To compare
an existing candidate, add `--candidate PATH`. The runner accepts repeated
`--case` options and rejects unknown IDs. A reference-only pass validates the
selected observations without qualifying a candidate.

## Choose a profile

Parallel rows specify the jobserver transport and output synchronization mode.
All profiles use the C locale and the manifest's compiler and environment inputs.

| Exact profile ID | Target and execution | Main behavior |
| --- | --- | --- |
| `xxhash-0.8.3` | `xxhsum`, serial | Object/executable rebuild and known empty-input hash. |
| `zlib-1.3.2` | `static`, serial | Static library and executable rebuild with compression round trip. |
| `zlib-1.3.2-staged-shared` | `all`, two pipe slots, target synchronization | Static/PIC builds and two staged installs with shared-library checks. |
| `lua-5.4.9` | `linux`, two pipe slots, recursive synchronization | Recursive build, version-header rebuild and executable results. |
| `lua-5.4.9-parallel-fifo` | `linux`, four FIFO slots, recursive synchronization | The Lua build and rebuild through a shared FIFO jobserver. |
| `gzip-1.14` | `all`, serial | Recursive build and a header change observable in compressed data. |
| `gzip-1.14-parallel-fifo` | `all`, two FIFO slots, recursive synchronization | Parallel gzip build with exact recursive announcements. |
| `patch-2.8` | `all`, serial | Recursive build, version change and unified-diff application. |
| `patch-2.8-parallel-fifo` | `all`, four FIFO slots, recursive synchronization | Parallel Patch build and the same executable checks. |
| `gnu-make-4.4.1` | `all`, serial | Build GNU Make, change its version header and execute original stdin rules. |
| `gnu-make-4.4.1-parallel-fifo` | `all`, two FIFO slots, recursive synchronization | Parallel GNU Make build with exact recursive announcements. |
| `zstd-1.5.7` | `zstd-release`, serial | Recursive CLI build, no-op copy behavior and version-header rebuild. |
| `busybox-1.37.0` | `defconfig` | Configuration-only comparison with independent output digests. |
| `busybox-1.37.0-executable` | `allnoconfig`, then `busybox`, serial | Minimal executable configuration, build, no-op, rebuild and printf result. |
| `bzip2-1.0.8` | `bzip2`, serial | Static library and executable rebuild after editing a C source, then a compression round trip. |
| `bzip2-1.0.8-parallel-fifo` | `bzip2`, four FIFO slots, target synchronization | The same build and file checks with parallel recipes. |

### xxHash and zlib

[xxHash v0.8.3](https://github.com/Cyan4973/xxHash/releases/tag/v0.8.3) is pinned
to commit `e626a72bc2321cd320e953a0ccf1584cad60f363`. Its profile watches
`xxhash.o` and `xxhsum` and checks the known empty-input XXH32 value.

The serial [zlib 1.3.2](https://zlib.net/) profile watches `adler32.o`, `libz.a`
and `minigzip`, then checks a compression/decompression round trip. Both serial
profiles append an original comment to their watched header to trigger a rebuild.

The shared zlib profile prepares a common `--shared --prefix=/usr` configuration.
It watches `adler32.lo`, `libz.a`, `libz.so.1.3.2` and `minigzipsh`. Both installs
use the upstream target with `DESTDIR=stage LDCONFIG=:` inside the disposable
workspace. The [original witness](../tests/integration/zlib-staged-shared.py)
checks staged files, modes, relative symlinks and pkg-config metadata, then loads
the staged shared object for a binary compression round trip. Changing the
version header must rebuild the outputs and change the staged library's reported
version.

### bzip2

The [bzip2 1.0.8 release](https://www.sourceware.org/bzip2/downloads.html)
builds with its original Makefile. Both profiles watch `bzlib.o`, `libbz2.a`,
and `bzip2`. An edit to `bzlib.c` must rebuild all three, and a separate
smoke command compresses and decompresses known text. The source archive was
verified against the publisher key `EC3CFE88F6CA0788774F5C1D1AA44BE649DE760A`.

### Lua

Both [Lua 5.4.9](https://www.lua.org/ftp/) profiles build through the original
recursive Linux target. They watch `src/lapi.o`, the deterministic static library,
and the `lua` and `luac` executables. A single version fragment in `src/lua.h`
changes before the rebuild. Fresh and rebuilt Lua executables must sort an
independent input and print the expected result with their respective versions.
Silent mode provides stable output while preserving every observed byte.

### GNU gzip, Patch and Make

[GNU gzip 1.14](https://lists.gnu.org/archive/html/info-gnu/2025-04/msg00007.html)
uses a common configure preparation with dependency tracking. The profiles build
through `lib` and the top-level executable targets and watch `zip.o` and `gzip`.
The `tailor.h` change replaces the compressed frame's Linux OS byte `3` with
`255`. Executable checks verify both values and a nonempty round trip. The
parallel profile also requires the exact upstream recursive announcements.

[GNU Patch 2.8](https://lists.gnu.org/archive/html/info-gnu/2025-03/msg00014.html)
configures once per profile and builds through `lib`, `src` and `tests`. It watches
`src/version.o` and `src/patch`. A version definition in `config.h` changes before
the rebuild. Both resulting executables must report the expected version and
apply a unified diff to produce the expected file. The parallel profile retains
exact directory announcements.

The GNU Make profiles use the same release archive as the pinned reference,
with their own dependency-tracking configuration and Guile/NLS disabled.
They watch `src/version.o` and `make`. Changing `PACKAGE_VERSION` in `src/config.h`
must rebuild both outputs. The fresh and rebuilt executables report their
respective versions and execute a small original Makefile from stdin. Here the
built GNU executable is an integration output checked by the suite. The parallel
profile also compares the `lib`, `po` and `doc` announcements.

### Zstandard and BusyBox

[Zstandard 1.5.7](https://github.com/facebook/zstd/releases/tag/v1.5.7) builds its
CLI through the recursive `zstd-release` target. Its hash-based copy check must
preserve the published binary's timestamp on a no-op. Changing the version in
`lib/zstd.h` must rebuild `programs/zstd` and change its reported version. Both
versions compress and decompress the same independent input.

The BusyBox configuration profile compares the original `defconfig` target.
Independent digests cover the anchored configuration stream, `.config` and
`.kernelrelease`. The executable profile instead runs `allnoconfig` under each
engine, verifies the configuration and enables only `BUSYBOX`, `PRINTF` and
`SHOW_USAGE`. It checks the complete no-op snapshot, a rebuild after changing
`include/platform.h`, newer `busybox` and `coreutils/printf.o` timestamps, and
exact output from `./busybox printf`.

## Understand the comparisons

Each selected profile runs GNU Make twice, then the candidate, at the same reset
path. Source extraction is fresh for each profile. A configure step, when needed,
runs once and becomes common frozen input, including its complete log.

Executable profiles compare a fresh build, a no-op, a header-triggered rebuild
and an executable check. Profiles with a semantic header change also check the
fresh executable. The staged zlib profile adds an install after each build.
BusyBox `defconfig` compares configuration only, retaining the common input and
all filesystem changes.

Each phase compares raw stdout/stderr, status, process limits, cleanup and
filesystem bytes, types, modes and link relationships. Executable profiles also
require real ELF/archive outputs and exact executable results. Every invocation
must succeed within its bounds and leave no background processes. A matching failure
or incomplete sequence cannot pass.

Watched output timestamps must remain unchanged on the no-op and become strictly
newer after the header change. These are relationships within each run. Absolute
wall-clock timestamps are not compared across engines. Date, PID, path, compiler,
recipe and filesystem output remain intact. The manifest selects explicit
compiler, archive and linker inputs for reproducibility.

The runner executes reviewed upstream code with the user's permissions. Process
and output limits do not create a filesystem or network security sandbox.
Installs are staged inside the workspace. The BusyBox executable profile permits
8,192 snapshot entries and 48 MiB of total file data. Shared per-file, source and
subprocess limits continue to apply.

## Read reports and retained evidence

`make test-integration` writes its summary to
`artifacts/integration/development.json`. The optimized acceptance gate writes
`artifacts/integration/optimized.json`. Direct invocations can select another
path with `--report`. Raw observations are stored beside the report in a unique
`*-raw-*` directory.

Check the overall `passed` field, each project's `passed`, per-run `errors` and
paired `differences`. A report also records the manifest, executable identities,
compiler/binutils/shell identities, host profile, runtime path and harness hashes.
The report and raw parts retain the common input, preparation logs, each phase's
streams and filesystem observations, and timestamp witnesses. Preserve the report and
its associated raw directory together when reporting a difference.

[Record storage](../tools/integration_records.py) deduplicates full file bytes by
digest and reconstructs each original JSON record with integrity checks.
Comparisons use complete observations before storage. Repeated campaigns do not
increase the number of qualified profiles.

The immutable [history](../tests/integration/history/index.json) retains the
original plan, source discovery, prototype, development and optimized runs, and
harness sources. The [qualification index](../tests/integration/qualification/index.json)
adds the final original Makefile-driven campaigns. The
[Lua qualification](../tests/integration/lua-qualification.json) and
[semantic rebuild records](../tests/integration/lua-semantic-rebuild.json) retain
that follow-up's full observations. Earlier records remain unchanged, including
the failed archive-decoder experiment and its correction. Failed research earns
no compatibility credit.

## Source provenance and scope

The manifest pins archive URLs, SHA-256 values, extracted license digests and
source identities. Cached archives are verified before reuse. Acquisition retries
transient DNS, connection and selected HTTP failures up to three times per URL.
A configured fallback can handle a permanent HTTP failure. Size or integrity
failures stop without publishing an archive. Warm source verification preserves
the existing cache.

The BusyBox archive uses the [publisher's release](https://busybox.net/downloads/busybox-1.37.0.tar.bz2)
and [SHA-256](https://busybox.net/downloads/busybox-1.37.0.tar.bz2.sha256).
GNU Patch's source verification recorded archive SHA-256
`308a4983ff324521b9b21310bfc2398ca861798f02307c79eb99bb0e0d2bf980`
and detached-signature verification against signer
`259B3792B3D6D319212CC4DCD5BF9FEB0313653A`. The
[release announcement](https://lists.gnu.org/archive/html/info-gnu/2025-03/msg00014.html),
[archive and signature](https://ftp.gnu.org/gnu/patch/) and
[GNU release keyring](https://ftp.gnu.org/gnu/gnu-keyring.gpg) retain the source
references. The normal runner verifies the pinned archive and license digests.

Upstream projects and retained source observations keep their own licenses and
notices under the [provenance policy](licensing.md). Integration profiles are
counted separately from ordinary curated Makefiles. Broader compiler, project,
jobserver and output combinations remain part of
[M6 acceptance](m3-m6-audit.md#integration-and-performance). These functional
checks provide compatibility evidence for their selected Linux/GCC profiles.
Performance requires separate measurements. The harness caches verified source
archives only. Persistent engine caching and its correctness criteria remain
[M8 work](implementation-plan.md#milestones-and-exit-criteria).
