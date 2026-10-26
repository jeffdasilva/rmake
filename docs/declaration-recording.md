# Declaration recording and prerequisite observations

A rule header freezes its immediately expanded text, but does not yet enter the
rule or its recipes into the database. Blank, comment and conditional lines can
leave that declaration open. GNU records it when the next active statement closes
it, or at the end of the input. Conditional expansion can therefore change the
filesystem or run nested `eval` before the outer rule is recorded.

For example, this rule observes `late.in` as its prerequisite:

```make
out: *.in
ifeq ($(file >late.in,input),)
endif
	@printf '%s\n' '$^'
```

The earlier implementation observed ordinary prerequisite globs at the header.
Only tilde prerequisites waited for the declaration to close. The same early
registration could reverse which recipe wins when a conditional evaluates a
second declaration for the same target.

## Implementation

The reader now retains expanded prerequisite bytes, optional static-pattern
metadata and unexpanded recipes in its current declaration. At closure it resolves
filename sequences, applies the current secondary-expansion policy, validates
rule families and merges each target with its recipes. Nested evaluation has its
own pending declaration; it can update the database before the outer declaration.
Target globs still precede prerequisite-tail effects. Ordinary variable expansion
still happens at the header. Includes and assignments close the previous rule
before their own effects; inactive statements do not close it.

Filename observations happen once per declaration before per-target static stem
replacement. This includes ordinary globs, archive member groups and HOME effects.
The existing borrowed ordinary filename path is retained. Prerequisite and recipe
buffers are reused across declarations, and duplicate recipe targets use a set
rather than a quadratic scan. There is no new unsafe code, runtime dependency,
persistent cache or skipped expansion effect.

The first target determines whether a rule is implicit. An implicit-first rule
with a normal target is fatal; a normal-first rule treats later percent names as
literal records and emits GNU's deprecation diagnostic. Quoting provenance is
retained per target, including two spellings that reduce to the same filename.
Recipe warnings for an earlier target remain visible if a later target fails.
Default-goal scanning stops at a percent-containing spelling, including escaped
percent names, matching the pinned executable.

The read-time `.SECONDEXPANSION` flag is independent of file-record insertion.
It applies to its own header and to an outer pending rule when a conditional
nested evaluation enables the flag. Target-specific assignments do not enable it.
After reading, rule mutation through `eval` is rejected at recording, after
conditional effects but before prerequisite filename effects. An empty target
list adds no rule and does not trigger that rejection.

## Original regression evidence

There are 73 new required Makefiles under `rules.declaration-recording`:

- Create/remove globs in normal and order-only positions across explicit,
  static, double-colon, grouped and implicit declarations.
- Archive membership replacement at the declaration boundary for six rule forms.
- Frozen header values, earlier target observations, nested/else conditionals,
  ignored statements, include/assignment/empty-expansion boundaries.
- Nested rule/recipe evaluation and late secondary-expansion activation.
- Mixed-family failures, literal/quoted percent names, duplicate warnings,
  per-target diagnostic order and automatic default-goal selection.
- HOME/static/group/unterminated-condition error order, including a merged-stream
  anchor, and recipe-time mutation boundaries.

Each case has independently authored byte-exact stream and status anchors and
full filesystem comparisons. Oracle self-comparison passes all 73. The preceding
main release passes 13 and fails 60. All existing required fixture inputs,
expectations and pending classifications are retained. One older internal unit
assertion incorrectly expected automatic default selection of an escaped percent
name; it now checks GNU's empty default goal. New black-box cases cover both
explicit invocation and the no-default failure, while its literal-rule/scope
assertions remain intact.

Inventory: **4,169 curated cases / 4,157 required matches / 12 unchanged pending
fixtures**; **64 stateful scenarios / 273 invocations per engine**; **385 Rust
and 280 tooling tests**. Existing `test-oracle`, `test-diff`, `test-secondary`,
`test-stateful`, `test-graph-progress`, `check` and `ci` commands discover or exercise
the new behavior. No new exemption or successful placeholder gate is added.

## Measurements

Full local `make ci` passed with an immutable debug executable, including all
4,157 required cases, generated suites, 64 stateful scenarios, 61 jobserver
programs, 720 repeated parallel comparisons per engine and self-hosted checks.
The immutable optimized build also drove top-level `test-diff`, `test-stateful`
and `test-graph-progress` (12 repeats / 216 comparisons per engine) successfully.

Paired local runs use 101 samples per engine, two warmups and alternating engine
order, after functional tests with no concurrent local validation workload.
Every timed invocation checks exact output and successful exit. Times include
startup and warm OS caches. There is no CPU isolation, CPU/RSS/I/O profile,
confidence interval or performance acceptance threshold.

| Workload | Current rmake (ms) | Previous main (ms) | GNU 4.4.1 (ms) |
| --- | ---: | ---: | ---: |
| tiny-noop | 0.699 | 0.714 | 0.627 |
| large-noop | 30.326 | 31.516 | 11.620 |
| expansion | 4.912 | 4.763 | 2.829 |
| rule-filenames-quoted-2000 | 7.043 | 7.141 | 4.743 |
| rule-filenames-glob-1000 | 2.753 | 2.768 | 1.691 |
| rule-filenames-secondary-1000 | 2.595 | 2.551 | 1.631 |
| rule-filenames-implicit-500 | 3.433 | 3.420 | 1.912 |
| rule-tilde-literal-2000 | 4.535 | 4.470 | 2.176 |
| rule-tilde-glob-1000 | 2.637 | 2.613 | 1.694 |
| rule-tilde-secondary-1000 | 2.510 | 2.472 | 1.654 |
| rule-tilde-implicit-500 | 3.567 | 3.551 | 1.718 |
| secondary-ready-256 | 1.453 | 1.458 | 0.824 |
| secondary-ready-1024 | 3.778 | 3.821 | 1.599 |
| secondary-empty-256 | 1.142 | 1.142 | 1.003 |
| secondary-targets-1000 | 4.757 | 4.747 | 2.987 |

The selected controls are close or mixed: the large no-op median is about 4%
lower and expansion about 3% higher. These observations do not establish general
improvement or GNU speed parity; GNU remains faster in these workloads. Reports
retain all samples and input/executable identities under ignored
`artifacts/bench/declaration-recording-{core,filenames,tilde,secondary}-101.json`.

Current release SHA-256:
`2679de9f5c1fa7460620831c4040c951cb0dcc49a93c9be1cc04881ef67dd847`.
Previous main release SHA-256:
`454d70c8ac4aee69807840a1cf4769e9fd43fe4497f384647c12e25f3b63d661`.

## References and remaining M4 work

Consulted the verified GNU Make **4.4.1** release archive, SHA-256
`dd16fb1d67bfab79a72f5e8390735c49e3e8e70b4945a15ab1f81ddb78658fb3`:
`src/read.c` (`eval`, `record_waiting_files`, `record_files`, `check_specials`,
`check_special_file`) and `src/file.c` (`split_prereqs`, `enter_prereqs`).
Source inspection explains phase boundaries; executable probes supply the
behavioral evidence. The Rust implementation, Makefiles, scripts and anchors
are independently written. No upstream implementation or test was copied or
translated.

This closes the selected declaration-recording gaps, not M4's complete exit gate.
Concrete outstanding M4 failures include GNU file-table ordering of intermediate
cleanup (`chain-two-roots`), ordinary stat interruption/error reporting (confirmed
by independent injected-fault probes), and the recursive HOME filename scratch
corruption described in [the tilde notes](rule-tilde.md). Wider archive I/O,
file-record alias/mtime and lifecycle interaction audits remain required.
Escaped-semicolon reading, dynamic MAKEFLAGS replay, runtime/profile matrices,
release-scale compatibility and persistent cache correctness remain on the broader
compatibility path. M4/M8 and general GNU speed parity are not complete.
