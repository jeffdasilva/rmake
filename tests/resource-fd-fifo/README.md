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

An additional research case at limit four recorded GNU's
`init jobserver pipe: Bad file descriptor` failure and its remaining
PID-named FIFO. That difference remains open. rmake currently reports
descriptor exhaustion and cleans its own partially created FIFO. This
gate adds no safety exception and leaves the existing fatal token
recovery checks separate.
