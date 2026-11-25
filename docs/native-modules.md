# Use Linux native modules

rmake supports the tested GNU Make 4.4.1 native module interface on Linux.
Makefiles can load shared libraries with `load` or `-load`, select a setup
function, and call functions registered by the module. The public allocation,
expansion and evaluation interfaces are available in the default build.

## Test and inspect support

From the repository root, run the native comparison gate or select one model.

```sh
make test-native-modules
make test-native-modules NATIVE_CASE=native-arguments
```

The target builds rmake, verifies the pinned GNU reference and compiles the
original C fixtures with `cc`. It writes
`artifacts/native/development.json`. The broader `make test-m4-release` suite
runs the native gate against the optimized binary and writes
`artifacts/native/optimized.json`. `make test-selfhost` includes the focused
`native-arguments` case through rmake itself.

Start with [the original C module](../tests/native/api.c) to inspect API use and
[the model definitions](../tools/native_modules.py) to inspect Makefiles and
expected results. The [fixture guide](../tests/native/README.md) describes the
runner, compiler selection and retained evidence. Compiled modules live in
disposable test directories and are never checked in.

The runner checks two GNU executions and the candidate against independently
authored byte streams, exit status and ordered file witnesses. It compares full
filesystem snapshots and uses identical executable aliases, workspace paths,
inputs and initial timestamps. Compiler failures, reference disagreement,
candidate differences, reached limits and leftover children fail the gate.
The gate runs trusted fixtures with process bounds. It provides no filesystem
or network security sandbox.

## Load and call a module

GNU documents the [loading interface](https://www.gnu.org/software/make/manual/html_node/Loading-Objects.html)
and [public C ABI](https://www.gnu.org/software/make/manual/html_node/Loaded-Object-API.html).
These define the setup function, `gmk_floc` source locations, registration and
memory ownership. rmake's comparisons target the pinned 4.4.1 behavior.

| Makefile directive | Setup behavior |
| --- | --- |
| `load ./example.so` | Looks up `example_gmk_setup` using the default naming rule. |
| `load ./example.so(example_init)` | Uses the explicit `example_init` symbol. |
| `-load ./optional.so` | Makes a load failure optional during reading. The object can still participate in remaking. |

The module must export `plugin_is_GPL_compatible`. Its setup function registers
Makefile functions with `gmk_add_function`. Those functions can use `gmk_expand`
and `gmk_eval` to enter the active interpreter. Allocate returned strings with
`gmk_alloc`. Release an expansion result with `gmk_free` when the module has
finished using it. Once a registered function returns its result, the
interpreter owns that buffer.

The [reader preparation](load-reader.md) describes operand expansion, filename
handling and setup-symbol syntax before the runtime loads an object. A repeated
load of the same name performs no new setup-symbol lookup, object open or
unload. Verbose debug output reports those operations when they occur. BASIC
debug output reports the raw loader error for an optional object that could
not be opened.

## Registration and expansion context

The function registry belongs to the active expansion context. Scoped snapshots
see newly registered functions, and a selected registration remains owned while
its arguments are evaluated.

Direct calls check minimum arity after eager argument effects.
`GMK_FUNC_NOEXPAND` preserves raw arguments. `call` expands its arguments before
dispatch and passes every argument even when the registered direct-call maximum
is smaller. Callback expansion and evaluation retain lexical, call, target and
automatic-variable context. A NULL eval location inherits the active caller.

Successful setup appends the raw module name to `.LOADED` with normal origin
precedence. When a load occurs in a lexical or target scope, the active raw
`.LOADED` value seeds the global append. The local binding keeps its value, and
a higher-priority global binding still controls publication. Optional setup
failure retains registrations and file records. A missing optional library
remains a required remake goal after reading.

Finite callback expansion is tested through depth 1,024 using the bounded host
stack adapter. The models preserve lexical scope, cycle checks and effect order.
That result applies to the selected depth and resource profiles.

## Foreign calls and error ownership

Native operations run on the process interpreter thread. Module setup,
constructors, callbacks, unloading and process finalization execute on guarded C
stacks on that same OS thread. Public API wrappers suspend the C stack and send
owned requests to a Rust loop on the interpreter stack. The Rust handler
finishes before the foreign wrapper resumes. Nested calls use separate sessions,
and signal masks follow the thread across each switch.

An interpreter error leaves the foreign call suspended for the rest of the
process. Its stack, arguments and module ownership remain resident until exit.
Rust then performs ordinary cleanup for children, output and jobserver permits.
This avoids unwinding or jumping over Rust or C++ frames. Active calls prevent
explicit unloading, and invalidated callbacks cannot call an unloaded image.
The ABI relies on trusted modules supplying valid C strings, callback pointers
and allocation ownership.

The implementation is split across three boundaries.

| Code | Responsibility |
| --- | --- |
| [Core native traits](../crates/rmake-core/src/native.rs) | Scoped interpreter hosts, registrations and owned callback requests. |
| [Application registry](../crates/rmake-app/src/native.rs) | The shared namespace and its lifetime through finalization. |
| [Linux runtime](../crates/rmake-linux/src/native.rs) | Dynamic loading, image ownership and the C call bridge. |

Foreign stacks are not automatically roots for conservative garbage collectors.
GC-using native runtimes need explicit suspended-stack root integration and
separate acceptance. The [optional Guile frontend](guile-frontend.md) documents
its qualified stack, value and callback paths. Broader GC/native combinations
remain open.

## Remaking and finalization

The dynamic-loader namespace survives ordinary evaluation. When a Makefile or
module changes, a real process exec resets it. A setup return of `-1` keeps the
module and its `.LOADED` entry while excluding it from automatic Makefile
remaking. An explicit goal can still rebuild the module.

A module is unloaded before its recipe expands. After an unchanged ordinary
remake, the module reloads without a restart. A query verdict leaves it
unloaded for the ordinary goals. Dropping the runtime does not
prematurely run shared-library finalizers. At process exit, the interpreter
stays alive while libc invokes module finalizers through the request bridge.
After libc closes stdout, a finalizer's shell capture reports a bad file
descriptor and records `.SHELLSTATUS` as 127. The capture follows the C stream
state even when the finalizer reopens descriptor 1. Captures in an earlier
`atexit` callback or unload use the open stream.
For a direct `$(error ...)`, finalizers can read the active source location,
target variables, and lexical bindings from the failing expansion. Ordinary
child failure and successful completion use the normal exit context.
[Startup exit owners](native-startup-exits.md) retain the actual decoder,
constructor or restart state for the covered exit phases.

Native C output bypasses output-synchronization spools. An interpreter warning
does not flush a partial C stdout buffer. The tests preserve GNU's stdout-close
ordering at finalization while retaining interpreter and file effects.

## Source locations

A NULL source filename and an explicitly empty filename remain distinct through
eval frames, definitions, deferred recipes and module reloads. Source bytes are
shared immutably without increasing `Location` size. Evaluation borrows the
existing location while each read frame retains its owned source.

Builtin recipes have a separate ownership marker. A user source named
`<builtin>` therefore keeps its suffix recipe when late `-r` removes builtins.
The [source-location models](../tests/native/source-provenance.toml) cover setup
locations, immediate and deferred warnings, include restoration, unchanged
module reloads and user suffix recipes.

## Acceptance and remaining work

The native adapters and C fixtures are original work. They include no copied or
translated GNU implementation, header or test. GNU Make serves only as the test
reference and never as a runtime fallback. The qualified registration and
lifecycle cases use the pinned C-locale x86-64 Linux/glibc profile.

The [retained evidence index](../tests/native/README.md#retained-evidence-and-open-profiles)
links historical qualification and measurement records for their exact binaries.
It preserves incorrect predictions and early candidate failures. In particular,
[source-name discovery](../tests/native/source-provenance-history.json) retains
the rejected line-number predictions, and the
[first full-gate record](../tests/native/full-gate-first.json) retains the builtin
diagnostic regressions found during integration. Current acceptance requires
validation of the selected source revision. The historical performance reports
establish no general speedup or GNU performance parity.

Two recorded GNU registration crashes remain quarantined outside the default
gate. They receive neither ordinary compatibility credit nor approved-exception
credit. Native qualification also leaves the following work open.

- Broader combinations of native modules and stdin Makefile restarts.
- Interruptions during foreign code that does not call back into the interpreter.
- Complete restart, export and descriptor combinations.
- Callback depths beyond the selected models, aggregate allocation and other
  stack or resource profiles.
- Broader locale, architecture, GC and unusual nonlocal control-flow profiles.

Native support does not close M3 or M6. The [M3/M6 audit](m3-m6-audit.md) also
tracks the remaining CLI, debug, database, locale, integration and developer
flows, enabled Guile coverage, persistent cache correctness and general
performance requirements. All existing acceptance gates remain required.
