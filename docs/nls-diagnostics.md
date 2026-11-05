# Localized system diagnostics

This slice matches the pinned GNU Make 4.4.1 French catalog for selected
reader, expansion, and build diagnostics: missing separators, unterminated
function calls, invalid `intcmp` arguments, undefined variables, missing
rules, skipped goals, and numeric recipe failures. It also covers the fatal
stop suffix and the ignored recipe failure suffix. The shipped French data
comes from the pinned GNU release; older translations that have no matching
message ID retain the source text. See [catalog provenance](../catalogs/provenance.json)
for the pinned source and [the complete notice](../catalogs/fr.po) for attribution.

The core distinguishes system messages from user and native text. The Linux
adapter selects each template through public `gettext` in the active locale
and text domain. The formatter checks byte-string and numeric arguments before
substitution. It does not pass a translated template to C varargs, and never
looks up a user or module's text as a message ID. Expansion errors that can
cross a native callback boundary retain the rendered body and stop suffix,
so restoring the caller's locale cannot change an already established
diagnostic. Warnings are rendered when published, before output buffering.

The initial `make` domain is bound to rmake's own catalog directory. A normal
development build uses the checked-out `catalogs` directory. For installed
builds, set the logical absolute path with `RMAKE_LOCALEDIR` at build time
and install that same directory with `make catalog-install`. For example:

```sh
RMAKE_LOCALEDIR=/usr/local/share/rmake/locale make release
make catalog-install PREFIX=/usr/local DESTDIR=/path/to/stage
```

The initial binding does not use the reference installation or a system
Make catalog. Native callers may change their active domain and bindings
through the public libc interface, including selecting an unbound custom
domain. Application-owned catalogs are checked against the typed message
schema at build time. Custom templates may reorder, omit, or reuse compatible
arguments. Unsupported or mismatched conversions are rejected by the checked
formatter; malformed GNU templates have not been exercised as reference cases.

Run `make test-nls` for development and `make test-nls-release` for optimized
builds. `NLS_CASE` selects one exact case. The gate provisions or verifies the
pinned Ubuntu Noble x86-64/glibc 2.39 locale in `LOCALE_DATA_ROOT`, verifies the
reference binary and French catalog, and compares two GNU runs and one
candidate against fixed expectations for raw stdout, raw stderr, exit status,
file effects, and process cleanup. The 30 original profiles cover French
diagnostics, C controls, live private and global `LC_MESSAGES`, and custom
domains. Native fixtures retain ordered side effects and verify that fatal
expansion stops later calls. No diagnostic output is normalized. User
text stays unchanged even when the surrounding system diagnostic is translated.

The remaining diagnostic inventory, additional languages, plural and broader
character-set formats, output from optional Guile workers, and the database,
debug, and usage surfaces need separate qualification. This slice
does not establish general NLS or M3/M6 compatibility.

## Structural diagnostics

The follow-up adds 16 original cases for absent or empty makefiles, an
invalid default goal, structural reader errors, recursive and unterminated
variable references, and duplicate recipes. The warning pair preserves its
original source lines, order, and raw target bytes. A following user warning
retains its original bytes. Native controls select private French or C
`LC_MESSAGES` during include failures and warning publication, then check
that restoration to the global selection has the expected effect.

These cases extend the same development and release gates to 46 profiles.
The 16 new expectations were recorded before either engine ran. Both GNU
observations matched all 16; the previous candidate matched four and
differed only on stderr in the other 12. The combined diagnostic gate runs all 46 cases.

## Directory output

Each entering or leaving line uses the current domain and message locale at
the point of output. The message arguments keep the base program name,
recursion depth, and directory separate so translated templates can reorder
them. Directory framing still uses the existing output synchronization and
C stdout flush boundaries. Database output adds its literal comment prefix
outside the template.

A failed `getcwd` reports the OS error and continues. Without `-C`, an initial
failure leaves `CURDIR` empty. A failure to refresh the directory after a
successful `-C` retains the earlier `CURDIR` and reports an unknown directory.
The test launcher creates these states with a genuinely deleted cwd or a
deleted directory held by an open fd.

Run `make test-nls-directory` or `make test-nls-directory-release` for 17
additional profiles. `NLS_DIRECTORY_CASE` selects an exact case. The gate
covers C and French, root and recursive invocations, target and line spools,
live locale changes, custom templates, and four deleted-cwd cases.
Expected directory paths come from the fresh test workspace. Expected OS
error bytes come from an independent public libc call in the selected locale.
The Make diagnostic wording, streams, exit status and file effects are
literal expectations. The test drivers write raw observations to the local
artifact directory.

On the combined application, all 63 NLS cases pass with both development
and optimized builds, with and without Guile. The Guile checks also cover
private French messages in a joined worker and preservation of that locale
through fatal diagnostics and finalizers. The default profile retains the
restart, stdin, database, and native-module behaviors covered by their
existing gates.

## Scheduler, parser, and file arguments

The diagnostic gate also covers the up-to-date summary, cycle drops,
static-pattern errors, and warnings about text after directives. The
`file` function reports typed argument errors for a missing name, an
extra read argument, or an invalid operation. The tests check source lines,
side effects, and raw bytes in target names and operation arguments. User and
native error text can match a catalog message ID byte for byte and still
retain its original text.

These 18 cases extend `test-nls` and `test-nls-release` to 64 diagnostic
profiles. Their literal expectations use translations from the pinned
compiled catalog. The 17 directory profiles run through the separate
directory gate.

## Native loader diagnostics

The loader and registration checks in this gate use checked system templates.
A missing object still emits the raw loader error before the localized
failure, and a missing setup symbol passes the raw loader error as one
template argument. Function names and numeric argument limits retain
their original values. Private message locales remain active for failures
reported through an included makefile.

The 15 loader cases bring the diagnostic gate to 79 cases. They include
optional module handling and custom templates that reorder loader
arguments. For the raw loader argument, the gate queries the same public
`dlopen` and `dlsym` interfaces in a separate process before running either
engine. The remaining diagnostic bytes, exit statuses, and file effects
come from the literal expectations.

## Include and source I/O diagnostics

Makefile open failures retain their resolved search names and error numbers.
Ordinary failed include remakes keep GNU's fixed colon separator. Skipped
required makefiles report after the remake recipes, with the active locale
and domain. Explicit `-f` open failures and fatal stream read failures use
their respective catalog templates. A stream read error keeps the original
include operand even if `-I` found the source under another path.

Twenty additional cases check these boundaries in the diagnostic gate,
bringing it to 99 profiles. The gate obtains the raw error detail from an
independent libc call. The paths, translated templates, output, exit status
and filesystem effects are exact. One remake recipe selects a custom domain
before a deferred diagnostic. Its template reorders the error and filename
arguments, and the filename retains its non-UTF-8 byte.
A separate fixture binds a test-owned libc catalog during a failing remake
recipe. It records different translations for the saved open error and a
new, unrelated error, then checks that the deferred diagnostic uses the
saved error number and the locale active at publication.

Default makefile lookup selects the first matching directory entry in GNU's
filename order. The ordinary reader then handles directories, symlinks and
open errors. With `-k`, required makefiles that fail during remake report
summaries after the remake chain. The summaries retain the include source
when applicable and use the locale active at publication. Optional
makefiles suppress these errors. The goal-phase blocked-target warning is
reserved for ordinary goals.

Fourteen cases for default source selection and failed remake summaries
bring the diagnostic gate to 113 cases.

A required makefile can appear in several include directives. Its remake
recipe runs once, and each required read gets its own summary at the end of
the remake pass. With `-k`, a failed update that leaves a required makefile
present at a changed timestamp can still trigger a restart. The failure
summary is published before restarting. The timestamp is checked after the
whole remake chain, so a later recipe can restore or change it. A makefile
that could not run its own recipe retains its cached timestamp for this
decision.

Thirteen further cases bring the diagnostic gate to 126. The restart cases
assert the original and final timestamps, ordered read and recipe markers,
the output of the next process and its exit status. Controls cover a file
whose contents change with the same timestamp, an optional makefile, and
stop mode without `-k`.
