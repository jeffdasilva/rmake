# Output synchronization validation after ordered definitions

GNU Make 4.4.1 validates a source's final output-sync selection after applying
its payload definitions. An invalid type must not skip assignments that precede
or follow that option. A later valid type within the same source can replace the
invalid selection. A different flag source is a separate boundary: invalid
GNUMAKEFLAGS stops before clearing that binding or expanding MAKEFLAGS.

The live receiver retains an invalid pending selection while returning its
owned definition steps. It avoids activating that invalid mode or constructing
a replacement include search. The existing empty-input refresh validates the
final state after the steps finish. Valid modes retain their existing activation
and include publication behavior. Passive sources validate their completed
state before publishing invocation inputs or starting the next source. Output
ownership, active buffers, warning positions and explicit job precedence retain
their existing separate responsibilities.

## Original models and independent effects

The required family has 818 original Makefiles: a 640-case matrix
across reader/eval/include/secondary/recipe phases, assignment form, priority,
option position, raw bytes and initial synchronization; 128 GNUMAKEFLAGS
controls; 48 next-source effect controls; and two separately refined historical
pilot inputs. Each has independently specified raw streams/status and ordered
`seen` bytes. Two bounded GNU observations and both frozen builds agree on all
818 models, closing 553 preceding mismatches. The strict qualifier checks
streams, status, repeats, bounds, leftovers and ordered file bytes, rather than
trusting a successful observer exit.

The next-source controls distinguish completed READY/DONE assignments from
MAKEFLAGS expansion. Invalid GNU flags must stop without NEXT or END. Valid
replacement within that source yields NEXT after clearing GNUMAKEFLAGS, END
during recipe expansion, then another NEXT during recursive export preparation.
The duplicate export effect is intentional and must not be memoized away.

The [qualified manifest](../tests/research/output-validation-order.json) and
[raw archive](../tests/research/output-validation-order.json.gz) retain all
inputs, models, identities and observations.

The [discovery archive](../tests/research/output-validation-discovery.json.gz) retains the original two pilot hypotheses, 128 premature
secondary-expansion syntax failures, 48 rejected cross-source rescue hypotheses
and 24 incomplete export-effect witnesses. Refinements use separate IDs and
preserve original input bytes and observations. These discovery observations
have no additional acceptance credit. This work adds no safety exceptions or
output normalization. The owner-approved nested-short scope remains separate.

[Six paired elapsed reports and twelve allocation-request profiles](benchmarks/output-validation-index.md)
retain mixed timing results and unchanged request counts. General performance
parity is not established. Full frozen rmake-driven acceptance and exact-head CI
remain required before merge. This is a bounded ordering fix;
contextual HOME/tilde, other malformed-value phases, complete owned decoding,
directory/debug/database output, native/Guile profiles, full M3/M6 and general
performance parity remain open.

## Reference provenance

Reference study used GNU Make 4.4.1 `src/main.c` (`decode_switches`,
`decode_env_switches`, and `decode_output_sync_flags`) from the locally pinned
archive identified in `oracle/manifest.json`. Source inspection establishes the
boundary to test; it does not substitute for raw differential observations.
No GNU implementation code or test cases were copied or translated. GNU remains
an external test oracle, never a runtime fallback.

A live `MAKEFLAGS` assignment now also completes the selected output lock
before Makefile evaluation continues. Earlier payload definitions keep their
side effects; a failed lock open stops subsequent file reads and recipes.
When both the mode and lock are invalid, mode validation takes precedence.

`make test-output-lock-completion` and `make test-output-lock-completion-release`
exercise 20 original models in C, French, German, Japanese, and Brazilian
Portuguese. They compare two pinned GNU runs
and rmake against independently specified status, byte streams, and complete
filesystem effects, including repeated options, an included reader, raw path
bytes, and reopening a lock removed between live assignments.

Each nonempty post-read flag source completes the selected mutex before the
next source can expand. Empty sources and an absent `MAKEFLAGS` binding do
not create a lock-open attempt. When output buffering is activated, setup
reopens the selected path independently. Nineteen required cases cover
fatal and warning order, retained authentication, pathname removal, and
suspended include option decoding.

After a replacement mutex fails to open, buffered fatal output no longer
waits for the previous lock. The gate holds that previous lock during live,
post-read, and active setup failures and checks the complete output, status,
and filesystem against GNU without releasing it early.

A successful reopen of the root's own mutex keeps its original pathname
lease while closing the replaced descriptor. The reopened file must match
the original regular inode; an external replacement never becomes owned.
Nested live flag and include tests use that same pathname through successive
reopens and confirm cleanup after the process exits.
