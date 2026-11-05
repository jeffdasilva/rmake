# Database output: implementation in progress

GNU Make 4.4.1's `-p` output exposes execution history as well as the final
Makefile model. Reconstructing only sorted variables and rules would give an
incorrect result. The merged mainline continues to reject `-p` while the complete
protocol is being implemented. The unmerged implementation now passes the
thirty-three selected complete profiles
in frozen development and optimized builds, including exact logical statistics.
The original fourteen models remain unchanged; nineteen additive phase models
cover assumed ages, double-colon scopes/results/timestamps, actual parallel
children, native setup/finalizers, recursion/restarts, early failures, late
selection, shared local tables and post-update deletion controls.
Its first complete run failed all fourteen stdout comparisons; the subsequent
failed runs and initial model corrections remain historical evidence. This is a
finite profile result, not mainline enablement or a complete database claim.

The current pure components live in `crates/rmake-core/src/database.rs`.
`VariableStore::database_definitions()` borrows the root definitions in GNU hash
slot order without flattening active target layers or evaluating dynamic
metadata. The printer reads an already-populated `.VARIABLES` cache without
refreshing its generation; an unqueried cache stays uninitialized. Target scopes
and pattern declarations have their own borrowed views.
The renderer preserves origins, private modifiers, source identities/line
numbers, raw recursive values, dollar escaping for simple values, multiline
`define` values, whitespace-only values, recipe prefixes, and declaration order.
Formatting has no expansion context and performs no stat, file search, module
callback, shell call, or assignment.

`FileView` requires the host to supply cached timestamp/update/search facts,
explicit flags, the selected rule and retained automatic bindings. It does not
infer that a missing file succeeded, rerun an implicit search, or recreate a
finished target scope. The host must supply the observed GNU file hash and
double-colon branch traversal order. `DirectoryView` similarly distinguishes
failed stat, failed open, complete enumeration, and partial enumeration, and
requires actual cached device/inode/count information. Linux filesystem cache
accessors now borrow existing generations, physical aliases, stat/open state,
stream state and entry counts without new I/O or runtime fields. The host
supplies GNU ordering and retains actual impossible-file history at the search
actions that create it. `DatabaseView` composes
those immutable sections; the host supplies a fresh clock independently at each
opening and closing emission point.

`TableTelemetry` models the observable logical table, independently of the Rust
collections: counted lookups/collisions, tombstones, growth/cleanup rehashes and
slot traversal. `StringCacheTelemetry` models the pinned 64-bit Linux cache's
unique NUL-terminated storage, buffer accounting, hit rate, and logical hash
history without allocating a second string arena. Both remain absent from the
merged production path. The unmerged work now
starts a shared `History` before startup definitions, records actual assignment,
reference, undefine, filename-preparation and file-enter events, and gives target
and pattern tables distinct identities. Late history enablement is rejected.
Host metadata reads use an explicit recorded accessor; ordinary map inspection
and formatting stay neutral. Coverage of the complete GNU phase history remains
incomplete; final-map cardinalities cannot stand in for missing lookup events.

The initial recording mode deliberately bypassed pure, alias, linear and text
plans. The current optimization work restores those paths with deferred logical
reads: declined plans publish nothing, selected plans publish each completed
prefix before an effect or diagnostic, and repeated immutable lookups retain
their multiplicity. Core units cover discarded prefixes, cyclic traversal and
receiver-visible warning order. The restored paths pass all thirty-three complete transcripts;
ordinary/diagnostic allocation and timing measurements remain required. Each accepted pure prefix now snapshots each scope layout once and accumulates
lookup/collision deltas without storing every referenced name or taking a lock
for each read. Shared history locks and retained early events remain a performance
review item; this work does not establish general performance parity.

The driver samples GNU's actual clock domain, libc `time(NULL)`, before and after
each execution, independently of output. Linux vdso/coarse time can lag Python's
high-resolution realtime near a second boundary. Both observations remain in
reports, but a ctime slot must match the same-domain bounds exactly; the driver
adds no tolerance. The original high-resolution-only boundary failure is retained
as a harness-model defect rather than converted into a compatibility exception.

The selected implementation now retains actual variable/file/string events,
borrows cached directory and engine outcomes, preserves automatic-variable
contexts, and emits the initial selected fatal/success dump lifecycles. GNU
algorithms were consulted as reference; diagnostic formatting does not replay
reader, expansion or search effects. The remaining acceptance work is:

1. Broaden startup, option-change, native, recursion, remake/restart,
   parallel and double-colon ownership evidence beyond the finite thirty-three
   profiles. Preserve earlier evidence and failed models while fixing actual
   event placement. Help/version database boundaries need separate cosmetic
   slots; they are not covered by the current early-failure controls.
2. Continue checking logical scope identity and finished state across wider
   interactions. Selected explicit target/automatic tables now share one history
   despite separate physical Rust layers; double-colon branches retain separate
   values/results and share the final timestamp after the chain finishes.
3. Retain the default regressions and measure default operation as well as `-p`:
   later selection requires early history, but must not force interpreter
   fallbacks or repeated user effects. Both profiles pass native221, debug64,
   directory40 and JOBS9; workspace628 Rust units,681 tooling tests and strict
   Clippy pass. These functional checks are not performance evidence.
4. Broaden resource, native/Guile, locale and real-project integration evidence
   before claiming the database surface complete. Mainline remains unsupported
   until the selected change passes its required exact-head gates.

The original fixture/model boundary and narrow clock/physical identity relations
are described in [the database evidence inventory](../tests/database-output/README.md).
Initial independent hypotheses were corrected for GNU's simple-looking pattern
variable declaration format and missing optional include state; their original
predictions and complete raw observations must remain historical evidence.
These selected profiles do not close broader asynchronous and native-finalizer
interactions, enabled Guile, non-C locales, deep resource cases or
all real projects. M3/M6 remain open.

The additive input/hypothesis/refinement history and strict boundaries are
described in [the phase inventory](../tests/database-output/PHASES.md). The first
28/28 checkpoint and final 33/33 checkpoint have separate immutable qualification
archives. The final source closure includes the 86 application-build and selected
database-harness Rust/C/data/build inputs and both binary identities; optional Guile source is identified but its
feature is disabled. Two deletion controls passed the preceding implementation
and did not reproduce a suspected stale timestamp fallback, so no speculative
change was made. Confirmed strong-local metadata and double-colon final-time
mismatches retain their preceding failures and independent GNU2 evidence.

The 681-tool count comes from the current combined development checkout,
including eleven separate optional-component tooling controls. A database-only
isolated change must rerun its own exact inventory and retain that count.
Broader regression driver/input identities remain in their raw reports; the
selected database source closure is not presented as the entire repository.

The isolated database branch now passes all 33 original transcripts in both
builds, 629 Rust tests and 671 tooling tests. It reproduces the observed builtin
definition order through an explicit declarative name-order file, with
`make test-builtins` checking regeneration. Earlier combined-checkout counts
remain historical and do not describe this isolated branch.

Always-on history has a measured cost. The first arena compaction reduced
allocation requests but still regressed large and expansion timing. The
exploratory performance archive and `optimization-history-v1` retain all raw
results, including the temporary build-prefix mismatch and corrected exact
comparisons. Arena deletion now bounds dead key bytes independently of hash
rehashing; accepted snapshots remain immutable. Single-pass scope lookup and
in-place timestamp replacement remove additional repeated work. Their final
timing qualification remains pending. Native rounding/registration history,
locale integration and full selected-head acceptance remain open before merge.
