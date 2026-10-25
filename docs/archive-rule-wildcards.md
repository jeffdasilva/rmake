# Archive wildcards in rules

Rule declarations now expand archive filename and member wildcards, including
archive groups. They keep complete member references, sorted per archive, with
repeated archive entries preserved. Unmatched archive/member patterns remain
literal prerequisites. Matching includes zero-date entries, GNU long-name tables
and BSD inline names; the build phase still applies normal member-date rules.

The core requests a separate `ArchiveGlob` filesystem effect. Its NUL-separated
complete names preserve member/archive blanks, tabs and arbitrary Linux bytes.
The existing `$(wildcard ...)` function retains GNU 4.4.1's different archive
output behavior. Both paths share the bounded Linux scanner without sharing an
archive-content or member-index cache. Archive reads remain fresh after effects.

Target wildcard observations precede expansion of the prerequisite tail. A
shell/file effect in that tail cannot retroactively select new targets. Each
prerequisite clause is globbed once, before static stem replacement. Clauses
requiring secondary expansion wait for activation, including wildcard reads and
errors. Ordinary rule words remain borrowed and avoid an extra prerequisite
vector. Scoped assignments use the same target filename pass.

GNU also checks the complete `.DEFAULT_GOAL` spelling in its file table before
splitting it. That permits a known filename containing blanks, such as an archive
or member discovered by a glob. The engine now performs this lookup before its
existing multiple-goal check.

There are 50 original required fixtures, of which 37 fail on the preceding
build. They cover wildcard kinds, sorting/duplicates, unmatched forms, mixed
archive/member patterns, rule/scoped/static/secondary timing, repeated reads after
shell and file mutations, inactive rules, malformed headers, long names, bytes,
blank/tab names, default selection, modes and parallel execution. Two additional
fault-campaign cases anchor target and prerequisite archive opens interrupted by
EINTR: each keeps GNU's single-open fallback. Corpus: 2,714 cases, with 2,702
required matches and 12 pending. The filesystem fault gate has 21 cases; stateful
coverage remains 48 scenarios / 122 invocations. No upstream code or tests were
copied; GNU source was consulted for behavior.

GPATH/permanent renaming, broader implicit secondary expansion, per-clause
static stem metadata and the remaining format/fault matrix are open. General
ordinary-file rule wildcards and escaped rule filenames are outside this slice.
This does not complete M4 or establish performance parity.

A separate 101-sample local core run measured startup at 0.783 ms (previous
0.798 ms), 10,000 no-op targets at 28.572 ms (28.324 ms), and expansion at
4.718 ms (4.753 ms). A 31-sample archive run measured 64 members at 1.502 ms
(previous 1.504 ms) and 256 at 11.920 ms (11.859 ms). These small differences do
not establish a performance change. GNU remained faster on all five workloads.
Samples include startup, alternate engine order and check identical output;
OS caches are warm, with no CPU isolation, resource profile or confidence
intervals. Raw samples are ignored artifacts:
`artifacts/bench/archive-rule-wildcards-core-101.json` and
`artifacts/bench/archive-rule-wildcards-archives-31.json`.

[Archive-first VPATH resolution](archive-vpath.md) is now implemented as a
separate slice; GPATH/permanent renaming and the other remaining M4 gates are open.

Subsequent [GPATH coverage](gpath.md) adds permanent filename renaming and
selected alias/parallel interactions. Earlier corpus counts and open-work lists
above describe this document's original slice; M4 still has broader open gates.
