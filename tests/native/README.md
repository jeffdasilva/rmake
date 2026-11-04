# Original native module acceptance profile

These C modules and Makefile models are original fixtures. Their declarations
match the public GNU Make 4.4.1 ABI; no GNU implementation, header or tests are
included. The negative modules deliberately omit the license marker or setup
symbol. They remain GPL licensed source files.

`tools/native_modules.py` compiles every module once per selected case into
disposable inputs, then runs the pinned GNU reference twice and the candidate
with identical executable aliases, workspace paths, input bytes and timestamps.
The existing conformance runner bounds processes, output and filesystem capture,
resets each execution and rejects leftover children. The compiler also uses the
bounded process-group capture. This is a trusted-fixture gate, not a filesystem
or network security sandbox.

The 215 independent models cover allocation/free, NULL and NUL results, byte
arguments, eager/raw arguments, direct versus call arity, local/target/automatic
scope, fresh registrations, name/arity/flag validation across reading/includes/recipes,
expansion/eval effects, errors and source locations,
`.LOADED`, setup results and malformed libraries, constructor re-entry, loaded
module aliases, includes/restarts, atomic module replacement, destructor lifetime,
process-exit API calls, closed-stdout FD reuse, explicit line boundaries, and fatal callbacks with parallel FIFO/pipe jobservers.
Sixteen stdio profiles cover reading and recipes in none/line/target/recurse modes,
with separate and merged output descriptors. Seven finite native-expansion profiles
cover depths through 1,024, active locals, one leaf effect, fatal suppression and
active-variable cycles. Native stdio bypasses GNU's spools;
pending partial C stdout is not flushed by an interpreter warning. At normal and
fatal process exit GNU closes stdout before library destructors, while their
interpreter/file effects and stderr remain available.

Every expected stream, status and ordered `seen` witness is explicit. Repeated
GNU differences, wrong independent models, candidate differences, resource bounds,
and leftover children always fail. `--oracle-only` checks both GNU observations
without accepting a candidate; it explicitly grants zero compatibility credit.
This separate profile does not add or change ordinary requirement-ledger counts.
The quarantined built-in and active-function replacement crashes are excluded
from both these source modules and the default campaign.

Run `make test-native-modules`, or select a profile with
`make test-native-modules NATIVE_CASE=native-arguments`. The underlying runner accepts `--oracle`, `--candidate`, `--report`, repeated exact
`--case` IDs and `--cc` as a compiler executable path. For model qualification,
replace `--candidate` with `--oracle-only`. Compiled shared libraries are temporary
and are never checked in.

The compressed historical reports preserve initial incorrect optional-missing
module predictions, stdio/warning/call hypotheses, a recipe-construction escaping
error, the destructor-stdout correction, and the first candidate's 53/55 result.
Their manifests and source hashes are checked by the tooling tests. These records
receive no additional fixture or safety-exception credit. The historical
source/archive paths are evidence and are not executable default cases.


## Retained evidence and open profiles

| Archive manifest | Contents |
| --- | --- |
| [startup-exit-history.json](startup-exit-history.json) | Complete original first/relative/refined startup-finalizer discovery, 31 strict GNU2-qualified models, 27 preceding candidate differences and four matching controls. Includes independent full-filesystem verification and initial rejected hypotheses; candidate acceptance remains separate. |
| [startup-exit-performance.json](startup-exit-performance.json) | All three startup-owner elapsed reports and 16 allocation reports/raw traces, helper bytes/hash and immutable final source reference. Every mixed timing result and allocation increase remains retained; CPU23 applies only to elapsed campaigns. |
| [qualification-startup-215.json](qualification-startup-215.json) | Complete frozen startup-exit debug/release215-pass reports and preceding main188-pass/27-fail report, with coherent source/input/module hashes. Ordinary and full developer acceptance remain separate gates. |
| [qualification-borrowed-184.json](qualification-borrowed-184.json) | Complete final borrowed-location debug/release184-pass and preceding main184-fail reports, with the coherent source/input/module hash snapshot. Full developer/ordinary acceptance remains separate. |
| [qualification-184.json](qualification-184.json) | Complete corrected frozen debug/release184-pass and preceding main184-fail reports, with source/input/module hashes. Ordinary and full developer acceptance remain separate gates. |
| [qualification-147.json](qualification-147.json) | Complete immutable debug/release147-pass and preceding147-fail reports, executable identities and packaging-time source hashes. This historical set establishes its recorded profile rather than a complete native inventory. |
| [borrowed-performance.json](borrowed-performance.json) | All four final timing reports, both historical-baseline comparisons, four allocation reports/raw traces, helper source/hash and immutable final source reference. Mixed/slower results remain included; CPU23 applies only to timing. |
| [provenance-performance.json](provenance-performance.json) | All three source-provenance timing reports and 16 allocation reports/raw traces, frozen executable/source references, helper bytes/hash, mixed timing results and requested-byte increases. CPU23 applies only to the timing campaigns. |
| [benchmark-evidence.json](benchmark-evidence.json) | All nine original benchmark/control metadata reports, including negative and mixed results, with CPU23 affinity metadata for lookup and CPU follow-ups. |
| [null-floc-history.json](null-floc-history.json) | Original C sources, observer and complete five-model NULL-versus-empty setup filename discovery. Records three preceding normal-input mismatches and two matching explicit-empty controls. The later 37-model source-name suite checks the fix; this historical discovery adds no extra gate or exception credit. |

Each manifest checks both its compressed bytes and uncompressed JSON. Complete
original reports are retained inside the archives, with their own byte hashes.
The [measurement note](../../docs/benchmarks/native-runtime.md) explains the
comparisons and limitations. No general speedup or GNU performance parity is
established. The 184-model gate accepts the selected nullable filename profiles;
enabled Guile/GC interaction and broader native/resource/profile compatibility
remain open in M3/M6.


The source-name suite adds 37 independently authored profiles for nullable,
explicitly empty, named and literal `<builtin>` filenames. It observes setup
location pointers, immediate and deferred warnings, nested include restoration,
module reloads, and late builtin removal. The first 15 incorrect fixed-line
predictions and complete observations are preserved in
[source-provenance-history.json](source-provenance-history.json). The existing
`builtin-builtin-failure` and `trace-builtin` controls retain their original
ordinary-fixture credit and are not counted again among these profiles.

[full-gate-first.json](full-gate-first.json) preserves the failed full frozen
acceptance at c9499: the oracle passed 27,766 cases, while the candidate passed
27,764 and failed those two ordinary built-in diagnostics. It includes the exact
log, status, binary/source identity text, fixture inputs, streams and reports.
That failed head is not final acceptance evidence. The original 147-model native
qualification and all benchmark archives remain unchanged.


Thirty-one startup/finalizer profiles observe early modules loaded through
GNUMAKEFLAGS, MAKEFLAGS, eager command-line assignments and HOME operands. They
cover help/version exits, invalid synchronization, directory failures, successful
`-C`, eval failures, partially published include metadata, restarted decoding and
fatal finalizer suppression. The trusted original C fixture redirects stdout to
`/dev/null`; stderr, status, ordered `seen` and the complete filesystem remain
strict. Two additional observations within this set check pending C stdout at
help/version exit. No banner or diagnostic normalizer is used. Distinct restart
bindings and origins reject finalization through the preceding database, and
missing/default metadata distinguish the actual early phase from a completed
synthetic Engine. The historical 184-profile qualification remains unchanged;
these 31 models first establish GNU-only qualification. Candidate acceptance
remains separate, and compiled profiles receive no ordinary-ledger credit.
