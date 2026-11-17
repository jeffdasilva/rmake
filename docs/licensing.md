# Licensing and provenance

rmake uses the GNU General Public License, version 3 or any later version
(`GPL-3.0-or-later`). The full text is in [LICENSE](../LICENSE), and the project
grant is in [README.md](../README.md#license). GNU Make uses the
[same license designation](https://www.gnu.org/software/make/#licensing).

The project grant covers rmake's own material. Third-party material retains its
own terms and notices. Contributions must use terms compatible with the project
license, and future Cargo packages must declare `GPL-3.0-or-later`.

Using rmake to build another program does not require that program to use the
GPL. Incorporating covered code into a program has separate licensing
implications. See [GNU's FAQ on using GPL tools](https://www.gnu.org/licenses/gpl-faq.en.html#CanIUseGPLToolsForNF).

## Source-informed implementation

Contributors may read GNU Make 4.4.1 source to understand its behavior. The owner
superseded the original clean-room restriction. Preserve the Rust architecture,
performance objectives and independently authored differential corpus. Use source
research alongside documentation and executable experiments.

Record provenance according to the work performed.

- For a source reference, record the upstream version, file, function and behavior
  investigated. Link a precise research note when the details need more space.
- For copied, translated or adapted material, also retain applicable copyright
  and license notices, identify the affected rmake files, and record the changes
  and dates. Preserve upstream authorship and describe adaptations accurately.
- For tests, write independent programs and expectations. Identify any later
  upstream-derived tests separately and retain their attribution. They supplement
  the original coverage.

The current Rust implementation and ordinary conformance fixtures were authored
for this project. GNU implementation code and upstream tests have not been reused
as the engine or its ordinary fixture programs. GNU Make serves as an external
reference executable and a separate real-project build input. It cannot provide
a product runtime fallback.

Translating code into Rust retains the original code's applicable license
obligations. See the [GNU FAQ on translation](https://www.gnu.org/licenses/gpl-faq.en.html#TranslateCode).
Inspect the terms of each reused file. The GNU Make release contains materials
with their own notices, and the manual has a separate documentation license.
Use links and original explanations when documenting behavior. See the
[manual's license notice](https://www.gnu.org/software/make/manual/html_node/index.html).

## Retain attribution for included material

### Built-in declarations

The files in [crates/rmake-core/data/](../crates/rmake-core/data/) contain Linux
default variables and recipe declarations observed from GNU Make 4.4.1's public
database output. The variable data also records declaration-order observations
from the qualified GNU event trace. Each file retains Free Software Foundation
copyright attribution and the `GPL-3.0-or-later` grant.

The [built-in notes](builtins.md) describe the data, original extraction and
conversion tools, and `src/rule.c` and `src/default.c` references. The
[library-resolution notes](library-resolution.md) record the `.LIBPATTERNS`
data extension under the same attribution.

### Message catalogs

The [catalog directory](../catalogs/README.md) contains the GNU Make 4.4.1
release translations, with original PO sources, compiled MO bytes, copyright
and translator notices. The [provenance manifest](../catalogs/provenance.json)
records their release source and byte identities. The retained upstream package
license is [catalogs/COPYING](../catalogs/COPYING).

These are attributed declarative translation assets. The application uses its
own catalog directory. Catalog installation and identity checks are described
in the catalog README.

### Runtime and build dependencies

[THIRD_PARTY_NOTICES.md](../THIRD_PARTY_NOTICES.md) lists the pinned Cargo packages,
versions, declared licenses and retained license files. The
[license manifest](../licenses/third-party/manifest.json) records package and
license-file checksums. It includes runtime, build-time and target-specific
locked dependencies.

The Linux evaluation-stack adapter uses `stacker` through its safe API. Keep
dependency authorship and license notices with the corresponding material.
Dependencies remain subject to their declared terms.

### Reference and integration inputs

The [integration suite](project-integration.md) builds pinned upstream projects
with their own Makefiles, including GNU Make itself. Source acquisition verifies
archive and license digests recorded in the
[project manifest](../tests/integration/projects.json). These projects retain
their own licenses.

Retained [integration evidence](../tests/integration/README.md) can contain full
source and build filesystem observations, including upstream file bytes and
license notices. Those records preserve the original material's attribution.
Their presence is separate from the independently authored ordinary fixtures
and the Rust implementation. Keep applicable notices with any distributed test
inputs or evidence.

## Distribution policy

When distributing source or binaries, preserve the GPL text and applicable
notices. Mark modifications to adapted GPL material with the relevant dates.
Distribute covered work under the applicable GPL terms and preserve recipients'
granted rights without additional restrictions.

Binary releases must provide their exact Corresponding Source through a
GPL-compliant method, including the required build/install scripts and other
applicable material. The project plans to publish a matching source archive
beside each binary release. The source must match that binary. A moving branch
link alone cannot provide that guarantee. Review the requirements for the chosen
distribution form in [GPLv3 sections 4–6](https://www.gnu.org/licenses/gpl-3.0.html#section4).

Review third-party dependencies and plugin or adapter combinations for license
compatibility before distribution. A plugin protocol or process boundary does
not automatically create a license exception. The project grants no blanket
proprietary-plugin exception.

## Transition from the initial scaffold

The initial scaffold commit,
[`22dc143`](https://github.com/jeffdasilva/rmake/commit/22dc14330383a4f9f0c098a215e421ca74b74cc8),
was published under Apache-2.0. On 2026-10-18, the project owner selected
GPL-3.0-or-later for the current project and future development.

The history remains available. Apache rights already granted for the initial
snapshot continue under their terms. The later project grant does not change
third-party licenses. See [Apache-2.0 section 2](https://www.apache.org/licenses/LICENSE-2.0#copyright).

## Find source-reference records

The following notes retain the original source references and distinguish
observations from independently written implementations and tests. They provide
the detailed provenance for the areas below.

### Reading and evaluating Makefiles

- [NUL reader](nul-reader.md), [initial byte-order mark](initial-bom.md) and
  [directive whitespace](directive-whitespace.md) cover buffer and reader phases.
- [Assignment syntax](assignment-syntax.md),
  [qualified definitions](qualified-definitions.md) and
  [conditional whitespace](conditional-whitespace.md) cover declaration and
  diagnostic phases.
- [Dynamic makefiles](dynamic-makefiles.md) records `src/main.c` and
  `src/remake.c` research on remake staging and deferred include diagnostics.
- [Pattern scopes](pattern-scopes.md), [export bindings](export-bindings.md),
  [export order](export-order.md) and [variable metadata](variable-metadata.md)
  cover binding lifetime, membership, lookup and environment traversal.

### Resolving rules and files

- [Hidden prerequisites](extra-prerequisites.md),
  [file-record order](file-record-order.md) and
  [declaration recording](declaration-recording.md) cover recording, hash order,
  rule families and diagnostics.
- [Implicit chains](implicit-chains.md) and [default recipes](default-recipes.md)
  cover candidate resolution, intermediate lifecycle and special flags.
- [Rule filenames](rule-filenames.md), [rule tilde expansion](rule-tilde.md),
  [recursive HOME](recursive-home-names.md) and [wildcards](wildcard.md) cover
  name processing and filesystem queries.
- [VPATH records](vpath-file-records.md) and
  [shared directory search](shared-directory-search.md) cover record merging,
  search names and Linux/glibc directory interfaces.
- [Archive name encoding](archive-name-encodings.md),
  [archive observations](archive-observations.md),
  [symlink timestamps](symlink-timestamps.md) and
  [ordinary stat](stat-timestamps.md) cover conversion, I/O, errors and freshness.

### Running recipes and recursive builds

- [Ordinary recipe lifecycle](forced-touch.md),
  [filesystem effects](filesystem-effects.md),
  [shell argv replay](shell-argv-replay.md) and
  [environment vectors](environment-vectors.md) cover command construction,
  Linux launch, capture and completion phases.
- [Fatal continuations](fatal-continuations.md),
  [cancellation and completion](runtime-cancellation.md),
  [load and stdin](load-limits.md), [signals](runtime-signals.md) and
  [output synchronization](output-sync.md) cover child ownership, descriptors,
  interruption and cleanup.
- [Command replay](command-replay.md), [runtime flags](runtime-flags.md),
  [flag payloads](flag-payloads.md), [switch origins](switch-origins.md),
  [restart directories](restart-directories.md) and
  [profile inputs](profile-inputs.md) cover option, startup and restart phases.
- [Rule warnings](rule-warnings.md) and [POSIX mode](posix-mode.md) record their
  reader, rule, shell and startup references.

The [source-name allocation study](shared-source-names.md) uses an independently
written development-only glibc interposer. That sampler is not linked into rmake.
The shell and flag research also retains allocator-abort and nontermination
observations with no passing conformance credit.
