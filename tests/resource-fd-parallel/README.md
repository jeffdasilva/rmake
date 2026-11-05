# Parallel job counts under descriptor pressure

These six original Linux profiles run a 12-slot jobserver under descriptor
limits of eight and 64. A compiled C witness checks its actual soft and hard
descriptor limit, writes a ready record, and waits for every recipe before
writing a done record. Ten workers must overlap at the eight-descriptor limit.
The two smaller pipe controls keep the number of live workers at or below eight.

The runner compares two verified GNU Make 4.4.1 runs with independent expected
streams, status and complete final filesystem contents. It then applies the same
checks to rmake. The process group and captured output have fixed bounds.

Linux counts every slot passed to `poll` against the descriptor limit, including
negative placeholders. The preceding candidate could exceed that limit while
tracking jobs that had no pidfd. The scheduler now keeps those child owners in
the bounded PID fallback and passes only real descriptors to the kernel.
