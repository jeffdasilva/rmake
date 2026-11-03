# Contextual HOME in runtime include operands

An include option beginning with `~` must expand HOME at that argument's
position. Definitions before and after the option have different effects;
normalization retains the expanded operand for later resets. Repeating a reset
must not repeatedly expand a previously resolved HOME operand.

The CLI parser now owns its argument vector, cursor, option state and end-marker
state in a resumable decoder. Startup callbacks use the same parser. Runtime
streams containing tilde include operands return owned policy, definition and
filename steps before invoking the interpreter. No option-state borrow survives
HOME expansion, shell execution or nested eval/include callbacks. The ordinary
non-tilde runtime path retains its existing replay implementation.

Raw stored-name membership is checked before expansion, preserving ordered
normalization history. Invocation-only switches remain ignored in runtime
sources; their operands must not expand HOME. A suspended decoder restores
invocation ownership on success, cancellation and failure, while retaining
completed option and variable effects. HOME warnings are masked and restored,
including when expansion fails. Post-read sources expand operands without
rebuilding active include search.

The required family has 460 original models across reader, eval, include,
secondary, recipe and post-read GNU sources, simple/recursive HOME, argument
positions, warning policy, raw bytes, parallel output modes, shell/eval/include
callbacks, environment/user-name fallbacks, ignored operands and fatal effects.
Independent raw stream/status and ordered file witnesses agree with two bounded
GNU observations and both selected frozen builds, closing 398 preceding
mismatches. The [manifest](../tests/research/runtime-home-includes.json),
[raw proof](../tests/research/runtime-home-includes.json.gz) and
[discovery archive](../tests/research/runtime-home-discovery.json.gz) retain
input bytes, identities, source and observations. Historical
warning-quoting and missing-environment authoring failures and the first
prototype's sixteen ignored-operand failures remain separate discovery evidence.
No safety exception is added. Full frozen rmake-driven acceptance and exact-head
CI remain merge gates. Full M3/M6 and general performance parity remain open.

Reference study used the pinned GNU Make 4.4.1 `src/main.c` filename-option
processing and `src/read.c` HOME/include construction. No GNU implementation
code or test cases were copied or translated. GNU is an external test oracle.
Broader malformed-value, nested flag-reset, secondary tilde/search construction,
directory/debug/database, native/Guile, exec/profile, integration and persistent
cache acceptance remain open.

[Six elapsed reports and twelve allocation-request profiles](benchmarks/runtime-home-index.md) show mixed timing and unchanged request counts on the existing controls. No general performance gain or parity is claimed.
