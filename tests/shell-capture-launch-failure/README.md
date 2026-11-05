# Shell capture launch failures

This fixture attempts one missing executable through the shell function and one
nonexecutable file through a shell assignment. A bounded direct child pauses at
four phases so the runner can inspect the live interpreter and child in procfs.
The existing process-restart constructor records two actual executable entries
without relying on address randomization. The gate checks literal streams, all
three shell statuses, the complete temporary filesystem, and process reaping.

Each failed capture adds one anonymous pipe with two blocking close-on-exec
ends. The ends retain the same inode and access modes through subsequent
captures, and neither observer children nor the replacement process inherit
them. The gate permits the candidate's independently observed private
nonblocking wake pipe and checks that its ownership stays stable before exec.

The relation-only gate records actual descriptor numbers without comparing
their absolute values. In the initial bounded observations, GNU used readers
4, 6, 8 and 4, while rmake with the older wake pipe at 5 and 6 used 4, 8, 10 and
4. The explicit roomy glibc profile checks readers 4, 6, 8 and 4 and retained
pairs 4/5 and 6/7 on both engines. This profile requires a hard descriptor limit
of at least 64 and runs through `make test-roomy-wake-slots`. The checks cover
these capture slots and preserve raw reports of private descriptors.

The relation-only runner and the EOF controls run together in
`test-shell-capture-lifetime` and `test-shell-capture-lifetime-release`.
The roomy gate has separate development and optimized targets.
