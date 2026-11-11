# Negative switches and GNU option origins

Support `--no-silent`, `--stop` and `--no-keep-going`, and retain explicit `-S`
in recursive flags. GNU's legacy `-b` and `-m` options are accepted and ignored.
Aliases and clustered options follow their normal byte-order behavior. An
explicit negative silent setting is serialized as `--no-silent`; an explicit
negative keep-going setting is serialized as `S`. An unspecified false default
produces neither spelling.

Silent, keep-going and directory-reporting settings retain their switch origin.
GNU decodes startup `GNUMAKEFLAGS` and `MAKEFLAGS` with command-line priority.
Explicit command-line switches can replace those startup settings. Direct file
and eval assignments use their assignment origin, while post-read replay has
environment priority. This allows `override MAKEFLAGS` to change an earlier
command-line setting and preserves the inherited setting against ordinary file
assignments. Reconstructing the flag text retains the existing origin.
Other switches retain their existing policies, including the separate CLI job
count behavior. Broader CLI ordering/presentation remains in the
[M3/M6 audit](m3-m6-audit.md).

## Runtime metadata scopes

GNU resets `MAKEFLAGS` in the current file's variable set. A new local binding
can shadow a stronger global value after recipe-time eval, while a stronger
explicit binding in that same file retains its priority. `MFLAGS` uses the same
current-file boundary, with its own environment origin. Ancestor/global bindings
remain separate; normalization in one child must not poison a sibling.

Include-path metadata follows a separate global update path. User file/override
`.INCLUDE_DIRS` bindings retain their precedence, and a successful default update
becomes visible to later files. The actual include search still follows decoded
`-I` options even when stronger user metadata hides those directory words.
The implementation preserves existing binding metadata when materializing an
explicit current-file value and does not merge or rewrite ancestor/global sets.

## Evidence

The original 468 comparisons have two raw GNU Make 4.4.1 observations and a
final candidate comparison. A further 51 ordinary and 27 directory-reporting
cases cover opposing startup and file settings, four assignment forms, both
environment sources, recursive flag publication, and explicit CLI and override
controls. The include-path cases confirm that unrelated live switches still take
effect. Cases cover aliases and clusters,
ignored compatibility options, recursive flags, actual recipe echo/failure
behavior, import/CLI/file/override precedence, eval, recipe expansion, secondary
expansion, siblings, target bindings, include metadata and POSIX mode. Include
queries use original controlled directory names rather than compile-prefix
anchors. Recursive command echoes invoke the harness alias by basename, keeping exact
expectations independent of the temporary workspace. No output sorting or
normalization is introduced.

A core test verifies that a direct file value survives normalization, a child can
shadow its ancestor, and parent/global values remain intact. Four tooling tests
protect required coverage, explicit negative spellings and independent sibling
metadata anchors. The corpus is 7,825 total / 7,824 required / one pending; unit
totals are 437 Rust / 345 tooling tests. Full sequential development/optimized
acceptance and exact-head CI remain merge gates. This slice does not close M3/M6
or the existing oracle safety conflicts.

## Performance observations

Alternating 101-sample unisolated medians, with no builds or acceptance runs
concurrent with timing:

| Workload | GNU | Current | Previous |
| --- | ---: | ---: | ---: |
| Tiny no-op | 0.562 ms | 0.646 ms | 0.651 ms |
| 10,000-target no-op | 12.383 ms | 32.202 ms | 32.229 ms |
| 5,000 expansions | 2.816 ms | 4.723 ms | 4.848 ms |
| Parallel branches, depth 1 | 87.960 ms | 90.149 ms | 90.586 ms |
| Parallel branches, depth 4 | 87.705 ms | 89.993 ms | 90.103 ms |

These observations stay near the preceding implementation and establish no
statistical or general speed-parity claim. GNU is faster on every selected
workload. Reports: `artifacts/bench/switch-final-{core,parallel}.json`.
Candidate SHA256:
`d15be250ba9f0c95efb92967d1b9332ead74ac01cffce22556eb00bc5706a58e`;
previous candidate:
`85b1508a291920d75153f4caa8a07769a8901387fe4ee3c5875cf499fe5b8a9b`.

## Reference provenance

Reference study used `src/main.c` (switch descriptors, `decode_switches`,
`reset_makeflags`, `define_makeflags`), `src/read.c` (`construct_include_path`)
and `src/variable.c` (definition/scope behavior) from the verified GNU 4.4.1
archive, SHA256
`dd16fb1d67bfab79a72f5e8390735c49e3e8e70b4945a15ab1f81ddb78658fb3`.
The Rust implementation, tests, Makefiles and byte anchors are original; no GNU
implementation or upstream tests were copied or translated.
