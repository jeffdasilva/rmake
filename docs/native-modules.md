# Linux native module runtime

The Linux application can load GNU-compatible shared libraries through `load`
and `-load`, discover default or explicitly named setup functions, validate the
GPL marker, and register functions using the public allocation, expansion and
evaluation interfaces. The runtime uses original Rust/C adapters and original
C test modules. No GNU implementation, header or test is included or translated,
and GNU is never an execution fallback.

The registry is an explicit expansion-context effect. Scoped snapshots therefore
see newly registered functions without copying or freezing the function table.
A selected registration remains owned while its arguments are evaluated. Native
minimum arity is checked after eager effects, NOEXPAND preserves raw arguments,
and `call` expands arguments before dispatch and passes every argument even when
the registered direct-call maximum is smaller. Callback expansion/eval retains
lexical, call, target and automatic context; a NULL eval location inherits its
active caller. Successful setup appends raw module names to `.LOADED` with normal
origin precedence. Optional setup failure retains registrations and file records;
a missing optional library remains a required remake goal after reading.

## Foreign calls and error ownership

Module setup, constructors, callbacks, unloading and process finalization execute
on guarded C stacks on the same OS thread. Public API wrappers suspend that C
stack and return owned requests to a Rust loop on the ordinary interpreter stack.
The Rust handler finishes before the foreign wrapper resumes. Nested calls use
separate sessions, and signal masks follow the thread across each switch.

On an interpreter error, the suspended foreign call is never resumed. Its stack,
arguments and image ownership remain resident until process exit. The error then
uses ordinary Rust cleanup for children, output and jobserver permits. This does
not unwind or jump over Rust or C++ frames. Invalidated callbacks cannot call an
unloaded image; active calls prevent an explicit unload. The native ABI assumes
valid C strings, callback pointers and allocation ownership from trusted native
modules, as required by the public interface.

The dynamic-loader namespace survives ordinary evaluation and resets through a
real process exec when a Makefile or module actually changes. A setup return of
`-1` keeps the module and its `.LOADED` entry while excluding it from automatic
makefile remaking. An explicit build goal can still rebuild that module. A module
is unloaded before its recipe expands. An unchanged rebuilt module is reloaded
without a restart. Runtime destruction does not invoke premature DSO finalizers.
At process exit the interpreter remains alive while libc executes module finalizers through
the same request bridge. GNU's stdout-close ordering and raw native stdio behavior
are retained: raw C output bypasses output-sync spools, and a warning does not
flush a partial C stdout buffer.

## Acceptance and remaining work

`make test-native-modules` checks 220 original models. It compiles original C
modules into disposable inputs and checks two GNU runs and the candidate against independently written byte streams,
exit status, ordered file witnesses and full filesystem snapshots. It rejects
leftover children, compiler failures and reached limits. `NATIVE_CASE` selects a
case. Development, optimized and focused self-host flows invoke this gate.
Historical incorrect predictions and the first candidate's failures remain
separate, checksum-protected research records under `tests/native`.

This is a supported native runtime slice, not complete M3/M6 acceptance.
[Startup exit owners](native-startup-exits.md) now retain decoder, constructor
and restart state for the selected 31 new profiles. Native use with stdin
Makefile restarts, interruptions during foreign code that
does not call back into the interpreter,
complete restart/export/descriptor cross-products, broader callback recursion and
resource, locale, architecture and unusual nonlocal-control-flow profiles
remain open. Two independently recorded GNU registration crashes remain
quarantined outside the default gate and receive no ordinary or approved-exception
credit. The current registered-function validation and lifecycle evidence covers
the pinned C-locale x86-64 Linux/glibc profile; expanded native cases retain their
own explicit acceptance work.

Nullable source names now distinguish a NULL filename from an explicitly empty
filename through eval frames, definitions, deferred recipes and module reloads.
The representation shares immutable source bytes without increasing `Location`
size. Eval borrows the existing location while each read frame retains its owned
source. Builtin recipe ownership has its own marker, so an explicit user source
named `<builtin>` cannot cause a user suffix recipe to be removed by late `-r`.
37 original provenance models cover setup locations, immediate/deferred warnings,
include restoration, unchanged module reloads and user suffix recipes. The
original native-runtime qualification passed all 184 models in both builds;
its reader-only baseline differed on all 184. Startup-exit qualification later
extends the current gate to 215 models.
The initial NULL/empty discovery and incorrect line-number predictions remain
unchanged historical evidence. The first full gate's two builtin diagnostic
regressions are retained separately; the corrected builds pass both controls
three times each. Full frozen acceptance and exact-head CI are required before merging.

The GNU C ABI runs on the process interpreter thread. Foreign stacks are not
automatically roots for conservative garbage collectors; enabled Guile and
GC-using native runtimes require explicit suspended-stack root integration and
separate acceptance. This remains an open compatibility requirement.

Finite callback expansion through depth 1,024 now uses the existing bounded
host-stack adapter while preserving lexical scope, cycle checks and effect order.
Broader callback depth, aggregate allocation and stack/resource profiles remain open.

The ordinary fixture inventory remains incomplete. Enabled Guile, remaining
CLI/debug/database/locale surfaces, broader integration and developer flows,
persistent cache correctness and general performance parity remain open in the
[M3/M6 audit](m3-m6-audit.md). Native support does not waive any earlier gate.
