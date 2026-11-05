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

In the initial bounded observations, GNU used reader descriptors 4, 6, 8 and 4
across the four phases. The fixed candidate used 4, 8, 10 and 4 because its wake
pipe occupies descriptors 5 and 6. The gate retains actual descriptor numbers in
generated reports and asserts ownership relations within each process. It does
not compare absolute descriptor numbers across engines.

Wire the runner into the existing top-level capture targets. Keep the existing
EOF runner in both targets.

- Add the following to `test-shell-capture-lifetime`.

  ```make
  $(PYTHON) tools/shell_capture_launch_failure.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --report artifacts/shell-capture/launch-failure-development.json
  ```

- Add the following to `test-shell-capture-lifetime-release`.

  ```make
  $(PYTHON) tools/shell_capture_launch_failure.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --report artifacts/shell-capture/launch-failure-optimized.json
  ```

The existing targets already feed `test`, `check`, and `test-m4-release`.
