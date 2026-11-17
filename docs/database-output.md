# Database output and logical execution history

The database starts explicit file printing with the recipe prefix active when
the database is emitted. Each file can then publish a transition to its own
stored prefix. Implicit rule commands use the active prefix throughout their
section. Fifteen original cases cover assignments, includes, eval, empty
recipes, and grouped and terminal implicit rules.


GNU Make 4.4.1's `-p` output exposes execution history as well as final values.
Recording starts during invocation. The renderer uses retained variables,
rules, file records, directory observations, scope identity, update outcomes,
timestamps and cache statistics. Late `MAKEFLAGS` selection can use events
captured before the request.

The first combined database/locale checkpoint passed 640 Rust and 727 tooling
checks with strict Clippy. Frozen development and optimized builds both pass
41 complete database profiles, six native rounding/registration profiles,
18 floating-environment controls, 74 collation profiles and 183 startup-locale
profiles. These are finite functional results. Full selected-head local/CI
acceptance and final total-cost measurements remain merge requirements; they do
not establish complete M3/M6 or general performance parity.

## Replay a database comparison

Run the commands from the repository root. The comparison uses the pinned GNU
Make 4.4.1 build. Use `make bootstrap-oracle` if the oracle is missing, then
verify it before reproducing a case.

```sh
make oracle-verify
make test-database-output DATABASE_CASE=database-pattern-used
```

Choose the gate that covers the difference you observed.

| Difference | Target and case selector |
| --- | --- |
| Complete dump, including startup and builtin records | `test-database-output` with `DATABASE_CASE` |
| Normal and order-only prerequisites, implicit stems, and late print selection | `test-database-order-roles` with `DATABASE_ORDER_ROLE_CASE` |
| Names and metadata after VPATH or GPATH search | `test-database-search` with `DATABASE_SEARCH_CASE` |
| Per-branch metadata for declared double-colon rules | `test-database-branch-metadata` with `DATABASE_BRANCH_CASE` |
| Explicit grouped double-colon successes and failures | `test-database-grouped-double` runs both suites |

Replay the same case against both build profiles. The order-role target writes
the same report path on each run, so copy the first report before selecting the
release candidate.

```sh
case_id=implicit-substituted-prerequisite-lookup
make test-database-order-roles DATABASE_ORDER_ROLE_CASE="$case_id"
cp artifacts/database-order-roles/development.json artifacts/database-order-roles/debug.json

make release
make test-database-order-roles DATABASE_ORDER_ROLE_CASE="$case_id" \
  CANDIDATE="$PWD/target/release/rmake"
cp artifacts/database-order-roles/development.json artifacts/database-order-roles/release.json
```

If you use a custom Cargo target directory, pass the actual release binary
path as `CANDIDATE`. The report keeps the two GNU observations before the
candidate observation. For an order-role failure, the following command
extracts their original stdout streams for inspection.

```sh
python3 - <<'PY'
import json
from pathlib import Path

reports = Path("artifacts/database-order-roles")
case = json.loads((reports / "debug.json").read_text())["results"][0]
for name, observation in zip(("gnu-first", "gnu-second", "rmake"), case["observations"], strict=True):
    (reports / f"{name}.stdout").write_bytes(bytes.fromhex(observation["stdout_hex"]))
PY
```

Check both reference observations before investigating the candidate. A raw
diff can include the actual opening and closing times and directory identity.
Use the driver's positioned checks to distinguish those fields from literal
output differences. Compare the observed status, stderr, and filesystem before
tracing the first differing deterministic line. For a lookup-counter difference,
trace the operation that should enter or look up the record. For late `-p`,
record whether the request happened during parsing, prerequisite expansion, or
recipe expansion.

## Recording and rendering

`crates/rmake-core/src/database.rs` contains the pure borrowed renderer and
independent logical table histories. GNU-observable slot order, counted probes,
collisions, tombstones, rehashing and string storage remain separate from Rust's
physical collections. Target-local and automatic layers share the actual GNU
scope-table identity; pattern scopes and double-colon branches retain their own
values and outcomes. The printer reads an already observed `.VARIABLES` cache
without initializing or refreshing it.

The engine supplies retained file aliases, selected rules, automatic bindings,
update state and timestamps. Directory views borrow existing stat/open/stream,
physical identity, cached generation and entry-count state. A diagnostic cannot
reopen a directory, rerun an implicit search or reconstruct a finished target
scope. Opening and closing clocks are sampled at the actual emission points.
Early/fatal, successful, native-finalizer and restart ownership remains explicit.

The [search follow-up](database-search.md) adds eight VPATH/GPATH profiles to the
original 33. It records argv preparation once, actual file removal/entry,
selected filename caching and supplied string-buffer extents. A borrowed name
resolver prints retained aliases without mutating declarations. The first
integrated renderer had target-name shadowing; existing unit expectations caught
it and remain unchanged. Earlier failures and corrected source are separate
immutable evidence.

Linux percentage formatting uses public libc on the current OS thread, retaining
locale decimal formatting and floating-point rounding. Native function
registration records accepted names at the actual publication boundary, after
validation. Six independently modeled profiles caught the earlier missing
registration-cache bytes and nondefault-rounding differences.

## Default-path cost

Early recording is required even when `-p` is selected late. Accepted pure plans
therefore accumulate logical read deltas against immutable scope-layout
snapshots; rejected plans publish nothing. Completed prefixes publish before an
effect or diagnostic. Restored fast paths preserve multiplicity and avoid
replaying effects.

Logical tables use compact slot offsets and a key arena with bounded dead-key
reclamation. Scope lookup locates the physical binding and records its logical
read in one pass. Retained file timestamps update in place. Physical variable
bindings use standard randomized `HashMap`, while names, exports and database
output retain their separate GNU traversals. The storage experiment reduced
expansion/large-argv time but increased requested allocation bytes. Inline-only
and combined alternatives were rejected. All slower samples and allocation
traces remain in the
[storage screen](../tests/database-output/performance-storage-collation-screen.json).
The [selected total-cost measurements](benchmarks/database-history.md) retain
the default-path regressions, the owned key move, and the two rejected sharing
experiments. They provide a baseline for reducing the large no-op cost.

## Evidence boundaries

Every selected database transcript compares all literal sections, values,
origins, declaration order, recipes, filenames and logical statistics. Each
[portable expectation](database-portability.md) uses verified build configuration
and independently observed raw directory entry order to compute exact cache
fields. Anchored libc `time(NULL)` and independently statted directory device/
inode slots retain their positioned checks. Clock bounds use the same libc domain,
without widening tolerance. Each observation retains exact streams, status,
full work/home/tmp state, ordered effects and process bounds against two GNU
runs. The independent eight search plans predate observation; full literal
transcripts were transparently derived and GNU2-qualified afterward.

The [original inventory](../tests/database-output/README.md),
[phase inventory](../tests/database-output/PHASES.md), and
[search inventory](../tests/database-output/SEARCH.md) index immutable predictions,
observer/model errors, preceding mismatches, instrumentation trust checks and
source/binary/helper identities. GNU implementation/header/test source remains
external. Earlier counts and measurements describe their archived checkpoints.
The HashMap-only ordinary screen passed 27,766/27,766 before later search changes;
the final combined runtime still requires its own full acceptance.

Broader database/native/Guile interactions, asynchronous lifecycle, non-C numeric
locales, descriptor/resource faults, help/version database boundaries and wider
real projects remain open. No new safety exception is introduced.

The generic lifecycle gate adds a fixed whole-second timestamp, four live
restart environment witnesses, and one failed-exec lifecycle. It checks that
regular stderr, exit 127 and finalizer absence agree with the reference. Owned
stdin and the advertised private descriptor observer have separate profiles
when the stdin runtime is enabled.

[Prerequisite role observations](../tests/database-order-roles/README.md)
require the active recipe to record GNU’s ordered duplicate-role promotion
after `$<` and `$+` are prepared. Untouched rules keep their original roles.

`make test-database-dry-run-time` checks the synthetic GNU timestamp for a
successful direct, nonphony recipe simulated by pure `-n`, including an explicit
empty recipe and flags enabled at two later phases. The renderer uses this
logical timestamp without changing dependency freshness or physical file times.
The ten cases also check physical timestamps, outputs, and preserved controls.
Grouped, double-colon and mixed execution-mode timestamps remain separate.

Double-colon records retain their own update-time flags and target-variable
views. The first branch owns command-line target identity and explicit
assignments; a later recipe can inherit values through a separate local table
while the first recipe remains inactive. Nine original full-transcript cases
cover attribution, inactive branches, and original physical file effects.

Pure dry-run simulation also retains branch-local synthetic times, while
mixed forced and simulated executions can publish their observed physical
time. Six further original GNU2 comparisons cover this distinction. Explicit
and implicit grouped rule records have separate remaining requirements.

Grouped database cases separate the owner, peer records and the one logical
rule declaration. Explicit groups retain each peer's observed filesystem
mtime and GNU's reported peer order. Implicit multi-output rules also retain
owner simulation timestamps and the peer's independently observed directory
identity. The fixtures compare full output and separately check physical
filesystem times and effects. Explicit grouped double-colon is tracked
separately.

Explicit grouped double-colon records also track peer completion independently
of the rule owner. The selected failure and cancellation models cover recipe
exit, actual signal, `.DELETE_ON_ERROR`, and aliases. The database keeps the
declared peer spelling and reports its observed physical timestamp after
cleanup; an interrupted ordinary run does not publish a misleading dump.
