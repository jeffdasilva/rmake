# Optional Guile architecture component

The opt-in Linux `guile` Cargo feature provides a process-leading C dispatcher
and an isolated architecture suite. It does not register a make function,
advertise `.FEATURES` support or enable a Guile frontend. The production
interpreter retains its default feature selection.

Foreign native/Guile entry uses the leading pthread's ordinary C stack. Rust
evaluation runs on guarded private/grown stacks on that same OS thread. Owned
commands and replies cross the existing `NativeRuntime`/`NativeHost` boundary;
SCM objects stay in C. Public GC active/blocking APIs preserve conservative
collector roots while Rust executes. Nested callbacks, loader entry, finalizers
and fatal abandoned sessions retain their C stack and host lifetimes. Current
signal masks follow the OS thread across context swaps.

A C notification pipe uses private CLOEXEC descriptors published before signal
delivery. The handler performs lock-free atomics and nonblocking writes only.
The terminal helper registers with Guile only after an actual fatal notification;
public thread enumeration queues a leading-TID-guarded async. Its ordinary C
stack remains alive through terminal signal death. Normal Scheme thread counts
and overridden Scheme handlers retain their separately tested behavior. This is
component policy, not the application's final signal/SA_RESTART policy.

Twenty bounded subprocess controls cover lazy empty entry, nested grown host
callbacks and collection, recoverable/uncatchable errors, live roots, normal
thread counts, native constructor/unload/finalization, cancellation across
initialization and worker-first Guile, Scheme override, and rejection of native
entry without the leading dispatcher, plus floating-point state observed and
changed by the Rust host before and after Guile initialization. The outside
control prevents a feature build
from invoking Guile through the older native fibre path. Its first test matched
the wrong Rust error variant; the failure and corrected assertion are retained.
These are architecture/safety checks with zero GNU CLI conformance credit.
Separate original GNU2 signal/initialization evidence remains distinct.

`make bootstrap-guile` provisions a pinned Ubuntu 24.04 x86-64/glibc2.39 profile
and a separate GNU 4.4.1 executable configured with Guile. The existing disabled
oracle remains separate. The tool verifies the signed noble InRelease, both
signed Packages digests, all 17 package/version/archive/license identities and a
pinned complete installed-tree digest. Installation is local extraction;
maintainer scripts and system package installation are not used. Immutable
lock-keyed prefixes preserve existing runtimes, and concurrent bootstrap writers
use a profile lock. Corrupt archives or installed trees fail verification.

The portable path has been exercised from a cold HTTPS cache, then a warm cache.
The enabled reference requires actual `guile` capability and an independently
authored Scheme result. Fresh source extraction prevents stale disabled objects
from being reused. `make test-guile-components` verifies the profile and runs
the isolated target in a separate Cargo directory. CI requires its safety job
alongside the existing development/optimized gates and retains provenance.

The ordinary native adapter and leading dispatcher carry the current thread's
floating-point environment through each context switch and first entry using
public fenv APIs while preserving errno. Eighteen separately required original
GNU2 models cover constructors/setup, repeated and nested calls, explicit
save/restore, sticky flags and finalizers. The preceding adapter fails 17 of 18;
the fixed default adapter passes 18. The leading C-only controls already passed;
two direct Rust-host controls exposed its distinct stale-host boundary and now
pass with the explicit propagation. Full raw observations and failed hypotheses
remain separate from general compatibility credit.

The privileged application entry/lifetime boundary, full Scheme value/error
conversion, real CLI registration, signals/finalizers, native resource/fork/exec
interactions, wider floating-point/locale formatting and full Guile compatibility remain additional
work. Neither these controls nor their separate GNU2 observations close M3/M6.
Default and enabled performance remain independent measurement requirements.
