# Reader-side native load preparation

GNU Make 4.4.1 `load` and `-load` now finish the pending rule, expand their
entire operand, and accept an empty filename list. Expansion effects, errors,
undefined-variable warnings and active source locations use the ordinary full
database, including eval, includes and recipe-time eval.

Physical filename preparation shares the rule filename implementation with a
separate archive policy. Module parentheses remain setup-symbol syntax;
archive groups and archive-member glob lookup do not apply. Blank quoting,
leading dot-prefix removal, tilde expansion, literal bytes and complete-name
glob results retain their existing behavior. Preparation of every filename
finishes before the first module is examined. An empty explicit setup symbol
is fatal even for `-load`, with the exact prepared filename bytes.

The initial reader-side change is now followed by the [Linux native runtime](native-modules.md), which loads modules and implements registration, expansion/eval, metadata and qualified lifecycle behavior. The evidence below records the earlier reader slice at its original point in time. Complete M3/M6 acceptance remains open.

## Original models and retained discoveries

324 original cases cover both directive forms, four entry phases, separate and
target-synchronized output, lazy/nested effects, mutable bindings, raw filename
bytes, globbed names containing blanks, quoting, NUL returns, tilde preparation,
error priority, warnings, comments and pending rule boundaries. Each case has
independent expected stdout/stderr, exit status and ordered `seen` contents.
Both bounded GNU observations and both selected frozen builds match all models,
with no lingering processes. The preceding build differs on 312 cases. No new
safe-error exception is needed.

The first 160 stream models had sixteen wrong dot-prefix predictions. GNU strips
leading `./` here despite a nearby source comment suggesting otherwise. A second
296-case set had correct stream/status predictions but 116 wrong filesystem
witnesses: exported HOME is expanded after recipe expansion when preparing the
child environment. The original predictions and observations remain unchanged
in the discovery archive. The final models preserve that event and distinguish
it from operand-phase HOME expansion. Neither historical set receives additional
fixture credit. No GNU implementation, header or test was copied or translated.

Three Rust tests protect distinct filename policies, full glob preparation,
byte-preserving errors and pending-rule completion. Four tooling tests verify
fixture/ledger/archive identity, every model and ordered witness, retained
incorrect hypotheses, bounds and deliberately corrupted observations. All flow
through the existing top-level Makefile and GitHub development/optimized gates.

No elapsed speedup or general performance parity is claimed. The shared filename
policy retains borrowed ordinary names and prepares owned names only for the
existing tilde/glob/archive paths. Persistent cache correctness and full M3/M6
acceptance remain open.
