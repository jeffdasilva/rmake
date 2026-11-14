# Pipe jobservers under descriptor pressure

These original fixtures cover real top-level and recursive pipe jobs at
descriptor limits 4 through 8, with limit-12 controls. A separate process
creates an inherited pipe and retains the original open file descriptions.
The child witness checks both descriptor numbers, their physical pipe identity,
their read/write status flags, and privacy in ordinary recipes. One case
forces a real makefile restart before launching a recursive child.

Two live recipe cases consume and recover a canonical GNU token. The
controller confirms the token is absent while both recipes are live and
releases them in an observed order. Both a normal and a failing recipe
completion must restore the token and leave no process or temporary resource.
Noncanonical bytes and the separate fatal token loss observed in GNU Make
remain compatibility questions outside this canonical gate.

When private setup descriptors exhaust an inherited table, the owned pipe
can schedule and seed tokens with its two original wire endpoints. Its ordered
launch keeps a live parent wake pipe private. The
`jobserver-owned-inherited-compact-*` conformance fixtures exercise the
fallback with ordinary and recursive recipes.

An owned normal pipe can launch an inherited-stdio recipe through the
same ordered backend when the standard spawn reports descriptor exhaustion.
The retry uses the published wire numbers for recursive recipes. The
`jobserver-owned-normal-launch-pressure-*` fixtures check this boundary.

Run `tools/resource_fd_pipe.py --candidate PATH --report PATH`, or use
`--oracle-only`. The focused case is `borrowed-wire-fd6`. Exact streams,
status, fixture filesystem and containment are checked against two verified
GNU runs.
