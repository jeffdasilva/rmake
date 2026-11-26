# Use the optional Guile frontend

The default Linux build leaves Guile disabled. The optional Cargo `guile` feature
supports the selected GNU Make 4.4.1 Scheme interfaces and callback paths
described below, using a verified Guile SDK. rmake never calls GNU Make as a
runtime fallback.

## Build and test

From the repository root, run the application and component gates.

```sh
make test-guile-frontend
make test-guile-components
```

The frontend target bootstraps the pinned Rust toolchain, private Guile SDK and
Guile-enabled GNU Make 4.4.1 reference. It then builds separate debug and release
binaries with `--features guile --locked`, runs workspace tests and Clippy,
compares both builds with the reference, and runs `build check-core` through each
enabled executable. The ordinary compatibility gates use the default profile.
CI requires the frontend gate and the separate component safety gate.

The [SDK lock](../oracle/guile/toolchain.json) pins Ubuntu 24.04 x86-64 packages.
Bootstrap verifies their provenance and extracts them into a private directory.
It does not install system packages or run package maintainer scripts. Other
platform and resource profiles need separate validation.

The [AArch64 GNU reference workflow](../.github/workflows/guile-aarch64-reference.yml)
uses a separately [locked ARM SDK](../oracle/guile/aarch64-toolchain.json).
On native Ubuntu 24.04 it builds the pinned GNU Make 4.4.1 reference, checks
the Guile feature and records a Scheme scalar result. The optional rmake
frontend and component qualification use the x86-64 SDK.

With the default Makefile settings, the enabled binaries are
`artifacts/guile-frontend/cargo-target/debug/rmake` and
`artifacts/guile-frontend/cargo-target/release/rmake`. The gate prints a fresh
`artifacts/guile-frontend/run-*` directory containing reports and copies of the
tested binaries. `GUILE_TOOLCHAIN_ROOT` and `GUILE_FRONTEND_OUTPUT` select the SDK
and output roots. `make bootstrap-guile` prepares the SDK and enabled reference
without building the application.

Use the verified profile wrapper to run an enabled binary with its library and
Scheme load paths. For example, after the frontend gate builds the binary, run a
Makefile in the current directory with the default paths.

```sh
python3 tools/guile_profile_exec.py \
  --profile .tools/guile/reference-profile.json -- \
  artifacts/guile-frontend/cargo-target/debug/rmake
```

The [gate](../tools/guile_frontend_gate.py) keeps the Guile Cargo output separate
from the default build. Preserve that separation when changing the build flow.

## Supported interfaces

The optional frontend shares the interpreter, functions and variable scopes
used by ordinary Makefiles. `$(guile ...)` evaluates Scheme, and the `(gnu make)`
module exposes `gmk-expand`, `gmk-eval` and `gmk-var`. The
[GNU Make manual](https://www.gnu.org/software/make/manual/make.html) describes
the extension interface. The
[Guile reference manual](https://www.gnu.org/software/guile/manual/guile.pdf)
documents Scheme and the native API. The pinned 4.4.1 observations define this
project's compatibility expectations.

Public Scheme source is decoded as UTF-8 under both the C and C.UTF-8 locales.
The [valid-source models](../tests/guile-frontend/valid-text.json) distinguish
source decoding from result conversion. They also require `gmk-eval` to return
Scheme false after publishing its Makefile effect, as observed in the pinned
reference.

The conversion and module tests preserve several observable details.

| Area | Behavior covered by the tests |
| --- | --- |
| Lists | Conversion uses the module's live `pair?`, `car`, and `cdr` bindings. The iterative converter calls the predicate once at each visit and reads each tail after converting its head. Tests cover binding changes, shared pairs, and temporary links created by predicates, accessors, and scalar converters. Stable native pair links retain cycle detection. |
| Conversion procedures | Conversion selects the module's `string-join` procedure before calling `reverse!`. A local shadow introduced after the first lookup leaves the resolved import in use. An assignment to that binding takes effect on the next conversion. Scalar and empty results have controls too. |
| `gmk-var` | The public binding takes one argument. It selects the module's expansion and formatting procedures before converting the variable name. Rebinding during conversion affects the next call. |
| Unsupported scalar values | The first conversion resolves the module's `error` procedure. Tests cover definitions before lookup, later shadowing, changes to a resolved binding, and effects through scalar returns and each public bridge. |

The [suite manifest](../tests/guile-frontend/suites.json) lists the exact models
and their source hashes. Public bridge controls also cover shell status in the
reader, a `foreach` scope and a target recipe. Missing-file diagnostics are
checked during target output synchronization in the reader and a serialized
recipe.

## Callbacks and process waits

Worker callbacks are covered at specific waits in the application. The tests
observe live child processes and the waiting interpreter before releasing a
worker, then require its effects to appear at the expected point.

| Wait path | Current coverage and evidence |
| --- | --- |
| Serial recipes | [Detached worker tests](../tests/guile-detached/README.md) cover an idle worker, a callback during later Scheme evaluation, and expansion, evaluation and nested Scheme entry during a recipe wait. The waiting thread handles accepted work before closing its callback admission window. |
| Parallel recipes | [Detached parallel tests](../tests/guile-parallel-detached/README.md) cover pipe and FIFO jobservers, draining children, waiting for a token and consecutive worker requests. |
| Shell capture | [Shell capture tests](../tests/guile-shell-capture/README.md) cover workers reading the active lexical scope or publishing an evaluation effect before capture returns. Capture waits for stdout and worker notifications. Nested captures retain the outer wait's admission window and descriptor. |
| Serial descriptor pressure | [Late descriptor pressure tests](../tests/guile-eventfd-pressure/README.md) cover a missing private wake descriptor after a recipe starts. The main thread continues accepting callbacks and checks the queue at bounded intervals. The regular descriptor wake path has its own control. |
| Parallel descriptor pressure | [Parallel pressure tests](../tests/guile-parallel-pressure/README.md) cover pipe and FIFO drain and token waits. Polling preserves callback admission across requests while actual child and token readiness advances the scheduler, including a control with unavailable pidfds. |

The [named stdin restart model](../tests/guile-frontend/stdin.json) requires both
Makefile reads, cleanup of the owned source and the final goal effect. Scheme
port output from the first process image remains buffered at exec and is
discarded. Only the second image emits its line.

## Runtime and ABI ownership

The [thin executable](../crates/rmake/src/main.rs) owns privileged startup and
the native entry boundary. The interpreter lives in
[`rmake-app`](../crates/rmake-app/src/lib.rs), which forbids unsafe Rust, and
portable expansion remains in `rmake-core`.

The [Linux adapter](../crates/rmake-linux/src/guile.rs) and its C code own Scheme
values. Native calls run on the correct thread's original C stack so Guile can
trace its roots. Rust evaluation uses a guarded private stack. Joined worker
calls transfer thread state and return owned messages. Scheme values stay on
the C side of the boundary.

Decoder, reader, signal, standard-stream and native-finalizer lifetimes remain
explicit. Output and process owners stay alive through native shutdown. The
[component design and safety evidence](guile-component.md) describe the lower
level boundary that the application gate exercises.

## Read the evidence

Both GNU runs and each enabled rmake build must independently match the models'
literal streams, exit status, ordered effects, complete filesystem and process
bounds. Startup phase, HOME and signal models accompany the language and native
integration models. The signal controls also run against the default binary.

The frontend runner can select a suite or case for a focused comparison after
the full gate has built the enabled binary. Use a fresh report path.

```sh
python3 tools/guile_frontend.py \
  --profile .tools/guile/reference-profile.json \
  --candidate artifacts/guile-frontend/cargo-target/debug/rmake \
  --suite valid-text \
  --report artifacts/guile-frontend/valid-text-review.json
```

Use `--case` with an ID from the selected model to narrow the comparison. The
runner refuses to overwrite an existing report. It retains a new process or
resource boundary failure as evidence and stops without replaying that input.

The [original qualification record](../tests/guile-frontend/qualification-original.json)
retains model qualification, development binary identities and negative
observations. The [first integrated stage](../tests/guile-frontend/qualification-staged.json)
retains its comparison and self-host results. These historical records apply to
their recorded binaries. Validate a new source revision with the current gate.

## What remains open

The accepted wait paths leave several areas open.

- Arbitrary asynchronous native entry points and detached callbacks outside the
  tested admission windows.
- The fatal callback race where the reference produces variable wait-error
  output, and broader asynchronous and parallel lifecycle behavior.
- Full error backtraces and resource profiles beyond the selected models.
- Guile behavior outside the covered language, bridge and process cases.

These results do not close M3 or M6 or establish general performance parity.
The [compatibility audit](m3-m6-audit.md#native-modules-and-guile) tracks the
remaining milestone requirements.
