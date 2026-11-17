# GNU Make 4.4.1 options in rmake

Use the [GNU option reference](https://www.gnu.org/software/make/manual/html_node/Options-Summary.html)
for syntax and meanings. Use this table to find rmake's tests and the combinations
that still need checking. The [M3/M6 audit](m3-m6-audit.md) tracks broader startup,
runtime, and release requirements.

## Find the tests for an option

| Options | Tested behavior and remaining work |
| --- | --- |
| `-b` and `-m` | Accept and ignore these options, as GNU Make does. |
| `-h`, `--help`, `-v`, and `--version` | rmake identifies itself in its own help and version output. The [startup usage tests](startup-usage.md) cover selected errors, empty arguments, diagnostics, and immediate command effects. Check early-exit interactions across the remaining profiles. |
| `-C`, `--directory`, `-f`, `--file`, and `--makefile` | The [restart directory tests](restart-directories.md) and [source profiles](profile-inputs.md) cover directory changes, source selection, and inherited options. The startup usage tests cover empty directory operands. Extend coverage of duplicate and computed paths, startup ordering, and process restarts. |
| `-I` and `--include-dir` | See [ordered arguments](ordered-cli-startup.md), [startup include search](startup-include-search.md), [runtime include operands](runtime-home-includes.md), and [second-stage include search](secondary-include-search.md). Check combinations of tilde expansion, changing `HOME`, nested flag resets, and include order. |
| `-B`, `-e`, `-i`, `-k`, `-L`, `-n`, `-q`, `-r`, `-R`, `-s`, `-S`, and `-t` | The [option origin tests](switch-origins.md) and [mode tests](testing.md#choose-a-suite) cover supported spellings and runtime behavior. Qualify the remaining startup and invalid-argument combinations. |
| `-j`, `--jobs`, `-l`, `--load-average`, and `--max-load` | Verify the [parallel and load tests](load-limits.md). Extend their interactions with source remakes, recursion, extension calls, and process limits. |
| `-O` and `--output-sync` | The [output validation tests](output-validation-order.md) check definitions, errors, and locks across flag sources. The [output synchronization tests](output-sync.md) cover observable recipe output. |
| `--jobserver-auth`, `--jobserver-fds`, `--jobserver-style`, and `--sync-mutex` | The [local jobserver tests](jobserver.md) cover Linux pipe and FIFO authentication. Extend coverage of inherited lifetime, restart, and native integration. |
| `--no-silent`, `--no-keep-going`, and `--stop` | The [option origin tests](switch-origins.md) check negative spellings, precedence, and recursive propagation. Check the remaining startup combinations. |
| `-w`, `--print-directory`, and `--no-print-directory` | The [directory output tests](directory-reporting.md) and [restart traces](imported-directory-trace.md) cover flag ordering, recursion, output groups, and shutdown. Extend database, debug, path, and resource coverage. |
| `-d` and `--debug` | See [debug output](debug-output.md), [child job diagnostics](job-debug.md), and the [debug test suites](../tests/remake-parallel/README.md). Preserve exact debug banners, identities, and ordering. More asynchronous visits, implicit search, default Makefile discovery, and profile combinations remain open. |
| `-p` and `--print-data-base` | The [database output tests](database-output.md) check GNU ordering, formatting, and independently observed timestamps. Extend lifecycle and locale coverage. |
| `--trace` | The [trace tests](trace.md) check rebuild reasons, identity, command echo, modes, and remakes. Extend debug, startup, and locale interactions. |
| `-E` and `--eval` | The [command-line eval tests](command-line-eval.md) cover evaluation before Makefile reads, metadata, source locations, and restarts. Extend startup, usage, and profile coverage. |
| `-o`, `--old-file`, `--assume-old`, `-W`, `--what-if`, `--new-file`, and `--assume-new` | The [assumed file tests](assumed-files.md) check which prerequisites appear old or new, how this affects rebuilds, and when assumptions propagate through a remake. Extend decoder and profile combinations. |
| `--warn-undefined-variables` | The [warning tests](undefined-warnings.md) and [CLI assignments](cli-assignment-effects.md) cover selected scopes, source phases, and warnings. Extend combinations with inherited expansion. |
| `--shuffle` | The [shuffle tests](shuffle.md) cover seeds, barriers, scopes, recursion, restarts, and failure. Add malformed-option, cycle, and broader runtime coverage. |
| `--temp-stdin` | The [stdin source tests](stdin-sources.md) cover this internal restart option, including the owned temporary file and argument rewrite. Extend resource and startup interactions. |

## Check startup and live option handling

Treat each source as a separate input. GNU can handle the same spelling
differently in explicit arguments, inherited flags, a later assignment to
`MAKEFLAGS`, and recursive invocation. Keep tests for `GNUMAKEFLAGS` and
`MAKEFLAGS` order, warning policy, and effects that occur before an error.

- Compare [flag assignments](flag-payloads.md), [live option errors](live-invalid-options.md),
  and [missing operands](live-option-arity.md) to check which definitions and
  effects run after a rejected option.
- Check [live warning order](live-warning-phases.md),
  [post-read warning order](passive-warning-phases.md), and
  [source completion](passive-source-order.md) when changing evaluation order.
- When changing a decoder, test [long option prefixes](long-option-prefixes.md),
  short clusters, raw argument bytes, a standalone `-`, and option terminators.
  See the [terminator tests](../tests/flag-terminator/README.md).
- The [runtime flag tests](runtime-flag-inspection.md) cover selected option
  queries and failure precedence. Test startup, reader, and recipe paths,
  including cases that update options more than once.

Run the relevant [test suites](testing.md#run-the-checks) against the pinned
reference. Use the [safety exception policy](safety-exceptions.md) for approved
crash, hang, or unstable nested-option inputs. Record any other finite mismatch
as compatibility work.
