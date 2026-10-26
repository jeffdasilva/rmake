# M4 exit audit: rule resolution

Audit date: 2026-10-26. **M4 is open.** A green development CI run protects
implemented behavior; it does not satisfy M4's complete rule-resolution gate.
The twelve historical pending fixture IDs are not an inventory of all missing
features. The observations below use the pinned GNU Make 4.4.1 oracle.

## Covered families

Required ledger coverage exists for explicit/static/implicit/suffix/grouped and
double-colon rules, built-in declarations, candidate secondary expansion,
VPATH/GPATH, archive members/groups/name encodings, intermediate policies,
lazy records, quoted filenames, tilde resolution and declaration recording.
The [progress record](progress.md) and individual slice notes identify exact
case counts and limits. Default stat faults and simulated-completion state are
covered by the [timestamp follow-up](stat-timestamps.md).
The [library slice](library-resolution.md) now implements `-l` / `.LIBPATTERNS`
lookup, ranked paths and record merging; its original failing probe below now
matches GNU.

These are implemented slices. Their combined tests are necessary but insufficient
for the exit requirement that search priority, automatic variables, lifecycle and
filesystem edge cases match across every rule family.

## Confirmed blockers

| Area | Direct evidence | Required follow-up |
| --- | --- | --- |
| Hidden-prerequisite combinations | The [hidden-prerequisite slice](extra-prerequisites.md) implements the probe below, snapshot phases and edge roles. | Retain strict archive/alias/secondary combinations in the final audit. |
| File-record cross-effects | The [file-record slice](file-record-order.md) resolves `chain-two-roots` and covered collision/growth/lifecycle paths. | Hidden-prerequisite recording is covered by its slice; finish archive-container recording and combination checks. |
| Recursive HOME filename queries | The [tilde notes](rule-tilde.md) preserve a probe where GNU's shared scratch buffer yields `earlyarker`. | Resolve the compatibility/safety boundary explicitly. Do not introduce unsafe memory corruption or count it as matching. |
| Archive observations | Earlier slices retain standalone-member/container timestamp and metadata-error ordering gaps. | Expand the I/O/error matrix and file-record timing tests before claiming completeness. |

The hidden-prerequisite and historical library probes now pass and have required
coverage in their slices. GNU is never a product implementation fallback.

### Hidden prerequisites

```make
.EXTRA_PREREQS=hidden
all: normal;@printf '[%s|%s|%s|%s]\n' '$<' '$^' '$+' '$?'
normal:;@printf 'normal\n'
hidden:;@printf 'hidden\n'
```

Under `-rR --no-print-directory`, GNU prints:

```text
hidden
normal
[normal|normal|normal|normal]
```

rmake now matches the complete output, status and empty stderr. Required fixtures
also check missing/hidden/order-only inputs and scope/recording effects.

### Library lookup

```make
.LIBPATTERNS=lib%.a
$(file >libdemo.a,original library probe)
all: -ldemo;@printf '[%s|%s]\n' '$<' '$^'
```

Under `-rR --no-print-directory`, GNU prints `[libdemo.a|libdemo.a]` and returns
zero. rmake now matches this result. The explicit
`.LIBPATTERNS` matters: `-R` removes the default definition, so a probe without
that assignment fails on GNU as well and would not establish this gap.

## Remaining acceptance work

After the confirmed failures are implemented, audit combinations rather than
checking only individual switches: scoped/secondary/implicit rules with hidden
prerequisites and libraries; grouped/double-colon lifecycle and errors; alias and
directory-cache transitions; archive formats, faults and raw names. Retain exact
streams, status and filesystem effects, and add stateful transition scenarios.
Promote the existing rule-resolution pending case only after its implementation
and strict comparisons pass. Run the combined development and self-hosted gates
on the exact proposed commit before marking M4 complete.

M3 reader/dynamic-option gaps and M5/M6 runtime, locale and extension profiles also
remain on the full compatibility path. Persistent-cache correctness and general
GNU performance parity have separate M8 gates; neither is a substitute for M4.
