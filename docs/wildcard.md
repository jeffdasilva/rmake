# Wildcard filesystem observations

`wildcard` now runs through an invocation-owned filesystem runtime, shared by
command-line expansion, parsing, target activation, recipes and exports. Core
parses filename sequences and expands make's HOME only when a tilde pattern
needs it. The Linux adapter owns directory queries, matching, account lookup and
archive scanning. Existing `file` and `realpath` behavior stays behind the same
explicit effect interface.

Filename sequences split on unescaped space and tab; leading C whitespace is
skipped while interior LF, CR, VT and FF remain filename bytes. Quoting blanks
and glob punctuation are distinct stages. Pattern groups retain their original
order and duplicates; results within each pattern use the pinned C profile's
unsigned byte order. Dot names, dangling links, directory suffixes, path spelling,
POSIX classes and escaped literals follow the observed Linux glob boundaries.

Archive groups expand into individual references. Traditional, GNU long-name
and BSD inline-name archives are scanned with checked offsets and bounded name
storage. Thin archives retain GNU 4.4.1's no-member-match behavior. Literal
members remain when the archive exists even if the member does not. Matching
member patterns expose GNU's original archive-pattern repetition quirk. Invalid
numeric header fields and unsupported archive symbol references preserve their
raw-byte, program-only fatal diagnostics through parsing and recipe expansion.

## Cache lifetime and performance

The [shared-directory follow-up](shared-directory-search.md) also connects these
observations to implicit and VPATH probes, using incremental streams and lazy
generation refresh. Directory observations have both path and device/inode keys. Missing-directory
observations are cached too. Successful file-write opens and completed recipe
launches invalidate the observations. Reads and realpath calls do not. External
mutations without those events can leave wildcard names stale while ordinary
literal existence checks remain fresh. Escaped filename patterns can require a
cached directory scan even when the resulting filename is literal.

The runtime stores owned, terminated directory names once and compiles each
pattern once per query. Simple single-star patterns use a safe prefix/suffix
match; other patterns use libc `fnmatch`. A direct libc comparison checks the
fast path over raw byte names, dot boundaries and slash cases. Sorting directory
names once makes repeated selected subsets already ordered. Final output still
retains each query's required ordering.

Ordinary filename sequences return their owned tokens directly when archive
grouping is unnecessary. Unescaped literal queries borrow the lookup path until
the fresh existence check completes, then return one owned result without
building a list of paths or copying its output again. Single-result glob queries
also transfer their owned result. Root-separator spelling, escaped-pattern cache
behavior, archive processing, syscall interruption boundaries and output limits
remain the same.

`make bench-compare BENCH_SUITE=wildcard` measures cold, repeated, complex and
literal queries over 1,000 original files prepared before timing. This is a
focused workload suite, not evidence of complete GNU parity or general speed.
No persistent cache, daemon or GNU runtime fallback is introduced.

## Verification and limits

Original curated Makefiles cover filename bytes, escapes, tilde scopes/effects,
archives, malformed headers, fatal locations, recipe timing and write refresh.
`make test-wildcards` adds reproducible ASCII query programs with independent
matching and complete filesystem anchors. `make test-filesystem` includes six
deterministic external-mutation interposer cases covering stale names, fresh
literals, missing directories, inode aliases and replaced paths.

This slice targets the existing C-locale x86-64/glibc reference profile. Matching
and collation in other locales remain part of the open locale/profile milestone.
The interpreter's 16 MiB expansion limit remains. Directory-cache accounting is
bounded at 128 MiB of names/entry estimates; glob recursion is bounded at 512,
archive headers at one million and name tables at 16 MiB. These development
resource limits require broader release-scale validation, as do kernel/libc
variants and directory permission/fault behavior.

The libc boundary provides matching, reentrant account lookup and archive opens. It copies
account data into owned bytes, bounds lookup buffers and retains no returned
pointers. It also owns a single-attempt archive open: the returned descriptor
becomes a Rust File and closes once. This retains GNU's interrupted-open fallback
without Rust's automatic open retry. Core and application crates continue to
forbid unsafe Rust. Filesystem
errors carry explicit caller/program-only context instead of encoded sentinels.
Parser errors preserve that distinction without assuming every fatal has a
source location.

GNU Make 4.4.1 `src/read.c` (`parse_file_seq`, `find_map_unquote`, `tilde_expand`),
`src/function.c` (`string_glob`, `func_wildcard`, `func_file`), `src/dir.c`,
`src/ar.c` and `src/arscan.c` were consulted for behavior. The Linux path-spelling
boundary was checked against glibc 2.39 `posix/glob.c`. Rust code and fixture
inputs are independently authored; no upstream implementation files or tests
are incorporated.

The shared-directory follow-up moves native directory reads into a small owned
libc adapter, preserving dot entries, stream position, EINTR and fatal readdir
errors. Its independent gate also checks the stream cap and descriptor lifetime.
