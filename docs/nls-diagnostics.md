# Use and test localized diagnostics

rmake ships the 30 message catalogs distributed with GNU Make 4.4.1. The Linux
application uses them for supported system diagnostics. Catalog selection,
individual messages and locale-dependent behavior have separate tests. This
guide explains how to stage the catalogs and which profiles the evidence covers.

## Stage catalogs for installation

A development build binds the `make` text domain to the checkout's `catalogs`
directory. For an installed binary, set its absolute catalog path at build time
and stage the same logical directory.

```sh
RMAKE_LOCALEDIR=/usr/local/share/rmake/locale make release
make catalog-install PREFIX=/usr/local DESTDIR="$PWD/artifacts/catalog-stage"
```

This stages the catalogs, source notices and provenance under
`artifacts/catalog-stage/usr/local/share/rmake/locale`. The binary's configured
runtime path remains `/usr/local/share/rmake/locale`. `DESTDIR` supplies the
packaging prefix. Distribute the executable and staged catalog tree together.

| Setting | Default and purpose |
| --- | --- |
| `RMAKE_LOCALEDIR` | Build-time environment variable for the absolute runtime catalog directory. Defaults to the checkout's `catalogs` path. |
| `PREFIX` | `/usr/local`. Supplies the installation prefix. |
| `CATALOG_INSTALL_DIR` | `$(PREFIX)/share/rmake/locale`. Selects the logical destination for `catalog-install`. Keep it equal to the binary's `RMAKE_LOCALEDIR`. |
| `DESTDIR` | Empty. Prepends a staging root to `CATALOG_INSTALL_DIR`. |

Run `make catalog-check` to verify catalog and notice identities. Cargo also
checks every shipped compiled catalog against the supported typed message
schemas. The [catalog guide](../catalogs/README.md),
[staging tool](../tools/catalogs.py) and [build setup](../crates/rmake-app/build.rs)
record these flows.

## Run locale checks

Run the diagnostic gate and replay an individual case from the repository root.

```sh
make test-nls
make test-nls NLS_CASE=nls-private-messages-warning
```

The NLS gates build the candidate, verify the reference and catalogs, and prepare the
required locale data. Use the following targets for related checks.

| Command | Coverage and replay setting |
| --- | --- |
| `make test-nls-release` | The localized diagnostic gate with the optimized binary. Supports `NLS_CASE`. |
| `make test-nls-directory` | Localized entering and leaving lines, including deleted working directories. Supports `NLS_DIRECTORY_CASE`. |
| `make test-nls-directory-release` | The directory gate with the optimized binary. Supports `NLS_DIRECTORY_CASE`. |
| `make test-catalog-coverage` | Every shipped catalog and language fallback. Supports `CATALOG_COVERAGE_CASE`. |
| `make test-locales` | Selected C and C.UTF8 parsing, byte and filename behavior. Supports `LOCALE_CASE`. |

The diagnostic reports are `artifacts/nls/development.json` and
`artifacts/nls/optimized.json`. Directory reports use
`artifacts/nls/directory-development.json` and
`artifacts/nls/directory-optimized.json`. Catalog selection reports go to
`artifacts/catalog-coverage/development.json`.

`LOCALE_DATA_ROOT` defaults to `.tools/locale-data` under the repository. The
[pinned locale profile](../oracle/locale-data.json) provides private French UTF-8
data for the recorded Ubuntu Noble x86-64/glibc 2.39 environment. Provisioning
verifies the package and generated data identities and leaves system locale
configuration unchanged. `make bootstrap-locale-data` prepares this test data
on its own.

The [AArch64 locale lock](../oracle/locale-data-arm64.json) verifies the same
French source data and an Arm `localedef` package from the signed Noble Ports
index. Both locks pin the same compiled French locale tree. The
[native AArch64 job](../.github/workflows/native-aarch64.yml) checks the pinned
GNU Make reference twice on the C and French startup debug diagnostics after
verifying the private locale and GNU Make catalog. The optimized rmake build
then runs the same diagnostics against the verified reference and retains its
own report.

## Language and profile coverage

The [catalog selection gate](../tests/catalog-coverage/README.md) exercises all
30 catalogs through glibc. It uses the private French UTF-8 message locale and
selects each catalog with `LANGUAGE`. Controls cover an unavailable language,
a partially translated message and the C locale. An independent catalog reader
checks expected bytes, and a native callback checks actual gettext selection
alongside a parser diagnostic. This establishes catalog selection and fallback
for those messages.

The diagnostic models below cover French and C controls, private and global
`LC_MESSAGES`, custom domains, and selected German, Japanese and Polish
messages. The [C/C.UTF8 profiles](../tests/locales/README.md) and
[startup locale guide](startup-locale.md) cover other locale behavior separately.
The [Guile guide](guile-frontend.md) records its own enabled-profile acceptance.
Recorded Guile checks include private French messages in a joined worker and
preservation of that locale through fatal diagnostics and finalizers.

## Diagnostic bytes and ownership

The core distinguishes system messages from user and native text. The
[Linux gettext adapter](../crates/rmake-linux/src/messages.rs) selects a system
template in the active locale and text domain. The formatter checks byte-string
and numeric arguments before substitution. Translated templates never pass
through C varargs. User and module text remains literal even when it matches
a catalog message ID.

An expansion error that crosses a native callback boundary retains its rendered
body and stop suffix. Restoring the caller's locale therefore preserves the
already-established diagnostic. Warnings are rendered when published, before
output buffering. File names, target names, OS errors and other byte arguments
retain their original spelling.

The initial domain uses rmake's catalog directory. Native callers can change
their domain and bindings through public libc interfaces, including selecting
an unbound domain. Custom templates can reorder, omit or reuse compatible
arguments. The checked formatter rejects unsupported or mismatched conversions.
Malformed GNU templates have not been exercised as reference cases.

## Read the evidence

The [diagnostic runner](../tools/nls_diagnostics.py) compares two GNU runs and
the candidate against independently specified stdout, stderr, status, ordered
effects and complete filesystem expectations. Selected restart cases also
assert timestamps. Process bounds and cleanup remain part of acceptance.
A new signal, reached bound or leftover process is retained as evidence and
stops replay. Diagnostic output is compared byte for byte.

Some messages contain details supplied by the operating system. The gate obtains
errno text through an independent libc call in the selected locale. Loader
messages use separate `dlopen` and `dlsym` observations. These exact bytes fill
explicit expected fields. The Make wording, paths, status and effects remain
independently specified. Reports retain the observations, input models, source
hashes, reference identity, catalog identity and locale profile.

## Structural diagnostics

The [structural models](../tests/nls/structure.json) cover absent or empty
Makefiles, invalid default goals, reader syntax, recursive or unterminated
variable references and duplicate recipes. Paired warnings retain their source
lines, order and raw target names. A following user warning keeps its own bytes.
Mixed implicit and normal or static pattern rules use the translated parser
message. The models check anonymous `--eval` input and Makefile line locations
in C and French, with German and Japanese catalog fallback.
Legacy declarations that begin with an explicit target warn for each following
pattern target. The localized warning retains its source location and the
selected recipe still runs.
An unquoted semicolon with a blank rule prefix reports a missing rule before
the recipe. The parser reports it before recipes run, including for anonymous
`--eval` input.
Native controls change private French or C `LC_MESSAGES` during include failures
and warning publication, then check restoration to the global selection.

## Directory output

Each entering or leaving line uses the domain and message locale active at
publication. The program name, recursion depth and directory remain separate
arguments so a template can reorder them. Output synchronization and C stdout
flush boundaries still apply. Database output adds its literal comment prefix
outside the template.

A failed `getcwd` reports the OS error and continues. An initial failure without
`-C` leaves `CURDIR` empty. Failure to refresh the directory after a successful
`-C` retains the earlier `CURDIR` and reports an unknown directory.
The [directory models](../tests/nls/directory.json) create deleted
working directories or hold a deleted directory through an open descriptor.
They check root and recursive calls, line and target spools, live locale changes
and custom templates. Expected paths come from the test workspace, and expected
OS error bytes come from an independent libc call.

## Scheduler, parser, and file arguments

The [producer models](../tests/nls/producers.json) cover up-to-date summaries,
cycle drops, static-pattern errors and warnings about text after directives.
The `file` function has typed errors for a missing name, an extra read argument
or an invalid operation. Tests preserve source lines, side effects and raw
argument bytes. [Low-resolution timestamp models](../tests/nls/low-resolution.json)
use fixed timestamps to check warning publication and effects in C and French.
The [debug option models](../tests/nls/debug-options.json) check invalid debug
requests at startup and during secondary expansion. They cover French,
`LANGUAGE` fallback, and the original bytes of an invalid C-locale operand.

## Native loader diagnostics

A missing object emits the raw loader error before the localized failure.
A missing setup symbol passes the raw loader error as a template argument.
Function names and numeric argument limits keep their values. Private message
locales remain active through failures reported in an included Makefile.
The [loader models](../tests/nls/loader.json) cover optional modules and custom
templates that reorder arguments.

## Include and source I/O diagnostics

A failed startup `-C` formats the path and libc detail with the Make catalog. The [producer models](../tests/nls/producers.json) capture libc error
text separately and check argument effects, raw paths and successive directory
changes in C and French.

Makefile open failures retain resolved search names and error numbers. Failed
include remakes use GNU's fixed colon separator. Explicit `-f` failures and
fatal stream-read errors use their own templates. A stream-read error retains
the original include operand even when `-I` found the file elsewhere. The
[source I/O models](../tests/nls/include.json) also check a changed domain before
a deferred diagnostic, non-UTF-8 filenames, and the saved errno rendered under
the locale active at publication.

Default lookup chooses the first matching directory entry in GNU's filename
order. The ordinary reader handles directories, symlinks and open errors. Under
`-k`, failed required Makefile updates report summaries after the remake chain.
Optional Makefiles suppress those errors. The blocked-target warning used for
ordinary goals remains separate. See the [remake models](../tests/nls/remake.json).

A Makefile included more than once runs its remake recipe once, while each
required read receives its own summary. A failed update can still trigger a
restart under `-k` if it leaves the required file present with a changed
timestamp. The summary appears before restarting. Freshness is checked after
the whole chain, so a later recipe can change or restore the timestamp. A file
that could not run its own recipe retains its cached timestamp for this decision.
The [repeated-include models](../tests/nls/repeat.json) preserve these distinctions.

An optional read can encounter a failed update before a later required read of
the same file. The required read reports that cached failure. A blocked remake
retains the selected prerequisite and its immediate parent without re-expanding
prerequisites or rerunning recipes. The first open error keeps its location and
raw errno. The [failure replay models](../tests/nls/optional-replay.json) cover
repeated sites, direct and transitive failures, secondary expansion, early stop
and real restarts. They also distinguish an early intermediate-prerequisite
error from a later required-read summary.

## Touch open failures

Ordinary and one-shell recipes entering touch mode use the active catalog's OS
error template. The [touch models](../tests/nls/touch-open.json) cover directory
targets, missing parents and non-directory parents. They retain initial recipe
and configuration effects, exact paths, errno text and the files left afterward.

## Recipe cleanup

Removal and archive-member warnings use the active catalog, including grouped
peers. File names and group owners remain raw byte arguments. The
[cleanup models](../tests/nls/cleanup.json) cover serial recipes, both jobservers
with target output sync, non-UTF-8 names, retained archive members, precious
siblings and ignored failures.

## Outstanding recipes

The notice emitted while a failed build drains its remaining recipes uses the
active catalog. The [wait-notice models](../tests/nls/wait-notice.json) hold a
sibling until the failure and notice are captured, then compare both streams
and the released worker's files. They cover C and French, both jobservers and
target output sync.

## Intermediate-file policy

Conflicting global or named intermediate-file declarations use the active
catalog. The [policy models](../tests/nls/intermediate-policy.json) check German,
Japanese and Polish messages, C and French fallback, raw target bytes and a
nonconflicting control.

## File function I/O

Failures in `$(file ...)` select the active open, read, write or close template.
The path and libc error remain byte arguments. The
[file I/O models](../tests/nls/file-io.json) check reader and recipe expansion,
raw filenames, buffered write and close failures, and effects completed before
the failure.

## Output lock failures

A borrowed output-lock failure during spool flushing emits a localized warning
after libc's errno detail. The [lock models](../tests/nls/output-lock.json) check
Japanese, C and French fallback, continued recipe output and preservation of the
caller's mutex.

## Attribution and remaining work

The shipped PO sources and MO binaries are unchanged declarative data from the
pinned GNU Make 4.4.1 release. [Catalog provenance](../catalogs/provenance.json)
records the archive and file hashes. The [French catalog notice](../catalogs/fr.po),
the notices in each other PO file and [the package license](../catalogs/COPYING)
retain the original attribution. Older translations with no matching message
ID use the source text.

Catalog availability does not establish every diagnostic in every language.
Remaining work includes broader message producers, plural and character-set
formats, native and Guile interactions, and additional Linux/libc profiles.
Database, debug, trace and usage output have their own qualification. The
[M3/M6 audit](m3-m6-audit.md#diagnostics-and-locales) links that evidence and tracks
the remaining combinations. These selected profiles do not establish complete
NLS or M3/M6 compatibility.
