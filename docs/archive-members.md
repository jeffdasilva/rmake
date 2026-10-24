# Archive-member metadata and recipes

References such as `lib.a(path/one.o)` now read member metadata from the archive
instead of treating the reference as a filesystem filename. The Linux reader
supports ordinary headers, GNU long-name tables and BSD inline names. It is
shared with the existing wildcard implementation. Malformed numeric fields retain
byte-preserving diagnostics; invalid archive structure generally means the member
was not found. GNU 4.4.1 does not recognize thin archives on this profile.

Member dates are integer seconds. A zero date is considered absent, including
the zero dates produced by deterministic archivers. A standalone member file
with a strictly later integer-second timestamp also makes the archived member
stale. A member file's extra nanoseconds within the same second do not do so.
Target-side prerequisite comparisons use the entire member-date second, while
prerequisite outcomes retain the actual member timestamp.

Recipes receive the archive path in `$@` and the member spelling in `$%`.
Explicit-rule suffix stems use the member name. Automatic prerequisite lists
project archive references to member names after deduplicating by complete
dependency identity; two different archives can therefore contribute the same
member spelling twice. `$<` retains its prerequisite spelling, with GNU's
archive-path behavior for `.DEFAULT`. Secondary expansion receives the archive
and member automatic values too.

`-t` updates the selected header date to the archive's timestamp observed before
the write, matching GNU. It does not create missing archives or members. The
implementation scans and writes through one owned descriptor and overwrites only
the twelve date bytes, preserving all other header, name-table and payload bytes.
Recipe-failure cleanup preserves archives and emits GNU's changed-member warning
instead of unlinking them. Precious-member policy still applies.

Header validation waits for a target's final recipe command to finish. Recipes
may temporarily leave partial headers while replacing an archive. Completed
targets retain normal invocation-local completion behavior; later unrelated
effects do not force a new validation of an already completed target. No archive
contents, negative lookup or timestamp is cached across invocations.

Archive read descriptors are opened atomically close-on-exec. Interpreter scans
can overlap another recipe worker's process launch; a read descriptor must not
escape into that child. The Linux adapter retains the single open attempt used
by the interrupted-open profile. A regression test holds the scan descriptor
open across a real child exec and checks its observed descriptor path. It
reproduced inheritance before the flag was added.

Validation adds 52 original required cases and three stateful scenarios covering
twelve invocations. Tests cover formats, duplicate members, malformed fields,
automatic variables, modes, timestamp boundaries, touch preservation, failures,
partial writes and corruption left by completed serial/parallel recipes.
The full corpus is 2,594 cases: 2,582 required and 12 pending. Stateful coverage
is 47 scenarios / 118 invocations. Upstream read/scan, command-variable and
remake code was consulted as a behavioral reference; no GNU implementation or
upstream test material was copied.

Archive-member implicit search and suffix conversion are covered by the
[archive resolution slice](archive-rule-resolution.md). Remaining M4 work includes
GPATH/permanent renaming and broader format/fault coverage.
[Archive groups](archive-groups.md) and
[rule wildcard expansion](archive-rule-wildcards.md) are now implemented. This slice does not complete archive support
or establish performance parity. `make bench-compare BENCH_SUITE=archives`
measures original 64- and 256-member no-op workloads with fixed historical dates.

A local 101-sample paired core check against `e1f7861` measured tiny startup at
0.711 ms versus 0.695 ms, large no-op at 29.941 ms versus 29.873 ms, and expansion
at 4.935 ms versus 4.905 ms. These differences show no material core regression
in this run. A 31-sample archive comparison measured 64 members at 1.589 ms
(GNU 1.106 ms) and 256 members at 12.964 ms (GNU 6.686 ms). GNU was faster in that baseline. The subsequent
[adaptive reader](archive-reader-performance.md) records the current scan policy
and comparisons.
Reports retain all samples and identities under the ignored
`artifacts/bench/archive-members-core-101.json` and
`artifacts/bench/archive-members-31.json`. Timings include startup and warm OS
caches, without CPU pinning, confidence intervals or resource measurements.

[Archive-first VPATH resolution](archive-vpath.md) is now implemented as a
separate slice; GPATH/permanent renaming and the other remaining M4 gates are open.
