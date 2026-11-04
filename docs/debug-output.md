# Structured debug output on the pinned GNU 4.4.1 profile

The application now emits selected GNU-compatible BASIC, VERBOSE, IMPLICIT,
MAKEFILE, PRINT and WHY output, including `-d`, `--debug` selection and `--trace`
interactions. The 54 original deterministic profiles pass both frozen builds;
the immutable preceding main passes one and fails 53. This is a bounded debug
slice, and complete debug/database behavior and M3/M6 remain open.

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

`make test-debug-output` runs the separate strict gate; `DEBUG_CASE` selects an
exact ID, including `debug-bare-basic` for the focused self-host flow. Paths come
only from the predetermined reset workspace and executable alias. All stream
bytes, status, ordered effects, independent complete filesystem state, GNU repeat
agreement and process bounds are checked. Controlled graphs exclude dynamic job
addresses/PIDs. No compiler or native module is needed by these debug fixtures.
The gate is wired into development, optimized and focused self-host flows.

Automatic debug-triggered version headers are exact GNU protocol, including the
pinned build identity. Only the case that explicitly requests `--version` with an
invalid debug selection permits one fixed branding block. Its GNU calibration
must match the stored original GNU block, and each engine's isolated version is
checked twice without reader or directory/debug effects. The complete eager
output, fatal diagnostic, version position, status and file effects remain exact.
Negative controls reject moved/extra blocks, changed paths, indentation, state,
headers and boundaries. Ordinary debug headers have no branding exemption, and
there is no broad stream or job-identity normalization.

[Complete qualification](../tests/debug-output/qualification-v2.json) retains all
seven final reports: both builds pass debug 54, native 215 and directory 40,
while the preceding-main debug control passes one and fails 53. The original
forty-profile [history](../tests/debug-output/history.json) and fourteen-case
[review extension](../tests/debug-output/extension-history.json) remain separate
and immutable. The latter closes twelve v1 differences in eval-header timing,
BOM reporting, reader annotations and debug separator/suffix errors. Earlier
incorrect predictions, all observations and the original explicit-version raw
GNU model are preserved. Original fixtures and adapters include no copied or
translated GNU implementation, headers or tests.

Formatting, Clippy, 576 Rust tests and 634 tooling tests pass. The existing 215
native and 40 directory profiles remain unchanged. The ordinary ledger remains
27,766 required / zero pending across 158 families; the 32 documented safety
exceptions / 128 separately gated cells are unchanged. These separate debug
profiles add no ordinary-ledger or safety-exception credit. Full frozen local
acceptance and exact-head CI remain merge gates. The separate
[measurement note](benchmarks/debug-output.md) retains timing/allocation evidence
and its limits; these compatibility results do not establish general GNU parity.

Known open work includes JOBS records for actual children, their PID/address and
lifetime relationships, and asynchronously completed remake output. The selected
IMPLICIT and MAKEFILE cases do not establish full implicit-chain, built-in search
or default-Makefile discovery diagnostics. Database output (`-p`) remains
unimplemented. Broader recursion, native/debug interactions, stdout/descriptor,
non-C locale, architecture, Linux/libc, interruption and resource profiles need
separate acceptance. Enabled Guile/GC integration, persistent cache correctness,
external-project integration and complete developer-flow coverage also remain
open. No selected-profile result waives those M3/M6 requirements.
