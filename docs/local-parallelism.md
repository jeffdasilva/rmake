# Local jobs and recursive invocations

`-jN`, `--jobs=N`, `-j` and `-j1` are accepted. Independent targets with complete
multi-command recipes can run concurrently. The interpreter owns all variable, rule,
filesystem-cache and graph state; workers own only prepared child commands and
report completion through a channel. A target is reserved once, including shared
prerequisites and multiple command-line goals. Dependents wait for completion and
use the completed timestamp and status. Failed and ignored jobs retain their
recipe diagnostics and deletion policy; outstanding children are reaped before
returning, including after errors. Completed jobs invalidate directory caches.

An empty `.NOTPARALLEL` forces serial execution. Named `.NOTPARALLEL` targets
serialize their prerequisites. One-shell scripts, ordinary multi-command recipes
and recursive invocations can run concurrently. Each target retains a slot until
its complete recipe finishes. Interpreter-owned continuations keep the expanded
command queue, target variables, automatic variables, export snapshot, stdin
ownership and recipe flow. A successful or ignored child resumes that state; an
unignored failure stops the target. Already-running targets finish their queued
commands after another target fails. Workers still own only one prepared command.
Shared FIFO and pipe jobservers are implemented; see [jobservers](jobserver.md)
for cross-engine validation and remaining scheduler boundaries.

Recursive children receive command-line bindings through MAKEFLAGS/MAKEOVERRIDES,
including escaped spaces, backslashes and dollars. Child arguments take precedence;
clearing MAKEOVERRIDES stops command-line-origin transport. Inherited scheduling
joins the parent's shared pool, including GNU parents/children. Explicit child job
options reset that mode. Unavailable inherited servers fall back to serial.
General expression evaluation in imported MAKEFLAGS, load limiting, output
synchronization and complete signal behavior remain follow-up work.

The initial local-pool validation added 28 original differential fixtures and promotes five existing
fixtures. The new two-process barrier cannot pass under serial execution. A
four-target flock-protected counter verifies a two-job bound and zero active
workers at completion. Other cases cover shared ownership, goal completion,
.NOTPARALLEL, errors, ignored failures, cleanup, one-shell scripts, command-line
quoting, overrides and dry-run recursion. That initial slice had 1,971 cases:
1,961 required exact passes and ten pending mismatches. That pending count is
not a complete compatibility inventory. The existing recursively named jobserver
fixture proves results, not jobserver protocol conformance.

These are trusted tests with process containment, not a security sandbox.
Sleep/barrier fixtures have bounded waits; they are functional tests, not a
performance threshold or a timing-normalization exception.

A local optimized-build measurement of 16 independent 20 ms sleep recipes used
15 alternating measured samples after two warmups. Medians were 334.8 ms for
rmake -j1, 87.4 ms for rmake -j4, and 85.8 ms for GNU Make -j4. This demonstrates
parallel scaling on that synthetic workload, not general superiority over GNU.
Outputs and statuses matched. There was no CPU pinning or isolation; raw samples
and binary/source hashes are in the local ignored artifacts/bench/local-pool.json.
The new searched-file lookup also retains its first stat result instead of
immediately performing the same stat again; this is local observation reuse,
not a persistent timestamp cache.

## Recipe continuation follow-up

A target expands all recipe bodies before its first command. Shell configuration
is evaluated as each command starts. The exported environment is expanded once,
when the first real child is prepared, then reused across commands, matching GNU
in serial and parallel execution. Optimized no-child commands do not create that
snapshot. This also avoids repeated environment expansion and side effects.

Twenty-four new required differential cases cover two-stage concurrency barriers,
a whole-target slot counter, shared prerequisites, eager expansion, export/config
boundaries, ignored launch failures, later-command failures, deletion, precious
files, keep-going, double-colon branches, automatic and target variables, and
recursive concurrency. A manual piped-input comparison also checks stdin retention
across child-command boundaries. The full mode matrix and stateful gates pass.

One additional pending case records GNU's fatal expansion during parallel recipe
resumption. GNU 4.4.1 prints an extra wait/ECHILD error; its default FIFO variant
also leaks a PID-named FIFO. The corpus uses GNU's pipe style to keep that oracle
observation reproducible. The pipe protocol option is now implemented; the GNU
fatal-resumption diagnostic path remains pending. No leaked FIFO is reproduced or normalized away. Total corpus: 2,101
cases, with 2,088 required passes and 13 pending mismatches.
