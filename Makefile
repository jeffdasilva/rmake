# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva

.DEFAULT_GOAL := help

.PHONY: help doctor bootstrap bootstrap-rust bootstrap-oracle build release fmt fmt-check lint test-unit test-tools check-core oracle-verify test-jobserver test-builtins regenerate-builtins test-oracle test-diff test-generated test-graphs test-scopes test-pattern-scopes test-filesystem test-whitespace test-word-index test-wildcards test-modes test-intcmp test-diagnostics test-stateful test-selfhost test check ci bench bench-compare clean

PYTHON ?= python3
ORACLE ?= $(CURDIR)/.tools/oracle/bin/make
CANDIDATE ?= $(CURDIR)/target/debug/rmake
BENCH_CANDIDATE ?= $(CURDIR)/target/release/rmake
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
MODE_CASE ?=
DIAGNOSTIC_CASE ?=
FILESYSTEM_CASE ?=
WHITESPACE_CASE ?=
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
		'  test-generated  Compare seeded pure-expansion programs' \
		'  test-graphs     Compare seeded explicit dependency graphs' \
		'  test-scopes     Compare scoped variables across seeded dependency graphs' \
		'  test-pattern-scopes Compare mixed pattern and explicit variable scopes' \
		'  test-filesystem  Compare Linux filesystem effects under injected faults' \
		'  test-whitespace  Compare pure-function whitespace and effect ordering' \
		'  test-word-index  Compare word and wordlist numeric boundaries and errors' \
		'  test-wildcards   Compare seeded filesystem queries and write invalidation' \
		'  test-modes      Compare 512 ordinary/one-shell execution mode combinations' \
		'  test-intcmp     Compare seeded arbitrary-length integer expressions' \
		'  test-diagnostics Compare diagnostic output and expansion ordering' \
		'  test-stateful   Compare incremental builds and timestamp boundaries' \
		'  test-jobserver  Compare mixed GNU/rmake FIFO and pipe jobserver trees' \
		'  test-selfhost   Run build and core checks through the serial candidate' \
		'  bench           Record original GNU oracle workload baselines' \
		'  bench-compare   Compare release rmake and GNU with paired timings' \
		'  check / ci      Run all current development gates' \
		'  clean           Remove Cargo build output' \
		'' \
		'Overrides: PYTHON, ORACLE, CANDIDATE, SEED, GENERATED_CASES, EXPRESSIONS.' \
		'Graph overrides: GRAPH_SEED, GRAPH_CASES, GRAPH_NODES, START_INDEX.' \
		'Mode replay: MODE_CASE=<case ID>.' \
		'Diagnostic replay: DIAGNOSTIC_CASE=<case ID>.' \
		'Filesystem fault replay: FILESYSTEM_CASE=<case ID>.' \
		'Whitespace replay: WHITESPACE_CASE=<case ID>.' \
		'Word index replay: WORD_INDEX_CASE=<case ID>.' \
		'Wildcard overrides: WILDCARD_SEED, WILDCARD_CASES, WILDCARD_EXPRESSIONS, START_INDEX.' \
		'Benchmark suite: BENCH_SUITE=core|word-spans|wildcard|bindings|all.' \
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

regenerate-builtins: oracle-verify
	$(PYTHON) tools/builtin_data.py --oracle "$(ORACLE)" --write

test-diff: build oracle-verify
	$(PYTHON) tools/conformance.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)"

test-selfhost: build
	+"$(CANDIDATE)" -rR --no-print-directory build check-core
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" MODE_CASE=mode-ordinary-first-force-qt test-modes
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" DIAGNOSTIC_CASE=message-204-child-output-order test-diagnostics
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" FILESYSTEM_CASE=close-eio-write test-filesystem
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" WHITESPACE_CASE=let-final-remainder-0b test-whitespace
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" WORD_INDEX_CASE=effect-wordlist-all-before-validation test-word-index
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" WILDCARD_CASES=1 test-wildcards

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

test-whitespace: build oracle-verify
	$(PYTHON) tools/whitespace_matrix.py --oracle "$(ORACLE)" --self-test $(if $(WHITESPACE_CASE),--case "$(WHITESPACE_CASE)")
	$(PYTHON) tools/whitespace_matrix.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" $(if $(WHITESPACE_CASE),--case "$(WHITESPACE_CASE)")

test-word-index: build oracle-verify
	$(PYTHON) tools/word_index_matrix.py --oracle "$(ORACLE)" --self-test $(if $(WORD_INDEX_CASE),--case "$(WORD_INDEX_CASE)")
	$(PYTHON) tools/word_index_matrix.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" $(if $(WORD_INDEX_CASE),--case "$(WORD_INDEX_CASE)")

test-wildcards: build oracle-verify
	$(PYTHON) tools/wildcard_generated.py --oracle "$(ORACLE)" --self-test --seed "$(WILDCARD_SEED)" --cases "$(WILDCARD_CASES)" --expressions "$(WILDCARD_EXPRESSIONS)" --start-index "$(START_INDEX)"
	$(PYTHON) tools/wildcard_generated.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --seed "$(WILDCARD_SEED)" --cases "$(WILDCARD_CASES)" --expressions "$(WILDCARD_EXPRESSIONS)" --start-index "$(START_INDEX)"

test: test-unit test-tools test-oracle test-builtins test-diff test-generated test-graphs test-scopes test-pattern-scopes test-filesystem test-whitespace test-word-index test-wildcards test-modes test-intcmp test-diagnostics test-stateful test-jobserver

check: check-core test-oracle test-builtins test-diff test-generated test-graphs test-scopes test-pattern-scopes test-filesystem test-whitespace test-word-index test-wildcards test-modes test-intcmp test-diagnostics test-stateful test-jobserver

ci: check test-selfhost

bench: oracle-verify
	$(PYTHON) tools/benchmark.py --oracle "$(ORACLE)" --suite "$(BENCH_SUITE)" --repeat "$(BENCH_REPEAT)"

bench-compare: release oracle-verify
	$(PYTHON) tools/benchmark.py --oracle "$(ORACLE)" --candidate "$(BENCH_CANDIDATE)" $(if $(BENCH_BASELINE),--baseline "$(BENCH_BASELINE)") --suite "$(BENCH_SUITE)" --repeat "$(BENCH_REPEAT)"

clean:
	+$(CARGO) clean
