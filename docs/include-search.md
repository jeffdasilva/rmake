# Included makefile search

`-I DIR`, `-IDIR` and `--include-dir[=]DIR` contribute ordered include search
directories. Reads try the requested name first, then existing search directories
for relative names whose initial open failed with ENOENT. Other errors stop
search. Explicit `-f` files do not use this path. Include globbing happens before
search; it does not enumerate wildcards in every search directory.

Option arguments preserve byte spelling, with leading `./` removal and tilde
expansion at their position among command-line assignments. Identical expanded
option arguments are removed before processing `-I-`. A reset discards preceding
directories and disables standard defaults; following directories remain active.
Consequently, repeating exactly the same reset option does not apply it twice.
Trailing slashes are removed for lookup and `.INCLUDE_DIRS`, but retained in
`MAKEFLAGS`. Spaces and backslashes are escaped for recursive flag decoding.

`.INCLUDE_DIRS` is a default-origin simple variable. Overriding its value does
not reconfigure search. Include reads retain the resolved path in `MAKEFILE_LIST`
and the remake graph, while diagnostics use the requested include spelling.
Optional includes suppress missing-file errors, but not fatal parse or directory
read errors. Recipe-time eval uses the same runtime reader. Each read opens the
file again: no negative result or file content is cached across effects.

The configured default include directory is selected at compilation by
`RMAKE_INCLUDEDIR`, followed by `/usr/gnu/include`, `/usr/local/include` and
`/usr/include` when those directories exist. Plain Cargo builds use
`/usr/local/include` as the configured directory, as a conventional GNU install
does. The repository Rust wrapper selects `.tools/oracle/include` to match the
pinned reference installation; an explicit environment value overrides this
build setting. This is configuration data, not runtime delegation to GNU.

The slice adds 54 original required fixtures, plus CLI boundary tests. All 40
initial fixtures failed on the preceding binary. Coverage includes ordering,
resets, raw path distinctions, metadata precedence, nested and eval reads,
recursive flags, read errors, directory existence at startup, regenerated files,
and `-n/-q/-t` restarts. Expectations pin independent output and exit status;
the runner additionally compares both executions' filesystem contents and modes.
GNU Make 4.4.1 `read.c` and `main.c` were consulted as behavioral references;
no implementation code or upstream tests were copied.

A 101-sample alternating core comparison measured tiny startup at 0.610 ms
(previous rmake 0.594 ms; GNU 0.566 ms), large no-op at 27.866 ms (28.179 ms;
11.534 ms), and expansion at 4.755 ms (4.631 ms; 2.869 ms). Paired previous/current
ratios were 0.973, 1.014 and 0.972. This is a regression check, not a speedup
claim: include-path setup adds work, and GNU remains faster. Raw samples are in
the ignored `artifacts/bench/include-search-core-final-101.json`. Measurements
include process startup and warm OS caches, with no CPU pinning or confidence
intervals; the previous source is `667b16f`.

Remaining work includes MAKEFILES preloading, dynamic MAKEFLAGS changes,
reconstructing the directory path after all flag/restart transitions, and the
full GNU include-error recovery surface. The current reader and restart safety
bounds remain documented in [dynamic makefiles](dynamic-makefiles.md).
