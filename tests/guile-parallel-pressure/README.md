# Guile workers during parallel descriptor pressure

The six original Linux x86-64 cases cover pipe and FIFO jobservers during
recipe drain and token reservation. The observer verifies that the same two
real recipe processes own their slots at both worker releases. Each recipe
checks the reduced descriptor limit inherited after Guile initialization.

The pressure witness fills its own descriptor slots during the real
`eventfd2` call, after both recipes record successful exec. It verifies
`EMFILE` and closes only its own descriptors. A separate trace detects any
attempt to rearm the wait before the second worker callback completes. One
FIFO reservation profile uses a child-only seccomp filter and verifies that
`pidfd_open` returns `ENOSYS`. The scheduler must still reap the actual owned
PIDs after the callbacks finish.

Run `tools/guile_parallel_pressure.py` with a verified enabled `--profile`,
an optional enabled `--candidate`, and a fresh `--report`. The gate compares
both GNU repeats and the candidate independently against the declared
streams, status, complete filesystem and process bounds. The existing
parallel observer checks the actual jobserver descriptor or FIFO identity.
