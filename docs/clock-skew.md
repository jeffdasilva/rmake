# Future timestamps and clock skew

GNU Make 4.4.1 warns when a graph timestamp is ahead of the realtime clock.
The first warning names the file and reports the offset. The invocation keeps
its last observed clock limit, so ordinary timestamps below that limit need
no further clock reads. A read failure can fall back from nanosecond to
microsecond or whole-second precision.

The initial warning can occur while remaking an included makefile. A successful
restart begins a new observation window. The summary follows normal goal
completion, including errors continued under `-k`, and precedes intermediate
cleanup. A fatal exit or a stopping build error can leave just the initial
warning.

`make test-clock-skew` runs original Makefiles with fixed file timestamps and a
Linux clock interposer. Its anchors cover raw output order, exit status,
filesystem effects, warning suppression, archive containers and members,
VPATH/GPATH, recipe-created files, restart, fallback precision and clock reads.
The optimized suite also runs through `make test-clock-skew-release`.
