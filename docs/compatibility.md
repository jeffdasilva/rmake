# GNU Make 4.4.1 compatibility contract

Compatibility target established 2026-10-18. Implementation is partial; the
[progress record](progress.md) identifies verified slices and open requirements.

rmake will reimplement GNU Make 4.4.1 behavior in Rust on Linux. Compatibility
comes before performance or new features. The reference is the pinned executable's
observable behavior, informed by the public manual, rather than a simplified
interpretation of what a build tool ought to do.

The [official release directory](https://ftp.gnu.org/gnu/make/) lists 4.4.1 as the
latest stable release at the time of this plan. The
[manual](https://www.gnu.org/software/make/manual/) identifies its 4.4.1 edition.
The compatibility target will stay pinned even if GNU publishes a later version.

## Supported scope

| Area | Contract |
| --- | --- |
| Platform | Linux; initial reference environment is x86-64 with glibc. Add aarch64 and musl validation before claiming those environments. No Windows, macOS, DOS, or VMS implementation. |
| Language | All syntax, expansion phases, functions, variables, rules, special targets, and documented Linux behavior in 4.4.1, plus reproducible binary-observed quirks. |
| Execution | Serial and parallel execution, recursive make, shell behavior, signals, file descriptors, output synchronization, errors, and timestamps. |
| Command line | GNU options and option interactions, environment handling, diagnostics, exit codes, trace, debug, and database output. |
| Branding | `--help` and `--version` can identify rmake. Other deterministic diagnostics remain compatibility work, not a general cosmetic exemption. |
| Extensions already in GNU | Dynamic object loading and Guile-enabled/disabled reference profiles. These are compatibility features, separate from future rmake plugins. |
| New rmake extensions | Explicitly enabled only; never silently modify default make semantics. |
| Distributed execution | No remote worker transport, cluster scheduler, or distributed cache. |

GNU's jobserver coordinates local recursive builds and cooperating tools. Keep
Linux FIFO and pipe interoperability, including mixed GNU/rmake process trees.
Omitting it would break ordinary local parallel builds. See
[job slots](https://www.gnu.org/software/make/manual/html_node/Job-Slots.html) and
[the POSIX protocol](https://www.gnu.org/software/make/manual/html_node/POSIX-Jobserver.html).

Linux-only does not mean UTF-8-only paths, a mandatory Bash shell, second-resolution
timestamps, or ignoring archives and symlinks. Locale, shell, filesystem, and
toolchain conditions are part of the test environment.

## Pin a reference profile, not just a version string

Milestone 0 records an immutable oracle manifest with:

- Executable digest, trusted acquisition/build provenance, upstream version,
  package patches if any, architecture, libc, and linked runtime identities.
- Feature configuration, `.FEATURES`, built-in variables/rules, default include
  directories, shell behavior, locale catalogs, and load/Guile availability.
- Container/root filesystem digest, toolchain and shell versions, locale,
  timezone, umask, environment, filesystem properties, and fixture runner version.

Prefer an unpatched upstream-compatible binary. Treat distro patches as a
different profile rather than silently changing the target. Maintain both a
Guile-disabled baseline and a Guile-enabled profile; full-profile compatibility
requires the enabled behavior too. Guile is a build option, as documented in
[Guile integration](https://www.gnu.org/software/make/manual/html_node/Guile-Integration.html).

Initial fast tests run in `LC_ALL=C`; non-C locales and supported profile variants
have separate gates. Translation catalogs and compile-time path differences must
be inventoried, not silently excused. The current development host's GNU Make 4.3
is suitable for bootstrapping workflows but is not the conformance oracle.

## Invocation identity and observable output

`MAKE_VERSION` exposes `4.4.1` in compatibility mode. `rmake --version` separately
reports the rmake release and compatibility target. Do not add default-visible
variables, feature words, environment variables, or startup messages that change
existing Makefile conditionals or `.VARIABLES`.

GNU defines `MAKE` using the invoked program name. Therefore `$(MAKE)` cannot be
literally identical when one command is named `make` and the other `rmake`: it must
reinvoke the correct implementation. This is input-dependent behavior, not a
general output waiver. See
[the MAKE variable](https://www.gnu.org/software/make/manual/html_node/MAKE-Variable.html).

Run strict comparisons with both implementations installed at the same absolute
path and invoked with the same name in separate sandboxes. Also run direct-name
tests for `make`, `gmake`, `rmake`, absolute paths, symlinks, and mixed recursive
trees. Do not replace every occurrence of a program name in captured output.

Compare deterministic stdout and stderr bytes, source locations, exit status,
recipe effects, environment inheritance, and filesystem results. Preserve GNU
ordering where specified or reproducibly observable. Debug/database rendering is
a dedicated compatibility surface, including deterministic ordering/statistics;
it must not simply dump rmake's internal Rust structures.

Time, PIDs, file descriptors, and unconstrained parallel completion can differ
between two executions of GNU itself. Fixtures must control such inputs or use
narrow, named validators tied to their meaning. Unknown discrepancies fail.
Do not globally sort output, strip whitespace, or discard diagnostics to obtain a
pass. Finite testing provides strong regression evidence, not a proof for every
possible effectful Makefile.

## Source reference and provenance

The project uses `GPL-3.0-or-later`, matching GNU Make's stated license. GNU Make
4.4.1 source, headers and tests may be inspected to understand behavior. This is
a source-informed Rust reimplementation, not a clean-room project. This policy
supersedes the earlier source-inspection prohibition. See
[licensing and provenance](licensing.md).

1. Derive requirements from documentation, release announcements, version-pinned
   source references, and independently designed black-box experiments. Confirm
   behavioral conclusions against the actual reference executable.
2. Keep the Rust architecture appropriate to rmake; source access does not require
   a mechanical port. Record the upstream version, file/function and relevant
   behavior when source inspection informs a change.
3. If code, declarations, tables or tests are copied, translated or adapted, record
   their origin, retain applicable copyright/license notices, identify changes
   and dates, and satisfy the actual file's license. Source access does not make
   all files in an upstream archive uniformly licensed or erase attribution.
4. Keep oracle execution reproducible and separate from the product. A verified
   binary or a reproducible source build is acceptable; record its provenance.
   GNU Make must never be an implementation fallback at rmake runtime.
5. Maintain an independently authored differential corpus. Source-informed cases
   and any explicitly tracked upstream-derived supplemental cases must still
   compare executable behavior; do not derive expected answers from rmake code.
6. Record each requirement's references, oracle profile, observed result, fixture
   provenance and regression test. Review license compatibility for dependencies
   and adapters, and retain the relevant notices in release artifacts.

The compatibility ledger will track `specified`, `fixture-ready`, `implemented`,
`passing`, and `blocked` independently. A known failure is never turned into a
passing test by broad normalization or silently removing it from the suite.

## Safety and performance boundary

GNU timestamp semantics remain authoritative. Hashes used to validate interpreter
caches must not turn make into a content-based build system. Do not skip a recipe,
shell expansion, file write, diagnostic, include-remake pass, or plugin call that
the reference performs. Do not run Makefiles in a production sandbox by default:
that would change their authorized shell behavior. The test harness is isolated.

Caching introduces private implementation files, which GNU does not create. During
rollout, strict mode leaves persistent caching disabled. Cache-enabled testing
uses an explicitly designated private namespace outside the build tree; cache
effects are not claimed to be invisible to arbitrary shell commands that inspect
the whole machine. No automatic enabling, shared cache, or daemon before the
namespace and invalidation contract have been validated and accepted. Default
execution must still function with the cache absent, corrupt, or inaccessible.

No unapproved compatibility exclusions are hidden in performance work. If an
observed GNU defect conflicts with memory/process/cache safety, retain a failing
compatibility entry and resolve the conflict explicitly rather than claiming
equivalence or deliberately reproducing memory corruption.
