# Raw process restart environments

Three original profiles observe the process entry environment vector through
an original loader constructor and verify the complete ordered witness. They
cover duplicate and unkeyed entries, non-UTF-8 bytes, a live native mutation,
multiple restart counters, and a wrapped-zero count whose live entry was
removed. A fresh actual exec is required; rebuilding interpreter state would
miss the second constructor.

`tools/restart_environment.py` compiles original modules once per case and
runs pinned GNU twice and the candidate in identical reset workspaces. Status,
stdout, stderr, bounds and complete filesystem (including the independently
selected executable symlink) are strict. Use `--candidate` or `--oracle-only`,
`--report` and optional exact `--case`. `raw-ordered-vector` is the focused
self-host profile. No GNU Make implementation/header/test code is reused.

The host preserves the live ordered vector in owned C storage, changing only
existing keyed counters and appending a nonzero absent restart count. The
interpreter records the last imported startup count in its existing import
pass. Synchronous native environment changes are supported; concurrent mutation
while accessing the POSIX environment remains outside that interface contract.

These bounded profiles do not establish full M3/M6 compatibility or speedup.

The [qualification archive](raw-vector-history.json) retains the three preceding
failures, GNU2 and both corrected binaries, exact source and related regression
observations. It also retains the distinct inherited pipe success discovery
without granting that case compatibility credit.
