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

## Host libc option diagnostics

The Linux GNU Make 4.4.1 reference delegates lexical option diagnostics to
glibc's `getopt_long`. Those lines use the active message locale and the
`libc` domain. GNU Make's usage text and value diagnostics still use the
Make catalog. The startup decoder supplies the exact original invocation
name, or an empty name for an inherited option source. Long-option
templates receive the option prefix and operand as separate arguments.
Malformed argv bytes are retained.

rmake uses public `dgettext` for those specific templates and applies the
existing checked byte formatter. The host provides the libc catalog and
native locale and domain bindings take effect at publication. The
compatibility Make catalog continues to use its own binding.

`make test-startup-usage` includes ten comparisons with an original native
constructor and a test-owned libc catalog. The cases cover C, French
`LC_MESSAGES` on a private UTF-8 locale, inherited option sources, raw
option bytes, and reordered catalog fields. They check the complete
usage transcript, both output streams, status and file effects. The
optimized acceptance flow runs the same comparisons.

Independent verification used the French `libc.mo` from Ubuntu Noble
`language-pack-fr-base` version `1:24.04+20240419`. The archive's signed
`InRelease` authenticates the package index and the downloaded package.
The checked catalog SHA-256 is
`716d8d95928401c9eed84ffcbbde676c0dac79af97582e0a710d05a925bf93fe`.
[Ubuntu package and catalog listing](https://packages.ubuntu.com/noble/all/language-pack-fr-base/filelist)
