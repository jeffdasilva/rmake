# Write a compatibility fixture

The ordinary corpus contains original Makefiles that compare rmake with the
pinned GNU Make 4.4.1 reference. Use a small fixture to capture one behavior or
interaction. The [testing guide](../docs/testing.md) explains the wider suites,
and the [gap audit](../docs/m3-m6-audit.md) tracks remaining compatibility work.

## Add a case

1. Create `tests/cases/CASE_ID/` with `case.toml`, a Makefile and any input files.
   The ID must match the directory name, start with a lowercase letter or digit,
   and contain only lowercase letters, digits, underscores or hyphens.
2. Write the expected status and output independently of rmake. Include error
   paths and observable file effects where they matter. Record how you obtained
   the expectation in `provenance`.
3. Add the ID to exactly one family in [requirements.toml](requirements.toml).
   Match its `requirement` field, update `fixture_count` and `candidate_required`,
   and retain an accurate `candidate_status`. A new family also needs a title and
   a public behavioral reference.
4. Check the expectation with the pinned oracle, then compare rmake using the
   replay commands below. Keep a confirmed implementation gap `pending` until
   an exact match qualifies it as `required`.
5. Run the affected nearby cases and the required repository checks before
   review. Keep the original discrepancy and its provenance available with the
   change.

The ledger's `fixture-ready` status means that examples exist. Completing a
requirement family needs its full acceptance criteria. The
[corpus checks](../tools/test_corpus.py) enforce one ledger mapping per case and
independent output/status expectations. Accepted M4 rule and M5 runtime families
must retain required coverage without pending exemptions.

## Layout and manifest

Each directory holds one invocation. The runner copies the fixture inputs into a
disposable workspace, excluding the top-level `case.toml`. It resets that
workspace between the two engines. Both runs use the same absolute working
directory and `bin/make` executable alias.

The existing [recursive-variable example](cases/variable-recursive-late/Makefile)
checks that a recursive binding sees a later assignment. Its
[manifest](cases/variable-recursive-late/case.toml) provides a small starting point.

```toml
id = "variable-recursive-late"
requirement = "variables.flavors"
candidate = "required"
description = "A recursive variable sees the later value of BASE."
provenance = "Original rmake fixture, independently authored without upstream implementation or test material."
args = ["-rR", "--no-print-directory"]
timeout_seconds = 5

[expected]
exit_code = 0
stdout = "late\n"
```

Use a new ID for a new case and write a provenance statement that describes
your research.

The [loader](../tools/conformance.py) accepts the following top-level fields and
rejects unknown fields.

| Field | Format and behavior |
| --- | --- |
| `id` | Required directory-matching ID. |
| `requirement` | Required family ID from the ledger. |
| `candidate` | `pending` or `required`. Defaults to `pending`. Set it explicitly in committed fixtures. |
| `description`, `provenance` | Strings describing the behavior and its research history. Include both in new fixtures. |
| `args` | Complete argument list. Defaults to `["-rR", "--no-print-directory"]`. Strings cannot contain NUL. Choose flags deliberately when testing built-ins, output or other option behavior. |
| `args_hex` | Alternative complete argument list encoded as hexadecimal bytes. Mutually exclusive with `args`. Encoded NUL is rejected. |
| `timeout_seconds` | Finite number greater than zero and at most 60. Defaults to 5 seconds per engine invocation. |
| `output` | `separate` by default. Use `merged` to give stdout and stderr the same output descriptor and compare their combined order. |
| `[environment]` | Additional string-valued environment inputs under the restrictions below. |
| `[expected]` | Independent status and stream assertions under the rules below. |

### Expected bytes and status

Every committed fixture needs `exit_code` and exactly one of `stdout`,
`stdout_hex` or `stdout_template`. Assert `stderr` or `stderr_hex` whenever its
expected bytes are known. The differential comparison still checks stderr when
an independent stderr assertion is absent.

- `exit_code` is an integer from -127 through 255. Negative values describe
  signal termination as reported by the runner. Use GNU's observed status for
  deliberate failures.
- `stdout` and `stderr` are UTF-8 strings after TOML escape processing. Whitespace
  and final newlines are significant.
- `stdout_hex` and `stderr_hex` encode arbitrary output bytes, including invalid
  UTF-8 and NUL. For example, `stderr_hex = "ff0a"` asserts byte 255 followed by
  a newline.
- `stdout_template` is a UTF-8 expectation containing `@BIN@`. The runner replaces
  each token with its exact temporary `bin/make` path before comparison. This is
  useful for restart diagnostics. Other paths and output bytes remain exact.

Each stream accepts only one representation. A literal `@BIN@` in `stdout`
remains literal. There is no stderr template field. For `output = "merged"`,
assert the combined bytes in stdout and use an empty stderr expectation. See the
[merged diagnostic case](cases/diagnostic-merged-stream-output-order/case.toml)
and [raw-byte diagnostic case](cases/intcmp-nonutf8-left/case.toml).

All hex fields require an even number of ASCII hexadecimal digits, with either
letter case accepted and no spaces. An empty string encodes zero bytes. In
`args_hex`, each string is one argument, so `""` preserves an empty argument.
Output hex can contain `00`, while argument hex cannot. The manifest keeps hex
as strings and the comparator decodes the expected output to bytes.

### Environment and file inputs

The runner starts with a controlled environment. Its `PATH` contains the common
executable alias directory followed by `/usr/bin:/bin`. It creates private `HOME`
and `TMPDIR` directories and sets `LC_ALL=C`, `LANG=C` and `TZ=UTC`. Host variables
are otherwise absent unless the fixture supplies them.

Environment values must be strings without NUL. The loader accepts names matching
`[A-Za-z_.][A-Za-z_0-9.]*` and the D/F automatic-variable names such as `@D` and
`<F`. It reserves `PATH`, `HOME`, `TMPDIR`, `PWD`, `OLDPWD`, `LC_ALL`, `LANG` and
`TZ`. There is no raw-byte environment field in this format. Use a dedicated
suite for inputs outside these constraints.

Use regular files, directories and relative symlinks whose resolved targets stay
inside the fixture. The loader rejects absolute, escaping or cyclic symlinks and
special input files. Keep helpers executable when needed. Input copies retain
file metadata and existing hard-link relationships, but Git does not preserve
arbitrary modes, timestamps or hard links. Use a controlled setup or the
[stateful format](stateful/README.md) when those inputs matter.

## Verify and replay

Run commands from the repository root. The normal full-corpus entry points are
`make test-oracle` and `make test-diff`. Follow the
[setup instructions](../docs/testing.md#run-the-checks) if the pinned tools are
not yet available.

For one existing case, validate the independent expectation first, then compare
the candidate.

```sh
python3 tools/conformance.py --self-test --case variable-recursive-late
python3 tools/conformance.py --candidate target/debug/rmake \
  --case variable-recursive-late
```

Build the candidate with `make build` when source changes require it. Repeat
`--case` to select several exact IDs. With an explicit selection, `--repeat 3`
runs each comparison three times in reset workspaces. The permitted repetition
range is 1–1000. The runner rejects unknown IDs.

The default oracle is `.tools/oracle/bin/make`. `--oracle PATH` and
`--oracle-profile PATH` select another verified reference installation and its
profile. Direct runner invocations verify the profile and executable identity,
just as the Makefile workflows do.

| Result | Meaning |
| --- | --- |
| `PASS` | A required candidate case matched, or two oracle runs matched and satisfied the independent expectations. Oracle self-comparison establishes harness repeatability and expected outcomes. |
| `PENDING` | A pending candidate case still differs. It earns no compatibility credit. |
| `XPASS` | A pending case now matches. The run fails so the status can be reviewed and promoted deliberately. |
| `FAIL` | An expectation or required comparison failed, a bound was exceeded, or processes remained after completion. Matching limit failures also fail. |

The runner exits 1 for `FAIL` or `XPASS`, and 2 for setup or harness errors.
Pending-only differences can produce exit 0, so retain the result counts when
reporting a run. Oracle self-comparison never establishes candidate compatibility.

### Replay saved evidence

For `PENDING`, `XPASS` and `FAIL` results, the runner saves an evidence directory
under `artifacts/conformance/`, or the directory selected by `--artifacts`. It
contains the input tree, a JSON report and separate raw stdout/stderr files for
each engine. The report records arguments,
environment, working directory, output mode, timeout, executable paths and hashes,
reference profile, status and filesystem observations.

To replay a saved input, copy its `input` directory beneath a cases directory
using the manifest's ID as the new directory name. The saved directory name
`input` alone cannot satisfy the loader's ID check. For example, replace
`SAVED_RUN` below with the evidence path and use the ID from its `case.toml`.

```sh
fixture_id=variable-recursive-late
replay_cases=$(mktemp -d /tmp/rmake-replay.XXXXXX)
cp -a "SAVED_RUN/input" "$replay_cases/$fixture_id"
python3 tools/conformance.py --cases "$replay_cases" --self-test \
  --case "$fixture_id"
python3 tools/conformance.py --cases "$replay_cases" \
  --candidate target/debug/rmake --case "$fixture_id"
```

Replay creates a fresh temporary execution path. Keep the original evidence when
reducing a failure and record any changed executable identities. Check the raw
capture files for byte differences. JSON file snapshots retain full content as
Base64, size and SHA-256, alongside type, mode, symlink target and hard-link
relationships. Comparison uses those observations, including full content.
Hashes do not replace the byte assertions or authorize output normalization.

## Keep fixtures deterministic and contained

The runner executes trusted recipes and helpers with the invoking user's
permissions. Its process-group limits and disposable directories provide
containment for reviewed tests. They do not restrict filesystem or network
access. Use a suitable isolated environment for untrusted programs.

Keep fixture work within the disposable directories, bound loops and output, and
wait for child processes. The runner supplies `/dev/null` as stdin. It limits
combined output to 1 MiB, observes at most 4,096 filesystem entries and 16 MiB of
file data, and limits process-group size. Exceeding a bound is a harness failure.
A specialized observer is needed for terminal, descriptor or other process
relationships beyond the ordinary runner's observations.

The ordinary comparison covers raw streams, status and final state in `work`,
`home` and `tmp`. It omits timestamps. Use the [stateful suite](stateful/README.md)
for multiple invocations, deterministic time changes and timestamp relationships.
Avoid sleeps, clocks, PIDs and uncontrolled completion order in fixed-output
expectations. For parallel cases, coordinate the event under test and assert
completed artifacts before printing a stable result. Use a dedicated runtime
suite when the schedule itself is the observation.

Assert independent file effects in the fixture where practical. For example,
check a generated file's content before emitting the expected success marker.
This catches a shared wrong result even when both engines produce identical
snapshots. Preserve unexpected output and effects when reducing a discrepancy.
The [generated-suite guide](../docs/testing.md#choose-a-suite) covers seeded
language and graph campaigns and their replay controls.

## Record provenance

Fixture names, programs, inputs and expectations in this corpus were written for
this repository. No upstream test programs or manual example programs were
copied. Earlier fixtures came from documentation and executable experiments
without implementation-source inspection. Later fixtures can follow
source-informed research. Describe that distinction accurately.

When source inspection informs a case, record the upstream version, file,
function and investigated behavior in the manifest or a precise linked research
note. The [ordinary recipe lifecycle notes](../docs/forced-touch.md) provide an
example. Requirement references identify public behavioral specifications.
Expectations must remain independent of rmake's implementation.

The corpus uses `GPL-3.0-or-later`, matching the project. Follow the
[licensing and provenance policy](../docs/licensing.md) for source references,
notices and any separately identified upstream-derived material. Dated counts
and implementation history belong in the [progress record](../docs/progress.md).
