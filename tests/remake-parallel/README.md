# Parallel makefile remakes

These original profiles hold real recipe children until the harness observes
the make process waiting. Each release records the live children, their parent
PID and start times, and the file effects already visible. Both named workers
must be held at the first release in a parallel profile. The second release
must observe the same right worker. The serial controls require one live
worker at each release. On Linux x86-64, the observer also recognizes a
blocking poll syscall when a kernel exposes a different wait-channel symbol.
The immediate launch profiles hold one worker while
another recipe fails to start.

The success profiles compare independent required and optional include
remakes, serial `-j1`, and global `.NOTPARALLEL`. Both generated makefiles must
be loaded after one restart. The failure profiles hold two prerequisites of one
included makefile and compare the exact diagnostics, output, and order of
ordinary goal effects. Separate mixed-root profiles distinguish the child’s
error policy from the traversal’s current source-open diagnostic and cover
declaration order and a successful restart after an optional failure. The
immediate launch profiles cover ignored errors, optional prerequisite ordering,
an explicit wait barrier, and required failure draining. Shared failure profiles
hold the prerequisite first launched by an optional remake and check the
required frame's diagnostic after its completion. Separate, implicit grouped,
explicit grouped, and grouped double-colon targets all exercise this path.
Each parallel profile runs with the pipe and FIFO jobserver.

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

Run `make test-remake-parallel` or `make test-remake-parallel-release`
to use the developer gates. Set `REMAKE_PARALLEL_CASE` to select one profile.
The merged debug baseline `d1171e8f1` passes the four serial controls and
fails all twenty-six parallel profiles. The observer releases a single held
child when the engine waits serially so those mismatches can be diagnosed
without timing out.

The serial remake path remains for broader debug combinations and makefile
debug with load limits. With `-n`, `-q`, or `-t`, grouped or double-colon
optional sources and optional sources with prerequisites under a load
limit use the serial path. Makefile debug and explicitly requested primary
makefiles also use the serial path. An included makefile named as a goal
uses the requested mode for its visit.

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

The selector runs 26 duplicate-read profiles separately. The developer gates
run all 353 profiles. Leading `./` spellings share one target record and still appear
twice in `MAKEFILE_LIST`. A spelling with an internal `../` retains a separate
target record even when it refers to the same physical file.

The prototype at `0dc03fcaf` serialized the workers in all 20 initial
duplicate-read cases and matched the final ordinary observations in ten.
The overlap checks detected all twenty.

## Load limits

The development and release gates include twenty-eight held load profiles.
For a standalone run, add `--include-load` to compile the tracked shim
with `cc` in the harness temporary directory. Use `--load-shim PATH` to
reuse a compatible shim, such as the one built by `make test-load`.

The load cases distinguish a recipe that has already reserved a job slot
from one still unvisited in a stopped graph. They observe high-load
deferral, a low-load overlap control, and a controlled load drop during
token wait. A stopped optional remake retains its queued sibling while
a required failure cancels the queue. The optional-stop ordinary goal
records its secondary expansion before the queued sibling releases.
Ordinary goal profiles check file effects, successful and failed exits,
ignored failures, and the summary for work dispatched outside a goal
visit. A low-load control starts both children during their visits.

## Remake debug traces

The 27 debug profiles check the entire debug trace and the exact trace prefix
visible at each worker release. They cover basic remake debugging with both
jobserver transports and combined remake and verbose debugging with the pipe
transport. Verbose FIFO output contains a live controller PID and is outside
this exact fixture set.

~~~sh
python3 tools/remake_parallel.py --remake-debug --self-test
python3 tools/remake_parallel.py --remake-debug --candidate target/debug/rmake
~~~

The standard developer gates include these profiles. The selector runs
only the debug profiles and does not build the load shim.

## Dry-run, question, and touch

Default goals and simple explicit goals can run required makefile remakes
in parallel under `-n`, `-q`, and `-t`. The explicit controls cover leading
`./` in goal spelling and a named prerequisite of a required makefile.
Directory-search and other expanded file identities use the serial path.
The held profiles require both children to overlap and the ordinary mode
to take effect after a successful restart. The
required-failure profiles check that the sibling finishes its second command
in normal execution mode for both stopping and keep-going failures.

The optional-source controls cover rule-free present and missing includes,
reader-time and worker writes, and an absent default source created before
a required remake triggers a restart. Admission requires disabled built-ins,
an empty implicit rule inventory, and unique simple read identities without
search or assumed ages. Only a real remake triggers a new reader pass for
these controls.

The direct optional-source cases cover independent explicit recipes with no
prerequisites. They compare successful restarts with two optional sources or a
mixed required and optional pair. They also verify that a failed optional
recipe leaves the other root's running recipe and its remaining command in
the remake phase, with and without `-k`. The 48 profiles run with and without built-in rules. Each checks both
worker releases, the exact output and status, and the resulting files.

The named-source profiles compare `-n`, `-q`, and `-t` when an included
makefile is also a command-line goal. Only the unnamed worker runs. The
cases check the second dry-run print after a restart, the retained query
result that prevents a restart, optional query diagnostics with and without
keep-going, and the new input created by touch mode.

The optional prerequisite cases hold a sibling worker across a stopping
failure. Under ordinary dry-run, question, or touch mode, they compare
the point where the first goal resumes, the diagnostic while draining,
and how the remaining command is handled. The keep-going controls finish
the sibling in the remake phase. The 48 profiles cover both transports and
include line, target, and recursive output synchronization.

The 42 parallel named-source profiles place the named include before,
between, and after two held workers. Both two-slot and three-slot runs
check the named source mode, worker releases, recipe tail, and restart.
The keep-going cases preserve the earlier query verdict while reporting
a later worker failure.

Two additional query cases use an ordinary recipe expansion without a file
write. They require the unfinished-jobs diagnostic while the last optional
remake worker is still held.
