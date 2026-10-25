# Archive implicit and suffix rules

Implicit search first exhausts ordinary candidates for the complete
`archive(member)` name, including its intermediate-chain pass. Only if that
search fails does it try the `(member)` spelling. The second candidate list is
constructed lazily. Both searches retain original rule identities, cancellation,
terminal-rule handling, and candidate priority. Archive references do not strip
directory bytes before pattern matching; member directories remain in the stem.

This enables the existing declarative built-in `(%) : %` archiver rule, including
object-file intermediate chains. Custom `.X.a` suffix rules generate both the
archive pattern `(%.o) : %.X` and the ordinary `%.a : %.X` rule. User pattern rules
retain priority over converted suffix rules. No reference executable is invoked
by rmake; the selected recipe can invoke the user's archiver as GNU does.

GNU keeps the literal peer spellings from a member-only multi-output pattern.
Adding an archive prefix to those peers would incorrectly combine distinct
archive-member goals. Full-name multi-output patterns retain normal grouped
completion. Original cases exercise both forms and literal peer files.

Deferred static-pattern prerequisites also need GNU's word-wise rewrite: the
first `%` in each whitespace-delimited word becomes `$*` before secondary
expansion. This includes surviving expression syntax; rewriting a `$%` can
therefore produce a literal `$*` in an output function. The rewrite is pure;
evaluation and effects still wait for target activation. Non-archive controls
cover stem substitution, function arguments, and multiple percents in a word.

The slice adds 34 original required fixtures and one four-invocation stateful
scenario. The latter builds through the real archiver, queries and rebuilds a
deterministic zero-date member, and verifies a positive-date no-op. Its rebuild
step anchors the archive's outer mtime before execution to avoid comparing
incidental equal wall-clock ticks. Ten GNU/GNU and ten GNU/rmake replays verify
the scenario without output or timestamp exemptions.

Corpus: 2,628 cases, with 2,616 required and 12 pending. Stateful coverage:
48 scenarios / 122 invocations. Full development gates, optimized conformance
and the existing scheduler/jobserver/self-hosting checks validate the change.
GNU 4.4.1 implicit, rule-conversion and dependency-expansion sources were consulted
for behavior; no upstream implementation or tests were copied.

[Archive groups](archive-groups.md) now work in declarations and secondary
expansion, and [archive rule wildcards](archive-rule-wildcards.md) now retain
complete member names and observation timing. Remaining work includes GPATH/permanent renaming, broader implicit secondary expansion and per-clause static stem state,
and the remaining format/fault matrix. This does not complete M4 or the GNU
compatibility gate. See [archive member metadata](archive-members.md) for the
timestamp, touch, cleanup and performance baseline.

Local paired checks against the preceding archive-member build show no material
core regression. The 101-sample core run measured tiny startup at 0.591 ms
(previous 0.595 ms), large no-op at 27.366 ms (28.103 ms), and expansion at
4.724 ms (4.659 ms); median paired previous/current ratios were 0.996, 1.020,
and 0.998. A 31-sample implicit-search run measured 64 unmatched rules at
1.628 ms (previous 1.674 ms, GNU 1.017 ms; paired 1.006) and 256 at 2.168 ms
(previous 2.260 ms, GNU 1.408 ms; paired 1.043). These local measurements do not
establish a general speed gain, and GNU remains faster. Raw samples are in the
ignored `artifacts/bench/archive-implicit-core-101.json` and
`artifacts/bench/archive-implicit-search-31.json`; the startup/cache/isolation
limitations described in the metadata baseline apply.

[Archive-first VPATH resolution](archive-vpath.md) is now implemented as a
separate slice; GPATH/permanent renaming and the other remaining M4 gates are open.

Subsequent [GPATH coverage](gpath.md) adds permanent filename renaming and
selected alias/parallel interactions. Earlier corpus counts and open-work lists
above describe this document's original slice; M4 still has broader open gates.
