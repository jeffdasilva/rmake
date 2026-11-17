# Linux load limits and child stdin leases

The M5 runtime implements `-l`, `--load-average` and `--max-load`, including
recursive propagation, explicit child overrides, bare-option reset, C floating
prefix parsing and `%g` flag spelling. Negative values disable gating without
losing their spelling (except the default -1); NaN also disables the comparison.
These observations are invocation-local, never persisted or used as graph facts.

The interpreter expands all recipe bodies before waiting for a local slot or
jobserver token. Shell configuration and exports remain deferred until dispatch.
A load-delayed recipe retains its token and expanded continuation, but owns no
process or stdin lease. Delayed recipes resume in LIFO order on scheduling
activity or child completion. The first active command always runs, allowing
progress even with a zero limit. Ordinary abort discards queued work and returns
its permits; keep-going and ignored errors let that work continue. Running recipes
retain their existing ordinary-failure continuation policy.

On Linux the primary observation is the instantaneous runnable count in
`/proc/loadavg`, compared strictly greater than the limit. The cached descriptor
is rewound and reread for each observation. Interrupted operations retry; invalid
or failed observations select libc's load average with recent-launch pressure.
Repeated fallback errors report once per error number. No load observation is
reused after a child or scheduling decision as a supposedly permanent fact.
Load changes alone do not wake an ordinary child wait. A blocked jobserver wait
with load-delayed recipes has GNU's one-second retry; token arrival still wakes
immediately. A controlled wait-entry load transition proves this retry rather
than relying on a host load change or a completion sleep.

Stdin belongs to one child command at a time. On completion another target's
continuation can acquire it. Other commands receive an owned read pipe with its
writer closed, preserving both EOF and the FIFO descriptor type. Descriptor
creation is atomically close-on-exec, including concurrent worker launches.

## Evidence

33 original required fixtures cover numeric/recursive flags, serial load queues,
expansion-before-token acquisition, stdin handoff, EOF and descriptor type.
The four stdin fixtures passed 20 repeated oracle and candidate executions each;
the previous release fails the handoff/type observation. Load fixtures use bounded
expansion barriers rather than a completion sleep.

`make test-load` runs 21 native controlled profiles three times against both GNU
and rmake: above/equal/below load, integer boundaries, disabled limits, load changes
after a peer completion, EINTR, malformed/read/open fallbacks, unsupported/error
sampling, and queued ordinary/ignored/keep-going failures. A test-only original
LD_PRELOAD helper supplies system observations, never Make behavior. Explicit
concurrency/zero-active-worker, exact stream/status, queue/membership and journal
anchors reject two engines agreeing on an incorrect result. Only the inherently
unordered parallel completion journal uses a named membership validator; its raw
order is retained. All other file observations remain exact. Four tooling tests
exercise these anchors with deliberately corrupted results.

The campaign is in development CI and the optimized regression flow. The
[M5 exit audit](m5-exit-audit.md) combines it with signal/cancellation, output
synchronization, synthetic completion and mixed-tree runtime acceptance.

## Verbose named includes

During a verbose remake pass for required named includes, eligible load-delayed
recipes start before exited children are collected. A simulated recipe can finish
without reaping a child. Its completion still starts a new pass for the debug
visit set. The `-q` path reports a completed cached question result when another
pending source causes the graph to revisit it.

The 36 held profiles cover `-n`, `-q` and `-t`, each named-source position,
high and low load, and both pipe and FIFO jobservers. Two high-load shapes hold
the short named recipe until both pending sources appear in the captured trace.
The observer checks the actual child identity and retains exact release streams,
final streams and filesystem effects. These profiles cover required, direct
sources with bounded nonrecursive recipes.

Direct optional named sources use the same load-pass boundary. An optional
update can finish the pass while a required sibling is queued. Dry run can
simulate the sibling before re-reading the sources, and question mode can
finish without starting it. Verbose output prunes completed earlier sources
before visiting a later root. An additional 72 held profiles cover these
optional sources with and without verbose tracing across both transports and
all three build modes. More complex named sources remain part of the M3/M6 audit.

The load-limited remake path also accepts a directly named include in a simple
relative directory. The 180 directory profiles cover required and optional
sources, quiet execution, and basic and verbose debug output. Each trace uses
the directory-qualified target and the actual child that writes it.

Named load remakes can run with the stock built-in rules enabled when every
source has an explicit direct recipe. An empty explicit recipe on the primary
makefile also qualifies. The 180 stock-rule profiles exercise the same quiet,
basic and verbose schedules through both jobserver transports. Sources that
need an implicit recipe still require separate remake scheduling coverage.

An existing first makefile without an explicit rule can also complete its normal
stock implicit search before the named includes overlap. Parallel admission
requires the primary to settle unchanged without selecting a recipe. Another
180 profiles verify this case, including the primary search's directory-cache
diagnostic in verbose traces. The rule selection and visit run once in the
normal graph.

With a single required named include, a stock implicit primary recipe can
overlap that include. The last named build mode stays in effect while the
primary's running command advances, including when the include's recipe has
already simulated completion. Twelve held shell-suffix profiles verify
the recipe's live child and load sample, dry-run command order and file mode,
question-mode cleanup, and touch-mode restarts.

Twelve additional held profiles cover a single optional named include. When
its query finishes in the load queue, the cached outcome retains the later
required-goal diagnostic, just as for a query that settled during its visit.

When every remaining makefile source is directly named, the stock primary can
also overlap more than one include. Twenty-four additional held profiles cover
two direct includes, including an optional first include that completes its
pass while the second waits. The profiles compare the live release, command
order, query cleanup, touched files and final mode. Sixteen further held
profiles combine a named source and a required unnamed source in quiet dry-run
and query modes. They verify real include contents and the primary file's mode
after the pass. Sixteen further held profiles cover standalone touch in both
include orders. A later named touch runs before the queued unnamed include
can resume in its mode. At the pass boundary the scheduler restores the
unnamed include's mode before refilling the load queue. The 40 mixed-source
debug profiles cover basic and verbose remake traces, both load levels, and
both include orders for standalone touch. After an optional source ends a
verbose pass, the next wait uses the progress recorded at that boundary.
The 48 quiet combined-mode profiles check touch with dry run, question,
or both. A query can stop the active primary continuation and remove the
file while a successful ordinary touch still determines the final status.
Fourteen held combined-mode debug profiles cover an optional named source
followed by a required unnamed source. The verbose pass prunes a completed
primary immediately when its query verdict ended the recipe. Other debug
masks and the number of visits during unheld high-load races remain open.
The 128 held query profiles cover three direct includes, one or two named
goals, every read order, and the stock primary worker under three job slots.
They exercise `-q` alone and with `-n`. An optional source read after the
first include can end the pass before the later includes run. High-load
queries with an optional named include and a final unnamed include showed
different sets of generated files under identical held primary releases.
Those schedules remain outside the exact gate. Eighty further held profiles
place named goals first and exercise quiet dry-run, touch, and mixed flags.
Twenty-four standalone dry-run profiles interleave named and unnamed roots.
They verify which recipes leave files and which print commands across rereads.
Twelve required-source profiles place one named include in the middle
under `-t`, `-n -t`, and `-q -t`. They compare the named touch before
unnamed load reservations, the primary's mode, and actual file effects.
Sixteen optional-source profiles put the optional named include between two
required unnamed includes. The earlier unnamed job waits for ordinary mode
after the optional pass. Eight required-source dry-run profiles place
two named includes last, with a slot for every source. They verify the
load queue order in both restarted images. The high-load profiles hold
the unnamed recipe until its restarted parent waits for it. Lower slot counts and unbounded dry runs under controlled load remain open.
Thirty-two related profiles omit the load option. The held cases release
the primary or earlier unnamed recipe first at a verified parent wait with
two, three, and four slots. They compare both raw release prefixes and
whether the primary recipe's chmod actually changes the file mode. Eight
fast unnamed-recipe controls cover both transports with three and four slots.
Sixty-four required-source profiles cover final-adjacent touches with three
or four slots under
`-t`, `-q -t`, `-n -t`, and `-n -q -t`. They check touch order,
primary removal under high-load query, and the earlier unnamed recipe's
file contents.
Thirty-two related touch profiles omit the load option and verify the
named visits and earlier recipe complete in the same order with no load probe.
Sixty-four held profiles also cover an optional include in either of the
final two named positions, including its effect on the active primary
continuation, the queued earlier include, and a later required visit.
The same required and optional touch sequences run with two slots.
The no-load cases also cover an optional named source with two, three,
and four slots. These 128 additional profiles verify the same bytes,
primary mode and file effects across the tested job counts.
Ninety-six further touch profiles exercise unbounded `-j` with and
without a load option, including both optional-source positions.
The required named sources can also extend beyond the final pair.
Ninety-six more cases check three or eight named includes with two or
four slots, including exact order, primary mode, and file effects.
One optional include can also appear anywhere among the named sources.
The 144 additional cases cover all three optional positions in a
three-name tail and the pass boundary before a later required visit.
Ninety-six further cases make both named sources optional and confirm
the same touch policy through the first optional completion.
Four cases also cover three named includes with two optional ones, where the
required include comes first or in the middle. They check normal output and
primary mode or deletion with and without a load limit.
Four more cases put both optional includes before the required include.
The high-load touch cases check that the earlier queued include resumes
at the second optional pass, before the final named include.

If a missing required include is skipped because it is phony or has an
unconditional double-colon recipe, the failed read prevents a restart after an
otherwise successful remake. A separate remake failure still lets physically
updated peers trigger a reread. The eight serial Makefiles cover both rule
forms, an existing or optional skipped file, and a failing optional peer.
Thirty-six held profiles cover the missing phony source among two successfully
remade peers under `-n`, `-q` and `-t` with both jobserver transports.

## Fallback diagnostics

A failed load-average probe renders the active catalog's warning. The
reported operating-system error uses the active libc message locale.
Four French profiles cover an unsupported probe and an I/O error with
both jobservers. They retain the verified probe order, queue order, and
byte-for-byte streams alongside the existing C controls.

## Reference study

Consulted the verified GNU Make 4.4.1 source archive, SHA-256
`dd16fb1d67bfab79a72f5e8390735c49e3e8e70b4945a15ab1f81ddb78658fb3`:
`src/job.c` (`new_job`, `start_waiting_job`, `start_waiting_jobs`, `reap_children`,
`load_too_high`), `src/main.c` (floating option parsing, flag rendering and the
skipped-source restart decision),
`src/misc.c` (`make_toui`) and `src/posixos.c` (`get_bad_stdin`). The Rust state
transitions, owned descriptor adapter, fault helper and fixtures are original;
no GNU implementation or test material was copied or translated.

Basic makefile diagnostics (`--debug=b,m`) retain the parallel final named
touch schedule. Twenty-eight held profiles cover required names under a load
limit and one optional name without a limit, through both pipe and FIFO
jobservers with two, three, four, and unbounded job settings. They check the
primary-worker release, Makefile mode or deletion, restart output, and final
goal effects across touch, question, and dry-run combinations.
Five more held profiles test three or eight required names with `-j2`
and a load limit. They verify the same release and completion effects.
Six further profiles cover higher and unbounded job counts under load and
no-load runs with two or unbounded slots.
Six more profiles place a single optional include at either end of three
named makefiles and compare release, restart, deletion, and file modes.
Four more profiles place the optional include in the middle.
Six more profiles cover two optional includes among three named sources
under a load limit. They check all three positions for the required source.

## Performance observation

Paired, alternating optimized runs used two warmups, 101 core samples and 31
parallel-branch samples per engine. No other local functional gate ran during
measurement. Median milliseconds (GNU / current / prior main): tiny no-op
0.677 / 0.805 / 0.804; 10,000 no-op rules 11.567 / 32.329 / 31.737; expansion
2.930 / 4.414 / 4.466; parallel depth one 88.187 / 89.582 / 89.575; depth four
88.310 / 89.395 / 89.341. These selected observations show roughly unchanged
runtime cost, not general parity or an acceleration claim. There was no CPU
isolation or CPU/RSS/I/O acceptance threshold. Raw observations and executable
identities remain in ignored `artifacts/bench/m5-load-{core-101,parallel-31}.json`.
