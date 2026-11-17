# Keep parallel build logs readable

Use `rmake -j4 -Otarget` to see each target's output together. This is useful
when several compiler commands run at once and you need to tell which command
printed an error. The engine prints each completed group as it becomes
available. Either target can finish first.

| Option | When output appears |
| --- | --- |
| `-Onone` | As each process writes it. This is the default. |
| `-Oline` | After each recipe command completes. Commands from different targets can still appear between one another. |
| `-Otarget` or `-O` | After each target's recipe completes. A recursive build can print its own completed target groups as it runs. |
| `-Orecurse` | When the recipe completes, including the output of recursive builds that it runs. This can leave the log quiet for longer. |

Attach the mode to the option, as in `-Otarget` or `--output-sync=target`.
With `-Oline` or `-Otarget`, a build configured for one job prints output
directly. `-Orecurse` can still group output with one job. Some tools suppress
colors when their output goes to a file. This can also happen when rmake
buffers their output and later prints it to the terminal. See GNU Make's
[output synchronization reference](https://www.gnu.org/software/make/manual/html_node/Parallel-Output.html).

## Try a parallel build

In an empty directory, save this example as `Makefile`. Start each recipe line
with a tab.

```makefile
.PHONY: all compile link-check
all: compile link-check

compile link-check:
	@printf '%s: start\n' '$@'
	@sleep 0.1
	@printf '%s: done\n' '$@'
```

1. Group the output by target.

   ```sh
   rmake -j2 -Otarget
   ```

   The two messages for each target appear together. The groups can appear in
   either order.

   ```text
   compile: start
   compile: done
   link-check: start
   link-check: done
   ```

2. Compare the result with `rmake -j2 -Onone`. With direct output, the jobs can
   both print their start message before either prints its end message.

For builds that call `$(MAKE)` from a recipe, see [how recursive builds share
job slots](jobserver.md). For `Entering directory` and `Leaving directory`
messages, see [directory reporting](directory-reporting.md).

## Reproduce a difference

From the rmake checkout, run the focused suites.

```sh
make bootstrap
make test-output-sync test-directory-reporting
```

The [output synchronization models](../tools/output_sync.py) cover separate and
shared stdout and stderr, large output, temporary spool failures, and mixed
GNU Make and rmake recursion. The tests compare both streams, status, and
filesystem effects. Large-output models allow either complete target group
first. They check each group's bytes and reject mixing lines from different
groups. For a standalone reproduction and the report format, see the
[testing guide](testing.md).

When debugging a change in output order, check these boundaries.

- Expansion, environment setup, command echoes, child output, and errors can
  each write output. Check whether each write takes place before a recipe, in
  a buffered recipe command, or during completion. `-Oline` drains before
  the next command. A forced command drains the earlier buffer before writing
  directly in line and target modes.
- During startup, the reader drains its output before remaking Makefiles. A
  restart begins with a fresh reader buffer. If startup cannot create a spool,
  check the first warning and shell stderr, and whether recipe setup retries.
- Separate spools drain stdout then stderr. A shared spool preserves their
  relative order. The buffer descriptors are private, close on exec, and stay
  above closed standard descriptors. Inherited regular output files use append
  mode. `MAKE_TERMOUT` and `MAKE_TERMERR` describe the inherited terminals.
- `--sync-mutex=fnm:PATH` lets recursive builds coordinate through a POSIX
  byte-range lock. A top-level line or target build cleans up the lock file it
  owns. Borrowing another process's lock leaves its bytes and path intact.
  See the [interruption and completion tests](runtime-cancellation.md).

The GNU Make 4.4.1 source files `src/output.c`, `src/posixos.c`, `src/main.c`,
`src/job.c`, `src/file.c`, and `src/commands.c` served as references for the
Linux implementation. The [oracle manifest](../oracle/manifest.json) identifies
the verified source archive. See [licensing and provenance](licensing.md).
