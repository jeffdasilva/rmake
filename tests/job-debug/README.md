# Original JOBS identity and token profiles

Nine independently modeled GNU Make 4.4.1 profiles qualify actual-child JOBS
output. They cover a serial child, multiline recipe, separate serial targets,
failed/ignored children, FIFO/pipe pairs, and externally witnessed token return.
Both frozen JOBS builds pass all nine; this does not establish complete JOBS,
M3/M6, transport, recursion, restart or loaded-module compatibility.

Run `make test-job-debug`, optionally with
`JOB_DEBUG_CASE=jobs-serial-multiline`. The standalone `tools/job_debug.py` accepts
`--oracle`, `--candidate`, `--report` and repeated exact `--case` IDs. Both GNU
executions and the candidate receive identical reset inputs.

Each model declares an exact event sequence. No extra or missing Live line is
accepted. All nonidentity stdout bytes, stderr, status, process bounds and the
independent complete filesystem must match. Worker PIDs are verified against the
actual make child's PID, `/proc` parent/starttime and declared command line before
the host acknowledges and unblocks the worker. The host does not obtain those
identities from debug output. FIFO paths are predetermined from the reset runtime
and observed make PID; the selected pipe profile requires descriptors 3,4.

Pointer values remain raw in every report. The comparator requires aligned,
well-formed, stable recipe identities, active-record uniqueness, correct reuse
across command PIDs, exact newest-first live order and complete removal. It does
not require independently allocated GNU/rmake addresses to be numerically equal.
Every expected PID field must match its separately verified live worker. This is
a strict relationship protocol, not broad output normalization.

The paired worker stays alive until the host has observed the two required
second-worker Live records. Its `/proc` identity loop distinguishes disappearance
or replacement of the original PID/starttime and handles both ENOENT and ESRCH.
Returned-token cases additionally borrow and return the exact paid byte while
another recipe remains alive; the final `borrowed-token` file must contain
`borrowed=+ returned=+\n`. Missing tokens, changed effects, leftover processes,
extra files and resource bounds fail.

| Evidence manifest | Contents |
| --- | --- |
| [history.json](history.json) | Complete original eight-input discovery, rejected first observer/root-directory and worker ESRCH observations, all refined GNU2 sources/reports, 30 two-case GNU2 repeat controls, preceding candidate, v1/v2/v3/v4 candidate reports, and both failed pre-transfer token witnesses. Initial/fixture failures receive zero acceptance credit. |
| [qualification.json](qualification.json) | Complete frozen development/release reports passing JOBS 9, deterministic debug 64, native 215 and directory 40; source/input/binary hashes, final observer/worker/Makefile bytes and core-check logs. |
| [performance.json](performance.json) | All three completed elapsed reports, sixteen allocation reports and sixteen traces, twelve exact inputs, tool/helper source bytes, compiled-helper hashes, and immutable qualification/C1/v2 references. Every mixed result is retained. |

Each small JSON manifest supplies the compressed and uncompressed SHA256 values
and uncompressed byte count for its adjacent `.json.gz` archive. Archives retain
complete raw observations and source bytes, including negative results. No GNU
implementation, headers or tests were copied or translated. These separate gates
add no ordinary-ledger or approved safety-exception credit. The
[measurement page](../../docs/benchmarks/job-debug.md) describes the mixed results
and inherited shared CPU23 elapsed-only scope. A dedicated tooling unit checks
all three manifest/compressed/raw identities and raw sizes. Full exact-head
developer acceptance remains a separate merge gate.
