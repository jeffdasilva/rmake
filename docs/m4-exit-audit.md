# M4 rule-resolution acceptance

Audit date: 2026-10-26. The M4 rule-resolution implementation is complete for the
pinned Linux development profile. Its exit requires successful combined
development and optimized gates on the proposed commit; an incomplete or failed
run does not satisfy that requirement. This is a feature-tranche acceptance,
not a declaration of complete GNU Make compatibility on arbitrary inputs or
all Linux/runtime profiles.

## Implemented families

The requirement ledger maps **32 rule/archive/resolution families to 2,599
required Makefiles**, with no pending cases in those families. A tooling test
rejects missing mappings or a pending exemption in any of these families.

| Family | Evidence and behavior |
| --- | --- |
| Explicit/static/implicit/suffix | Declaration and candidate priority, built-ins, cancellation, chains, stems, automatic variables and cycle handling; original curated and seeded graph suites. |
| Grouped/double-colon | Independent and shared recipes, per-branch traversal, peer completion, failure, scoped variables and queued parallel consumers. |
| Secondary expansion | Explicit recipe-first clauses, speculative implicit candidates, effects and per-candidate automatic values. |
| VPATH/GPATH/libraries | Ranked search, retained versus permanent names, declared-record merging, shared directory observations, `-l` / `.LIBPATTERNS` and alias transitions. |
| Intermediate/special targets | Lazy existence, named/global precedence, hidden normal/order-only edges, file-record insertion/traversal, cleanup and protected files. |
| Archive rules and metadata | Member-first and complete-name inference; groups/formats/raw names; standalone/container timestamp phases; unknown completion state; metadata errors, I/O faults and safe touch. |
| Recursive filename processing | Quoting, glob/tilde phases, deferred names and safe owned modeling of nested HOME filename writes. |

The previously confirmed blockers are implemented and protected by required tests:

- [Hidden prerequisites](extra-prerequisites.md): snapshot and expansion phases,
  self-cancellation, local/global precedence and automatic-variable exclusion.
- [Library resolution](library-resolution.md): ordered `.LIBPATTERNS`, local and
  searched containers, aliases and prerequisite spelling.
- [File-record order](file-record-order.md) and
  [archive observations](archive-observations.md): collisions/growth, temporary
  hash keys, archive-container timing, cleanup and initial/completion/fatal phases.
- [Recursive HOME names](recursive-home-names.md): the original `earlyarker`
  failure, nested wildcard/eval/include calls, archive-group byte writes,
  environment fallback and stateful transitions, without unsafe pointers.

The historical `chain-two-roots` rule-resolution pending case is now required.
The eleven retained pending cases concern reader/environment/shell/continuation
surfaces outside this tranche; they are still observed mismatches, not passes.
No comparator normalization or new pending classification hides a rule failure.
GNU is never a product implementation fallback.

## Combined gate

Run **`make m4-acceptance`**. The top-level Makefile sequentially runs:

1. `make ci`: formatting/lint, 393 Rust and 294 tooling tests, the oracle checked
   against independent anchors, the complete curated corpus, generated language
   and graph/scope/secondary programs, syscall/error campaigns, stateful tests,
   mixed jobserver tests, forty repetitions of eighteen parallel programs and
   self-hosted developer flows.
2. `make test-m4-release`: the optimized required corpus, all stateful scenarios,
   all 2,600 archive fault cells, repeated parallel programs and self-hosted
   Makefile flows. Both phases compare against the pinned GNU Make 4.4.1 oracle.

GitHub CI runs both phases and retains reports. A merge must use the exact head
whose checks passed. After merging, synchronize main and verify its CI. Benchmark
runs are separate from functional runs, with frozen executables.

Inventory: **4,892 curated / 4,881 required / 11 historical pending**,
**68 stateful scenarios / 329 invocations**. Curated observations compare exact
streams, exit status and filesystem bytes/types/modes/link topology. Stateful
scenarios add fixed and relational timestamps. Generated/fault campaigns have
independent anchors and replay evidence; matching wrong engines fail their tests.
See [testing](testing.md) for observation boundaries and nondeterministic inputs.

## Profile and subsequent work

This acceptance uses the verified 4.4.1 Linux/glibc, C-locale development oracle.
The [compatibility contract](compatibility.md) remains broader. Finite tests are
regression evidence, not proof for every possible effectful Makefile. Existing
resource/search/expansion bounds remain explicit; arbitrary undefined allocator
behavior is not emulated. Archive touch refuses a replaced descriptor identity
before writing, preserving safety rather than reproducing a possible wrong-file
write during external mutation.

M3 still owns remaining reader/assignment/eval, dynamic-option and export-order
work. M5 still owns complete continuation, signal/cancellation, jobserver and
runtime stress profiles; M6 owns remaining CLI/debug/database/locale and native
extension profiles. M7 adds release-scale real projects, fuzzing and supported
platform/profile acceptance. These gaps remain full-compatibility work; completing
M4 does not waive them.

M8 owns general performance parity and persistent-cache correctness. The archive
follow-up's isolated 101-sample control recorded 256 existing members at **3.189 ms
for rmake / 6.703 ms for GNU / 4.895 ms for preceding main**. Startup, 10,000-rule
no-op and expansion remained slower than GNU. This selected improvement is not
a general speed claim. Future caching must preserve every observable effect.
