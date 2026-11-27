# Startup output mutex completion

The C-locale profiles check when make opens the output mutex while it decodes
the initial GNUMAKEFLAGS, MAKEFLAGS, and explicit arguments. A missing lock
stops later sources after the current source has finished its eager effects.
A nonempty source reopens the retained path. An empty inherited source keeps
its expansion effects and defers the reopen to the next completion. Invalid
authentication strings produce a diagnostic at each applicable completion.

The valid fixture uses a shell observer to verify the actual mutex file
identity, write access, and close-on-exec status before later effects.
Relative paths on the explicit command line are opened before changing
directories. The other controls check eager buffered output and an open
failure that occurs before the help branch. All profiles check literal
streams, status, and the complete fixture filesystem. Two explicit version
failures also require the separately calibrated banner at its declared position
in direct or spooled startup output.

The inherited-output profiles inspect the Make parent and recipe descriptors.
They check pipe and device flags, existing append and nonblocking flags, and
append writes after a seek on inherited regular files. The synchronized
control separately checks the recipe spool and inherited pipe flags.

The closed-output profiles keep stdin open and check direct recipe inheritance,
line, target and recursive synchronization, and explicit output redirection.
A recipe write to a closed stdout must fail while the open stderr still drains.
With both outputs closed, the named mutex is removed before a silent recipe
expands its exported environment. Its `MAKEFLAGS` entry remains visible.

Run `make test-startup-output-mutex` or
`make test-startup-output-mutex-release`. The existing live and passive
output-lock suites retain their own fixtures.
