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

## Remaining boundaries

Export behavior is still incomplete. Pending original fixtures retain these
distinct gaps:

- Export/unexport and inherited environment policy can outlive an `undefine`
  in the current implementation. GNU removes the binding's policy too.
- A syntactically empty directive differs from a nonempty list that expands
  empty; the latter must not toggle export-all mode.
- Vertical-tab-separated names introduced by expansion are not yet split like
  GNU's list tokens.
- Modifier combinations and ordering of effects from multiple exported
  variables require additional compatibility work.

The harness keeps metadata-only fixtures separate from recipe-launch fixtures;
it does not remove child execution from an existing failing case to make it pass.
Passing this slice does not establish full export or environment compatibility.

These behaviors and tests come from original executable probes against the
pinned oracle. No GNU implementation source or upstream tests were consulted
for this slice. See [implementation progress](progress.md) for exact validation
counts and the remaining milestone gates.
