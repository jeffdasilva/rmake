# Locale collation of wildcard results

The selected Linux/glibc profile sorts each completed filename glob using the
calling thread's current libc collation. This closes the observed French
locale gap while preserving Make's separately byte-ordered `sort` function.
The original 74-case gate passes in development and optimized builds; the
preceding release passes 41 and fails 33. All 183 existing C/C.UTF8 locale
profiles continue to pass in both builds.

The gate covers C, C.UTF-8 and an authenticated, privately generated
fr_FR.UTF-8 locale. Cases retain exact streams, exit status, full work/home/tmp
filesystems, process bounds and two independent GNU runs. They exercise
per-pattern groups, duplicate groups, aliases, spaces, question matching,
prerequisite/order-only/secondary globs, target selection, directory components
and suffixes, includes, archive pathnames and a byte-ordered archive-member
control. Native fixtures change global and thread-local collation after a
directory has already been cached; environment category precedence remains
separately checked.

Malformed filename bytes matter. In the selected French locale, distinct raw
names can have identical collation keys. The original byte expectations failed;
a separate public-libc comparison matrix records the equal keys. GNU's
directory traversal determines their stable tie order, which changes when its
filename table grows. Raw-name controls cover the initial table and 120, 240
and 500 additional names, plus live locale changes over the same cached names.
The first growth predictions were wrong and remain archived. The candidate's
raw observations matched both GNU runs even before those expectations were
corrected; that failed model run receives no acceptance credit.

Directory observations remain byte ordered for deduplication and binary
membership. Each entry retains its insertion ordinal in existing structure
padding. A borrowed filename-table traversal is built lazily for collation;
the retained boxed index participates in the existing directory resource
budget. Final result vectors receive one temporary terminator per name, and
libc comparisons allocate nothing per comparison. A stable sort preserves
equal-key traversal. Archive-member ordering is unchanged.

The byte path applies only when the actual global LC_COLLATE is C/POSIX and
there is no thread-local locale override. Each glob queries current state;
neither environment names nor startup selections are cached across native
callbacks. C.UTF8 and other selections use libc. This avoids assuming that a
locale alias or a later native `setlocale`/`uselocale` call has byte collation.
No elapsed or allocation improvement is claimed by this implementation.

`tools/locale_data.py` verifies a pinned noble InRelease and main Packages
index, then extracts authenticated `locales` and `libc-bin` packages locally.
The packaged localedef builds the private French locale from pinned input
bytes. Complete input/output trees and license identities are verified; no
system install or maintainer script runs. The gate therefore needs no
preinstalled host French locale. See the [data provenance](../tests/locale-collation/LOCALE-DATA.md).

The [history archive](../tests/locale-collation/history.json) retains original
predictions, observer errors, raw public-libc observations, every failed/refined
run and the preceding frozen executable identity. The
[qualification archive](../tests/locale-collation/qualification.json) retains
both selected builds, all complete observations, source/build/input hashes and
check logs. GNU implementation, header and test files are absent. The parent's
first observer snapshot-call failure has no serialized acceptance observation
and remains explicitly separate from the later qualified runs.

This is a finite collation slice. Wider locales/libcs, localized diagnostics,
native concurrent locale mutation, unusual resource/fork/exec histories and
equal-key ordering after implicit impossible-file cache mutations still need
separate acceptance. It does not complete M3/M6 or establish GNU performance
parity. Frozen exact-head local/CI gates and isolated measurements remain merge
requirements.
