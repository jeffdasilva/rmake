# Catalog and language selection

`make test-catalog-coverage` selects all 30 original message catalogs through
glibc, with three additional cases for an unavailable language, a partial
translation, and the C locale. A public native callback tests real gettext
selection alongside an actual localized parser error. Both streams, status
and the whole fixture filesystem compare to two verified GNU runs, and an
independent catalog reader checks the message bytes against the owned data.

The locale fixture uses the authenticated private French UTF-8 locale as
`LC_MESSAGES` and selects the desired language via `LANGUAGE`. This tests
catalog selection and fallback independently of system-installed locales.
Locale-dependent collation and numeric formatting have separate gates.
