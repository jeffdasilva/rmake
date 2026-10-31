# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva

.DEFAULT_GOAL := help

.PHONY: help doctor bootstrap bootstrap-rust bootstrap-oracle build release fmt fmt-check lint test-unit test-tools check-core oracle-verify test-jobserver test-load test-runtime-signals test-output-sync test-startup-exits test-runtime-effects test-graph-progress test-builtins regenerate-builtins test-oracle test-diff test-safety test-generated test-graphs test-scopes test-pattern-scopes test-secondary test-filesystem test-include-stat test-symlink-times test-archive-times test-directory-search test-whitespace test-word-index test-wildcards test-modes test-intcmp test-diagnostics test-stateful test-selfhost test-m4-release test-m5-release m4-acceptance m5-acceptance test check ci bench bench-compare profile-allocations profile-allocations-frozen clean

PYTHON ?= python3
ORACLE ?= $(CURDIR)/.tools/oracle/bin/make
CANDIDATE ?= $(CURDIR)/target/debug/rmake
M4_RELEASE ?= $(CURDIR)/target/release/rmake
BENCH_CANDIDATE ?= $(CURDIR)/target/release/rmake
PROFILE_WORKLOAD ?= large-noop
PROFILE_CANDIDATE ?= $(CURDIR)/artifacts/profile/target/release/rmake
PROFILE_OUTPUT ?= $(CURDIR)/artifacts/profile/allocations
BENCH_SUITE ?= core
BENCH_REPEAT ?= 15
BENCH_BASELINE ?=
SEED ?= 44101
GENERATED_CASES ?= 200
EXPRESSIONS ?= 32
START_INDEX ?= 0
GRAPH_SEED ?= 44109
GRAPH_CASES ?= 100
GRAPH_NODES ?= 8
PROGRESS_REPEAT ?= 40
SIGNAL_CASE ?=
SIGNAL_REPEAT ?= 3
SIGNAL_UNSTAGED ?=
MODE_CASE ?=
DIAGNOSTIC_CASE ?=
INCLUDE_STAT_CASE ?=
FILESYSTEM_CASE ?=
WHITESPACE_CASE ?=
ARCHIVE_TIME_CASE ?=
SYMLINK_CASE ?=
WORD_INDEX_CASE ?=
WILDCARD_SEED ?= 441031
WILDCARD_CASES ?= 100
WILDCARD_EXPRESSIONS ?= 32
INTCMP_SEED ?= 441011
INTCMP_CASES ?= 200
INTCMP_EXPRESSIONS ?= 32
CARGO = $(PYTHON) tools/rust.py cargo

help:
	@printf '%s\n' \
		'rmake - GNU Make compatibility development' \
		'' \
		'Available targets:' \
		'  bootstrap       Install local Rust and build the pinned GNU Make oracle' \
		'  doctor          Check the toolchain and oracle' \
		'  build / release Build debug / optimized rmake' \
		'  fmt / fmt-check Format Rust / check formatting' \
		'  lint            Run Clippy with warnings denied' \
		'  test-unit       Run Rust unit and integration tests' \
		'  test-tools      Test the independent tooling' \
		'  test-oracle     Validate fixtures against two oracle executions' \
		'  test-builtins   Verify embedded Linux defaults against the pinned oracle' \
		'  regenerate-builtins Refresh declarative Linux defaults from the oracle' \
		'  test-diff       Compare rmake against the oracle; report pending cases' \
		'  test-safety     Check documented safe errors without invoking GNU' \
		'  test-include-stat Check include-directory EINTR and cancellation behavior' \
		'  test-generated  Compare seeded pure-expansion programs' \
		'  test-graphs     Compare seeded explicit dependency graphs' \
		'  test-scopes     Compare scoped variables across seeded dependency graphs' \
		'  test-pattern-scopes Compare mixed pattern and explicit variable scopes' \
		'  test-secondary  Compare seeded deferred/static clauses with mixed scopes' \
		'  test-filesystem  Compare Linux filesystem effects under injected faults' \
		'  test-symlink-times  Compare default/-L timestamp observations under syscall faults' \
		'  test-archive-times  Compare archive metadata, scan and completion I/O faults' \
		'  test-directory-search  Compare shared directory/search observations and I/O faults' \
		'  test-whitespace  Compare pure-function whitespace and effect ordering' \
		'  test-word-index  Compare word and wordlist numeric boundaries and errors' \
		'  test-wildcards   Compare seeded filesystem queries and write invalidation' \
		'  test-modes      Compare 512 ordinary/one-shell execution mode combinations' \
		'  test-intcmp     Compare seeded arbitrary-length integer expressions' \
		'  test-diagnostics Compare diagnostic output and expansion ordering' \
		'  test-stateful   Compare incremental builds and timestamp boundaries' \
		'  test-graph-progress Stress repeated parallel barriers, groups and aliases' \
		'  test-runtime-signals Compare Linux signals, shutdown and token recovery' \
		'  test-startup-exits Compare early exit effects and cosmetic banner ordering' \
		'  test-runtime-effects Compare cancellation, lock waits and inherited PTYs' \
		'  test-output-sync Compare grouping, mutexes and mixed recursive output' \
		'  test-load       Compare controlled Linux runnable-load and fallback profiles' \
		'  test-jobserver  Compare mixed GNU/rmake FIFO and pipe jobserver trees' \
		'  m5-acceptance  Run combined development and optimized runtime gates' \
		'  m4-acceptance  Run development and optimized rule-resolution gates' \
		'  test-m4-release Run optimized corpus, stateful, archive-fault and self-host gates' \
		'  test-selfhost   Run build and core checks through the serial candidate' \
		'  bench           Record original GNU oracle workload baselines' \
		'  profile-allocations  Sample allocation requests in an isolated profile build' \
		'  profile-allocations-frozen Profile a frozen PROFILE_CANDIDATE with exact output checks' \
		'  bench-compare   Compare release rmake and GNU with paired timings' \
		'  check / ci      Run all current development gates' \
		'  clean           Remove Cargo build output' \
		'' \
		'Overrides: PYTHON, ORACLE, CANDIDATE, SEED, GENERATED_CASES, EXPRESSIONS.' \
		'Graph overrides: GRAPH_SEED, GRAPH_CASES, GRAPH_NODES, START_INDEX.' \
		'Mode replay: MODE_CASE=<case ID>.' \
		'Diagnostic replay: DIAGNOSTIC_CASE=<case ID>.' \
		'Filesystem fault replay: FILESYSTEM_CASE=<case ID>.' \
		'Symlink timestamp replay: SYMLINK_CASE=<case ID>.' \
		'Directory search replay: DIRECTORY_CASE=<case ID>.' \
		'Whitespace replay: WHITESPACE_CASE=<case ID>.' \
		'Word index replay: WORD_INDEX_CASE=<case ID>.' \
		'Wildcard overrides: WILDCARD_SEED, WILDCARD_CASES, WILDCARD_EXPRESSIONS, START_INDEX.' \
		'Benchmark suite: BENCH_SUITE=core|startup-arguments|include-operands|literal-chains|text-frames|linear-leaves|function-bodies|deep-appends|runtime-includes|builtin-transitions|word-spans|wildcard|bindings|implicit-search|parallel-branches|target-contexts|archives|archive-sequences|vpath-search|secondary|implicit-rejections|gpath|intermediates|lazy-records|vpath-records|directory-search|archive-names|symlink-times|rule-filenames|rule-tilde|libraries|all.' \
		'Benchmark samples per engine: BENCH_REPEAT=15 (3-1000).' \
		'Integer overrides: INTCMP_SEED, INTCMP_CASES, INTCMP_EXPRESSIONS, START_INDEX.' \
		'GNU compatibility is incomplete.'

bootstrap: bootstrap-rust bootstrap-oracle

bootstrap-rust:
	$(PYTHON) tools/rust.py bootstrap

bootstrap-oracle:
	$(PYTHON) tools/oracle.py bootstrap

doctor:
	+$(CARGO) --version
	$(PYTHON) tools/oracle.py verify --binary "$(ORACLE)"

build:
	+$(CARGO) build --workspace --locked

release:
	+$(CARGO) build --workspace --release --locked

fmt:
	+$(CARGO) fmt --all

fmt-check:
	+$(CARGO) fmt --all -- --check

lint:
	+$(CARGO) clippy --workspace --all-targets --locked -- -D warnings

test-unit:
	+$(CARGO) test --workspace --locked

test-tools:
	$(PYTHON) -m unittest discover -s tools -p 'test_*.py' -v

check-core: fmt-check lint test-unit test-tools

oracle-verify:
	$(PYTHON) tools/oracle.py verify --binary "$(ORACLE)"

test-oracle: oracle-verify
	$(PYTHON) tools/conformance.py --oracle "$(ORACLE)" --self-test

test-builtins: oracle-verify
	$(PYTHON) tools/builtin_data.py --oracle "$(ORACLE)" --check

test-jobserver: build oracle-verify
	$(PYTHON) tools/jobserver_interop.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)"

test-runtime-signals: build oracle-verify
	$(PYTHON) tools/runtime_signals.py --oracle "$(ORACLE)" --self-test --repeat "$(SIGNAL_REPEAT)" $(if $(SIGNAL_CASE),--case "$(SIGNAL_CASE)") $(if $(filter 1,$(SIGNAL_UNSTAGED)),--unstaged)
	$(PYTHON) tools/runtime_signals.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --repeat "$(SIGNAL_REPEAT)" $(if $(SIGNAL_CASE),--case "$(SIGNAL_CASE)") $(if $(filter 1,$(SIGNAL_UNSTAGED)),--unstaged)

test-startup-exits: build oracle-verify
	$(PYTHON) tools/startup_exits.py --oracle "$(ORACLE)" --self-test
	$(PYTHON) tools/startup_exits.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)"

test-runtime-effects: build oracle-verify
	$(PYTHON) tools/cancellation_effects.py --oracle "$(ORACLE)" --self-test
	$(PYTHON) tools/cancellation_effects.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)"
	$(PYTHON) tools/output_lock.py --oracle "$(ORACLE)" --self-test
	$(PYTHON) tools/output_lock.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)"
	$(PYTHON) tools/runtime_terminals.py --oracle "$(ORACLE)" --self-test
	$(PYTHON) tools/runtime_terminals.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)"

test-output-sync: build oracle-verify
	$(PYTHON) tools/output_sync.py --oracle "$(ORACLE)" --self-test
	$(PYTHON) tools/output_sync.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)"

test-load: build oracle-verify
	$(PYTHON) tools/load_limits.py --oracle "$(ORACLE)" --self-test
	$(PYTHON) tools/load_limits.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)"

test-graph-progress: build oracle-verify
	$(PYTHON) tools/conformance.py --oracle "$(ORACLE)" --self-test --repeat "$(PROGRESS_REPEAT)" --case graph-ready-two-barriers --case graph-ready-wait-nested --case grouped-parallel-queued-peer --case double-grouped-parallel-queued --case gpath-destination-built-parallel-first --case gpath-overlap-failure --case gpath-overlap-ignored-failure --case gpath-overlap-scope-refresh --case lazy-record-freshness-normal-parallel --case lazy-record-freshness-promoted-parallel --case lazy-record-double-first-new-parallel --case lazy-record-group-double-first-new-parallel --case vpath-record-overlap-scope-refresh --case vpath-record-overlap-failure --case vpath-record-overlap-ignored-failure --case vpath-record-destination-built-parallel-first --case directory-search-recipe-late-gpath-parallel --case directory-search-candidate-write-parallel
	$(PYTHON) tools/conformance.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --repeat "$(PROGRESS_REPEAT)" --case graph-ready-two-barriers --case graph-ready-wait-nested --case grouped-parallel-queued-peer --case double-grouped-parallel-queued --case gpath-destination-built-parallel-first --case gpath-overlap-failure --case gpath-overlap-ignored-failure --case gpath-overlap-scope-refresh --case lazy-record-freshness-normal-parallel --case lazy-record-freshness-promoted-parallel --case lazy-record-double-first-new-parallel --case lazy-record-group-double-first-new-parallel --case vpath-record-overlap-scope-refresh --case vpath-record-overlap-failure --case vpath-record-overlap-ignored-failure --case vpath-record-destination-built-parallel-first --case directory-search-recipe-late-gpath-parallel --case directory-search-candidate-write-parallel

regenerate-builtins: oracle-verify
	$(PYTHON) tools/builtin_data.py --oracle "$(ORACLE)" --write

test-safety: build
	$(PYTHON) tools/safety.py --candidate "$(CANDIDATE)"

test-diff: build oracle-verify
	$(PYTHON) tools/conformance.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)"

test-include-stat: build oracle-verify
	$(PYTHON) tools/include_stat_faults.py --oracle "$(ORACLE)" --self-test $(if $(INCLUDE_STAT_CASE),--case "$(INCLUDE_STAT_CASE)")
	$(PYTHON) tools/include_stat_faults.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" $(if $(INCLUDE_STAT_CASE),--case "$(INCLUDE_STAT_CASE)")

test-selfhost: build
	+"$(CANDIDATE)" -rR --no-print-directory build check-core
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" test-safety
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" MODE_CASE=mode-ordinary-first-force-qt test-modes
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" DIAGNOSTIC_CASE=message-204-child-output-order test-diagnostics
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" FILESYSTEM_CASE=close-eio-write test-filesystem
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" INCLUDE_STAT_CASE=include-stat-reader-eintr-1 test-include-stat
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" DIRECTORY_CASE=stream-limit-and-exec test-directory-search
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" SYMLINK_CASE=readlink-eintr-normal test-symlink-times
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" ARCHIVE_TIME_CASE=local-container-default-touch-read-header-eio test-archive-times
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" WHITESPACE_CASE=let-final-remainder-0b test-whitespace
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" WORD_INDEX_CASE=effect-wordlist-all-before-validation test-word-index
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" WILDCARD_CASES=1 test-wildcards
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" GRAPH_CASES=1 test-secondary

# Keep the two phases sequential, including when the outer driver uses -j.
m4-acceptance:
	+$(MAKE) --no-print-directory ci
	+$(MAKE) --no-print-directory test-m4-release

m5-acceptance:
	+$(MAKE) --no-print-directory ci
	+$(MAKE) --no-print-directory test-m5-release

test-m5-release: test-m4-release

test-m4-release: release oracle-verify
	$(PYTHON) tools/safety.py --candidate "$(M4_RELEASE)"
	$(PYTHON) tools/conformance.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)"
	$(PYTHON) tools/stateful.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)"
	$(PYTHON) tools/load_limits.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)"
	$(PYTHON) tools/cancellation_effects.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)"
	$(PYTHON) tools/startup_exits.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)"
	$(PYTHON) tools/include_stat_faults.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)"
	$(PYTHON) tools/output_lock.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)"
	$(PYTHON) tools/runtime_terminals.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)"
	$(PYTHON) tools/output_sync.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)"
	$(PYTHON) tools/runtime_signals.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)"
	$(PYTHON) tools/archive_times.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)"
	+$(MAKE) --no-print-directory "CANDIDATE=$(M4_RELEASE)" test-selfhost test-graph-progress


test-generated: build oracle-verify
	$(PYTHON) tools/generated.py --oracle "$(ORACLE)" --self-test --seed "$(SEED)" --cases "$(GENERATED_CASES)" --expressions "$(EXPRESSIONS)" --start-index "$(START_INDEX)"
	$(PYTHON) tools/generated.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --seed "$(SEED)" --cases "$(GENERATED_CASES)" --expressions "$(EXPRESSIONS)" --start-index "$(START_INDEX)"

test-graphs: build oracle-verify
	$(PYTHON) tools/graph_generated.py --oracle "$(ORACLE)" --self-test --seed "$(GRAPH_SEED)" --cases "$(GRAPH_CASES)" --nodes "$(GRAPH_NODES)" --start-index "$(START_INDEX)"
	$(PYTHON) tools/graph_generated.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --seed "$(GRAPH_SEED)" --cases "$(GRAPH_CASES)" --nodes "$(GRAPH_NODES)" --start-index "$(START_INDEX)"

test-scopes: build oracle-verify
	$(PYTHON) tools/graph_generated.py --scopes --oracle "$(ORACLE)" --self-test --seed "$(GRAPH_SEED)" --cases "$(GRAPH_CASES)" --nodes "$(GRAPH_NODES)" --start-index "$(START_INDEX)" --artifacts artifacts/generated-scopes
	$(PYTHON) tools/graph_generated.py --scopes --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --seed "$(GRAPH_SEED)" --cases "$(GRAPH_CASES)" --nodes "$(GRAPH_NODES)" --start-index "$(START_INDEX)" --artifacts artifacts/generated-scopes

test-pattern-scopes: build oracle-verify
	$(PYTHON) tools/graph_generated.py --patterns --oracle "$(ORACLE)" --self-test --seed "$(GRAPH_SEED)" --cases "$(GRAPH_CASES)" --nodes "$(GRAPH_NODES)" --start-index "$(START_INDEX)" --artifacts artifacts/generated-pattern-scopes
	$(PYTHON) tools/graph_generated.py --patterns --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --seed "$(GRAPH_SEED)" --cases "$(GRAPH_CASES)" --nodes "$(GRAPH_NODES)" --start-index "$(START_INDEX)" --artifacts artifacts/generated-pattern-scopes

test-secondary: build oracle-verify
	$(PYTHON) tools/graph_generated.py --secondary --patterns --oracle "$(ORACLE)" --self-test --seed "$(GRAPH_SEED)" --cases "$(GRAPH_CASES)" --nodes "$(GRAPH_NODES)" --start-index "$(START_INDEX)" --artifacts artifacts/generated-secondary
	$(PYTHON) tools/graph_generated.py --secondary --patterns --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --seed "$(GRAPH_SEED)" --cases "$(GRAPH_CASES)" --nodes "$(GRAPH_NODES)" --start-index "$(START_INDEX)" --artifacts artifacts/generated-secondary

test-modes: build oracle-verify
	$(PYTHON) tools/mode_matrix.py --oracle "$(ORACLE)" --self-test $(if $(MODE_CASE),--case "$(MODE_CASE)")
	$(PYTHON) tools/mode_matrix.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" $(if $(MODE_CASE),--case "$(MODE_CASE)")

test-intcmp: build oracle-verify
	$(PYTHON) tools/intcmp_generated.py --oracle "$(ORACLE)" --self-test --seed "$(INTCMP_SEED)" --cases "$(INTCMP_CASES)" --expressions "$(INTCMP_EXPRESSIONS)" --start-index "$(START_INDEX)"
	$(PYTHON) tools/intcmp_generated.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --seed "$(INTCMP_SEED)" --cases "$(INTCMP_CASES)" --expressions "$(INTCMP_EXPRESSIONS)" --start-index "$(START_INDEX)"

test-diagnostics: build oracle-verify
	$(PYTHON) tools/diagnostic_matrix.py --oracle "$(ORACLE)" --self-test $(if $(DIAGNOSTIC_CASE),--case "$(DIAGNOSTIC_CASE)")
	$(PYTHON) tools/diagnostic_matrix.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" $(if $(DIAGNOSTIC_CASE),--case "$(DIAGNOSTIC_CASE)")

test-stateful: build oracle-verify
	$(PYTHON) tools/stateful.py --oracle "$(ORACLE)" --self-test
	$(PYTHON) tools/stateful.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)"

test-filesystem: build oracle-verify
	$(PYTHON) tools/stdio_faults.py --oracle "$(ORACLE)" --self-test $(if $(FILESYSTEM_CASE),--case "$(FILESYSTEM_CASE)")
	$(PYTHON) tools/stdio_faults.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" $(if $(FILESYSTEM_CASE),--case "$(FILESYSTEM_CASE)")

test-symlink-times: build oracle-verify
	$(PYTHON) tools/symlink_times.py --oracle "$(ORACLE)" --self-test $(if $(SYMLINK_CASE),--case "$(SYMLINK_CASE)")
	$(PYTHON) tools/symlink_times.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" $(if $(SYMLINK_CASE),--case "$(SYMLINK_CASE)")

test-archive-times: build oracle-verify
	$(PYTHON) tools/archive_times.py --oracle "$(ORACLE)" --self-test $(if $(ARCHIVE_TIME_CASE),--case "$(ARCHIVE_TIME_CASE)")
	$(PYTHON) tools/archive_times.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" $(if $(ARCHIVE_TIME_CASE),--case "$(ARCHIVE_TIME_CASE)")

test-directory-search: build oracle-verify
	$(PYTHON) tools/directory_search.py --oracle "$(ORACLE)" --self-test $(if $(DIRECTORY_CASE),--case "$(DIRECTORY_CASE)")
	$(PYTHON) tools/directory_search.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" $(if $(DIRECTORY_CASE),--case "$(DIRECTORY_CASE)")

test-whitespace: build oracle-verify
	$(PYTHON) tools/whitespace_matrix.py --oracle "$(ORACLE)" --self-test $(if $(WHITESPACE_CASE),--case "$(WHITESPACE_CASE)")
	$(PYTHON) tools/whitespace_matrix.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" $(if $(WHITESPACE_CASE),--case "$(WHITESPACE_CASE)")

test-word-index: build oracle-verify
	$(PYTHON) tools/word_index_matrix.py --oracle "$(ORACLE)" --self-test $(if $(WORD_INDEX_CASE),--case "$(WORD_INDEX_CASE)")
	$(PYTHON) tools/word_index_matrix.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" $(if $(WORD_INDEX_CASE),--case "$(WORD_INDEX_CASE)")

test-wildcards: build oracle-verify
	$(PYTHON) tools/wildcard_generated.py --oracle "$(ORACLE)" --self-test --seed "$(WILDCARD_SEED)" --cases "$(WILDCARD_CASES)" --expressions "$(WILDCARD_EXPRESSIONS)" --start-index "$(START_INDEX)"
	$(PYTHON) tools/wildcard_generated.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --seed "$(WILDCARD_SEED)" --cases "$(WILDCARD_CASES)" --expressions "$(WILDCARD_EXPRESSIONS)" --start-index "$(START_INDEX)"

test: test-unit test-tools test-oracle test-builtins test-diff test-safety test-generated test-graphs test-scopes test-pattern-scopes test-secondary test-filesystem test-include-stat test-symlink-times test-archive-times test-directory-search test-whitespace test-word-index test-wildcards test-modes test-intcmp test-diagnostics test-stateful test-jobserver test-load test-runtime-signals test-output-sync test-runtime-effects test-startup-exits test-graph-progress

check: check-core test-oracle test-builtins test-diff test-safety test-generated test-graphs test-scopes test-pattern-scopes test-secondary test-filesystem test-include-stat test-symlink-times test-archive-times test-directory-search test-whitespace test-word-index test-wildcards test-modes test-intcmp test-diagnostics test-stateful test-jobserver test-load test-runtime-signals test-output-sync test-runtime-effects test-startup-exits test-graph-progress

ci: check test-selfhost

bench: oracle-verify
	$(PYTHON) tools/benchmark.py --oracle "$(ORACLE)" --suite "$(BENCH_SUITE)" --repeat "$(BENCH_REPEAT)"

profile-allocations:
	+CARGO_TARGET_DIR="$(CURDIR)/artifacts/profile/target" RUSTFLAGS="-C debuginfo=1 -C force-frame-pointers=yes" $(CARGO) build --workspace --release --locked
	$(PYTHON) tools/allocations.py --candidate "$(CURDIR)/artifacts/profile/target/release/rmake" --workload "$(PROFILE_WORKLOAD)" --output "$(PROFILE_OUTPUT)"

profile-allocations-frozen:
	$(PYTHON) tools/allocations.py --candidate "$(PROFILE_CANDIDATE)" --workload "$(PROFILE_WORKLOAD)" --output "$(PROFILE_OUTPUT)"


bench-compare: release oracle-verify
	$(PYTHON) tools/benchmark.py --oracle "$(ORACLE)" --candidate "$(BENCH_CANDIDATE)" $(if $(BENCH_BASELINE),--baseline "$(BENCH_BASELINE)") --suite "$(BENCH_SUITE)" --repeat "$(BENCH_REPEAT)"

clean:
	+$(CARGO) clean
