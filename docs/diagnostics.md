# Read and check diagnostics

rmake targets GNU Make 4.4.1's diagnostic bytes, source locations, output order
and exit behavior. This guide explains the covered expansion rules and how to
check a difference. The [GNU error-message reference](https://www.gnu.org/software/make/manual/html_node/Error-Messages.html)
explains common messages such as missing separators, missing rules and failed
recipes. The [compatibility guide](compatibility.md) defines the reference profile
and permitted differences.

## Read a message

A diagnostic can identify a Makefile and line, or use the invocation name when
there is no source location. The line can refer to a variable definition or to
where its value was expanded. That choice depends on the kind of message.

| Message | Meaning |
| --- | --- |
| `$(info text)` | Prints the expanded text to stdout without a source or invocation prefix. Evaluation continues. |
| `$(warning text)` | Prints the expanded text to stderr with the active diagnostic context. Evaluation continues. |
| `$(error text)` | Stops expansion with the expanded text as its message. |
| An expansion validation error | Reports invalid syntax, arguments or another expansion failure. A recursive variable's definition can supply the location. |
| A recipe `Error N` report | Reports a child process's failure status. The command or shell may have printed its own diagnostic first. |

The [GNU control-function documentation](https://www.gnu.org/software/make/manual/html_node/Make-Control-Functions.html)
and the [rmake function guide](diagnostic-functions.md) describe `info`,
`warning` and their evaluation timing. An [explicit expansion error](fatal-errors.md)
stops new work even with `-k`. The `-i` option, recipe `-` prefix and `.IGNORE`
do not suppress it. Already completed work remains, and running children follow
the [fatal shutdown rules](fatal-continuations.md).

A missing executable can produce a launcher diagnostic or a shell diagnostic,
depending on how the command was prepared. `SHELL`, `.SHELLFLAGS`, quoting and
`.ONESHELL` affect that path. Preserve the original command and configuration
when investigating it. See [shell configuration](shell-configuration.md).

## Check a diagnostic

From the repository root, run the message matrix or replay one cell.

```sh
make test-diagnostics
make test-diagnostics DIAGNOSTIC_CASE=message-204-child-output-order
```

The target builds rmake and verifies the pinned reference. It checks the oracle
against the independent expectations, then compares the candidate. Reports and
failure inputs are written under `artifacts/diagnostic-matrix`. The runner prints
the campaign report path, and failures include raw captures and replay metadata.

For a curated source-location regression, use the ordinary runner after building
the candidate.

```sh
make build
python3 tools/conformance.py --candidate target/debug/rmake \
  --case location-recursive-immediate
```

The [fixture](../tests/cases/location-recursive-immediate/case.toml) records its
arguments and exact expectations. Other useful checks cover adjacent behavior.

| Command | Coverage |
| --- | --- |
| `make test-intcmp` | Integer validation, message bytes and expression context. |
| `make test-output-sync` | Output grouping, stream arrangements and child ordering. |
| `make test-nls` | Selected localized system diagnostics. Use `NLS_CASE` to select a case. |
| `make test-nls-release` | The same localized checks with the optimized binary. |
| `make test-diff` | The ordinary compatibility corpus, including focused diagnostic regressions. |

See the [testing guide](testing.md) for the full checks and instructions for
adding a regression case.

## Definition and evaluation locations

A validation failure while expanding a recursive variable uses that binding's
definition location. Nested references and user `call` bodies use the innermost
failing definition. A successful nested expansion restores the enclosing
context. A later error in the outer body therefore keeps the outer location.
Call arguments expand before entering the called body.

Command-line, environment, automatic and temporary parameter bindings have no
Makefile definition location. A validation failure in one of these bindings
inherits an enclosing definition or the calling parser or recipe location.
Referencing an already-expanded simple value adds no new definition context.

Assignment timing also affects the stored location.

| Assignment or definition | Location rule |
| --- | --- |
| Accepted redefinition | Replaces the stored location. |
| Nonempty append | Moves the whole resulting binding to the append's location, including an error in older text. |
| Empty append | Preserves the existing location and origin. |
| Skipped conditional assignment or assignment suppressed by origin precedence | Preserves the existing binding's metadata. Immediate right-hand sides still expand at their required time, even if their result is discarded. |
| Recursive `define` | Stores the definition header's location. |
| Immediate `define` | A direct body-expansion failure uses the closing `endef` location. A nested named variable can supply its own definition location. |
| Expanded definition name | Uses the definition header's context. |
| Ordinary continued assignment or recipe | Uses the logical line's starting location. |
| Detected variable cycle | Reports the definition of the binding being entered again. |

Explicit `error` and `warning` use the calling parser or recipe context, including
through recursive variables and user calls. Without a caller, the first entered
recursive binding establishes their context through nested references. A
command-line or environment binding can establish an explicitly absent location.
Numeric validation errors still select the innermost failing definition.

## Runtime expansion boundaries

Recipe bodies, shell configuration and exported bindings expand at different
points. Dry-run, query and touch modes can reach different expansion paths.
A stored message is emitted only when its expression is evaluated.

| Runtime value | Diagnostic context |
| --- | --- |
| Recipe `SHELL` and `IFS` | Validation failures use the normal variable-definition context. Explicit errors and warnings use the recipe context. |
| Exported recursive variable | Validation uses the normal definition context. An explicit error or warning uses the outer exported binding's location, which can be absent. |
| `.SHELLFLAGS` | Expands its stored text without entering its own definition context, even for a simple value. Nested references can establish their own locations. |
| Recursive `.DEFAULT_GOAL` | Expands its stored body without entering its own definition context. There is no parser or recipe caller. The first nested recursive binding establishes the explicit-error or warning context, while validation failures use the innermost definition. |

The [shell guide](shell-configuration.md) records configuration order and the
undefined-variable warning policy. The [fatal-error guide](fatal-errors.md)
records export timing, raw imported environment values and mode-dependent
expansion. These boundaries require separate tests.

## Output bytes and ordering

Makefile paths, variable names, invalid integer operands and user messages retain
their original bytes, including invalid UTF-8. A display-friendly rendering is
not suitable for comparing compatibility output. Preserve trailing whitespace
and embedded newlines in a report.

Parser warnings emit in sequence with user messages. Completed output survives
a later expansion failure. A partially expanded outer message is not emitted.
Tests with separate stdout and stderr check each stream. Tests with a shared
descriptor also check ordering between messages, command echoes and child output.

[Output synchronization](output-sync.md) controls when buffered recipe output
appears. Its line, target and recurse modes have different grouping rules.
[Localized diagnostics](nls-diagnostics.md) keep system templates separate from
user and native text. The active locale and text domain can affect a system
message while its filename and user-supplied byte arguments remain intact.

## Contributor implementation notes

The [core diagnostic types](../crates/rmake-core/src/diagnostic.rs) keep messages,
source locations and output events structured. Variable expansion attaches the
innermost definition to validation errors. Callers supply evaluation context,
and the error kind determines whether it takes precedence or provides a
fallback. This decision must not depend on parsing rendered message text.

The [application renderer](../crates/rmake-app/src/engine.rs) receives typed
output events. Definition fallback is local to an expansion and restored after
nested calls. Variable-store clones do not share a mutable output buffer.
Completed output remains an effect even if expansion fails. A future cache must
preserve its occurrence and order. The [diagnostic-function guide](diagnostic-functions.md#internal-effect-interface)
describes the output receiver and pure APIs in more detail.

## Report a difference

1. Keep the smallest Makefile and input files that reproduce the behavior.
   Preserve filename bytes, line endings, tabs and any earlier effects needed to
   reach the failure.
2. Record the rmake revision, candidate and reference identities, exact arguments,
   working directory and relevant environment. Include locale, `MAKELEVEL`,
   `MAKEFLAGS`, `GNUMAKEFLAGS`, shell configuration and output-sync settings when
   they affect the case.
3. Keep raw stdout, stderr, exit status and filesystem effects. Include the
   harness report and saved input when available. Use merged output only when
   the test is intended to observe a shared descriptor.
4. Reproduce through the pinned harness with matching invocation aliases and
   workspace paths. If repeated GNU runs differ, retain both observations and
   investigate the ordering before making the case required.

The [fixture format](../tests/README.md) supports explicit byte expectations and
records provenance. Do not remove prefixes, rewrite paths, sort messages or
replace invalid bytes to make a comparison pass. The test must retain the
observable difference it is intended to cover.

## Coverage and remaining work

The original variable-location implementation and fixtures were independently
authored from executable observations and public variable/function documentation.
GNU implementation source and upstream tests were not consulted for that work.
The linked runtime, shell and locale guides record their own provenance.

Coverage includes parsing and recipes, recursive and call chains, multiline
definitions, append and precedence, runtime configuration, multiple Makefiles
and non-UTF-8 filenames. Original generated probes also vary line padding,
chain depth, source order and evaluation context. These cases establish their
selected behavior and profiles.

The [M3/M6 audit](m3-m6-audit.md#diagnostics-and-locales) tracks broader diagnostic,
locale, native and Guile interactions, including asynchronous work, restarts,
live options and resource failures. Includes, `eval` and target/pattern scopes
need interaction coverage alongside these rules as the implementation evolves.
An unsupported-feature report or a reached resource bound still requires
assessment against the reference and the documented scope. The separate
[approved safety exceptions](safety-exceptions.md) define the permitted
departures. Passing the diagnostic tests does not establish complete diagnostic
or language compatibility.
