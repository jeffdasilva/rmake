# Optional Guile application

The default Linux build keeps the existing disabled Guile profile. An explicit
Cargo `guile` feature enables the GNU 4.4.1 Scheme interface using the verified
Guile SDK. The application does not call the reference executable at runtime.

The thin executable owns the privileged entry boundary. The interpreter lives
in `rmake-app`, which forbids unsafe Rust, and the portable expansion engine
remains in `rmake-core`. The Linux C adapter owns Scheme values and dispatches
native calls on the real leading stack of the correct thread. Rust evaluation
runs on a guarded private stack. Joined worker calls transfer thread state and
return through owned messages; Scheme values never become raw Rust handles.

The frontend registers functions and variable scopes in the same interpreter
that handles ordinary Makefiles. It keeps early decoder, reader, signal,
standard-stream and native-finalizer lifetimes explicit. Output and process
ownership stay live through native shutdown. The gate covers 57 language and
native integration cases, nine startup phase cases, three HOME cases and 14
signal cases in the enabled profile, plus one named stdin restart and three valid Scheme text cases. The signal cases also run against the
default binary. Two GNU runs and each rmake build must independently match
literal streams, status, ordered effects, complete filesystems and bounds.

Run `make test-guile-frontend` to verify the private SDK and enabled reference,
build separate Cargo profiles, run workspace tests and Clippy, compare both
build modes and drive core checks through each enabled executable. Reports are
kept in a unique directory under `artifacts/guile-frontend`. The ordinary
compatibility gates continue to use the default profile. The separate Guile
component safety gate remains required in CI.

The [retained original evidence](../tests/guile-frontend/qualification-original.json)
records the earlier development binaries, model qualification and negative
observations. The selected cases do not qualify arbitrary asynchronous native
callbacks, detached threads, full error backtraces, other resource profiles or
all Guile behavior. M3/M6 and general performance parity remain open.

The [first integrated stage](../tests/guile-frontend/qualification-staged.json)
passes all four comparison campaigns and both core self-host flows against the
main172 source base. The later database and stdin integration requires its own
full selected-head validation.

The stdin profile requires both Makefile reads, cleanup of the owned source,
and the final goal effect. The first image's Scheme port output is still
buffered at exec and is discarded, so only the second image emits its line. Both GNU runs and both
candidate builds are compared to that literal result.

Six original [detached worker cases](../tests/guile-detached/README.md) now
run in the enabled application gate for both builds. They use a live Linux
child-wait observation to separate an idle worker from a callback made
during the serial recipe wait. The waiting thread handles accepted callbacks
before closing its admission window. A reference-variable fatal wait error
and broader asynchronous/parallel lifecycle acceptance remain open.

Ten original [detached parallel cases](../tests/guile-parallel-detached/README.md)
cover pipe and FIFO jobservers, draining child jobs, waiting for a token,
and consecutive worker requests. Both enabled profiles require them.
Broader asynchronous entry points and the fatal reference race remain open.

The enabled gate also runs three [late descriptor pressure cases](../tests/guile-eventfd-pressure/README.md)
in each build. When the recipe has already started and no private wake descriptor
fits, the main thread keeps accepting Scheme callbacks and checks the queue at
bounded intervals. The regular descriptor wake path is unchanged.

Six [parallel descriptor pressure cases](../tests/guile-parallel-pressure/README.md)
check pipe and FIFO drain and token waits. Bounded polling keeps the same worker
admission active across callback requests. Actual child and token readiness
still advances the scheduler, including the control where pidfds are unavailable.
Both enabled builds run these cases.

Public Scheme source is read as UTF-8 under both the C and C.UTF-8 locales.
`gmk-eval` returns Scheme false after publishing its make effect. The three
valid-source comparisons distinguish source decoding from result conversion
and verify the resulting variable and file effects.

With BASIC debugging enabled, Guile's make callbacks publish their
expansion or evaluation line before applying the effect. The scoped host
formats the line for both direct and worker calls. Six cases cover the
C and French streams, disabled debugging, output spooling, and native
callbacks that do not request a Scheme diagnostic.
