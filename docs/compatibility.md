# GNU Make 4.4.1 compatibility

rmake targets GNU Make 4.4.1 on Linux. Use the
[GNU Make manual](https://www.gnu.org/software/make/manual/html_node/index.html)
for Makefile syntax and command-line options.

## Compare rmake on your project

1. Prepare two clean copies of the project with the same source and build
   settings. In the first copy, run `make --version` and confirm that it reports
   GNU Make 4.4.1. The rmake checkout can build a pinned executable with
   `make bootstrap-oracle` if you need one.
2. From each copy, build the default goal with the same job count.

   ```sh
   # First copy
   make -j1

   # Second copy
   rmake -j1
   ```

3. Run each build again. Check which recipes run and that both builds produce
   the expected files. Then make the same small source change in both copies
   and check that the same targets rebuild.
4. If your project uses parallel builds, repeat the comparison with your usual
   job count. For group-by-target logs, add `-Otarget` to both commands.
   See the [parallel output guide](output-sync.md).

If you find a difference, keep the commands, Makefile, executable versions,
errors, exit status, and changed files. A small working example makes it
easier to add a regression test. See the [testing guide](testing.md).

## What the project tests today

The main reference environment is x86-64 Linux with glibc. Tests cover Makefile
reading and expansion, rule selection, serial and parallel recipes, recursive
builds, native modules, and diagnostics. The
[real-project tests](project-integration.md) check clean and incremental builds
of pinned projects, including zlib and GNU Make itself.

The default build omits Guile. The `guile` Cargo feature enables the Scheme
interface and has a separate set of comparison tests. Work continues on broader
Linux, native-module, Guile, locale, and resource-limit coverage. See the
[Guile frontend guide](guile-frontend.md) and the
[compatibility work that remains](m3-m6-audit.md).

Benchmark results depend on the workload. The
[benchmark reports](benchmarks/README.md) describe the inputs, environment,
and measured times for each comparison.

## Compatibility target

The target stays pinned to GNU Make 4.4.1. The
[GNU release archive](https://ftp.gnu.org/gnu/make/) provides the reference
source. The executable determines the expected behavior when the manual leaves
a detail open.

| Area | What needs to match |
| --- | --- |
| Makefile language | Syntax, variable and function expansion, rules, special targets, and reproducible 4.4.1 behavior. |
| Execution | Recipe and shell behavior, serial and parallel builds, recursion, signals, file descriptors, output synchronization, and timestamps. |
| Command line | Options, environment handling, exit status, diagnostics, trace, debug, and database output. |
| GNU extensions | Dynamic loading and the Guile-disabled and Guile-enabled builds. |
| Platform details | Linux behavior for shells, locale, byte paths, nanosecond timestamps, archives, and symlinks. Qualify aarch64 and musl with separate profiles. |

The jobserver lets recipes and cooperating tools share a pool of job slots on
one machine. rmake targets both Linux FIFO and pipe protocols in recursive
builds with either engine as parent or child. See GNU's
[job slots](https://www.gnu.org/software/make/manual/html_node/Job-Slots.html)
and [POSIX jobserver protocol](https://www.gnu.org/software/make/manual/html_node/POSIX-Jobserver.html),
and rmake's [jobserver tests](jobserver.md).

`rmake --help` and `rmake --version` identify rmake. `MAKE_VERSION` in a
Makefile reports `4.4.1` for compatibility. `$(MAKE)` follows the command you
invoked so a recursive recipe runs the intended engine. See the GNU manual's
[MAKE variable](https://www.gnu.org/software/make/manual/html_node/MAKE-Variable.html).

## Reference builds

Tests use a verified GNU Make executable, called the *oracle*. Build options,
linked libraries, include directories, locale data, and distro patches can all
change what a Makefile observes. Record these details along with the source
version, executable digest, architecture, and runtime environment.

The [oracle manifest](../oracle/manifest.json) pins the 4.4.1 source archive,
checksum, and default build configuration. The
[bootstrap and verifier](../tools/oracle.py) check the executable and runtime
identities. The baseline enables native language support and uses `LC_ALL=C`
for the ordinary comparisons. Guile and locale tests have separate profiles.
Any distro-patched executable needs its own profile. See the
[pinned Guile toolchain](../oracle/guile/toolchain.json) and GNU's
[Guile integration documentation](https://www.gnu.org/software/make/manual/html_node/Guile-Integration.html).

A complete profile would also pin the runtime image, toolchain, shell, and
filesystem and record their observed properties. Immutable runtime images and
complete filesystem and network isolation are
[open test infrastructure work](progress.md#open-foundation-gates).

### Build on x86-64 musl

Use Rust 1.90.0, a musl C compiler, and the development libraries for
libucontext and its POSIX wrapper. Build with dynamic musl when using native
Make modules.

```sh
RUSTFLAGS="-C target-feature=-crt-static" \
  cargo build --locked --release --target x86_64-unknown-linux-musl
```

Set `RMAKE_UCONTEXT_LIBDIR` if the two libucontext libraries live outside the
linker's library paths. Use the linker and library paths for the same musl
profile. The profile experiments use [libucontext 1.5.2](https://github.com/kaniini/libucontext/releases/tag/libucontext-1.5.2).

The x86-64 musl verification covers selected wildcard, file ownership, remake,
and native setup behavior. Track broader lifecycle and locale coverage in the
[M3/M6 audit](m3-m6-audit.md).

## How rmake checks a difference

For exact comparisons, the runner resets the same temporary working directory
and executable alias for each engine. It compares stdout and stderr byte for
byte, exit status, and filesystem effects. Dedicated suites check execution
order, timestamps, descriptor behavior, native callbacks, and the real
invocation names and paths used in recursive builds. Debug and database output
must match where the inputs make them deterministic, including ordering and
statistics.

Control variable inputs such as timestamps or parallel worker release whenever
possible. When a PID or jobserver path has to vary, check its specific
relationship to the process or descriptor and keep the raw output. Keep known
differences visible in reports. A pending fixture tracks an open requirement,
and a new match requires review before the fixture becomes required.

For commands and fixture formats, see the [testing guide](testing.md) and the
[test corpus guide](../tests/README.md). The
[requirements ledger](../tests/requirements.toml) maps cases to behaviors, and
[the implementation plan](implementation-plan.md) defines the completion gates.

## Source reference and provenance

rmake is licensed under `GPL-3.0-or-later`. Contributors can read the GNU Make
4.4.1 source to understand behavior. Write original Rust and fixtures, and
confirm the result against the pinned executable. Keep GNU Make as a separate
test executable. rmake executes builds through its own engine.

When source inspection informs a change, record the GNU version, file,
function, and behavior. Identify the license of any upstream-derived material,
preserve the notices, and document the changes and dates. Test expectations
come from the specification and independent observations. For full attribution
requirements, see [licensing and provenance](licensing.md).

## Safety and performance boundary

Makefiles execute recipes with the caller's permissions. Use Makefiles and
helpers you trust. The test runners use disposable workspaces and bound
processes and output. Hostile inputs require a security sandbox.

The [documented safety exceptions](safety-exceptions.md) cover inputs that make
GNU crash or hang and an approved set of unstable nested options. rmake returns
deterministic errors for these cases. They have their own safety tests and earn
zero exact-conformance credit. Record new hazards and resolve their behavior
explicitly.

An optimization must preserve GNU's decisions and the order of observable
work, including shell expansions, warnings, writes, Makefile remakes, and
extension calls. GNU timestamp rules determine when to rebuild targets.

The planned persistent cache would reuse validated interpreter data, such as
parsed source or eligible dependency-graph fragments. It would require an
explicit opt-in and a private directory outside the build tree. If an entry is
missing, stale, or unusable, execute the ordinary interpreter path. Qualify
cold, warm, invalidated, corrupt, concurrent, and disabled cache states and
measure validation costs. See the
[caching design](architecture.md#safe-caching-progression).

Future rmake-specific plugins would use explicit opt-in and versioned
interfaces. GNU's native-module and Guile interfaces are part of compatibility
work today. See the
[extension design](architecture.md#extensibility-without-changing-defaults).
