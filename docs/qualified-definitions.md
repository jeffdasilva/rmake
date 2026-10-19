# Qualified definitions and header preparation

Global assignment modifiers also apply to `define`. `override`, `private`,
`export` and `unexport` retain their normal precedence and final export policy.
Private global definitions remain visible during parsing and are hidden from
target recipes and their environments. Conditional definitions can retain an
existing imported/command binding; modifiers then apply to that selected binding.
A conditional import retained under `-e` can keep environment origin because
the conditional does not reach the definition-collision promotion path.

Assignments retain priority after modifiers too: `override define := one` assigns
the variable named `define`, and `export undefine := one` assigns `undefine`.
Only an actual definition header opens body reading. A modifier chain that has
no variable operation still falls back to the original statement.

Active header processing happens before reading any body bytes:

1. Close the preceding rule and record its warnings.
2. Recognize the header operator. Nonempty text after it emits an extra-text
   warning and is otherwise ignored, without expansion.
3. Expand the name once, skip leading C whitespace and remove trailing space/tab
   bytes. Internal whitespace and other trailing bytes remain name data.
4. Reject an empty name before any body reads or body diagnostics.

After collecting the body, store it under the prepared name without a second name
expansion. Immediate body evaluation keeps its closing-line caller context and
header definition provenance. Recursive body effects remain deferred. Supported
header operators use existing recursive/simple, append, conditional and immediate
recursive semantics; shell assignments remain unsupported.

Ignored qualified definitions skip header warnings and name/value effects and use
the previously established ignored-definition body rules. Binding export and
private policy are applied only for active definitions. Normal precedence and
imported-policy lifetime remain in the shared variable store.

Sixty original fixtures cover modifier combinations, origins, export/private
behavior, header/body warning order, empty/error names, whitespace and keyword
assignment priority. A separate 336-program matrix across twelve modifier chains,
seven header forms and four startup-origin modes resolves 308 differences against
the preceding binary. All current exact stream/status comparisons match. The
2,380-program modifier fallback matrix also remains exact.

GNU Make 4.4.1 `src/read.c` (`parse_var_assignment`, assignment handling in `eval`,
`do_define`) was consulted for modifiers, name preparation, header warnings and
definition/evaluation positions. The Rust implementation and fixtures are
independently authored; no upstream source text or tests were copied. Shell/eval
effects, generated inputs and broader reader/runtime compatibility remain open.
