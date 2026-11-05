# FIFO jobserver descriptor pressure

These original Linux cases compare GNU Make 4.4.1 and rmake at soft and
hard descriptor limits from five through twelve. They check exact streams,
status, final filesystem and bounded process lifetime. The FIFO keeper
compares physical identity and observes live token capacity without
consuming the token while either recipe is held.

An owner and its recursive make both open the same named pool. Their
recipes verify its mode, queued token, and that no private pool descriptor
survives exec. The inherited-descriptor case checks that the application
supplied descriptor still refers to the same file, with its original
offset, while private jobserver descriptors remain closed.

At limit five, an owned pool can run a recipe after reading its Makefile.
A recursive explicit makefile fails during open. A borrowed pool has
already taken its two descriptors at default discovery, so make cannot
read the directory and reports that no makefile was found.

At limit four, GNU reports `init jobserver pipe: Bad file descriptor`
after creating its FIFO and failing to acquire the writer. Both engines
retain the owned empty mode-0600 artifact on this prepublication failure.
The observer identifies the exact child PID, verifies the FIFO owner and
physical identity, confirms the child was reaped, and probes for surviving
endpoints without blocking.

`make test-resource-fd-fifo-failure` and
`make test-resource-fd-fifo-failure-release` run this separate exact case
in C and in the verified French locale. The retention branch requires a
freshly created canonical name, process descriptor exhaustion at writer
acquisition, matching FIFO metadata, and a directory protected against
unrelated replacement. The reader stays owned through error formatting.
At that point no descriptor is available to open a cold catalog, so both
engines emit the English fallback. The reader closes before reporting
completes. No authentication or token was published.
