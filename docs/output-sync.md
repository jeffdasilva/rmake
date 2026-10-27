# Linux output synchronization

The independent Rust implementation accepts `-O[TYPE]` and
`--output-sync[=TYPE]`: `none`, `line`, `target`, and `recurse`. Omitted types
select `target`; an optional type must be attached. Recursive flags preserve
an explicit `none` and GNU's ordering relative to jobserver and load options.

The interpreter owns lazy recipe buffers. Body expansion happens before slot
reservation; initial shell configuration, continuation configuration, exports,
command echoes, child output and failure diagnostics keep their individual
output phases. Line mode drains before another command, target mode holds a
recipe's output, and recurse mode holds recursive descendants. Forced commands
bypass line/target buffering after earlier buffered output is drained. Reader
output drains before makefile rebuilding; restarts create a fresh reader buffer.
GNU disables line/target grouping for a configured single job, while retaining
its recursive option spelling. Recurse still groups serial recipes.

Linux spools are append-mode, close-on-exec, unnamed temporary files. Combined
stdout/stderr share one spool; separate streams drain stdout then stderr.
`--sync-mutex=fnm:PATH` joins GNU's POSIX byte-range locking protocol. A root
line/target build owns its named mutex; borrowed mutex bytes and paths remain
untouched. Private descriptors are promoted above intentionally closed standard
streams. `MAKE_TERMOUT` and `MAKE_TERMERR` describe inherited terminals before
redirection and retain imported bindings. Inherited regular output files use
append mode, as in GNU.

The required corpus adds 107 exact-byte cases for modes, startup/recursive
flags, mutex errors, reader/restart phases, touch/query/dry execution, retained
fatal owners and ignored launch errors. `make test-output-sync` runs 76 original
controlled profiles three times against the pinned oracle and candidate. Both
stream arrangements, owned/borrowed mutexes, and both GNU/rmake recursive
directions are included. Twelve large-output profiles exceed pipe capacity:
they accept either complete target first, while rejecting interleaving,
truncation, duplicate output or missing bytes. Raw streams are retained; only
that explicitly unordered target-group order has a named validator. All other
observations remain exact. Development and optimized CI run these gates.

The [interruption/completion follow-up](runtime-cancellation.md) covers the
output-lock and evaluation/effect interruption paths and remaining synthetic
owner/intermediate cases. M5's current position is tracked in the progress log.

Reference study: GNU Make 4.4.1 `src/output.c`, `src/posixos.c`, `src/main.c`,
`src/job.c`, `src/file.c` and `src/commands.c`, from the verified archive with
SHA-256 `dd16fb1d67bfab79a72f5e8390735c49e3e8e70b4945a15ab1f81ddb78658fb3`.
These sources informed observed requirements; no implementation or upstream
fixtures were copied or translated. The original bounded probes and campaign
provide the executable evidence.
