# Documented GNU Make 4.4.1 safety exceptions

On 2026-10-28 the project owner explicitly selected deterministic safe errors for
recorded inputs that crash or hang GNU Make. These exceptions do **not** count as
exact conformance. Other ordinary mismatches remain compatibility work. M3/M6
and general performance parity remain open.

| Code | Hazard class | Recorded inputs | Boundary |
| --- | --- | ---: | --- |
| S001 | Multiple `let` names with an empty expanded value list | 1 | Expand names and values in order, then fail before binding/evaluating the body. Zero names, one name, a nonempty short list and unreachable lazy branches retain normal behavior. |
| S002 | Deletion of live global `.DEFAULT_GOAL` metadata | 1 | Reject an effective `undefine` after runtime metadata activation. Respect origin precedence; ordinary empty assignment resets remain supported. |
| S003 | Deleting a global binding while expanding its exported value | 3 | Track selected global recursive exports capable of evaluation effects. Reject effective deletion of an active binding before removing it. Unwind markers on errors; target-local selection does not refer to the deleted global object. |
| S004 | Reentrant flag decoding that rewinds/repeats an outer assignment stream | 5 | Refuse the recorded single-short-option rewind; retain the bounded assignment replay limit with a safety diagnostic. Single-word outer streams and normal nested flag decodes remain supported. |
| S005 | Custom-shell `.ONESHELL` argument preparation with recorded allocator hazards | 7 | Refuse unquoted interpreter syntax, unmatched quotes, and the literal-SHELL-LF/leading-double-quoted-flag case. Preserve ordinary quoted/escaped flags and Bourne shells. Apply only when execution is selected; ordinary dry-run/query/touch skips remain supported. |

Errors exit normally with status 2 and a stable `rmake safety error [S00x]`
diagnostic. Ignore-errors, keep-going and parallel execution do not suppress a
fatal safety error. No deliberate signal, invalid memory access, GNU fallback or
unbounded loop is introduced. Previously completed effects are not rolled back.

The checks describe interpreter state and lexical classes, not hashes of
particular Makefiles. Input hashes serve only to protect the evidence ledger.
Literal/simple export values do not allocate active-export markers. Ordinary
origin checks occur before deletion guards, preserving ineffective undefines.

## Evidence and separate acceptance

[`tests/safety/manifest.json`](../tests/safety/manifest.json) maps 17 original
inputs to immutable raw GNU observations in `tests/research`, the exact input
bytes, observer modes, invocation and independent candidate error expectations.
The historical candidate observations are retained unchanged. The former pending
empty-`let` fixture moves out of the ordinary corpus; two bounded GNU SIGSEGV
observations are retained in its research record.

`make test-safety` runs only rmake. It checks four modes per exception (normal,
`-i`, `-k`, `-j4`), each twice: 68 cells / 136 observations. Status, raw streams,
resource limits, leftover processes and repeatable filesystem effects are
checked. Inputs must remain unchanged and no extra files may appear; only the
three original export probes may compile their owned observer before failure.
Reports retain candidate/manifest identities and explicitly record zero
conformance credit. Both development CI and optimized acceptance require this
gate; the self-hosted top-level flow also invokes it. Normal CI does not rerun
the known GNU crashes/hangs.

The ordinary corpus separately adds 99 exact-match controls covering lazy and
partial `let` lists, default-goal resets and losing undefines, active local versus
global exports, quoted/escaped shell flags, skipped execution modes and finite
nested flag streams. Explicit expected bytes/status were checked against two GNU
observations and rmake. Negative tooling controls reject success, signals,
timeouts, leftover processes, wrong output, nondeterministic effects, changed
evidence and unmapped exceptions.

The initial guard's full differential run exposed seven ordinary failing-exec
cases with a literal SHELL newline. They must keep GNU's normal diagnostics;
the guard was narrowed. A new balanced-double-quote control also exposed a real
argv mismatch: a slow flag parse re-enters `.ONESHELL` body preparation and
consumes the initial make modifier. rmake now preserves those resulting bytes.

## Remaining ordinary gaps

`flag-payload-ordered-14` exits normally in both recorded GNU observations. It
remains a research-only compatibility mismatch, **not** one of these exceptions.
Its source-informed lifetime explanation does not turn a status-0 observation
into crash/hang evidence.

An additional original boundary probe uses `BOOT:=$(eval undefine .DEFAULT_GOAL)`
on the command line. GNU succeeds before metadata activation; rmake's current
startup assignment evaluator rejects `eval`. The raw evidence is retained in
[`safety-boundary-cli-eval.json`](../tests/research/safety-boundary-cli-eval.json).
It receives neither conformance credit nor a safety exemption. Complete startup
evaluation/order and the other [M3/M6 surfaces](m3-m6-audit.md) still need work.

## Performance observations

Frozen development/release candidates are identified in the raw benchmark
reports. Each workload used 101 alternating samples against GNU and the preceding
release; no build or acceptance campaign ran concurrently. Host activity was not
controlled. Times include process startup and validate exact output.

| Workload | GNU ms | Current ms | Previous ms |
| --- | ---: | ---: | ---: |
| Tiny no-op | 0.591 | 0.707 | 0.725 |
| 10,000 explicit targets | 12.278 | 32.750 | 32.231 |
| Expansion | 2.931 | 4.804 | 4.772 |
| 0 explicit overrides | 0.637 | 0.747 | 0.756 |
| 1,000 explicit overrides | 2.132 | 1.982 | 1.974 |
| 5,000 explicit overrides | 25.724 | 8.207 | 8.284 |
| Parallel branches, depth 1 | 88.032 | 90.129 | 90.327 |
| Parallel branches, depth 4 | 88.125 | 90.722 | 90.612 |

The mixed small differences do not establish a statistical improvement or
regression. GNU remains faster on standard graph/expansion workloads. This slice
does not claim general parity, add semantic caches or trade correctness for speed.
Raw [core](benchmarks/safety-core.json),
[startup](benchmarks/safety-startup.json) and
[parallel](benchmarks/safety-parallel.json) reports retain every sample and identity.
