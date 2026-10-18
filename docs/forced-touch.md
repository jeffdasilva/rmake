# Ordinary forced commands in touch mode

This records ordinary-recipe lifecycle behavior for GNU Make 4.4.1 on the pinned
Linux profile. The first 20 `forced-touch-*` fixtures were independently written
and checked before any GNU source consultation. Sixteen further fixtures were
independently authored after the reference study recorded below; no upstream
implementation or test text was copied. All 36 cases are required matches in
[the requirement ledger](../tests/requirements.toml).

Observed execution rules:

- With no raw `+` modifier or direct `MAKE` reference, touch mode skips body and
  shell-configuration expansion. It prints one touch action. `-nt` suppresses
  the file write; `-qt` creates the target and succeeds.
- A raw forced line selects the ordinary command loop. All recipe bodies expand
  before execution. Every reached nonempty expansion evaluates `SHELL`,
  `.SHELLFLAGS`, and `IFS`, including commands that touch mode will skip. An
  expanded `+` can then activate a sibling; an expanded `+` alone never selects
  this route because the shortcut has already skipped its expansion.
- Forced commands execute in order. Skipped commands do not run. State probes
  around an intervening skipped command both observe an initially absent target.
  Completion touching happens later and preserves phony-target exclusions.
- Touch counts depend on command position and empty-body state. A forced command
  followed by a nonforced nonzero expansion prints `touch all` twice; a final
  executing forced command after a skipped line prints it once. A zero-byte
  expansion after a forced command also prints it once. Keep zero-byte bodies,
  modifier-only bodies, and executing forced commands distinct.
- In this command loop, query mode stops at a reached nonempty unforced command,
  returns 1, and skips completion touching. Empty ordinary commands can still
  lead to touching after successful forced commands under `-qt`.
- An unignored forced failure stops the target. An ignored failure retains its
  diagnostic and permits later commands. Under `-q`, an unignored child exit 1
  means out-of-date without an error diagnostic; ignored exit 1 follows ignored
  failure behavior. Exit 2 and higher remain errors.

The fixture state expectations below were separately asserted against the
oracle. Every case must leave `forbidden` absent. The harness compares resulting
file bytes, type, mode, links, stdout, stderr, and exit status between engines.
This curated single-invocation harness does not compare timestamps. The separate
[stateful gate](../tests/stateful/README.md) covers selected lifecycle sequences
with fixed timestamp inputs and explicit timestamp assertions.

| Fixture | Options | Exit | `all` exists afterward |
| --- | --- | ---: | --- |
| `forced-touch-only` | `-t` | 0 | no |
| `forced-touch-before` | `-t` | 0 | yes |
| `forced-touch-after` | `-t` | 0 | yes |
| `forced-touch-between` | `-t` | 0 | yes |
| `forced-touch-after-dry` | `-nt` | 0 | no |
| `forced-touch-before-query` | `-qt` | 1 | no |
| `forced-touch-after-query` | `-qt` | 1 | no |
| `forced-touch-empty-before-query` | `-qt` | 0 | yes |
| `forced-touch-empty-after-query` | `-qt` | 0 | yes |
| `forced-touch-empty-forced` | `-t` | 0 | no |
| `forced-touch-failure` | `-t` | 2 | no |
| `forced-touch-failure-query` | `-qt` | 1 | no |
| `forced-touch-ignored-failure` | `-t` | 0 | yes |
| `forced-touch-phony` | `-t` | 0 | no |
| `forced-touch-expanded-empty-after` | `-t` | 0 | yes |
| `forced-touch-expanded-plus-only` | `-t` | 0 | yes |
| `forced-touch-expanded-plus-activated` | `-t` | 0 | no |
| `forced-touch-recursive-make` | `-t` | 0 | yes |
| `forced-touch-skipped-config-error` | `-t` | 2 | no |
| `forced-touch-plain-skips-config` | `-t` | 0 | yes |

The ordinary recipe cursor must retain command preparation, skipped commands,
child completion, and target completion as separate events. Raw recursion flags,
source locations, and the pre-stripping empty-body state determine exact touch
counts and query-induced cleanup.

## Query cleanup after child completion

A subsequent source-informed study, checked with 195 isolated oracle probes,
resolved the surprising cleanup observed under `-q`, `-qt`, and `-nqt`:

- An unignored forced child exiting 1 reports query-outdated. It deletes its
  target only when ordinary `.DELETE_ON_ERROR` conditions apply, including a
  changed target timestamp.
- A successful or ignored-failing real child followed by an unforced command
  stops during continuation. It deletes an existing regular target even when
  bytes and mtime are unchanged, without `.DELETE_ON_ERROR`.
- Initial dispatch and commandless lines do not establish that continuation.
  GNU also optimizes an exact `:` shell invocation without creating a child:
  `+@:` followed by an unforced command preserves a preexisting target, but a
  preceding `+@true` makes the later query stop eligible for cleanup.
- An ignored direct launch failure clears that eligibility, even after an
  earlier real child. A successfully launched shell that exits 127 remains a
  real child and has different behavior.
- `.PRECIOUS` and phony targets prevent deletion. Directories and broken
  symlinks remain; a symlink resolving to a regular file is unlinked while its
  referent remains. The deletion diagnostic is still emitted under `-n` or `-s`.

The reference implementation explains the unchanged-file case by updating its
cached timestamp to a synthetic newest value before post-child query cleanup.
This is an explanation of an observable behavior, not a general cleanup rule.
The two cleanup policies must stay separate in rmake: changed-target cleanup on
ordinary failure, and regular-target cleanup after query continuation. Neither
query stop performs completion touching.

The implementation retains whether the last child attempt actually launched,
distinguishes an optimized no-child command from a real child, and represents
cleanup as an explicit result. Its pure `recipe_flow` helper preserves zero-byte
line distinctions and final recursion flags. Successful child completion with
only zero-byte bodies
remaining finalizes once; scanning remaining nonzero bodies without another
launch can finalize twice, matching duplicate touch output. Initial dispatch
exhaustion also follows the latter path. Focused helper tests check these
transitions separately from shell preparation and filesystem effects.

The 195 probes comprised 168 cleanup/protection combinations and 27 child-boundary
cases. Some early restoration probes used local-time `touch -t`; exact restored
mtime conclusions come from the separate boundary probes with `TZ=UTC` and fixed
mtimes. Sixteen further curated fixtures now cover these cleanup distinctions:

- Eleven cover created and unchanged targets; initial dispatch, optimized colon,
  direct-launch failure, and real-child exit; precious, phony, directory, and
  symlink protections.
- Three cover failure paths: an unignored missing direct executable preserves a
  previously created target even with `.DELETE_ON_ERROR`; failed completion
  touching deletes a read-only regular target after a child; the corresponding
  directory remains. The read-only case applies its mode during the recipe so
  Git's limited file-mode representation cannot weaken the fixture.
- A paired wrapper test distinguishes `./sh -c ':'`, which GNU optimizes without
  launching its wrapper, from `./mysh -c ':'`, which prints a marker from a real
  child. The following query stop preserves the first target and deletes the
  second. Shell basename and final arguments are observable inputs to this rule.

Each fixture declares exact output and status, and its target existence,
forbidden-output absence, retained bytes, and symlink referent were independently
checked against the oracle where applicable. The differential snapshot checks
these filesystem outcomes on every run. Exported recursive-variable timing and
broader timestamp behavior still require additional coverage.

## Reference provenance

The first 20 committed fixtures predate source inspection. The additional 16
fixtures, later explanation, and execution lifecycle above are source-informed
and must not be described as clean-room inference. The reference was GNU Make
4.4.1's official
[release archive](https://ftp.gnu.org/gnu/make/make-4.4.1.tar.gz), consulted on
2026-10-18. Its SHA256 is
`dd16fb1d67bfab79a72f5e8390735c49e3e8e70b4945a15ab1f81ddb78658fb3`.
The following upstream sections were inspected under their GPL-3.0-or-later
notices; reference files remain outside this repository:

- `src/job.c`: `reap_children` lines 953–1069, `start_job_command` 1174–1395,
  `start_waiting_job` 1645–1673, and `job_next_command` 1976–1995.
  File SHA256: `c1c32d4bd7fd56d898e952ed094a488edf2d4333a02058a3302d28f5e3f8ca09`.
- `src/remake.c`: `notice_finished_file` 939–1021 and `remake_file` 1288–1322.
  File SHA256: `a51b44dd016bb13e32a8091763443e7077b2ba23849f76b1e323f26b5acd323f`.
- `src/commands.c`: `execute_file_commands` 437–459, `delete_target` 611–654,
  and `delete_child_targets` 661–675.
  File SHA256: `e54c84e29f77c6086f7b87aaac242eec9fbc04746ea53a2a4b1b9ed0db85851d`.
