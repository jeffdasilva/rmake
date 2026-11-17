# Documented safety exceptions

rmake reports deterministic errors for **34 owner-approved inputs** covered by
S001–S006. These exceptions earn **zero exact-conformance credit**. Ordinary
mismatches remain compatibility work, and complete M3/M6 acceptance and general
performance parity remain open.

On 2026-10-28, the owner selected safe errors for recorded inputs that crash or
hang GNU Make 4.4.1. On 2026-11-02, the owner explicitly extended S004 to fifteen
recorded inputs that exit normally but belong to an unstable nested short-option
class. That approval has a fixed scope. It grants no general exception for
ordinary GNU behavior.

## Run the candidate-only gate

From the repository root, run the existing safety gate.

```sh
make test-safety
```

The target builds rmake and runs [tools/safety.py](../tools/safety.py). It invokes
only the candidate. Normal CI retains the recorded GNU evidence without rerunning
the known crashes or hangs.

Use `CANDIDATE=PATH` with the Makefile target to select another executable. To
check an existing executable and choose an evidence directory directly, use
this candidate-only command.

```sh
python3 tools/safety.py --candidate target/debug/rmake \
  --artifacts artifacts/safety
```

The default gate has **134 cells, each repeated twice, for 268 observations**.
S001–S005 run with their original arguments and with `-i`, `-k` and `-j4` added.
S006 runs with its original arguments, `-i` and `-j4`. Its `-k` behavior remains
an ordinary exact-comparison case. Direct runs accept `--repeat` values of at
least two.

A successful cell prints `SAFE`. The gate exits 0 when all cells pass, 1 when a
cell fails, and 2 for setup or validation errors. Reports are written beneath
`artifacts/safety/exceptions-*/report.json`, or the selected artifact directory.
They retain candidate and manifest identities, arguments, exact streams,
filesystem observations, failures and explicit zero conformance credit.

Development checks, optimized acceptance and the selected self-hosted workflow
all require this gate. Its trusted fixtures use the ordinary runner's process
and resource containment. That containment does not restrict filesystem or
network access.

## Approved behavior

Every safety error exits normally with status **2** and a stable
`rmake safety error [S00x]` diagnostic. Ignore-errors, keep-going and parallel
execution cannot suppress a safety error once its guard applies. Previously
completed effects remain in place. The implementation introduces no deliberate
signal, invalid memory access, GNU fallback or unbounded loop.

Runtime guards examine interpreter state and lexical classes. Hashes protect
the evidence ledger and input identity. They do not select runtime behavior.

| Code | Recorded inputs | Guarded behavior |
| --- | ---: | --- |
| S001 | 1 | A `let` binding with multiple names and a nonempty whitespace-only expanded value list. |
| S002 | 1 | Effective deletion of live global `.DEFAULT_GOAL` metadata. |
| S003 | 3 | Effective deletion of a global binding while its exported value is being evaluated. |
| S004 | 20 | Reentrant flag decoding that repeats an outer stream or retains unstable short-option state. |
| S005 | 7 | Custom-shell `.ONESHELL` argument preparation with recorded allocator hazards. |
| S006 | 2 | An unvisited double-colon branch after an earlier query verdict. |

### S001 — Empty expanded value lists

Expand names and values in order, then fail before binding or evaluating the
body when multiple names receive a nonempty whitespace-only list. A zero-length
value list still binds each name to empty. Zero names, one name, a short list
containing a value and unreachable lazy branches retain ordinary behavior.

### S002 — Live default-goal metadata

Reject an effective `undefine` after runtime metadata activation. Apply origin
precedence first, so an ineffective deletion retains ordinary behavior. An empty
assignment can still reset the default goal. Early deletion before metadata
activation remains an ordinary supported case.

### S003 — Active exported bindings

Track selected global recursive exports that can evaluate effects. Reject an
effective deletion of the active binding before removing it, and unwind the
markers on errors. A selected target-local binding has its own identity. Literal
and simple export values allocate no active-export markers. Origin checks still
precede deletion guards.

### S004 — Nested option state

S004 covers five recorded hangs and fifteen explicitly approved normal-exit
inputs. Retain the original rewind/replay guard. For the approved nested
short-option boundary, refuse the first nested short word when a following
definition and additional words leave the recorded hazardous continuation.
One-tail streams, long-option resets and an intervening option before the
definition remain supported. The exact approval is described below.

### S005 — Custom-shell argument preparation

For the recorded custom-shell `.ONESHELL` cases, reject unquoted interpreter
syntax, unmatched quotes, and the combination of a literal SHELL value containing
a line feed with a leading double-quoted flag. Preserve ordinary quoted or escaped
flags and Bourne shells.
Apply the guard when execution is selected. Ordinary dry-run, query and touch
paths that skip execution remain supported.

## Stalled double-colon query

S006 applies after goal advancement and job drain. If question mode still has
keep-going disabled and a later queued goal has an unvisited declared branch,
report the safety error. Single goals, completed branches and tails that resume
with keep-going retain their ordinary verdicts.

The [bounded GNU evidence](../tests/research/gnu-441-stalled-double-colon-query.json)
records grouped and ordinary double-colon forms. GNU can revisit the later goal's
already answered branch while other declarations remain unvisited. An earlier
goal has already set the query verdict. Keep-going allows traversal to advance
past that result.

This state also occurs without database printing, `.WAIT`, secondary expansion
or grouping. Six ordinary controls preserve the single-goal case, keep-going,
late keep-going, a completed grouped tail, a query at the final branch and a later
fatal error.

## Evidence and separate acceptance

The [manifest](../tests/safety/manifest.json) maps each input to immutable GNU
observations in `tests/research`, exact input bytes, observer modes, invocation
and independent candidate expectations. Historical candidate observations remain
unchanged. The former pending empty-`let` case is outside the ordinary corpus,
with its two bounded GNU SIGSEGV observations retained in the research record.

Before running the candidate, the validator checks evidence hashes, fixture
bytes, invocation, observer modes and complete mapping. Original fault rows need
recorded GNU fault evidence. Normal-exit rows need the explicit approval and its
additional checks. Exceptions cannot also appear in the ordinary corpus.

The candidate gate checks status, raw streams, limits, leftover processes and
repeatable filesystem effects. Inputs must remain unchanged and no additional
files may appear. The three S003 probes may compile their owned observer before
failing, and that output must also be repeatable. Negative
[tooling controls](../tools/test_safety.py) reject success, signals, timeouts,
leftover processes, wrong bytes, inconsistent effects, changed evidence and
unmapped exceptions.

The ordinary corpus retains **99 required boundary controls** for lazy and
partial `let` lists, default-goal resets, ineffective deletions, local/global
exports, quoted or escaped shell flags, skipped execution and finite nested
flag streams. Their expected bytes and status were checked against two GNU
observations and rmake.

Boundary testing also narrowed earlier guards. Seven ordinary failing-exec
cases with a literal SHELL newline must keep GNU's normal diagnostics. A balanced
double-quote case exposed a separate argv mismatch when slow flag parsing
re-entered `.ONESHELL` body preparation and consumed the initial make modifier.
The resulting bytes remain required ordinary behavior.

## Explicit normal-exit extension and remaining gaps

The [approval record](../tests/safety/nested-short-normal-approval.json) limits
the S004 extension to fifteen recorded inputs. This includes
`flag-payload-ordered-14`, which exits normally in both retained GNU observations.
The [immutable observations](../tests/research/gnu-441-nested-short-state.json)
remain separate from candidate error expectations.

Four recorded pairs differ in unrequested keep-going, debug or version behavior.
The validator requires the approved input and invocation identities and this
policy-instability evidence. PID or address differences alone cannot qualify a
normal-exit exception. All other manifest rows still require fault evidence.

The [boundary study](nested-short-options-proposal.md) preserves the initial
proposal and four additional ordinary regressions that narrowed the guard.
Its eighteen ordinary controls must match GNU exactly. The separate discovery
gate adds no acceptance credit. Its historical inventory of 128 candidate-only
cells preceded S006 and remains unchanged in the research records.

Early CLI evaluation that deletes `.DEFAULT_GOAL` before activation is covered
by [CLI assignment effects](cli-assignment-effects.md) as an ordinary required
case. Its [prototype evidence](../tests/research/safety-boundary-cli-eval.json)
remains available. The recorded CLI restart file-effect difference is also a
required ordinary control under [ordered startup replay](startup-replay.md).
Neither case adds an exception. Broader startup and profile behavior remains
[M3/M6 compatibility work](m3-m6-audit.md).

## Performance observations

These are historical measurements of the original crash/hang guard. The raw
reports identify the frozen development/release candidates. Each workload used
101 alternating samples against GNU and the preceding release. Builds and
acceptance campaigns ran separately, but other host activity was uncontrolled.
Times include process startup, and each workload checked exact output.

| Workload | GNU ms | Original guard ms | Preceding release ms |
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
regression. GNU was faster on the listed graph and expansion workloads. These
measurements do not qualify the later normal-exit extension or current general
performance parity. The change added no semantic cache or correctness tradeoff.
The raw [core](benchmarks/safety-core.json),
[startup](benchmarks/safety-startup.json) and
[parallel](benchmarks/safety-parallel.json) reports retain every sample and
executable identity.
