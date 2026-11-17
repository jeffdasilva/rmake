# GNU Make message catalogs

This directory contains all 30 message catalogs distributed with GNU Make 4.4.1.
The PO sources, compiled MO bytes, copyright notices and translator notices are
unchanged. See [licensing and provenance](../docs/licensing.md) for the
project’s policy on GNU reference material.

[provenance.json](provenance.json) identifies each file and its source in the
pinned release archive. [languages.txt](languages.txt) lists the catalog codes.
[COPYING](COPYING) retains the package license, and each PO file retains its
own notices. The [French source](fr.po) is one example.

## Verify the data

From the repository root, run the identity check.

```sh
make catalog-check
```

The check verifies the complete data and notice inventory against its recorded
hashes. Cargo also checks all compiled catalogs and every supported translated
argument schema before compiling the application. Formatting uses checked Rust
byte operations.

The initial `make` text domain is bound to rmake's own catalog directory.
Native modules can select domains and directories through public gettext
interfaces. By default, development builds use the checkout and installed builds use
the catalog path configured below.

## Stage the catalogs

Set the binary's absolute runtime catalog path at build time, then stage the
same logical directory.

```sh
RMAKE_LOCALEDIR=/usr/local/share/rmake/locale make release
make catalog-install PREFIX=/usr/local DESTDIR="$PWD/artifacts/catalog-stage"
```

The example stages catalogs, PO sources, notices and provenance under
`artifacts/catalog-stage/usr/local/share/rmake/locale`. The binary retains
`/usr/local/share/rmake/locale` as its runtime path. `DESTDIR` is the packaging
prefix. For another layout, keep `RMAKE_LOCALEDIR` equal to
`CATALOG_INSTALL_DIR`, whose default is `$(PREFIX)/share/rmake/locale`.

## Check language behavior

Run `make test-catalog-coverage` to check catalog selection and fallback.
The [catalog test guide](../tests/catalog-coverage/README.md) describes its
independent expectations and replay options.

The catalogs translate system messages that rmake emits through typed message
IDs. Individual diagnostic producers have separate tests. The
[localized diagnostics guide](../docs/nls-diagnostics.md) describes those checks,
installation settings and the remaining language and platform limits.
