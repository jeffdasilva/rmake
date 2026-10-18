# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva

.DEFAULT_GOAL := help

.PHONY: help doctor bootstrap bootstrap-rust bootstrap-oracle build release fmt fmt-check lint test-unit test-tools check-core oracle-verify test-oracle test-diff test check ci bench clean

PYTHON ?= python3
ORACLE ?= $(CURDIR)/.tools/oracle/bin/make
CANDIDATE ?= $(CURDIR)/target/debug/rmake
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
		'  test-diff       Compare rmake against the oracle; report pending cases' \
		'  bench           Record original GNU oracle workload baselines' \
		'  check / ci      Run all current development gates' \
		'  clean           Remove Cargo build output' \
		'' \
		'Overrides: PYTHON, ORACLE, CANDIDATE. GNU compatibility is incomplete.'

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

test-diff: build oracle-verify
	$(PYTHON) tools/conformance.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)"

test: test-unit test-tools test-oracle test-diff

check: check-core test-oracle test-diff

ci: check

bench: oracle-verify
	$(PYTHON) tools/benchmark.py --oracle "$(ORACLE)"

clean:
	+$(CARGO) clean
