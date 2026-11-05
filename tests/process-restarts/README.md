Three original cases qualify makefile remaking that requires a new program image. The prior implementation reread Makefiles inside the existing process when it had neither a loaded native module nor a named stdin source.

| Case | Independent witness |
| --- | --- |
| `process-restart-tempdir` | An invalid `MAKE_TMPDIR` is checked at both process entries, with two exact warnings and one final goal effect. |
| `process-restart-constructor` | An original `LD_PRELOAD` constructor marks each make image before its Makefile read. The marker excludes recipe processes. |
| `process-restart-replaced-image` | A remake atomically replaces the harness's `bin/make` symlink with a script. The script sees the restart environment, and the old program's goal must never run. |

`make test-process-restarts` compares two pinned GNU Make 4.4.1 runs and one candidate. It checks literal stdout and stderr, status, process bounds, and the complete work, home, temporary, and executable-alias trees against declared results. The replacement script changes only the owned alias; both executable hashes are rechecked after comparison.

`history.json` describes the deterministic compressed archive. It retains the original predictions and observations, all three preceding failures, frozen source and executable identities, full individual observations, and the 27,766-case ordinary optimized regression result. Runtime and helper evolution are separate; the archived source closure applies to the explicitly identified private qualification builds.
