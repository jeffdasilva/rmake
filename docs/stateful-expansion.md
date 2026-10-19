# Stateful expansion

Expansion now distinguishes immutable stores, writable stores, full databases,
and target execution scopes. Pure expansion retains its immutable fast path.
The runtime explicitly supplies shell capture, filesystem and include readers.
There is no GNU subprocess fallback, shared mutable pointer or unsafe aliasing.

`$(shell ...)` and `!=` execute synchronously, capture stdout, fold LF/CRLF,
preserve stderr, and install `.SHELLSTATUS` as an override simple binding.
Empty commands retain the previous status. Shell status follows the active
variable scope: recipe-time status is local to that target, while parse-time
status is global. Shell environment preparation respects export policy and
falls back to the original environment when an active recursive export would
re-enter itself. The runtime supplies the startup invocation depth independently
of user changes to the visible MAKELEVEL variable. Process capture uses the
existing direct-command selection and Linux ENOEXEC fallback, and bounds output.
Every completed shell effect invalidates cached directory observations.

`eval` performs argument expansion before reading the result and returns an
empty value. Its mutations are visible in the remainder of the same expansion.
Definitions and rules share the current database. Nested evaluation, call,
foreach and let bindings, computed names, includes, overrides and undefine are
covered by original fixtures. Generated syntax keeps the caller's line for
diagnostics. Recipe-time variable definitions update globals while target
bindings retain precedence; defining rules in recipes is a fatal error.

The ownership transfer around immediate assignments lets nested reads change
this same database without aliased mutable references. Target evaluation keeps
its scope separate from the global write destination. Nesting and capture limits
bound runaway evaluation. There is no persistent result cache: shell and eval
are observable effects and are never memoized.

Validation adds 81 original differential fixtures and promotes ten existing
fixtures. All 1,883 required cases pass; 22 mismatches remain pending in other
families. This does not establish complete shell/eval compatibility: definition
scope corner cases, startup MAKEFLAGS evaluation and full process/signal behavior
still require coverage and implementation. The Linux shell capture limit also
intentionally differs from GNU's unbounded allocation behavior.
