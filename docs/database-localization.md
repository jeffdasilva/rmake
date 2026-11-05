# Localized database output

The Linux `-p` renderer uses the active gettext domain and message locale when
it prints each dump. It passes source paths, stems, timestamps, and counts as
typed arguments. Makefile data stays byte-oriented. The percentage fields still
use the live libc numeric locale and floating-point environment.

`make test-database-nls` and `make test-database-nls-release` compare C and French
dumps with the pinned GNU Make 4.4.1 reference. The test uses the verified
French locale profile and the GNU compiled French catalog identified in
`tools/database_nls.py`. Catalog text is declarative data. See the translator
and license records in [catalogs/provenance.json](../catalogs/provenance.json)
and [catalogs/COPYING](../catalogs/COPYING).

The test models combine the original database input plans with the reference
catalog before any engine runs. They check normal output, early and retained
failure, and a real restart. The restart control expects a French first dump
and a C second dump. Additional controls check the French decimal separator
and a raw non-UTF-8 multiline value containing a line that looks like a
database header. Existing independent witnesses check directory identities and
cache fields, and bound each database timestamp with libc `time(NULL)` samples.
The comparison also checks exit status, effects, and the full filesystem.

GNU Make 4.4.1's `print_data_base` and `print_version` in `src/main.c`, plus the
presentation producers in `src/{variable,dir,rule,file,commands,vpath,hash,strcache}.c`,
identify the message keys and their argument shapes. The Rust writer and the
test fixture were authored independently. Runtime translation uses rmake's
owned catalog binding and honors a native caller's public gettext settings.
