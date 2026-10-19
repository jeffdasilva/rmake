# Local jobs and recursive invocations

`-jN`, `--jobs=N`, `-j` and `-j1` are accepted. Independent targets with a single
prepared command can run concurrently. The interpreter owns all variable, rule,
filesystem-cache and graph state; workers own only prepared child commands and
report completion through a channel. A target is reserved once, including shared
prerequisites and multiple command-line goals. Dependents wait for completion and
use the completed timestamp and status. Failed and ignored jobs retain their
recipe diagnostics and deletion policy; outstanding children are reaped before
returning, including after errors. Completed jobs invalidate directory caches.

An empty `.NOTPARALLEL` forces serial execution. Named `.NOTPARALLEL` targets
serialize their prerequisites. One-shell scripts can run as one job. Multiple
ordinary commands and recursive recipes use the synchronous path; they do not
have full parallel continuation support yet. This is a real local job pool, but
it is not a complete GNU scheduler or a shared jobserver implementation.

Recursive children receive command-line bindings through MAKEFLAGS/MAKEOVERRIDES,
including escaped spaces, backslashes and dollars. Child arguments take precedence;
clearing MAKEOVERRIDES stops command-line-origin transport. Inherited scheduling
without a shared jobserver is conservative and serial, avoiding independent
nested pools that multiply resource use. Explicit child job options are honored.
General expression evaluation in imported MAKEFLAGS, jobserver protocol/FIFO
interoperability, load limiting, output synchronization and complete signal
behavior remain follow-up work.

Validation adds 28 original differential fixtures and promotes five existing
fixtures. The new two-process barrier cannot pass under serial execution. A
four-target flock-protected counter verifies a two-job bound and zero active
workers at completion. Other cases cover shared ownership, goal completion,
.NOTPARALLEL, errors, ignored failures, cleanup, one-shell scripts, command-line
quoting, overrides and dry-run recursion. The corpus now has 1,971 cases:
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
