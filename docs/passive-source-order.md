# Sequential post-read flag sources

GNUMAKEFLAGS expansion, decoding and payload evaluation now finish before the
following MAKEFLAGS value is expanded. GNUMAKEFLAGS is cleared after its payload
definitions finish. A definition can still inspect the original GNU source;
the next source sees its completed definitions, file effects and option policy.

Each source uses the existing owned decoder and cursor interpreter. Its source
expansion can re-enter live MAKEFLAGS evaluation; no mutable Options borrow
spans a core callback. Each completed source publishes its current receiver
snapshot. A correct experiment that skipped final publication made fewer
allocation requests but established no reliable elapsed improvement; the simpler
publication path is selected.
Saved invocation files, directories, goals, definitions and warning provenance
retain their ownership. Explicit CLI job priority persists; final job selection,
MFLAGS and output-mode activation keep their separate commit boundary. Passive
include operands accumulate without constructing a new search; actual live
MAKEFLAGS resets retain fresh search publication and physical observations.

No unsafe code, dependency, value cache or filesystem-observation cache is added.
Known malformed-value ordering, contextual runtime HOME and the finite nested
getopt mismatch retain their ordinary compatibility requirements.

## Independent evidence

1,290 required original models include a 1,152-case declaration/assignment/
priority/environment/warning/byte matrix, 96 initial-warning controls, six exact
pilot sources and 36 actual jobserver/invocation controls. All models agree with
two bounded GNU Make 4.4.1 observations and both selected frozen builds.
Preceding rmake fails every case. Exact merged/separate output, statuses and
filesystem bytes/types/modes/links agree without reached bounds or leftover
children. Independent ordered file witnesses and snapshots distinguish eager
effects, lazy reads, stronger retained values, conditional skips and source
clearing. The main matrix compares descriptor order without output normalization.

The job controls use repeated `-f` files and an explicit goal; they verify CLI
job priority, last-source selection and actual available tokens on Linux FIFO
and pipe transports. One active recursive recipe observes nonblocking reads,
restores descriptor flags, and returns every token before exiting. Unexporting
the raw MAKEFLAGS source isolates the post-read effects from child-environment
re-expansion. Existing parallel/runtime gates remain required.

132 rejected original models remain unchanged: they expected an unlocated
warning for lazy A reads through MAKEFLAGS. That recursive source retains its
definition file/line. Separately named source-locus refinements derive that
location from the exact input. No observed stream was copied into an expected
model. The six original pilot observations also remain unchanged and earn no
extra credit. [Raw proof](../tests/research/passive-source-order.json) retains all
inputs, modes, arguments, environments, models, topologies and observations.

Eight tooling tests protect the full inventory, raw evidence, merged order,
file snapshots, exact pilots, rejection/refinement boundaries, the original
prototype archive and actual token controls. Formatting, Clippy, 523 Rust and
561 tooling tests are core gates. The first correct prototype passed the entire
preceding 21,601-case corpus and all 68 candidate-only safety cells. Full selected
frozen rmake-driven `m5-acceptance` and exact-head development/optimized CI remain
merge gates; final results are recorded in the PR.

The ordinary corpus becomes 22,891 required / zero pending across 149 families.
The same 17 approved safe-error exceptions remain separately gated, with zero
exact-conformance credit. M3/M6, native loading, enabled Guile, locale/Linux
profiles, directory/debug/database/shuffle output, true exec/stdin, expanded
integration and general performance parity remain open.

The reference is x86-64 Linux/glibc, C locale and disabled Guile; profile
`92525c9784f4dd471cceb12ce66a9d367e267b638e1384cdc29962b738d16de7`.
Oracle SHA256: `b8806e8eb7452ed9b7ed5e18cb7e76ff9abd488c9d62a266a398d8044fade974`.
Selected debug SHA256: `174c41f5cfeb42ac0907afe80abf80dcd1b93d6983939d9c32c8de1cdf711b75`.
Selected release SHA256: `01e505974a7d73602c237e0ac479702ca5097dbc1cd7d24fabf0e8fd7d8d43c7`.
All implementation, models, probes and fixtures are original; no GNU code or
tests were copied or translated. GNU is never a runtime implementation fallback.

## Prototype retention and performance controls

The first correct version published receiver state after both sources. Its
complete then-current 1,254-case debug/release observations, original source
patch and identity remain in the [initial archive manifest](../tests/research/passive-source-copy-prototype.json)
and [compressed raw proof](../tests/research/passive-source-copy-prototype.json.gz).
Compressed/uncompressed and source-patch hashes are checked. Those same executable
bytes are selected after investigating copy elision. This earlier archive adds
no conformance or performance credit and predates the 36 token controls; the
selected proof above qualifies all 1,290 cases.

The correct but unselected elision experiment retains its complete 1,290-case
debug/release proof and source patch in a separate
[manifest](../tests/research/passive-source-elision-prototype.json) and
[compressed archive](../tests/research/passive-source-elision-prototype.json.gz).
It reduced allocation requests. Two direct elapsed batches had 18–26% higher
large saved-argument medians; reverse-order controls were mixed and did not
reproduce that magnitude. These observations do not establish a reliable elapsed
advantage for elision. All measurements remain retained, and the original
publication path is restored without claiming a speed advantage for either
version. The cause of that timing variation remains unqualified.

Allocation and elapsed observations are recorded below. Newly corrected inputs
receive no speed credit against the incompatible preceding build; no general
performance-parity claim follows from these controls.

Twelve repeated, independently output-checked profiles measure glibc request
counts on existing required recipe-free controls. Both repeats give the same
counts and bytes. Selected-copy reports use the same executable bytes as the
selected frozen release, with the earlier prototype pathname.

| Saved definitions / 100 updates | Preceding calls / requested bytes | Selected calls / bytes | Unselected elision calls / bytes |
| --- | ---: | ---: | ---: |
| 0 | 9,593 / 531,660 | 9,604 / 531,793 | 9,592 / 531,560 |
| 512 | 20,544 / 1,181,445 | 21,071 / 1,211,054 | 20,544 / 1,181,857 |

The selected version adds 527 requests and 29,609 requested bytes on the larger
control versus preceding rmake, about 2.6% and 2.5%. These are allocation requests,
not retained/peak memory or CPU measurements. Aligned allocation entry points
are outside the observer. Elision's lower counts did not establish lower elapsed
time, and receive no selected-code improvement claim.

The [report index](benchmarks/passive-source-index.md) links every profile, raw
trace and timing report. Four reports from an accidentally overlapped validation/
timing sequence are retained separately with zero performance credit. Their
[exclusion record](benchmarks/passive-source-overlap.json) explains the overlap;
fresh selected batches began only after both validation and that sequence exited.

Four fresh selected/preceding reports have 101 alternating samples per engine
and workload, two warmups and exact output agreement. GNU drives A and selected
rmake drives B through the top-level Makefile. No unrelated local build, test
or probe campaign ran during those batches. Host activity is uncontrolled;
CPU/RSS/I/O, confidence intervals and warning/source-heavy performance remain
unqualified. All timed controls already match preceding rmake.

| Workload | GNU / selected / preceding median ms A | B | Paired preceding/selected A / B |
| --- | ---: | ---: | ---: |
| tiny-noop | 0.680 / 0.818 / 0.813 | 0.563 / 0.657 / 0.654 | 1.001 / 1.003 |
| large-noop | 11.264 / 30.432 / 30.964 | 11.210 / 29.910 / 30.340 | 1.014 / 1.018 |
| expansion | 2.909 / 5.113 / 5.159 | 2.800 / 5.091 / 5.183 | 1.006 / 1.018 |
| 0 definitions / 100 updates | 0.866 / 2.590 / 2.664 | 0.855 / 3.129 / 3.000 | 1.029 / 0.968 |
| 64 definitions / 100 updates | 0.908 / 2.636 / 2.602 | 0.938 / 2.963 / 3.039 | 0.960 / 0.986 |
| 512 definitions / 100 updates | 1.264 / 3.366 / 3.369 | 1.264 / 3.593 / 3.528 | 0.998 / 0.941 |

Ratios above one favor selected rmake. Large-noop medians are about 1.4–1.7%
lower; tiny-noop is slightly higher and runtime results remain mixed. GNU remains
faster. These controls establish neither general parity nor a general speedup.
The index retains four selected, six elision and two reverse-control reports,
plus the four separately excluded validation-overlap reports.
