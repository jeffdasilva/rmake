# Include operand history and fresh metadata

GNU compares each incoming `-I` filename byte spelling with the already stored
operand history before expanding HOME or removing leading `./`. An accepted
operand can therefore normalize to an existing spelling and still remain a
separate history entry. `inc` then `./inc` retains two entries; `./inc` then `inc`
retains one. Trailing slash and internal dot spellings remain distinct at this
stage. Resolved search presentation trims trailing slashes. A retained `-I-`
clears the resolved search accumulated before it and disables default paths;
it does not erase the decoder's operand history.

Startup now indexes stored filename bytes while preserving the ordered vector
and every accepted entry. This removes repeated linear history scans for both
include and directory operands. Runtime decoding seeds the existing include
history directly and serializes only other switches for their existing merge
policy. Incoming raw names are compared before literal prefix normalization.
Removing constructor and runtime post-filters prevents PathBuf equality or a
second set from discarding retained operands. Argument cursors, job priorities,
origins, eval transport and public recursive flag encoding keep their contracts.

Every retained directory receives a fresh physical metadata check, including
identical paths created by normalization and entries before a later barrier.
Each retained search entry performs its own full metadata append with effects.
Passive late flag decoding still waits for an actual special flag reset before
publishing a search. No filesystem observation or expansion result is cached.

941 new required independently authored cases cover 938 semantic cells and three
output-checked scaling controls. The semantic matrix covers raw spellings,
barriers, initial HOME effects, reader/recipe resets, passive late options,
real include reads, counted file effects, output modes and generated-include
rereads. Two bounded pinned GNU observations agree with every final model; both
frozen builds match bytes, status and files. Preceding rmake fails 706 cells and
passes 235 controls. The preceding 11,737-case corpus also passes under the change.
The full ordinary corpus is now 12,678 required / zero pending, plus 17 unchanged
separate approved safety exceptions. This is a bounded M3/M6 slice.

The [raw evidence](../tests/research/include-operands.json) retains every input,
mode, observation, binary identity and bound. The first three scaling models
mistakenly expected the ordinary up-to-date banner for a phony target without a
recipe. Their original failed models and observations remain without credit;
corrected exact expectations were independently qualified again. No semantic
model in the 938-cell matrix needed refinement.

The include-stat gate expands from twelve to twenty-four separately required
Linux/glibc profiles: startup, reader and recipe sites now also exercise two
retained identical physical paths, with no fault, one/three interruptions and a
fatal signal. The gate requires both real metadata observations after recovery,
or cancellation before a further syscall/effect. GNU repeats agree, and both
frozen builds pass three repetitions of all twenty-four profiles. Preceding
rmake fails eleven new probe expectations: three startup traces lose a retained
observation, while eight later probes miss the exact `inc` injection path because
that build keeps an unnormalized `./inc` spelling. Those missed injections do
not establish a baseline cancellation defect. Observer null-path ABI qualification
is unchanged. These profiles earn no extra ordinary-case or safety credit.

Formatting, warnings-denied lint, 462 Rust and 457 tooling tests pass.
Debug SHA256:
`fe1eba99ef9e734882db2360cce0f4f77b918403c2d48fecd8f6d9fa66bed2df`.
Release SHA256:
`641d56a45860ca133206dc55edd7311d052464a4441d3fd7898941c5a04cd9e3`.
Full frozen rmake-driven development/optimized acceptance and final-commit CI
remain merge gates; their results are recorded in the PR.

Runtime HOME/tilde decoding still needs the full contextual evaluator and its
reentrant ownership boundary. This literal-history change does not claim to
implement it. Broader startup/CLI/output, real process replacement, native
loading, enabled Guile, locale/Linux profiles and integration remain in the
[M3/M6 audit](m3-m6-audit.md). General performance parity and persistent cache
correctness remain open.

GNU Make 4.4.1 main.c filename decoding and read.c include-path construction were
consulted from the verified archive SHA256
`dd16fb1d67bfab79a72f5e8390735c49e3e8e70b4945a15ab1f81ddb78658fb3`.
GNU source is copyright Free Software Foundation, Inc., GPL-3.0-or-later. The
Rust implementation, fixtures, observer extensions and models are original;
no upstream implementation or tests copied or translated, no unsafe Rust added,
and no GNU execution fallback introduced.

## Timing observations

Four sequential reports contain two batches of 101 alternating samples per
engine/workload, with two warmups, matched inputs and exact output agreement.
No other local build, test or probe campaign ran during timing. The include
scaling workload retains every ordered switch and freshly stats every distinct
missing directory; stdout checks the complete ordered flag list and empty
resolved search. It does not earn timing credit from the formerly incorrect
multiplicity cases. The improved membership index avoids repeated history scans.

| Workload / batch | GNU ms | Current ms | Preceding rmake ms |
| --- | ---: | ---: | ---: |
| include-operands-0 / A | 0.611 | 0.729 | 0.746 |
| include-operands-1000 / A | 2.849 | 1.366 | 1.909 |
| include-operands-5000 / A | 46.122 | 4.530 | 18.029 |
| include-operands-0 / B | 0.683 | 0.825 | 0.794 |
| include-operands-1000 / B | 3.109 | 1.570 | 2.172 |
| include-operands-5000 / B | 46.684 | 4.766 | 18.498 |
| tiny-noop / A | 0.553 | 0.650 | 0.657 |
| large-noop / A | 12.000 | 33.095 | 32.430 |
| expansion / A | 2.936 | 5.279 | 5.276 |
| tiny-noop / B | 0.568 | 0.673 | 0.684 |
| large-noop / B | 11.872 | 31.721 | 31.007 |
| expansion / B | 2.831 | 5.025 | 4.993 |

The paired preceding/current ratio for 5,000 operands is 4.065 and 3.940;
GNU/current is 10.240 and 10.005 on this specific workload. Core observations
remain mixed/slower, with large-noop still substantially slower than GNU. Host
activity was not controlled; there are no confidence intervals or performance
acceptance thresholds. These results do not establish a general speedup or
GNU parity. Raw reports: [paths A](benchmarks/include-operands-paths-a.json),
[paths B](benchmarks/include-operands-paths-b.json),
[core A](benchmarks/include-operands-core-a.json),
[core B](benchmarks/include-operands-core-b.json).

Run `make bench-compare BENCH_SUITE=include-operands BENCH_REPEAT=101` and
`make test-include-stat` through GNU make or rmake.
