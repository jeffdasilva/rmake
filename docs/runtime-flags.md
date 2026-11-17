# Change build options in a Makefile

Assign to global `MAKEFLAGS` to change options while rmake reads a Makefile.
The change takes effect immediately and can affect later variable expansion,
include search, and recipes. Recursive builds also receive options through
`MAKEFLAGS`. See GNU Make's
[option syntax](https://www.gnu.org/software/make/manual/html_node/Options-Summary.html)
and [options for recursive builds](https://www.gnu.org/software/make/manual/make.html#Options_002fRecursion).

## Enable warnings after an earlier read

In an empty directory, save this example as `Makefile`. Start the recipe line
with a tab. Run it without inherited make options or a definition of `UNSET`.

```makefile
BEFORE := $(UNSET)
MAKEFLAGS += --warn-undefined-variables
AFTER := $(UNSET)

.PHONY: all
all:
	@printf 'build continues\n'
```

Run `rmake`. The first assignment expands `UNSET` silently. The assignment to
`MAKEFLAGS` enables undefined-variable warnings, so the third line reports this
message on stderr.

```text
Makefile:3: warning: undefined variable 'UNSET'
```

The recipe prints `build continues` on stdout and the command succeeds. Both
`BEFORE` and `AFTER` contain an empty value. The warning changes the diagnostic,
while the undefined variable still expands to empty.

To compare, run GNU Make 4.4.1 with the same Makefile and environment. Both
commands produce the same warning and recipe output. The `+=` assignment keeps
existing flags while adding the warning option.

## Pass options to recursive builds

Use `$(MAKE)` in recursive recipes, such as `$(MAKE) -C subdir`. The child
receives `MAKEFLAGS` through its environment. This also carries command-line
variable definitions and the information needed to share job slots. See the
[MAKE variable](https://www.gnu.org/software/make/manual/html_node/MAKE-Variable.html)
and the [rmake jobserver guide](jobserver.md).

A global `GNUMAKEFLAGS` assignment takes effect when options are interpreted
again after reading the Makefiles. Replacing `MAKEFLAGS` with `GNUMAKEFLAGS` in
the example therefore leaves the third line silent. The warning option is
active by recipe time and is included in the flags passed to recursive builds.
For Makefiles that require GNU-compatible behavior, use `MAKEFLAGS` directly.
GNU's [special-variable reference](https://www.gnu.org/software/make/manual/html_node/Quick-Reference.html)
explains the portability role of `GNUMAKEFLAGS`.

## Check a runtime option change

From the rmake checkout, prepare the comparison tools and run the relevant
suites.

```sh
make bootstrap
make test-diff test-runtime-option-values test-jobserver test-output-sync
```

The curated cases cover option changes during reading and recipe-time `eval`,
assignment precedence, include search, restarts, and FIFO and pipe jobservers.
`make test-runtime-effects` also checks cancellation during option expansion.
The [requirements ledger](../tests/requirements.toml) lists the cases. See the
[testing guide](testing.md) for reproductions and report formats.

Compare raw stdout, stderr, exit status, and target-file effects. Jobserver
helpers check descriptor and FIFO identities against the owning processes and
keep the captured output intact. The [M5 audit](m5-exit-audit.md) records the
accepted runtime scope. Broader option ordering and metadata combinations
remain part of the [M3/M6 work](m3-m6-audit.md).

## Preserve option timing in the implementation

- Finish expanding a `MAKEFLAGS` value, including nested `eval` effects, before
  handing it to the runtime option handler. Expansion contexts retain that
  handler and cancellation support. Keep mutable host borrows outside the
  expansion call.
- Preserve option spelling such as `-j2` separately from scheduler capacity.
  Explicit command-line job counts win during final option replay. Earlier
  assignment effects remain visible while reading.
- Reconstruct `MAKEOVERRIDES` once when appending flags, and quote literal dollar
  signs in recursive option data. Restore startup options before rereading
  Makefiles after a restart. See [assignments in flag values](flag-payloads.md).
- Join a borrowed jobserver before reader shell functions run. Create owned
  pools after reading. Inherited pipe descriptors are close-on-exec.
  Nonrecursive commands advertise unavailable pipe authentication before the
  command-variable suffix, while recursive commands keep usable authentication.
- Create output buffers only when needed. Runtime `eval` can enable buffering
  during recipe or shell-configuration expansion. Preserve the explicit flush
  points described in the [output guide](output-sync.md).

## Source references

Reference study used `src/main.c`, `src/variable.c`, `src/read.c`, `src/job.c`,
`src/output.c`, and `src/posixos.c` from the verified GNU Make 4.4.1 archive.
Its SHA-256 is
`dd16fb1d67bfab79a72f5e8390735c49e3e8e70b4945a15ab1f81ddb78658fb3`.
The [oracle manifest](../oracle/manifest.json) identifies the reference build.

The Rust state model, implementation, protocol helpers, and fixtures were
independently authored. No GNU implementation or upstream test code was copied
or translated. See [licensing and provenance](licensing.md).
