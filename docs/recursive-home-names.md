# Recursive HOME filename queries

The pinned GNU Make 4.4.1 parser shares filename scratch storage with recursive
filename queries made while expanding `HOME`. For example:

```make
.DEFAULT_GOAL=all
unexport HOME
HOME=$(if $(wildcard marker),late,early)
~/target: X:=$(file >marker,seen)value
all:earlyarker
earlyarker:;@printf '[%s|%s]\n' '$@' '$(X)'
```

With `marker` initially absent, both implementations print
`[earlyarker|value]`. The nested filename query writes `marker` even without a
match. HOME expands to `early`, then takes its suffix from the overwritten name
starting at byte one: `arker`. Expanding the assignment value creates `marker`
after the target names were selected. Later reads can therefore select different
names; the HOME value and filesystem observations are not memoized.

rmake reproduces the byte writes using owned, bounded Rust storage. Filename
sessions preserve nested wildcard/eval/include writes, the trailing NUL and
retained bytes. Archive-group continuation writes use the original member offset;
an empty closing group element can write bytes without producing a filename.
Ordinary unmatched queries also write their input pattern. Include names retain
their own leading-dot policy; wildcard patterns retain theirs.

The session lives only while parsing potentially recursive HOME filenames or a
nested parser called from them. Normal rule names retain their borrowed path and
avoid a second full scan. Nested errors release the session before propagating.
The environment/account fallback appends an overwritten suffix as bytes; it must
not reinterpret `arker` as the account name in `~arker`. No raw pointers, unsafe
code, persistent scratch/cache, skipped effects or GNU runtime fallback are used.
The existing expansion/resource bounds still apply. This models reproducible
byte effects; it makes no promise to reproduce arbitrary undefined allocator or
dangling-pointer behavior outside the tested Linux reference profile.

## Original evidence

253 new required fixtures cover bare tilde, slash and long suffixes; missing,
multiple, escaped-blank and dot-prefixed query names; nested wildcards, eval and
optional includes; archive-group writes; the wildcard function's own tilde path;
environment fallback; and normal/dry/query/touch/parallel controls. Exact streams,
status and file contents/types/modes/links remain strict. The corrected preceding
release fails 142 behavior cases; 111 are controls. No existing expectation or
pending classification changes.

`recursive-home-transitions` adds twelve stateful invocations with fixed input
timestamps. It creates/removes the queried file, changes query forms and modes,
and changes an optional include from absent to a file containing another rule.
Three core tests check nested byte effects, group continuation and cleanup after
an expansion error. Corpus checks require every mapped M4 rule/archive/resolution
fixture to remain required; adding a pending exemption fails the tooling gate.

Inventory: **4,892 curated / 4,881 required / 11 historical pending**,
**68 stateful scenarios / 329 invocations**, **393 Rust / 294 tooling tests**.
The historical pending cases belong to execution/reader/environment surfaces;
they are not counted as passing compatibility cases.

## Source study

Consulted the verified GNU Make 4.4.1 archive, SHA-256
`dd16fb1d67bfab79a72f5e8390735c49e3e8e70b4945a15ab1f81ddb78658fb3`,
`src/read.c` (`parse_file_seq`, `tilde_expand`, target-variable and include
recording) and the filename-query path in `src/function.c`. The implementation,
Makefiles, stateful actions and anchors are independently written. No GNU
implementation or upstream tests were copied or translated.

The [M4 exit audit](m4-exit-audit.md) records the combined acceptance gate and
separate M3/M5/M6/M8 requirements. Completing the M4 tranche is not a claim that
the whole compatibility contract or general GNU performance parity is complete.
