# Database output and logical execution history

GNU Make 4.4.1's `-p` output exposes execution history as well as final values.
The selected implementation records that history before startup, then renders
borrowed variables, rules, file records, directory observations, scope identity,
update outcomes, timestamps and cache statistics. A late `MAKEFLAGS` selection
retains earlier events. Formatting performs no expansion, lookup, filesystem
search, callback or assignment.

The first combined database/locale checkpoint passed 640 Rust and 727 tooling
checks with strict Clippy. Frozen development and optimized builds both pass
41 complete database profiles, six native rounding/registration profiles,
18 floating-environment controls, 74 collation profiles and 183 startup-locale
profiles. These are finite functional results. Full selected-head local/CI
acceptance and final total-cost measurements remain merge requirements; they do
not establish complete M3/M6 or general performance parity.

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
