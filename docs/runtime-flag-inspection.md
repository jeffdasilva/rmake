# Runtime flag inspection during parsing

Runtime flag updates previously parsed incoming arguments once to inspect
silent/keep-going/job selections, then parsed them again with the existing flag
prefix. The successful path now records those three selections while driving the
shared decoder through the merged arguments. Prefix selections retain their
origins, and ignored operands never become options. Startup and effectful tilde
streams keep their existing paths.

The parser loop owns its state. It temporarily clears and restores the three
selection markers at each argument boundary, preserving clusters, operand arity,
assignment and warning positions. No evaluator or I/O runs in this inspection.
Only errors retry the pure incoming parse, preserving incoming-error precedence
over an invalid reconstructed prefix. This retry performs no interpreter effects.

96 independently modeled recipe-echo and keep-going cases cover file, override,
environment and GNU flag sources, positive/negative CLI precedence, aliases and
output modes. Two bounded GNU observations and frozen development/optimized
builds agree with every raw stream, exit status and ordered file witness. The
preceding candidate also passes all 96; these are regression controls, with no
closed mismatch or new safety exception. The [proof](../tests/research/runtime-flag-inspection.json.gz)
and [discovery archive](../tests/research/runtime-inspection-discovery.json.gz)
retain bytes, identities, failed public-flag proxy hypotheses, separately authored
recipe models, source and measurement provenance.

The selected implementation also passes the preceding 3,064 focused runtime,
payload, origin, path, validation-order and HOME/search cases. Two Rust regressions
protect ignored option-like operands, prefix origins, command-job precedence and
incoming-error precedence. Formatting, Clippy and 536 Rust tests pass.

Full frozen rmake-driven acceptance and exact-head CI remain merge gates. The
ordinary corpus is 26,942 required / zero pending across 156 families, plus the
same 32 approved exceptions / 128 separate cells. Complete M3/M6, broader malformed
and nested decoding, profiles, integration, persistent cache correctness and
general performance parity remain open. No GNU implementation or tests were
copied or translated, and GNU remains an external test oracle.

[Repeated request profiles and paired timing](benchmarks/runtime-inspection-index.md) show 1,009 fewer requests/frees in the runtime-update control (about 4.8% fewer requests). Unpinned timing is mixed; two 301-sample CPU-pinned follow-ups show a small improvement. General elapsed speedup and GNU performance parity are not established.
