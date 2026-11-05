# Startup syntax errors and usage

The startup decoder reports a malformed argument when it reaches the argument,
then continues with that source's later flags and definitions. A syntax error
ends startup after the source completes, before later sources, deferred eval,
or the Makefile reader. Immediate definitions can still perform effects. GNU
Getopt errors identify explicit argv by its original bytes; Make-generated
operand errors use the Make program name. A separate already-supported fatal
debug or output-sync validation does not print usage.

Empty `-C` and `-f` operands are rejected before filename preparation. A path
that exhausts its `./` components uses the same canonical fallback as the
filename helper, without indexing the original buffer past its bounds. An
explicitly empty `-j` operand is distinct from bare `-j`.

`make test-startup-usage` checks 41 original cases in C and French, including
ordered effects, startup source boundaries, raw argument bytes, zero and empty
job counts, directory normalization and full stderr. The version control only
permits the expected, separately validated cosmetic stdout banner. The test
runs GNU Make 4.4.1 twice under the same invocation path as rmake and compares
all owned filesystem changes. Use `STARTUP_USAGE_CASE=empty-directory-then-assignment`
for a focused run.

The 4.4.1 release's `src/main.c` (`print_usage`, `decode_switches`, and
`decode_env_switches`) was inspected for argument and publication order. The
GNU usage message keys and option wording are declarative compatibility data,
observed from the verified executable and matched to the attributed release
catalogs. The implementation and test fixtures are original and never execute
GNU at runtime. Localized Make messages use the active catalog and checked
formatting. Localized libc getopt catalogs beyond the tested host remain an
independent profile requirement.

`make test-runtime-option-values` separately compares 35 exact cases for
empty string and invalid job count notices, ordered immediate and passive
assignments, contextual HOME includes, raw style names, and actual pool
creation. Live value errors preserve the earlier selection and continue,
while the same argv errors print usage. An unknown jobserver style is
reported only when make attempts to create a pool. Numeric job operands use
the observed unsigned 32-bit bound, including leading blank handling.
