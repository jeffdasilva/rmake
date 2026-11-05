# glibc startup locale and filename matching

The executable selects the process locale at entry, before argument effects,
the interpreter, native callbacks or workers. Libc applies `LC_ALL`, per-category
variables and `LANG` using its public locale API. A failed selection retains
the initial C locale and produces no extra diagnostic. Later Makefile/flag/
command-variable assignments update ordinary make variables and child exports;
they do not reconfigure the running parent's locale. A recursive make invocation
selects its own locale from its actual incoming environment.

This fixes filename matching under C.UTF8: libc's wildcard matcher now sees the
same startup character classes and multibyte state as GNU. Literal filenames,
invalid filename bytes, combining sequences and explicit dot entries retain
their byte identity. Sorting still follows the make filename-order contract.
Existing directory-cache observations and query lifetimes are preserved.

The separate `test-locales` gate has 183 original profiles across file, CLI eval,
inherited GNU flags, environment/category priority, alias spelling, invalid
selections, later assignments/exports and recursive child environments. GNU
executes twice before each candidate. Independent complete filesystem and exact
stream/status/process assertions reject shared wrong results. Both development
and optimized candidates pass all profiles; the preceding candidate passes 111
and differs on 72. The selected top-level flow also runs under rmake.

The [immutable archive](../tests/locales/history-ctype.json) retains complete
raw inputs/observations, both wrong initial wildcard hypotheses, an unexecuted
model placement mistake caught during static review, corrected pre-execution
plans, source closure, build inputs, tool identities and logs. In particular,
glibc's actual wide/byte wildcard fallback and explicit `..` matching remain
observable; neither is normalized away. Repeated campaigns do not multiply the
number of qualified profiles.

Development, optimized and self-host flows require this glibc C/C.UTF8 slice;
`LOCALE_CASE` selects one exact case. NLS catalog translation, other locales,
broader Linux libcs, native changes to locale and Guile/locale interactions
remain separate acceptance work. This profile does not close M3/M6 or establish
general performance parity. Performance records are kept separately from these
functional observations.

The [C-locale measurements](benchmarks/startup-locale.md) retain all mixed and
negative results: paired large no-op time increases 1.21%, expansion 0.42%, and
allocation request-call counts stay unchanged on four controls. C.UTF8 costs
and general performance parity remain open.
