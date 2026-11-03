# Startup include search

GNU Make 4.4.1 expands HOME at an include argument's position, stores that
operand, then performs another tilde lookup while constructing startup search.
Later HOME definitions and full database callbacks affect physical search;
the stored MAKEFLAGS operand retains its earlier spelling.

Startup now resolves residual tilde names before publishing include metadata.
Each callback receives owned name bytes, the current database, filesystem,
shell and unpublished search context. HOME warning suppression restores on
success and failure. Search retains dot prefixes and slash spelling until
directory selection; argument preparation retains its separate normalization.
Both paths share the tilde resolver. Literal dash directories stay distinct
from the original -I- control. Ordinary non-tilde search keeps direct construction.

Flag assignments during constructor evaluation retain GNU's delayed activation.
An earlier prototype incorrectly attached the live switch receiver here. New
controls showed early warning/default/recipe-policy changes and premature mode
errors. Its source and observations remain unchanged in the discovery archive.
Separately named models cover ordinary bindings during startup, later canonical
flag initialization, and subsequent HOME environment expansion with/without
unexport. No safety exception is added and no HOME effects are cached.

500 independently modeled cases agree with two bounded GNU observations and both
selected frozen builds, closing 438 preceding mismatches. Coverage includes a
384-case flavor/position/form/mode/warning/byte/spelling matrix, 68 nested,
fatal, deferred-validation, fallback and metadata controls, 32 full-database
callbacks and 16 NUL-bearing HOME-file controls. The [proof](../tests/research/startup-include-search.json.gz)
and [discovery archive](../tests/research/startup-include-discovery.json.gz)
retain input bytes, raw streams/status, ordered witnesses, bounds, identities,
original hypotheses and the rejected prototype. Initial 24 pilot observations
receive no additional fixture credit.

192 manifests use the new `args_hex` representation to preserve Linux byte
arguments through TOML. Text and hexadecimal forms are exclusive; empty arguments
are preserved and NUL is rejected. Independent validation and executable-vector
tests protect those boundaries. Ordinary text manifests retain their format.

Formatting, Clippy and 537 Rust tests pass. Full frozen rmake-driven acceptance
and exact-head CI remain merge gates. The selected corpus is 27,442 required /
zero pending across 157 families, plus the same 32 separate approved exceptions /
128 cells. Complete M3/M6, broader decoder/profile/integration, persistent cache
acceptance and general performance parity remain open.

Reference study used the pinned GNU Make 4.4.1 startup/search and flag-activation
ordering in main.c/read.c/variable.c. No GNU implementation or test cases were
copied or translated. GNU remains an external test oracle.

[Four paired 101-sample timing reports and twelve request profiles](benchmarks/startup-search-index.md) retain exact output checks and raw identities. Request/free counts are unchanged on all three controls. Timing is mixed, including higher large no-op medians; no general speedup or performance parity is claimed.
