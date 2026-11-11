# Structured debug output on the pinned GNU 4.4.1 profile

The application now emits selected GNU-compatible BASIC, VERBOSE, IMPLICIT,
MAKEFILE, PRINT and WHY output, including `-d`, `--debug` selection and `--trace`
interactions. All 64 deterministic profiles pass both current frozen builds;
the preceding debug build passes 62 and fails two child-lifecycle cases. The
earlier 54-profile slice and its one-pass/53-fail directory-build control remain
immutable historical evidence. This is a bounded debug slice, and complete
debug/database behavior and M3/M6 remain open.

The application keeps original debug requests in `debug::Requests` separately
from the effective `debug::Mask`. Unique raw argument values retain byte spelling,
case and order for MAKEFLAGS serialization; repeated identical values are not
reinserted. Consequently separate `--debug=b --debug=n --debug=b` selections
retain the first two raw values, while the single value `b,n,b` contains all
three operations. Effective category selection follows the first character of
each word, case-insensitively, with ordered resets. VERBOSE, IMPLICIT and MAKEFILE
also enable BASIC; JOBS, PRINT and WHY do not. TRACE initially contributes PRINT
and WHY, and a later reset can clear their effective selection while the original
request history remains available. Invalid separators diagnose the remaining
suffix after completed eager effects.

Reading and traversal produce borrowed events instead of constructing diagnostic
strings throughout the interpreter. Core `ReaderEvent` values borrow filenames
and carry search, optional, default-goal and tilde annotations; the application
chooses their presentation through its output receiver. Their emission preserves
actual read/effect order without repeating an expansion or filesystem lookup.
The selected profiles check annotation order and BASIC BOM notification. BASIC
header publication precedes command-line `--eval`, while eager argument effects
retain their earlier phase. MAKEFILE selection controls traversal diagnostics
while Makefiles are remade, with the previous effective mask restored afterward.

Reader and traversal debug records write directly to stdout at their event
boundaries, retaining GNU's observed ordering relative to synchronized ordinary
output. PRINT and WHY use the existing command-echo and rebuild-reason paths.
The event renderer checks its category before allocating indentation or rendering
text. IMPLICIT rule-label construction runs only when its category is enabled;
WHY reason construction and its automatic-variable expansion are also guarded.
These fast-path checks avoid unnecessary debug formatting and evaluation when
that output is disabled. They do not establish zero overhead or a general
performance improvement.

`$(file <path)` returns an empty value on an absent file. With VERBOSE active,
GNU Make 4.4.1's `func_file` in `src/function.c` also reports the failed open
immediately on stdout. The Linux effect exposes the original error to the app
without retrying the open. The renderer checks the current mask, preserves the
filename bytes, and uses the active catalog and libc error text. Eleven original
Makefiles check startup eval, inclusion, secondary expansion, live mask
activation, recipes, output synchronization and control paths. Two additional
models check C and French output and unchanged timestamps.


The remake preflight reports phony makefiles and unconditional double-colon
recipes before traversal enters its temporary debug mask. This follows GNU
Make 4.4.1's preflight in `src/main.c`. When a makefile has no recorded rule,
its graph visit observes freshness and reports consideration before the implicit
search. It retains the outcome for a later ordinary goal and reports an optional
missing file's cached failure before the error. Nine ordinary cases cover search
order, retained secondary-expansion effects, and reuse across the two phases.
Eight separate ordinary cases and one C/French model cover preflight skip
decisions, ordering, required and optional includes, and retained recipes.

`make test-debug-output` runs the separate strict gate; `DEBUG_CASE` selects an
exact ID, including `debug-bare-basic` for the focused self-host flow. Paths come
only from the predetermined reset workspace and executable alias. All stream
bytes, status, ordered effects, independent complete filesystem state, GNU repeat
agreement and process bounds are checked. Observed streams and final files exclude
dynamic job addresses/PIDs. No compiler or native module is needed by these
debug fixtures.
The gate is wired into development, optimized and focused self-host flows.

Eight implicit-stem profiles cover short relative names and the Linux stem
limit. Implicit search counts the bytes of the stem plus any directory prefix
ignored during matching. An overlong candidate is reported after the attempt
and skipped before prerequisite expansion, including when debug output is off.
The fixtures use bounded virtual names, preserving ordinary stat diagnostics
and comparing the complete filesystem after each run.

Implicit tracing keeps the full stem when matching the whole archive member
reference, including slashes in the container or member name. Seven archive
and ordinary controls distinguish that first search from member-only fallback
and the directory prefix ignored by an ordinary slashless pattern.

Automatic debug-triggered version headers are exact GNU protocol, including the
pinned build identity. Only the case that explicitly requests `--version` with an
invalid debug selection permits one fixed branding block. Its GNU calibration
must match the stored original GNU block, and each engine's isolated version is
checked twice without reader or directory/debug effects. The complete eager
output, fatal diagnostic, version position, status and file effects remain exact.
Negative controls reject moved/extra blocks, changed paths, indentation, state,
headers and boundaries. Ordinary debug headers have no branding exemption, and
there is no broad stream or job-identity normalization.

The earlier [54-profile qualification](../tests/debug-output/qualification-v2.json)
retains seven historical reports: its selected builds passed debug 54, native
215 and directory 40, while its preceding-main debug control passed one and
failed 53. The original
forty-profile [history](../tests/debug-output/history.json) and fourteen-case
[review extension](../tests/debug-output/extension-history.json) remain separate
and immutable. The latter closes twelve v1 differences in eval-header timing,
BOM reporting, reader annotations and debug separator/suffix errors. Earlier
incorrect predictions, all observations and the original explicit-version raw
GNU model are preserved. Original fixtures and adapters include no copied or
translated GNU implementation, headers or tests.

The current [BASIC-child qualification](../tests/debug-output/qualification-children.json)
retains both C1 builds passing all 64 debug profiles and the preceding debug
build passing 62 and failing two. Ten new original cases cover serial/parallel
success, stop-mode and keep-going failures, ignored errors, up-to-date targets
and a queued three-worker graph. The corrections restore the observed parent
Missing revisits and omit the serial stop-mode Failed record after a fatal
child. Serial keep-going retains its legitimate Failed record; parallel failures
and queued completions do not invent individual Failed/Success records that GNU
omits. The absence of queued-child Success lines is required behavior in these
cases, not evidence of an unimplemented feature.

The queued worker protocol forces START-first, START-second, END-first,
START-third, END-second, END-third. Its third worker verifies the second worker's
PID and Linux process starttime while that worker still blocks, then waits for
its completion and disappearance or replacement of that exact identity. The
sole temporary PID file is removed before final observation. All final output,
ordered effects and filesystem bytes remain static; there is no job-identity
normalizer or broader JOBS comparison. Workers and observation groups have
explicit deadlines and reject leftovers. The eight original first/corrected
models and two additive keep-going first predictions remain separately retained
in [child-history.json](../tests/debug-output/child-history.json), including the
incorrect first serial-failure expectation. All earlier 54 model fingerprints,
model files and archives remain unchanged.

Formatting, Clippy, 576 Rust tests and 634 tooling tests pass. The existing 215
native and 40 directory models/history remain unchanged; their earlier passing
qualifications are historical references. This C1 archive qualifies debug only;
new C1 native/directory acceptance awaits the full gate. The ordinary ledger remains
27,766 required / zero pending across 158 families; the 32 documented safety
exceptions / 128 separately gated cells are unchanged. These separate debug
profiles add no ordinary-ledger or safety-exception credit. Full frozen local
acceptance and exact-head CI remain merge gates. The separate
[earlier measurement note](benchmarks/debug-output.md) preserves its regressions,
and the [child-slice measurements](benchmarks/debug-children.md) retain the new
controls and limits. These compatibility results establish no general GNU parity.

Beyond the separately qualified JOBS profiles below, broader PID/address and
lifetime relationships and asynchronous traversal diagnostics remain open. The selected
IMPLICIT and MAKEFILE cases do not establish full implicit-chain, built-in search
or default-Makefile discovery diagnostics. Database output (`-p`) remains
unimplemented. Broader recursion, native/debug interactions, stdout/descriptor,
non-C locale, architecture, Linux/libc, interruption and resource profiles need
separate acceptance. Enabled Guile/GC integration, persistent cache correctness,
external-project integration and complete developer-flow coverage also remain
open. No selected-profile result waives those M3/M6 requirements.

A later bounded [actual-child JOBS slice](job-debug.md) qualifies nine strict
identity/token profiles on both selected builds. Its complete raw histories,
rejected observations and independent live-process/token witnesses are separate
from this deterministic 64-profile evidence. Broader JOBS combinations and
complete debug/M3/M6 acceptance remain open. Its [measurement page](benchmarks/job-debug.md)
and [performance archive](../tests/job-debug/performance.json) retain mixed elapsed
results, unchanged allocation calls/frees and all exact inputs; they establish no
general performance parity.

Verbose output reports the output-sync mutex after the makefile reader completes
and after any jobserver controller line. Four original named-lock cases
cover active target synchronization, serial spool drain, a live `MAKEFLAGS`
selection, and the quiet BASIC control. The selected Guile gate also checks
this ordering while a recipe expands a missing file through `gmk-expand`.

## Missing targets in double-colon declarations

The basic and verbose debug streams report an initially missing target
when each declared double-colon branch is visited. Later branches retain
that observation even when an earlier branch creates the file. Five
fixtures exercise query, dry-run, touch, normal execution, and an
initially present target.

During a serial makefile remake, the failed source read marks the head
declaration missing. A later declaration can then observe the file
created by an earlier recipe and find its own prerequisite older. An
initially present source retains its original comparison time across the
declared branches. Five fixtures check exact output and included values
across required and optional reads, with basic and verbose tracing and
without debug output. Optional failed-branch replay remains a separate
case.

Serial ungrouped double-colon makefile remakes use the optional source
policy for the first declaration. Later declarations report their own errors.
A cached prerequisite failure can be reported again when a later declaration
or required source read revisits the first declaration. With keep-going,
the first declaration determines the source status and the physical file can
be reread after either declaration changes it.

Serial remake tracing reports a failed prerequisite lookup when an optional
makefile or a required makefile under `-k` returns to the updater. A recipe
failure also produces a failed-update trace when an optional makefile returns
to the updater. A required stopping failure exits before that trace. These
diagnostics also apply when `-n`, `-q`, or `-t` governs a separate build goal.

Under `-k`, a required ungrouped double-colon makefile can finish its
remaining declarations after the head fails. Verbose tracing then reports
one cached-head prune per additional declaration. This also applies to the
default makefile and to a primary file named with `-f`.
