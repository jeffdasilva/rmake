# Export-list bindings

GNU Make 4.4.1 name-list directives change variable definitions as well as child
environment policy. For an undefined name, both `export X` and `unexport X`
create an empty, simply expanded variable with file origin. A subsequent
`X ?= fallback` therefore leaves it empty, while `X += expression` expands its
new value immediately. Original fixtures cover metadata, effects and child
environment observations separately.

The implementation creates an absent binding only in the name-list path. It
expands the list once, splits the resulting names, and keeps each name literal
when inserting the empty binding. It preserves every existing binding's value,
flavor, origin and source context. Assignment-form directives such as
`export X = value` continue through ordinary assignment handling.

Exporting the new empty variable gives the
child a present but empty value; unexporting it leaves the child name absent.
Tests distinguish these states and check that later variable operations retain
their GNU timing. A literal name such as `Y=value` also matches GNU's raw
environment entry `Y=value=`, which the child reads as `Y` with value `value=`.
The existing process launcher handles the tested cases without a serialization
change. No cache or deferred execution shortcut is involved.

Only a syntactically empty `export` or `unexport` directive changes global
export-all mode. A nonempty list that expands to no names leaves that mode
unchanged, while retaining any effects of its expansion. Expanded lists split
on all six C whitespace bytes, including vertical tab; non-ASCII space-like
bytes remain literal name contents. These rules are local to name-list handling
and do not establish complete whitespace compatibility for other directives.

## Binding lifetime

Export policy belongs to a binding. Imported environment bindings start with an
export policy, and reassignment retains it. Internal literal initialization and
metadata appends retain it too, including updates to `CURDIR`, `MAKEFILE_LIST`
and `MAKELEVEL`. Synthesizing a previously absent variable does not create an
inherited export policy.

A successful `undefine` removes the binding and its explicit export/unexport
membership. A later definition starts with default policy: an ordinary file
binding is absent from the child environment unless export-all or a new explicit
directive selects it. Refused undefines retain the old policy, including when
command-line priority or `-e` protects the binding. Explicit target and pattern
policies survive removal of an unrelated global binding.

The engine still visits startup environment names to remove stale entries from
the child environment. Membership in that name list alone does not select a new
binding for export. Default selection checks the current origin and export-all
mode, preserving ordinary export expansion and its effects.

## Remaining boundaries

Export behavior is still incomplete. Pending original fixtures retain these
distinct gaps:

- A vertical tab immediately after the directive keyword is not yet recognized
  as a directive separator. This differs from whitespace inside an expanded list
  or after an already recognized keyword separator.
- Modifier combinations and ordering of effects from multiple exported
  variables require additional compatibility work.

The harness keeps metadata-only fixtures separate from recipe-launch fixtures;
it does not remove child execution from an existing failing case to make it pass.
Passing this slice does not establish full export or environment compatibility.

The initial name-list behaviors and tests came from original executable probes
against the pinned oracle, without inspecting upstream implementation or tests.
The binding-lifetime follow-up also consulted GNU Make 4.4.1 `src/main.c`
environment import and `src/variable.c` (`define_variable_in_set`,
`undefine_variable_in_set`, `define_automatic_variables`, `should_export` and
`target_environment`). Rust changes and fixture inputs are independently
authored; no upstream code or tests were incorporated. See
[implementation progress](progress.md) for exact validation counts and remaining
milestone gates.
