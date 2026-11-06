# Pinned real-project compatibility profiles

The separate integration gate builds [xxHash v0.8.3](https://github.com/Cyan4973/xxHash/releases/tag/v0.8.3)
at its full commit identity, [zlib 1.3.2](https://zlib.net/),
[Lua 5.4.9](https://www.lua.org/ftp/),
[GNU gzip 1.14](https://lists.gnu.org/archive/html/info-gnu/2025-04/msg00007.html) and
[GNU Patch 2.8](https://lists.gnu.org/archive/html/info-gnu/2025-03/msg00014.html). The
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
executes all ten profiles across seven projects. `INTEGRATION_CASE` selects an
exact manifest ID.
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

The separate `zlib-1.3.2-staged-shared` profile reuses the pinned zlib archive
with a common shared-library configure preparation. It builds static and PIC
outputs with two pipe-jobserver slots and target output synchronization. Both
installs run the upstream target with `DESTDIR=stage LDCONFIG=:`. An original
witness checks staged files, modes, relative library symlinks, and pkg-config
metadata, then loads the exact staged shared object and checks a binary
compression round trip. A version-header change must rebuild the watched
outputs and change the staged library's reported version.

The Lua profile runs the original top-level `linux` target with two job slots,
pipe jobserver and recursive output synchronization. It builds through three
make processes and checks one object, the deterministic static library and
both executables. The dependency witness replaces one exact version fragment
in `src/lua.h`. The fresh and rebuilt Lua executables must sort and print
independently expected results containing their different version strings. Silent mode avoids presenting
parallel recipe completion order as a deterministic stream, and all observed
stream bytes remain required. The upstream source and its MIT notice are
pinned by digest. No patched source or system installation is required.

The GNU gzip profile freezes a common preparation from the release's
`configure`, using its generated Automake files. The serial `all` build
recurses through the original library and executable subtargets with
`SUBDIRS=lib .`. It watches `zip.o` and `gzip` through the no-op and rebuild.
The scratch header mutation changes the gzip frame's OS byte from the Linux
value `3` to `255`. Both independently expected byte values are checked along
with a nonempty compression and decompression round trip. The upstream archive
and GPLv3 license file are pinned by digest.

The same pinned gzip archive also runs as `gzip-1.14-parallel-fifo`. This
profile uses two jobs, the FIFO jobserver and recursive output synchronization
through the original `all` target. `-s V=1` leaves the two upstream recursive
directory announcements as the exact fresh, no-op and rebuild output. The
profile checks both executable smoke results and full filesystem snapshots.
It reuses the verified serial-profile source archive and has its own common
configure preparation.

The GNU Patch profile freezes one common `configure` preparation and runs the
original serial `all` target through `lib`, `src`, and `tests`. It watches
`src/version.o` and `src/patch`. The scratch `config.h` change must replace one
version definition, and the fresh and rebuilt binaries must report their
respective versions and apply a unified diff with the expected result.

The official archive SHA-256 is
`308a4983ff324521b9b21310bfc2398ca861798f02307c79eb99bb0e0d2bf980`.
The detached signature was verified against release signer fingerprint
`259B3792B3D6D319212CC4DCD5BF9FEB0313653A` using the GNU release keyring. The
manifest pins the archive and its `COPYING` file. [Release announcement and signer](https://lists.gnu.org/archive/html/info-gnu/2025-03/msg00014.html)
· [Official archive and signature](https://ftp.gnu.org/gnu/patch/) · [GNU release keyring](https://ftp.gnu.org/gnu/gnu-keyring.gpg)

The GNU Make source profile uses the same pinned archive as the GNU 4.4.1
oracle, with its own serial build and dependency-tracking configuration. The
original recursive `all` target builds `src/version.o` and `make`. The no-op
preserves both timestamps. Changing one `PACKAGE_VERSION` definition in the
disposable `src/config.h` rebuilds both outputs. The fresh and rebuilt
executables report their expected versions and execute a small original
Makefile read from standard input. The built executable is a smoke witness.

The same GNU Make source runs with a two-slot FIFO jobserver and recursive
output synchronization. The original directory announcements for `lib`, `po`,
and `doc` are the exact build, no-op and rebuild output. The serial and
parallel profiles each check the same version change and stdin rule execution.

The Zstandard profile builds the official 1.5.7 CLI through the original
recursive `zstd-release` target. Its hash-based copy check keeps the published
binary's timestamp on a no-op. Changing the version release definition in the
scratch `lib/zstd.h` rebuilds the executable and changes its version output.
Both versions compress and decompress the same independent input.
[Official Zstandard release](https://github.com/facebook/zstd/releases/tag/v1.5.7)

These ten Linux/GCC profiles are counted separately from ordinary curated
Makefiles. Broader toolchain, project, jobserver, and output profiles remain
part of M6. General performance parity remains unproven. Persistent caching
and its correctness proof remain M8 work; the test harness caches only its
verified source archives.

Pinned source acquisition retries transient DNS, connection, and selected HTTP
failures up to three attempts per configured URL. Permanent HTTP errors may
move to an already configured source. Existing size, digest, and transport
checks remain in force; integrity failures do not retry or publish the archive.
The verified cache stays the same on a warm run.
