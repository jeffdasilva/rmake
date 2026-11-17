# Catalog and language selection

This gate checks selection of all 30 GNU Make 4.4.1 catalogs shipped with rmake,
plus fallback for an unavailable language, a partial translation and the C
locale. It uses public gettext through an original native callback and checks
an actual localized parser error.

## Run or replay the gate

From the repository root, run the full gate or select one case.

```sh
make test-catalog-coverage
make test-catalog-coverage CATALOG_COVERAGE_CASE=partial-language-fallback
```

A catalog's language code is its case ID. See
[the language inventory](../../catalogs/languages.txt). The additional IDs are
`unavailable-language`, `partial-language-fallback` and `c-disables-language`.

The target builds rmake, verifies the pinned GNU reference and catalog hashes,
and prepares the required private locale data. `LOCALE_DATA_ROOT` defaults to
`.tools/locale-data` under the repository. The broader `make test-m4-release`
suite runs the catalog gate against the optimized binary. `make test-selfhost`
replays the partial-language fallback through rmake.

## What the comparison checks

The [original C callback](catalog_probe.c) asks public gettext for message
values. The [Makefile](Makefile) prints those values and then reaches a known
parser error. The runner compiles the helper with `/usr/bin/cc` into a disposable
directory.

The [runner](../../tools/catalog_coverage.py) resets the same workspace and
executable alias for two verified GNU runs and the candidate. An independent
catalog reader derives the expected translated values and parser message from
the owned data. Raw stdout, stderr, status and the complete fixture filesystem
must agree across the executions. Compiler failures, reached limits, leftover
children and failed independent expectations fail the gate.

The [pinned locale profile](../../oracle/locale-data.json) supplies authenticated
private French UTF-8 data for the recorded Linux x86-64/glibc 2.39 environment.
The gate uses that message locale and selects each desired catalog with
`LANGUAGE`. Its C control checks that the language request is disabled. Catalog
selection therefore has a fixed locale input independent of system-installed
locales.

## Read the results

The development report is `artifacts/catalog-coverage/development.json`.
The optimized suite writes `artifacts/catalog-coverage/optimized.json`.
Completed reports include each case's language chain, raw streams, status,
filesystem and process observations, plus the candidate hash and reference
profile ID. Save a report before rerunning if you need to retain both results.

## Scope and provenance

These cases establish catalog selection and fallback for the tested messages.
Individual diagnostic producers, locale-dependent collation and numeric
formatting have their own checks. See the
[localized diagnostics guide](../../docs/nls-diagnostics.md),
[startup locale guide](../../docs/startup-locale.md) and
[collation guide](../../docs/locale-collation.md).

The Makefile and native callback are original fixtures. The catalog data and
its attribution are recorded in [catalog provenance](../../catalogs/provenance.json)
and [the catalog guide](../../catalogs/README.md).
