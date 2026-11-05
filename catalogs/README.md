These are all 30 attributed declarative message catalogs distributed with
GNU Make 4.4.1. The original PO source, compiled MO bytes, and per-file
copyright and translator notices are unchanged. `provenance.json` identifies
each pair and its source in the pinned release archive. The package license is
in `COPYING`. These assets contain no GNU implementation, headers or tests.

The initial compatibility domain is `make`, bound to this project-owned
catalog directory. Native modules retain public gettext domain and directory
selection. The default never reads the GNU oracle's installed catalogs.
The catalog data can translate the system messages rmake already emits
through typed message IDs; individual diagnostic producers have separate
compatibility tests.

Use `make catalog-check` to verify data and notice identities. Cargo checks
all 30 compiled tables and every supported translated argument schema before
compiling the application. Formatting remains checked Rust byte formatting.

A staged installation uses `make catalog-install PREFIX=/usr/local
DESTDIR=/path/to/stage`. Compile the executable with
`RMAKE_LOCALEDIR=/usr/local/share/rmake/locale make build` for that logical
installation. A normal development build uses the checked-out catalog path.
