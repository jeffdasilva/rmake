# Signal masks through evaluation and execution

The launcher starts make with a known mask. An original process-entry
constructor and native module record SIGUSR2, SIGCHLD, and SIGTERM in that
order. The gate checks the inherited mask during eager evaluation, the
reader's mask through deferred evaluation and finalizers, empty masks in
shell and recipe children, and the live mask at real self-exec. One case
blocks SIGUSR2 in a native callback and checks that spawning a child leaves
the parent's live mask intact. Two limit-4 child cases independently observe
SIGCHLD with default and ignored SIGPIPE.

Explicit help and version cases send stdout to `/dev/null` through a fixture
callback. Their exact exit, stderr, and ordered native observations still
show whether reader startup ran. Fatal cases before and after the reader
transition check that native finalizers observe the mask from their phase.

Run `tools/signal_mask.py --candidate PATH --report PATH`, or use
`--oracle-only`. The oracle executes twice with the same verified input.
The gate checks exact streams, exit status, full filesystem and containment.
The focused case is `eager-to-read-signal-mask`.
