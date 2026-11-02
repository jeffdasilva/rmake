# Invalid options in live MAKEFLAGS

GNU's live `MAKEFLAGS` decoder ignores unrecognized options and ambiguous long
prefixes, then continues with valid options and payload assignments. rmake now
implements that behavior instead of aborting the makefile. For example,
`MAKEFLAGS += --w -Qsk SIDE:=after` applies silent/keep-going state and the
simple file-origin binding for SIDE. The same invalid options remain errors
at startup. Registered but unimplemented features such as debug/database/
directory output or shuffle are still reported as unsupported.

The existing compile-time parser distinction now names the startup/live phase.
Startup also retains immediate `-R` implication; live decoding preserves the
existing post-read built-in activation boundary. Invalid long words are consumed
once without consuming a following operand. Invalid short bytes are skipped
inside their cluster, so following valid letters still apply. Complete GNU
namespace membership distinguishes invalid syntax from registered features.
The scan stays iterative, owns its words and preserves argument positions.
No unsafe code, external package or global getopt state is introduced.

## Original models and qualification

641 new required cases comprise 320 payload forms, 320 pure-option forms and the
exact historical reader `--w` reproducer. Sixteen invalid/ambiguous forms include
unknown short/long options, mixed clusters, known options after errors, attached
unknown values and raw non-UTF-8 bytes. Reader, eval, include, recipe and secondary
expansion each cover replacement, append, override and inactive conditional
assignments. Payload cases check later definition, origin and flavor; pure cases
exercise the no-assignment continuation. Every modeled effectful case appends
an ordered file witness and reads it back into exact output.

Each independent model agrees with two bounded GNU Make 4.4.1 observations.
Both frozen builds match every output byte, status and filesystem byte/type/mode/
link relationship without reached limits or leftover children. Preceding rmake
fails 481 and passes 160 inactive controls. [Raw evidence](../tests/research/live-invalid-options.json)
retains all inputs, modes, arguments, environments, models and observations.
64 rejected initial secondary sources remain unchanged with zero credit: their
unescaped colon was parsed as static-pattern syntax. Independently named
recursive drivers defer the original effects without changing the expected
output or file order. No output normalization or safety exemption is involved.

The exact previously recorded reader `--w` input is now a required passing case.
The earlier [prefix research](../tests/research/long-option-prefixes.json) remains
unchanged; its initial expectation incorrectly selected undefined-variable
warnings. Startup ambiguity/usage remains a different, unfinished surface.

Two Rust tests protect phase distinctions, mixed clusters, payload positions,
origins and the refusal to hide registered unimplemented features. Five tooling
tests protect all observations, the exact historical source, pure/effectful
forms, byte options, file order and rejected-source history. Formatting, lint,
518 Rust and 529 tooling tests pass; the preceding 17,095-case corpus and all
68 safety cells pass before the full frozen acceptance run. Standard Makefile
flows discover the new cases. Full rmake-driven `m5-acceptance` and exact-head
development/optimized CI remain merge gates; results belong to the PR.

The ordinary corpus becomes 17,736 required / zero pending, with the same 17
separately gated approved safe-error exceptions. This does not complete M3/M6.
Startup usage/ambiguity, malformed known-argument values and arity, remaining
nested decoder/runtime HOME behavior, directory/debug/database presentation,
real exec/stdin lifecycle, native/Guile/Linux/locale/integration acceptance and
general performance parity remain open. Persistent caching remains deferred to
its correctness and acceptance design.

The qualified profile is x86-64 Linux/glibc, C locale and disabled Guile.
Oracle profile: `92525c9784f4dd471cceb12ce66a9d367e267b638e1384cdc29962b738d16de7`.
Oracle SHA256: `b8806e8eb7452ed9b7ed5e18cb7e76ff9abd488c9d62a266a398d8044fade974`.
Verified source archive: `dd16fb1d67bfab79a72f5e8390735c49e3e8e70b4945a15ab1f81ddb78658fb3`.
GNU interface names were used as reference; behavior was qualified side by side.
All Rust, Makefiles and tooling are original; no GNU implementation/tests were
copied or translated and GNU is never a runtime fallback.

Frozen debug SHA256: `195e605a7d27326e113892f9f30323017237546f2cbf46405b75cd838428ffbb`.
Frozen release SHA256: `3715c06d26556f0475ab922fa0f70876f938abdd532547b91b8577d020b8cd68`.

## Measurements

Four retained reports contain two sequential batches of 101 alternating samples
per engine/workload, two warmups and exact output agreement. GNU drives A; frozen
rmake drives B through the top-level Makefile. Preceding rmake was built in this
same worktree. All timed inputs already match on it; newly accepted invalid-option
cases receive no timing credit. No unrelated local build, test or probe campaign
runs during timing. Host activity is uncontrolled, without confidence intervals,
CPU/RSS/I/O measurements or performance thresholds. The runtime-update workload
checks final flag/count output; controlled metadata gates separately protect the
required physical observations.

| Workload | GNU / candidate / preceding median ms A | GNU / candidate / preceding median ms B | Paired preceding/candidate A / B |
|---|---:|---:|---:|
| tiny-noop | 0.676 / 0.817 / 0.831 | 0.575 / 0.683 / 0.687 | 1.004 / 1.017 |
| large-noop | 10.974 / 29.339 / 29.393 | 10.899 / 29.262 / 29.498 | 0.998 / 1.004 |
| expansion | 2.837 / 4.925 / 4.957 | 2.899 / 4.931 / 4.926 | 1.000 / 1.002 |
| runtime-includes-0 | 0.603 / 0.727 / 0.734 | 0.641 / 0.743 / 0.755 | 1.015 / 1.021 |
| runtime-includes-1000 | 2.724 / 19.519 / 17.724 | 2.805 / 17.965 / 18.252 | 0.996 / 0.985 |

Ratios above one favor the candidate. Results remain mixed; no general speedup
or GNU performance parity is claimed. Raw reports: [core A](benchmarks/live-invalid-core-a.json),
[core B](benchmarks/live-invalid-core-b.json),
[runtime A](benchmarks/live-invalid-runtime-a.json),
[runtime B](benchmarks/live-invalid-runtime-b.json).
