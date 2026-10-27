# Licensing and provenance

rmake is licensed under the GNU General Public License, version 3 or (at your
option) any later version: `GPL-3.0-or-later`. The full license is in
[LICENSE](../LICENSE), and the project grant is in [README.md](../README.md#license).
This matches the license stated by [GNU Make](https://www.gnu.org/software/make/#licensing).
Future Cargo packages must declare `license = "GPL-3.0-or-later"`.

## Source-informed implementation

GNU Make 4.4.1 source may be read as a reference for the Rust implementation.
The prior clean-room restriction is superseded. Preserve the Rust architecture,
performance objectives and independently authored differential corpus; source
access supplements documentation and executable experiments.

Inspecting source and incorporating material are different activities. For source
references, record the upstream version, file/function and behavior investigated.
For copied, translated or adapted material, also retain applicable copyright and
license notices, record the changes and dates, and identify the affected rmake
files. Do not remove upstream authorship or represent adapted material as entirely
original. Translation into Rust does not by itself remove the original license
obligations. See the [GNU FAQ on translation](https://www.gnu.org/licenses/gpl-faq.en.html#TranslateCode).

The primary conformance corpus remains original. Any later upstream-derived tests
must be separately identified and attributed; they supplement rather than replace
independent coverage. The reference executable remains a test tool, not a runtime
fallback. No GNU implementation code or upstream tests have been incorporated.
The [hidden-prerequisite notes](extra-prerequisites.md) record snapshot, implicit
edge-role and automatic-variable source references for the independent follow-up.
The [file-record notes](file-record-order.md) record the hash/table and lifecycle
references, original numeric observations and declarative suffix-name ordering.
The [ordinary recipe lifecycle notes](forced-touch.md) record the source sections
consulted for that independently written implementation and its later fixtures.
The [pattern-variable notes](pattern-scopes.md) record the binding, activation
and export-policy source references used for the independently written scope
implementation.
The [filesystem-effect notes](filesystem-effects.md) identify the filesystem and
recipe-command source references and the narrow Linux stdio interface.
The [wildcard notes](wildcard.md) record GNU Make and glibc semantic references,
the independently authored archive scanner and Linux ABI adapters. Upstream
implementation files and tests are not included in the product or corpus.
The [export-binding notes](export-bindings.md) record the source references used
for the independently authored binding-lifetime follow-up.
The [NUL reader notes](nul-reader.md) record GNU's file-buffer and diagnostic
references for the independently authored stream model.
The [command replay notes](command-replay.md) record GNU's startup, post-read
decoding and binding-precedence references for the independently authored replay.
The [rule warning notes](rule-warnings.md) record declaration-recording phases
and warning predicates used by the independently authored parser follow-up.
The [assignment notes](assignment-syntax.md) also record the later GNU source
reference for dispatching assignments before directive-shaped variable names.
The [directive whitespace notes](directive-whitespace.md) record active and
ignored definition-reader boundaries and diagnostic phases for the later slice.
The [initial-BOM notes](initial-bom.md) record the read/parse phase reference
used for independently authored byte-prefix handling.
The [conditional whitespace notes](conditional-whitespace.md) record name-token,
operand and expansion-order references used by the independent conditional fix.
The [qualified-definition notes](qualified-definitions.md) record modifier,
header-name and body-evaluation phases used by the independent follow-up.

Inspect each reused file's actual terms. GNU Make's release can contain material
with distinct notices, and the GNU manual has a separate documentation license.
Program licensing does not relicense that manual; prefer links and original
explanations over copying its prose. See the
[manual's license notice](https://www.gnu.org/software/make/manual/html_node/index.html).

## Distribution policy

Keep the GPL text and applicable notices with distributed source and binaries.
Where upstream GPL material is adapted, mark modifications and relevant dates.
Distribute the covered work under the applicable GPL terms, without imposing
additional restrictions on recipients' granted rights.

Binary releases must provide the exact Corresponding Source through a GPL-compliant
method, including required build/install scripts and any other applicable material.
Plan to publish a matching source archive beside each binary release; a link to a
moving default branch is not a substitute for its matching source. Check any
additional requirements applicable to the chosen distribution form. See
[GPLv3 sections 4-6](https://www.gnu.org/licenses/gpl-3.0.html#section4).

Review third-party dependencies and plugin/adaptor combinations for license
compatibility before distribution. A plugin protocol or process boundary is not
automatically a license exception. No blanket proprietary-plugin exception is
granted by this project.

Merely using rmake to build another program does not require that program to be
GPL-licensed; incorporation of covered code is a different question. See
[GNU's FAQ on using GPL tools](https://www.gnu.org/licenses/gpl-faq.en.html#CanIUseGPLToolsForNF).

## Transition from the initial scaffold

The initial scaffold commit,
[`22dc143`](https://github.com/jeffdasilva/rmake/commit/22dc14330383a4f9f0c098a215e421ca74b74cc8),
was published under Apache-2.0. On 2026-10-18 the project owner selected
GPL-3.0-or-later for the current project and future development. The history is
preserved; the change does not revoke Apache rights already granted for the
initial snapshot. See [Apache-2.0 section 2](https://www.apache.org/licenses/LICENSE-2.0#copyright).

The current GPL grant covers the project's own material. It does not change the
licenses of third-party material, and future contributions must be accepted on
terms compatible with the project license.

The dynamic makefile slice consulted GNU Make 4.4.1 `src/main.c` and
`src/remake.c` for remake staging and deferred include diagnostics. Its Rust
implementation and differential fixtures were authored for this project.

The [implicit-chain notes](implicit-chains.md) record the GNU Make 4.4.1
`src/implicit.c` and `src/file.c` references used for independently authored
transactional resolution and intermediate lifecycle handling.

The Linux default variable and recipe declarations in `crates/rmake-core/data/`
are reproduced as compatibility data from GNU Make 4.4.1's public database output.
They retain Free Software Foundation copyright attribution and the GPL grant.
The [built-in notes](builtins.md) distinguish these declarations from the
independently authored Rust conversion and extraction tools, and record the
`src/rule.c` and `src/default.c` source references.

The [default-recipe notes](default-recipes.md) record the GNU Make 4.4.1
`src/implicit.c` reference used to clarify selected-pattern special flags.
The Rust implementation and default-recipe differential fixtures are original.

The [declared VPATH record notes](vpath-file-records.md) record eligibility,
lookup spelling, record merging and traversal-phase references used for the
independently authored Rust follow-up and original fixtures.

The [shared-directory notes](shared-directory-search.md) record GNU directory,
VPATH and update-phase source references and the Linux/glibc directory interface
used by the independently authored Rust adapter and original test hooks.


The [archive name-encoding notes](archive-name-encodings.md) record the GNU
`arscan.c`, `misc.c` and `ar.c` references used for the independently written
conversion/lifetime follow-up and original archive/Makefile/stateful fixtures.


The [symlink-time notes](symlink-timestamps.md) record GNU `remake.c` / `main.c`
and Linux/glibc interface references for the independently written timestamp
observer, option handling, matcher reuse and original test programs.

The [rule filename notes](rule-filenames.md) identify the GNU 4.4.1 reader,
prerequisite and implicit-search phases consulted for the independently written
filename scanner and rule-expansion follow-up. No upstream code or tests were
incorporated.

The [rule tilde notes](rule-tilde.md) record the GNU reader, declaration-close,
implicit-search and default-goal source references used for the independently
written tilde-expansion and deferred-prerequisite follow-up.

The [declaration-recording notes](declaration-recording.md) identify the GNU
reader/file references used to establish recording, rule-family and diagnostic
phases. The Rust refactor and its 73 anchored Makefiles are independently authored;
no upstream implementation or tests were copied or translated.

The [ordinary stat notes](stat-timestamps.md) record the GNU timestamp and
completion references used for independently written error/retry and execution
state changes, original kernel-error Makefiles and anchored syscall programs.

The [library resolution notes](library-resolution.md) identify reader/file/search
references used by the independently written Rust and original tests. The pinned
`.LIBPATTERNS` declaration extends the existing public database extraction under
its FSF/GPL data attribution; no upstream implementation or tests were copied.

The [archive observation notes](archive-observations.md) identify the GNU
timestamp, header-I/O, hash-record and cleanup sources consulted for the
independent implementation and original anchored/fault/stateful tests. No GNU
implementation or upstream test code was incorporated.

The [recursive HOME notes](recursive-home-names.md) identify filename-parser,
tilde and function-query reference paths used to understand reproducible shared
scratch effects. The bounded Rust byte model and all tests are independently
written; no GNU implementation or upstream tests were incorporated.

The [fatal continuation notes](fatal-continuations.md) record GNU job and shutdown
source references used for the independent owner-state follow-up and original
fixture anchors. The [allocation/source-name study](shared-source-names.md) uses
an independently written, development-only glibc interposer and ordinary Linux
interfaces; the sampler is not linked into rmake.

The [load/stdin notes](load-limits.md) record the source-reference study for the
independent Linux runtime scheduling and descriptor follow-up.

The [signal/runtime notes](runtime-signals.md) record child ownership, fatal
signal, output ordering and intermediate-cleanup source references for the
independent Linux event loop and original controlled signal campaign.

The [output synchronization notes](output-sync.md) record output, recipe and
POSIX synchronization source references used for the original Linux implementation
and controlled output-phase and mixed-recursion fixtures.

The [cancellation/completion notes](runtime-cancellation.md) record the source
references for independently implemented evaluation cancellation, owned capture,
stream interruption, synthetic completion and the original PTY/lock campaigns.

The [runtime flag notes](runtime-flags.md) record reader/variable/option and
jobserver references used for independently authored immediate option effects,
post-read replay, restart restoration and original protocol fixtures.
