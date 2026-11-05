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

Run `tools/resource_fd_pipe.py --candidate PATH --report PATH`, or use
`--oracle-only`. The focused case is `borrowed-wire-fd6`. Exact streams,
status, fixture filesystem and containment are checked against two verified
GNU runs.
