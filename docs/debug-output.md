# Inspect a build with trace and debug output

Use `--trace` to see which recipes run and why their targets need updating.
Use `--debug` when you need to follow file visits, implicit rule searches,
Makefile remakes, or child processes.

## Choose an option

| Option | Use it to inspect |
| --- | --- |
| `--trace` | Rebuild reasons, recipe source locations, and commands, including recipes silenced with `@` or `.SILENT`. |
| `--debug=b` | Missing or out-of-date targets and the result of updating them. |
| `--debug=v` | Makefiles read, target visits, and prerequisites that are already current. It also enables basic output. |
| `--debug=i` | Implicit rule searches and basic output. |
| `--debug=b,m` | Basic output during both Makefile remakes and the ordinary goal pass. |
| `--debug=j` | Child process activity. See the [JOBS guide](job-debug.md) for process and token comparisons. |
| `--debug=p` | Recipe source locations and commands, including silent recipes. |
| `--debug=w` | The prerequisites that make a target need updating. |
| `-d` or `--debug=a` | All debug categories. |

A bare `--debug` enables basic output. Combine categories with commas or spaces.
`--trace` selects the print and why categories. A later `--debug=n` clears the
active categories, and subsequent debug options can enable them again.

The `-p` option prints the parsed database, variables, rules, and recorded file
state. Follow the [database guide](database-output.md) to inspect or compare
that output. The [GNU Make option summary](https://www.gnu.org/software/make/manual/html_node/Options-Summary.html)
describes the debug categories and their command-line spelling.

## Try a trace

This Makefile creates a file containing one line of text.

```makefile
result.txt:
	@printf 'ready\n' > $@
```

1. In an empty directory, save the example as `Makefile`. Start the recipe line
   with a tab.

2. Build the file with tracing enabled.

   ```sh
   rmake -rR --no-print-directory --trace result.txt
   ```

   With GNU Make 4.4.1 and the verified rmake release, stdout is identical.

   ```text
   Makefile:2: update target 'result.txt' due to: target does not exist
   printf 'ready\n' > result.txt
   ```

   The command exits with status zero, writes no stderr, and creates
   `result.txt` containing `ready` followed by a newline. The trace reports the
   source line and expanded recipe even though the recipe starts with `@`.

3. To repeat the comparison with GNU Make 4.4.1, remove `result.txt` and replace
   `rmake` with `make` in the command. Each run starts with a missing target.

## Replay a verified comparison

From the repository root, run the debug gate or select one named case.

```sh
make test-debug-output
make test-debug-output DEBUG_CASE=debug-bare-basic
```

The gate writes `artifacts/debug-output/development.json`. To compare an
already built release, run the driver directly.

```sh
python3 tools/debug_output.py --candidate target/release/rmake \
  --case debug-bare-basic \
  --case debug-remake-cache-include-current-existing-ordinary \
  --report artifacts/debug-output/focused-release.json
```

Each selected case runs GNU Make twice in a reset workspace, then rmake.
The driver compares stdout, stderr, exit status, ordered effects, and the
complete expected filesystem. Held-worker cases also check output before a
real recipe process can finish. A deadline or a leftover child fails the case.
See the [debug fixture guide](../tests/debug-output/README.md) for case groups,
reference setup, report interpretation, and historical campaign manifests.

Use the [traversal comparisons](../tests/debug-traversal/README.md) for implicit
search and cleanup, the [verbose comparisons](../tests/debug-verbose/README.md)
for timestamp decisions, and the [JOBS guide](job-debug.md) for process identity.
The [compatibility audit](m3-m6-audit.md) records remaining M3/M6 work.

## Enable verbose output during a build

A Makefile can append `--debug=v` to `MAKEFLAGS` while expanding a recipe or
its prerequisites. Later visits in the same goal pass still recognize files
considered before verbose output was enabled. Those visits print
`Pruning file '…'.` A later goal pass can consider the file again.

The maintained cases cover recipe and secondary expansion, existing and phony
prerequisites, double-colon rules, and dry-run, touch and question modes. They
also compare a newly encountered prerequisite and a build that starts with
verbose output enabled.

With verbose output active, `$(file <path)` reports a failed open immediately
on stdout and returns an empty value when the file is absent. The report uses
the error from that open, the active locale, and the original filename bytes.
Verbose output also reports an output-sync mutex after the reader finishes
and after any jobserver controller line.

## Missing targets in double-colon declarations

Basic and verbose output report an initially missing target for each declared
double-colon branch. Later branches retain that observation when an earlier
branch creates the file. The cases include question, dry-run, touch and
ordinary execution, with a separate initially present target control.

During a serial Makefile remake, a failed source read marks the head
missing. A later declaration can observe the file created by an earlier recipe
and find its own prerequisite older. An initially present source retains its
original comparison time across declarations.

Serial ungrouped double-colon remakes apply the optional source policy to the
first declaration. Later declarations report their own errors. A cached
prerequisite failure can be reported again when a later declaration or required
read revisits the head. Under `-k`, the first declaration determines source
status, and a physical change by either declaration can cause a reread.

An optional source, or a required source under `-k`, reports a failed
prerequisite lookup when it returns to the updater. An optional recipe failure
also produces a failed-update trace on that return. A required stopping failure
exits before that trace. These diagnostics also apply when `-n`, `-q`, or `-t`
governs a separate ordinary goal.

## Follow duplicate includes and cached failures

Under `-k`, a required ungrouped double-colon Makefile can finish its remaining
declarations after the head fails. Verbose output then reports a cached-head
prune for each additional declaration. This applies to default Makefiles and
to a primary file selected with `-f`.

A repeated required read revisits the cached head and every declaration.
Each duplicate read emits one verbose prune per declaration. If a restart
removes the declarations, each duplicate of the source emits one prune during
the new rule-free Makefile pass. A successful optional source with the same
observed timestamp also prunes each cached declaration on a duplicate read.
Grouped double-colon declarations contribute individually.

An optional double-colon source under `-k` can revisit every completed
declaration after its head fails. The retained prerequisite error follows the
cached-head trace and precedes the later declaration traces. A directly failed
head has one full revisit, followed by prunes for further cached goal entries.
A later required declaration or Makefile can also report a prerequisite failure
first observed under the optional policy.

When a required serial remake first fails as a prerequisite of an optional
source, its later root visit uses the original dependency edge in the error.
Verbose output reports the cached failure before that error, including when the
source was blocked by its own missing prerequisite. See the
[remake cache models](../tests/debug-output/remake-cache-phase.toml) for the
retained traces and [restart guide](restart-state.md) for reread behavior.

## Follow grouped Makefile remakes

An explicit grouped recipe that fails under the optional policy marks its read
peers as recently failed for their later verbose visits. A required group under
`-k` can fail after rewriting an existing include peer. The later serial root
reports the cached failure and compares the peer's physical timestamp with its
observation before the recipe to decide whether to reread the Makefiles.

A missing owner can stop the freshness scan before it visits the other group
members. A failed required group under `-k` still compares a read peer's saved
physical timestamp after the recipe. The controls cover forward and backward
timestamp changes, same-time rewrites, and removal. A successful missing owner
can also remove an existing peer and trigger a reread. The cases cover required
failure on that reread, question mode, and a missing peer that appears.

A present owner can stop scanning peers at a missing third member. If its
successful recipe removes or changes a previously read peer beyond that point,
the saved read timestamp still establishes the change. Required and optional
reads, verbose output, question mode, and reverse declaration order have
separate controls. The same comparison applies when the absent group member
is the optional owner of the recipe.

## Implementation and evidence

The reader publishes borrowed [reader events](../crates/rmake-core/src/diagnostic.rs)
and the application renders them at their actual stdout boundaries. The
[debug module](../crates/rmake-app/src/debug.rs) keeps serialized request history
and the active category mask. Reader annotations, Makefile preflight messages,
recipe output, and synchronized output retain their observed phase order.

The renderer checks its category before formatting indentation or implicit rule
labels. Rebuild-reason expansion is also guarded by its category. Debug output
therefore uses the existing visits and effects without repeating an expansion
or filesystem lookup to explain it. Local measurements and their regressions
are retained in the [historical reports](../tests/debug-output/README.md#historical-reports).

Automatic version headers emitted by debugging are part of the exact GNU
transcript. The fixture guide describes the separate explicit `--version`
branding case and its checks. Historical debug, native-module and directory
campaigns retain the executable and source identities they tested. Use the
current replay results and compatibility audit when evaluating a new revision.
