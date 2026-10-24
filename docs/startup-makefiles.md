# MAKEFILES startup loading

Before selecting the main Makefile, expand MAKEFILES once and read its whitespace
separated names in order. This list does not use include-directive filename
escaping or globbing. Missing entries are optional. Existing files share the
database and include-search reader; successful reads append their resolved names
to MAKEFILE_LIST. Changing MAKEFILES in a preload does not change the already
expanded list for that read pass.

Ordinary declarations in these preloads and their includes do not select the
default goal. Explicit .DEFAULT_GOAL assignments still work. GNU eval buffers
can select a default even within a preload, so the core reader saves and restores
the suppression state around eval. This reader policy is separate from rule
creation, variable definition and expansion effects.

Default main-file discovery happens after preloads. A preload can create a main
Makefile while being read, or define a rule that generates a missing default
Makefile during remaking. Successfully regenerated files trigger normal database
restarts. Preloaded files are optional remake goals: ordinary failed recipes and
missing dependencies are quiet during that update, but fatal expansion errors
remain fatal. Explicitly ignored errors retain their diagnostic unless silent
mode suppresses it. If an optional file is also required, the required use wins. A
later requested goal that failed as an optional remake receives GNU's missing-rule
diagnostic instead of silently retaining the optional failure.

MAKEFILES starts as a default-origin simple binding with export-if-set policy.
A later file or target assignment exports it to child recipes and shell effects;
the untouched default is not exported. Policy evaluation must not freeze a
default binding's initial non-exported answer. Explicit unexport overrides the
policy, and undefine destroys it; recreating the binding has ordinary export
behavior. These rules use binding metadata, including inherited scope policy.

The slice adds 52 original required differential cases. They cover ordered and
duplicate reads, list expansion effects, CLI precedence, search paths, source
diagnostics, missing/parse/directory errors, default suppression, eval, recursion,
automatic export, main-file creation, successful and failed generation, duplicate
required uses, -n/-q/-t modes, and -j/-i error-policy interactions.
Expectations pin status and streams; the
independent runner also compares complete filesystem observations. Stateful
multi-invocation tests remain a separate gate.

Full local `make ci` and optimized conformance pass: 2,530 required cases,
12 existing pending mismatches, no unexpected failures. Six restart/export/eval
cases also pass ten fresh-workspace repetitions each against GNU/GNU and
GNU/rmake. A 101-sample paired core comparison against source `a24cca9` measured
tiny startup at 0.832 ms versus 0.831 ms, large no-op at 27.341 ms versus
27.119 ms, and expansion at 4.620 ms versus 4.690 ms. Paired previous/current
ratios are 0.992, 0.989 and 1.010: no material regression is apparent in this
local run. GNU medians are 0.725, 11.266 and 2.765 ms, respectively, and remain
faster. Raw samples are in the ignored
`artifacts/bench/startup-makefiles-core-101.json`; the measurement limitations
in [target context performance](target-context-performance.md) apply.

GNU Make 4.4.1 read.c, main.c and variable.c were consulted for behavior only;
no upstream implementation or test material was copied. Complete restart
environment replay, dynamic MAKEFLAGS changes, GNU's full makefile-error recovery
surface, and existing resource bounds remain compatibility work. This slice does
not complete M3 or the broader compatibility gate.
