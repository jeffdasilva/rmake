# Original C/C.UTF8 profiles

`models.json` declares 183 original cases and exact independent output before
their qualifying GNU2/candidate runs. Fixtures preserve raw UTF8, combining and
invalid filename bytes. Expected complete filesystems are constructed from
known inputs rather than copied from either result. The single named profile
uses Linux/glibc C and C.UTF8; it is separate from ordinary curated fixtures and
NLS/other-libc acceptance.

`history-ctype.json` records the compressed/raw SHA256 and byte counts of the
complete immutable history archive. Raw streams, statuses, process bounds,
full filesystems, source closures and all rejected model assumptions are kept.
The 183-case reference qualifies twice; preceding passes 111/fails72, while both
new builds pass 183. `make test-locales` and `LOCALE_CASE` provide full and selected
replay. See [the behavior note](../../docs/startup-locale.md).
