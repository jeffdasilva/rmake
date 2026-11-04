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

The 147 independent models cover allocation/free, NULL and NUL results, byte
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
| [qualification-147.json](qualification-147.json) | Complete immutable debug/release147-pass and preceding147-fail reports, executable identities and packaging-time source hashes. This historical set establishes its recorded profile rather than a complete native inventory. |
| [benchmark-evidence.json](benchmark-evidence.json) | All nine original benchmark/control metadata reports, including negative and mixed results, with CPU23 affinity metadata for lookup and CPU follow-ups. |
| [null-floc-history.json](null-floc-history.json) | Original C sources, observer and complete five-model NULL-versus-empty setup filename discovery. Three current normal-input mismatches remain open; two explicit-empty controls match. This discovery adds no runnable default case, gate credit or approved exception. |

Each manifest checks both its compressed bytes and uncompressed JSON. Complete
original reports are retained inside the archives, with their own byte hashes.
The [measurement note](../../docs/benchmarks/native-runtime.md) explains the
comparisons and limitations. No general speedup or GNU performance parity is
established. NULL filename provenance, enabled Guile/GC interaction and broader
native/resource/profile compatibility remain open in M3/M6.
