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
fallback. No GNU implementation code or upstream tests have been incorporated at
this planning stage.

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
