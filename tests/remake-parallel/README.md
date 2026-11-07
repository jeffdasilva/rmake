# Parallel makefile remakes

These 30 original profiles hold real recipe children until the harness observes
the make process waiting. Each release records the live children, their parent
PID and start times, and the file effects already visible. Both named workers
must be held at the first release in a parallel profile. The second release
must observe the same right worker. The serial controls require one live
worker at each release.

The success profiles compare independent required and optional include
remakes, serial `-j1`, and global `.NOTPARALLEL`. Both generated makefiles must
be loaded after one restart. The failure profiles hold two prerequisites of one
included makefile and compare the exact diagnostics, output, and order of
ordinary goal effects. Separate mixed-root profiles distinguish the child’s
error policy from the traversal’s current source-open diagnostic and cover
declaration order and a successful restart after an optional failure. Each
profile runs with the pipe and FIFO jobserver.

Run the standalone gate after bootstrapping the pinned GNU Make oracle.

```sh
python3 tools/remake_parallel.py --self-test
python3 tools/remake_parallel.py --candidate target/debug/rmake --report /tmp/remake-parallel.json
```

The self-test requires two exact GNU observations and independent models for
the exit status, streams, filesystem, and release witnesses. The candidate
must match those requirements as well. To share the pinned oracle with a
sparse source checkout, pass its executable with `--oracle` and its profile
with `--oracle-profile`.

This gate stays outside `make check`, release checks, self-hosting, and CI
while parallel makefile remakes are incomplete. The merged debug baseline
`d1171e8f1` passes the four serial controls and fails all twenty-six parallel
profiles. The observer releases a single genuinely held child when the engine
waits serially so those mismatches can be diagnosed without timing out.

## Duplicate reads

The standalone duplicate-read suite requires independent recipe overlap even
when two include directives name the same file. It tests which read occurrence
selects the source error, whether an optional failure becomes visible at a
later required visit, and whether a successful remake leaves both read
occurrences visible after restart. Both jobserver transports are covered.

~~~sh
python3 tools/remake_parallel.py --duplicate-reads --self-test
python3 tools/remake_parallel.py --duplicate-reads --candidate target/debug/rmake
~~~

The selector runs the 20 duplicate-read profiles separately. The default
continues to run the 30 ordinary profiles. The prototype at 0dc03fcaf
serialized the workers in all duplicate-read cases and matched the final
ordinary observations in ten. The overlap checks detected all twenty.
