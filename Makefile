# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva

.DEFAULT_GOAL := help

.PHONY: test-developer-flows test-flag-terminator test-startup-usage test-runtime-option-values
FLAG_TERMINATOR_CASE ?=
STARTUP_USAGE_CASE ?=
RUNTIME_OPTION_VALUE_CASE ?=
DEVELOPER_FLOW_CASE ?=

.PHONY: bootstrap-integration test-integration
.PHONY: test-database-output test-database-search test-database-numeric test-database-order-roles test-database-dry-run-time test-database-branch-metadata test-database-double-dry-run test-database-grouped test-database-implicit-grouped test-database-grouped-double
.PHONY: test-database-lifecycle test-database-nls test-database-nls-release
.PHONY: test-locales bootstrap-locale-data test-locale-collation
.PHONY: bootstrap-guile test-guile-components test-guile-frontend
.PHONY: test-native-fenv test-resource-nproc test-resource-nproc-release test-resource-nproc-selfhost
.PHONY: catalog-check catalog-install test-nls test-nls-release test-nls-directory test-nls-directory-release test-catalog-coverage
.PHONY: test-stdin-sources test-resource-file-size test-stdout-file-size test-process-restarts test-restart-environment test-restart-pipe test-resource-fd test-resource-fd-recipes test-signal-mask test-resource-fd-spool test-resource-fd-pipe test-jobserver-release test-resource-fd-fifo
.PHONY: test-resource-fd-parallel

.PHONY: test-resource-fd-fifo-failure test-resource-fd-fifo-failure-release
.PHONY: test-shell-capture-lifetime test-shell-capture-lifetime-release
.PHONY: test-trace-nls test-trace-nls-release
.PHONY: test-trace-job-languages test-trace-job-languages-release
.PHONY: test-job-debug test-job-debug-nls test-job-debug-nls-release test-job-debug-load test-job-debug-load-release
.PHONY: test-debug-traversal test-debug-traversal-nls test-debug-verbose

.PHONY: test-debug-output test-directory-reporting test-native-modules help doctor bootstrap bootstrap-rust bootstrap-oracle build release fmt fmt-check lint test-unit test-tools check-core oracle-verify test-jobserver test-load test-runtime-signals test-output-sync test-startup-exits test-runtime-effects test-graph-progress test-builtins regenerate-builtins test-oracle test-diff test-safety test-generated test-graphs test-scopes test-pattern-scopes test-secondary test-filesystem test-include-stat test-source-streams test-imported-trace test-symlink-times test-archive-times test-directory-search test-whitespace test-word-index test-wildcards test-modes test-intcmp test-diagnostics test-stateful test-selfhost test-m4-release test-m5-release m4-acceptance m5-acceptance test check ci bench bench-compare profile-allocations profile-allocations-frozen clean

PYTHON ?= python3
PREFIX ?= /usr/local
DESTDIR ?=
CATALOG_INSTALL_DIR ?= $(PREFIX)/share/rmake/locale
INTEGRATION_CASE ?=
STDIN_CASE ?=
RESOURCE_FILE_SIZE_CASE ?=
STDOUT_FILE_SIZE_CASE ?=
PROCESS_RESTART_CASE ?=
RESTART_ENVIRONMENT_CASE ?=
RESOURCE_FD_CASE ?=
RESOURCE_NPROC_CASE ?=
RESOURCE_NPROC_REPORT ?= artifacts/resource-nproc/development.json
RESOURCE_FD_RECIPE_CASE ?=
SIGNAL_MASK_CASE ?=
RESOURCE_FD_SPOOL_CASE ?=
RESOURCE_FD_PIPE_CASE ?=
RESOURCE_FD_FIFO_CASE ?=
RESOURCE_FD_PARALLEL_CASE ?=
RESOURCE_FD_PARALLEL_REPORT ?= artifacts/resource-fd-parallel/development.json
JOBSERVER_RELEASE_CASE ?=
DATABASE_NUMERIC_CASE ?=
DATABASE_ORDER_ROLE_CASE ?=
DATABASE_DRY_RUN_CASE ?=
DATABASE_BRANCH_CASE ?=
DATABASE_DOUBLE_DRY_RUN_CASE ?=
DATABASE_GROUPED_CASE ?=
DATABASE_IMPLICIT_GROUPED_CASE ?=
DATABASE_SEARCH_CASE ?=
DATABASE_CASE ?=
DATABASE_LIFECYCLE_GROUP ?= all
DATABASE_LIFECYCLE_CASE ?=
COLLATION_CASE ?=
LOCALE_DATA_ROOT ?= $(CURDIR)/.tools/locale-data
GUILE_TOOLCHAIN_ROOT ?= $(CURDIR)/.tools/guile
GUILE_FRONTEND_OUTPUT ?= $(CURDIR)/artifacts/guile-frontend
NATIVE_FENV_CASE ?=
LOCALE_CASE ?=
NLS_CASE ?=
NLS_DIRECTORY_CASE ?=
CATALOG_COVERAGE_CASE ?=
ORACLE ?= $(CURDIR)/.tools/oracle/bin/make
CANDIDATE ?= $(CURDIR)/target/debug/rmake
M4_RELEASE ?= $(CURDIR)/target/release/rmake
BENCH_LOCALE ?= C
BENCH_LOCPATH ?=
BENCH_SILENT ?=
BENCH_CANDIDATE ?= $(CURDIR)/target/release/rmake
PROFILE_WORKLOAD ?= large-noop
PROFILE_CANDIDATE ?= $(CURDIR)/artifacts/profile/target/release/rmake
PROFILE_OUTPUT ?= $(CURDIR)/artifacts/profile/allocations
BENCH_SUITE ?= core
BENCH_REPEAT ?= 15
BENCH_VALIDATE ?= 0
BENCH_BASELINE ?=
SEED ?= 44101
GENERATED_CASES ?= 200
EXPRESSIONS ?= 32
START_INDEX ?= 0
GRAPH_SEED ?= 44109
GRAPH_CASES ?= 100
GRAPH_NODES ?= 8
PROGRESS_REPEAT ?= 40
NATIVE_CASE ?=
DIRECTORY_REPORT_CASE ?=
DEBUG_CASE ?=
DEBUG_TRAVERSAL_CASE ?=
DEBUG_VERBOSE_CASE ?=
JOB_DEBUG_CASE ?=
SIGNAL_CASE ?=
SIGNAL_REPEAT ?= 3
SIGNAL_UNSTAGED ?=
MODE_CASE ?=
DIAGNOSTIC_CASE ?=
INCLUDE_STAT_CASE ?=
SOURCE_STREAM_CASE ?=
IMPORTED_TRACE_CASE ?=
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
		'  bootstrap-integration Verify pinned external project source archives' \
		'  bootstrap-guile  Verify the optional Ubuntu Guile tools and separate oracle' \
		'  doctor          Check the toolchain and oracle' \
		'  build / release Build debug / optimized rmake' \
		'  catalog-check / catalog-install Verify and stage owned message data' \
		'  test-catalog-coverage Compare all GNU catalog languages and fallback' \
		'  test-nls / test-nls-directory Compare localized diagnostics and directory output' \
		'  fmt / fmt-check Format Rust / check formatting' \
		'  lint            Run Clippy with warnings denied' \
		'  test-unit       Run Rust unit and integration tests' \
		'  test-tools      Test the independent tooling' \
		'  test-oracle     Validate fixtures against two oracle executions' \
		'  test-builtins   Verify embedded Linux defaults against the pinned oracle' \
		'  regenerate-builtins Refresh declarative Linux defaults from the oracle' \
		'  test-diff       Compare rmake against the oracle; report pending cases' \
		'  test-shuffle    Validate random/zero-seed permutations and numeric replay' \
		'  test-nested-short-proposal Review the unapproved short-option error boundary' \
		'  test-safety     Check documented safe errors without invoking GNU' \
		'  test-include-stat Check include-directory EINTR and cancellation behavior' \
		'  test-source-streams Check live source read, close and interruption ordering' \
		'  test-imported-trace Check imported restart-directory output lifetime' \
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
		'  test-native-modules Compare original C modules and native ABI lifecycles' \
		'  test-integration Compare real-project builds, rebuilds and executable smokes' \
		'  test-database-output Compare complete databases, histories and phase effects' \
		'  test-database-nls / test-database-nls-release Compare exact localized databases' \
		'  test-database-lifecycle Compare restart environments and exact cleanup phases' \
		'  test-database-search Compare full VPATH/GPATH database and filename histories' \
		'  test-database-numeric Compare native rounding and registration history' \
		'  test-database-order-roles Compare prepared normal and order-only duplicates' \
		'  test-database-dry-run-time Compare synthetic and physical timestamps' \
		'  test-database-branch-metadata Compare per-branch double-colon records' \
		'  test-database-double-dry-run Compare double-colon recipe timestamps' \
		'  test-database-grouped / test-database-implicit-grouped Compare grouped file records' \
		'  test-guile-components Check the opt-in leading-C/GC architecture component' \
		'  test-guile-frontend Compare the enabled application with the verified Guile reference' \
		'  test-native-fenv Compare native thread rounding, flags and nested callbacks' \
		'  test-stdin-sources Compare named stdin lifetime, restarts and cleanup' \
		'  test-resource-file-size Compare stdin copy and signals under a file-size limit' \
		'  test-stdout-file-size Compare stdout writes and finalization under a file-size limit' \
		'  test-process-restarts Compare new process entry and executable replacement' \
		'  test-restart-environment Compare the complete live process environment vector' \
		'  test-restart-pipe Check original inherited jobserver descriptors across exec' \
		'  test-resource-fd Compare descriptor pressure and live cancellation' \
		'  test-resource-nproc Compare launches on an unprivileged Linux process limit' \
		'  test-resource-fd-recipes Compare low-limit recipe launch and reaping' \
		'  test-signal-mask Compare native, child and restart mask phases' \
		'  test-resource-fd-spool Compare borrowed recipe output at low limits' \
		'  test-resource-fd-pipe Compare compact pipe pools and child wire identity' \
		'  test-resource-fd-fifo Compare constrained FIFO pools and borrowed descriptor identity' \
		'  test-resource-fd-parallel Compare many live recipes under a small descriptor limit' \
		'  test-jobserver-release Compare ordinary token return and abort drain' \
		'  test-locales    Compare glibc C/C.UTF8 locale, wildcard and recursion phases' \
		'  bootstrap-locale-data Verify and generate pinned private French locale data' \
		'  test-locale-collation Compare exact locale and live native glob ordering' \
		'  test-directory-reporting Compare directory policy, output groups and restarts' \
		'  test-debug-output Compare debug policy, phases and deterministic traces' \
		'  test-debug-traversal Compare implicit search and intermediate cleanup traces' \
		'  test-debug-traversal-nls Compare debug events in the verified French locale' \
		'  test-debug-verbose Compare timestamp and remake decisions in C and French' \
		'  test-job-debug Compare live child identities, JOBS events and token return' \
		'  test-job-debug-nls Compare C and French JOBS events' \
		'  test-trace-job-languages Compare German, Japanese and Brazilian Portuguese trace and JOBS events' \
		'  test-job-debug-load Compare controller phase and controlled load events' \
		'  m5-acceptance  Run combined development and optimized runtime gates' \
		'  m4-acceptance  Run development and optimized rule-resolution gates' \
		'  test-m4-release Run optimized corpus, stateful, archive-fault and self-host gates' \
		'  test-selfhost   Run build and core checks through the serial candidate' \
		'  test-developer-flows Compare warm bootstrap, clean and benchmark checks' \
		'  test-flag-terminator Compare inherited option source boundaries' \
		'  test-startup-usage Compare malformed argument effects and usage output' \
		'  test-runtime-option-values Compare live notices and deferred pool style' \
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
		'Directory reporting replay: DIRECTORY_REPORT_CASE=<case ID>.' \
		'Debug replay: DEBUG_CASE=<case ID>.' \
		'Implicit search replay: DEBUG_TRAVERSAL_CASE=<case ID>.' \
		'Integration replay: INTEGRATION_CASE=xxhash-0.8.3|zlib-1.3.2|lua-5.4.9|gzip-1.14|patch-2.8.' \
		'Database replay: DATABASE_CASE=<case ID>; DATABASE_SEARCH_CASE=<search case ID>.' \
		'Locale replay: LOCALE_CASE=<case ID>.' \
		'JOBS replay: JOB_DEBUG_CASE=<case ID>.' \
		'Whitespace replay: WHITESPACE_CASE=<case ID>.' \
		'Word index replay: WORD_INDEX_CASE=<case ID>.' \
		'Wildcard overrides: WILDCARD_SEED, WILDCARD_CASES, WILDCARD_EXPRESSIONS, START_INDEX.' \
		'Benchmark suite: BENCH_SUITE=core|startup-arguments|include-operands|literal-chains|text-frames|linear-leaves|function-bodies|deep-appends|nested-evals|runtime-includes|runtime-arguments|builtin-transitions|word-spans|wildcard|bindings|implicit-search|parallel-branches|target-contexts|archives|archive-sequences|vpath-search|secondary|implicit-rejections|gpath|intermediates|lazy-records|vpath-records|directory-search|archive-names|symlink-times|rule-filenames|rule-tilde|libraries|all.' \
		'Benchmark samples per engine: BENCH_REPEAT=15 (3-1000).' \
		'Integer overrides: INTCMP_SEED, INTCMP_CASES, INTCMP_EXPRESSIONS, START_INDEX.' \
		'GNU compatibility is incomplete.'

bootstrap: bootstrap-rust bootstrap-oracle bootstrap-integration

bootstrap-integration:
	$(PYTHON) tools/integration.py --bootstrap-only

bootstrap-guile: bootstrap-oracle
	$(PYTHON) tools/guile_toolchain.py --root "$(GUILE_TOOLCHAIN_ROOT)" --reference

test-guile-components: bootstrap-rust bootstrap-guile
	$(PYTHON) tools/guile_toolchain.py --root "$(GUILE_TOOLCHAIN_ROOT)" --components

test-guile-frontend: bootstrap-rust bootstrap-guile
	+$(MAKE) --no-print-directory build release
	$(PYTHON) tools/guile_frontend_gate.py --profile "$(GUILE_TOOLCHAIN_ROOT)/reference-profile.json" --default-debug "$(CANDIDATE)" --default-release "$(M4_RELEASE)" --output "$(GUILE_FRONTEND_OUTPUT)"

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

catalog-check:
	$(PYTHON) tools/catalogs.py

catalog-install:
	$(PYTHON) tools/catalogs.py --install "$(DESTDIR)$(CATALOG_INSTALL_DIR)"

check-core: catalog-check fmt-check lint test-unit test-tools

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

test-shell-capture-lifetime: build oracle-verify
	$(PYTHON) tools/shell_capture_lifetime.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --report artifacts/shell-capture/development.json
	$(PYTHON) tools/shell_capture_launch_failure.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --report artifacts/shell-capture/failure-development.json
	$(PYTHON) tools/shell_capture_low_slots.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --report artifacts/shell-capture/low-slots-development.json
	$(PYTHON) tools/shell_capture_cached_spool.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --report artifacts/shell-capture/cached-spool-development.json

test-shell-capture-lifetime-release: release oracle-verify
	$(PYTHON) tools/shell_capture_lifetime.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --report artifacts/shell-capture/optimized.json
	$(PYTHON) tools/shell_capture_launch_failure.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --report artifacts/shell-capture/failure-optimized.json
	$(PYTHON) tools/shell_capture_low_slots.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --report artifacts/shell-capture/low-slots-optimized.json
	$(PYTHON) tools/shell_capture_cached_spool.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --report artifacts/shell-capture/cached-spool-optimized.json

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

.PHONY: test-shuffle
test-shuffle: build oracle-verify
	$(PYTHON) tools/shuffle_modes.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)"

.PHONY: test-nested-short-proposal
test-nested-short-proposal: build oracle-verify
	$(PYTHON) tools/nested_short_proposal.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)"

test-include-stat: build oracle-verify
	$(PYTHON) tools/include_stat_faults.py --oracle "$(ORACLE)" --self-test $(if $(INCLUDE_STAT_CASE),--case "$(INCLUDE_STAT_CASE)")
	$(PYTHON) tools/include_stat_faults.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" $(if $(INCLUDE_STAT_CASE),--case "$(INCLUDE_STAT_CASE)")

test-runtime-option-values: build oracle-verify
	$(PYTHON) tools/runtime_option_values.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --report artifacts/runtime-option-values/development.json $(if $(RUNTIME_OPTION_VALUE_CASE),--case "$(RUNTIME_OPTION_VALUE_CASE)")

test-startup-usage: build oracle-verify
	$(PYTHON) tools/startup_usage.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --locale-root "$(LOCALE_DATA_ROOT)" --report artifacts/startup-usage/development.json $(if $(STARTUP_USAGE_CASE),--case "$(STARTUP_USAGE_CASE)")
ifeq ($(STARTUP_USAGE_CASE),)
	$(PYTHON) tools/getopt_locale.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --locale-root "$(LOCALE_DATA_ROOT)" --report artifacts/startup-usage/getopt-development.json
endif

test-flag-terminator: build oracle-verify
	$(PYTHON) tools/flag_terminator.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --report artifacts/flag-terminator/development.json $(if $(FLAG_TERMINATOR_CASE),--case "$(FLAG_TERMINATOR_CASE)")

test-developer-flows: build oracle-verify
	$(PYTHON) tools/developer_flows.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --report artifacts/developer-flows/development.json $(if $(DEVELOPER_FLOW_CASE),--case "$(DEVELOPER_FLOW_CASE)")

test-selfhost: build
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" "LOCALE_DATA_ROOT=$(LOCALE_DATA_ROOT)" CATALOG_COVERAGE_CASE=partial-language-fallback test-catalog-coverage
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" RUNTIME_OPTION_VALUE_CASE=ordered-immediate test-runtime-option-values
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" "LOCALE_DATA_ROOT=$(LOCALE_DATA_ROOT)" STARTUP_USAGE_CASE=empty-directory-then-assignment test-startup-usage
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" FLAG_TERMINATOR_CASE=inherited-short-goal test-flag-terminator
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" DEVELOPER_FLOW_CASE=clean-parallel-pipe test-developer-flows
	+"$(CANDIDATE)" -rR --no-print-directory build check-core
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" "LOCALE_DATA_ROOT=$(LOCALE_DATA_ROOT)" NLS_CASE=nls-private-messages-warning test-nls
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" "LOCALE_DATA_ROOT=$(LOCALE_DATA_ROOT)" NLS_DIRECTORY_CASE=nls-directory-fr-inherited-depth test-nls-directory
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" INTEGRATION_CASE=xxhash-0.8.3 test-integration
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" INTEGRATION_CASE=lua-5.4.9 test-integration
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" DATABASE_CASE=database-minimal test-database-output
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" DATABASE_LIFECYCLE_CASE=database-native-restart-environment-vector-order test-database-lifecycle
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" DATABASE_SEARCH_CASE=database-search-repeated-aliased test-database-search
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" DATABASE_NUMERIC_CASE=database-native-numeric-minimal-upward test-database-numeric
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" DATABASE_ORDER_ROLE_CASE=late-recipe-eval test-database-order-roles
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" DATABASE_DRY_RUN_CASE=missing-dry-run test-database-dry-run-time
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" DATABASE_BRANCH_CASE=inactive-head-local test-database-branch-metadata
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" DATABASE_GROUPED_CASE=group-peer-exists test-database-grouped
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" NATIVE_FENV_CASE=native-fenv-successive-upward test-native-fenv
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" LOCALE_CASE=all-utf8-one-flags test-locales
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" COLLATION_CASE=live-thread-fr-fr-utf-8 test-locale-collation
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" STDIN_CASE=stdin-owned-goal-unlinked test-stdin-sources
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" RESOURCE_FILE_SIZE_CASE=stdin-4096-limit-128 test-resource-file-size
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" STDOUT_FILE_SIZE_CASE=native-host-stdout-ferror test-stdout-file-size
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" PROCESS_RESTART_CASE=process-restart-replaced-image test-process-restarts
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" RESTART_ENVIRONMENT_CASE=raw-ordered-vector test-restart-environment
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" test-restart-pipe
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" RESOURCE_FD_CASE=low6-term-live-recipe test-resource-fd
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" RESOURCE_FD_RECIPE_CASE=recipe-fdlimit-4 test-resource-fd-recipes
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" SIGNAL_MASK_CASE=eager-to-read-signal-mask test-signal-mask
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" RESOURCE_FD_SPOOL_CASE=spool-fd6-separate-identity test-resource-fd-spool
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" RESOURCE_FD_PIPE_CASE=borrowed-wire-fd6 test-resource-fd-pipe
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" RESOURCE_FD_FIFO_CASE=owned-fifo-inherited-user-descriptor-7 test-resource-fd-fifo
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" RESOURCE_FD_PARALLEL_CASE=pipe-8-workers-10 RESOURCE_FD_PARALLEL_REPORT=artifacts/resource-fd-parallel/selfhost.json test-resource-fd-parallel
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" JOBSERVER_RELEASE_CASE=implicit-child-finishes-first test-jobserver-release
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" test-safety
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" NATIVE_CASE=native-arguments test-native-modules
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" DIRECTORY_REPORT_CASE=directory-explicit-w test-directory-reporting
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" DEBUG_CASE=debug-bare-basic test-debug-output
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" DEBUG_CASE=debug-child-queued-parallel test-debug-output
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" DEBUG_TRAVERSAL_CASE=chain-secondary test-debug-traversal
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" "LOCALE_DATA_ROOT=$(LOCALE_DATA_ROOT)" DEBUG_TRAVERSAL_CASE=generated-include test-debug-traversal-nls
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" "LOCALE_DATA_ROOT=$(LOCALE_DATA_ROOT)" DEBUG_VERBOSE_CASE=vpath-no-update test-debug-verbose
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" JOB_DEBUG_CASE=jobs-serial-multiline test-job-debug
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" MODE_CASE=mode-ordinary-first-force-qt test-modes
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" DIAGNOSTIC_CASE=message-204-child-output-order test-diagnostics
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" FILESYSTEM_CASE=close-eio-write test-filesystem
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" INCLUDE_STAT_CASE=include-stat-reader-eintr-1 test-include-stat
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" SOURCE_STREAM_CASE=source-deep-read-eintr test-source-streams
	+"$(CANDIDATE)" -rR --no-print-directory "CANDIDATE=$(CANDIDATE)" "ORACLE=$(ORACLE)" IMPORTED_TRACE_CASE=traced-restart-traced-remake-l2-target-i1 test-imported-trace
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

test-m4-release: release oracle-verify test-nls-release test-nls-directory-release test-database-nls-release test-job-debug-nls-release test-job-debug-load-release
	$(PYTHON) tools/catalog_coverage.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --locale-root "$(LOCALE_DATA_ROOT)" --report artifacts/catalog-coverage/optimized.json
	$(PYTHON) tools/runtime_option_values.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --report artifacts/runtime-option-values/optimized.json
	$(PYTHON) tools/startup_usage.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --locale-root "$(LOCALE_DATA_ROOT)" --report artifacts/startup-usage/optimized.json
	$(PYTHON) tools/getopt_locale.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --locale-root "$(LOCALE_DATA_ROOT)" --report artifacts/startup-usage/getopt-optimized.json
	$(PYTHON) tools/flag_terminator.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --report artifacts/flag-terminator/optimized.json
	$(PYTHON) tools/developer_flows.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --report artifacts/developer-flows/optimized.json
	$(PYTHON) tools/safety.py --candidate "$(M4_RELEASE)"
	$(PYTHON) tools/stdin_sources.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --report artifacts/stdin-sources/optimized.json
	$(PYTHON) tools/resource_file_size.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --report artifacts/resource-file-size/optimized.json
	$(PYTHON) tools/stdout_file_size.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --locale-root "$(LOCALE_DATA_ROOT)" --report artifacts/stdout-file-size/optimized.json
	$(PYTHON) tools/process_restarts.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --report artifacts/process-restarts/optimized.json
	$(PYTHON) tools/restart_environment.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --report artifacts/restart-environment/optimized.json
	$(PYTHON) tools/restart_pipe.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --report artifacts/restart-pipe/optimized.json
	$(PYTHON) tools/resource_fd.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --report artifacts/resource-fd/optimized.json
	$(PYTHON) tools/resource_fd_recipes.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --report artifacts/resource-fd-recipes/optimized.json
	$(PYTHON) tools/signal_mask.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --report artifacts/signal-mask/optimized.json
	$(PYTHON) tools/resource_fd_spool.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --report artifacts/resource-fd-spool/optimized.json
	$(PYTHON) tools/resource_fd_pipe.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --report artifacts/resource-fd-pipe/optimized.json
	$(PYTHON) tools/resource_fd_fifo.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --report artifacts/resource-fd-fifo/optimized.json
	$(PYTHON) tools/resource_fd_parallel.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --report artifacts/resource-fd-parallel/optimized.json
	$(PYTHON) tools/jobserver_release.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --report-dir artifacts/jobserver-release/optimized
	$(PYTHON) tools/integration.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --report artifacts/integration/optimized.json
	$(PYTHON) tools/database_output.py --suite all --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --report artifacts/database-output/optimized.json
	$(PYTHON) tools/database_lifecycle.py --group "$(DATABASE_LIFECYCLE_GROUP)" --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --report artifacts/database-lifecycle/optimized.json
	$(PYTHON) tools/database_numeric.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --report artifacts/database-output/numeric-optimized.json
	$(PYTHON) tools/database_order_roles.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --report artifacts/database-order-roles/optimized.json
	$(PYTHON) tools/database_dry_run_time.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --report artifacts/database-dry-run/optimized.json
	$(PYTHON) tools/database_branch_metadata.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --report artifacts/database-branch-metadata/optimized.json
	$(PYTHON) tools/database_double_dry_run_time.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --report artifacts/database-double-dry-run/optimized.json
	$(PYTHON) tools/database_grouped_output.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --report artifacts/database-grouped/optimized.json
	$(PYTHON) tools/database_implicit_grouped_output.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --report artifacts/database-implicit-grouped/optimized.json
	$(PYTHON) tools/database_grouped_double_output.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --report artifacts/database-grouped-double/optimized.json
	$(PYTHON) tools/database_grouped_failure.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --report artifacts/database-grouped-failure/optimized.json
	$(PYTHON) tools/native_fenv.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --report artifacts/native-fenv/optimized.json
	$(PYTHON) tools/locale_profiles.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --report artifacts/locales/optimized.json
	$(PYTHON) tools/locale_collation.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --locale-root "$(LOCALE_DATA_ROOT)" --report artifacts/locale-collation/optimized.json
	$(PYTHON) tools/native_modules.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --report artifacts/native/optimized.json
	$(PYTHON) tools/directory_reporting.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --report artifacts/directory-reporting/optimized.json
	$(PYTHON) tools/debug_output.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --report artifacts/debug-output/optimized.json
	$(PYTHON) tools/debug_traversal.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --report artifacts/debug-traversal/optimized.json
	$(PYTHON) tools/debug_traversal.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --locale-root "$(LOCALE_DATA_ROOT)" --report artifacts/debug-traversal/french-optimized.json
	$(PYTHON) tools/debug_verbose.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --locale-root "$(LOCALE_DATA_ROOT)" --report-dir artifacts/debug-verbose/optimized
	$(PYTHON) tools/job_debug.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --report artifacts/job-debug/optimized.json
	$(PYTHON) tools/conformance.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)"
	$(PYTHON) tools/nested_short_proposal.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --report artifacts/nested-short-proposal/release.json
	$(PYTHON) tools/shuffle_modes.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --report artifacts/shuffle-modes/release.json
	$(PYTHON) tools/stateful.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)"
	$(PYTHON) tools/load_limits.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)"
	$(PYTHON) tools/cancellation_effects.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)"
	$(PYTHON) tools/startup_exits.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)"
	$(PYTHON) tools/source_streams.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)"
	$(PYTHON) tools/imported_trace.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)"
	$(PYTHON) tools/include_stat_faults.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)"
	$(PYTHON) tools/output_lock.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)"
	$(PYTHON) tools/runtime_terminals.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)"
	$(PYTHON) tools/output_sync.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)"
	$(PYTHON) tools/runtime_signals.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)"
	$(PYTHON) tools/archive_times.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)"
	+$(MAKE) --no-print-directory "CANDIDATE=$(M4_RELEASE)" test-selfhost test-graph-progress


test-integration: build oracle-verify
	$(PYTHON) tools/integration.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --report artifacts/integration/development.json $(if $(INTEGRATION_CASE),--case "$(INTEGRATION_CASE)")

test-database-numeric: build oracle-verify
	$(PYTHON) tools/database_numeric.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --report artifacts/database-output/numeric-development.json $(if $(DATABASE_NUMERIC_CASE),--case "$(DATABASE_NUMERIC_CASE)")

test-database-search: build oracle-verify
	$(PYTHON) tools/database_output.py --suite search --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --report artifacts/database-output/search-development.json $(if $(DATABASE_SEARCH_CASE),--case "$(DATABASE_SEARCH_CASE)")

test-database-grouped-double: build oracle-verify
	$(PYTHON) tools/database_grouped_double_output.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --report artifacts/database-grouped-double/development.json
	$(PYTHON) tools/database_grouped_failure.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --report artifacts/database-grouped-failure/development.json

test-database-grouped: build oracle-verify
	$(PYTHON) tools/database_grouped_output.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --report artifacts/database-grouped/development.json $(if $(DATABASE_GROUPED_CASE),--case "$(DATABASE_GROUPED_CASE)")

test-database-implicit-grouped: build oracle-verify
	$(PYTHON) tools/database_implicit_grouped_output.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --report artifacts/database-implicit-grouped/development.json $(if $(DATABASE_IMPLICIT_GROUPED_CASE),--case "$(DATABASE_IMPLICIT_GROUPED_CASE)")

test-database-branch-metadata: build oracle-verify
	$(PYTHON) tools/database_branch_metadata.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --report artifacts/database-branch-metadata/development.json $(if $(DATABASE_BRANCH_CASE),--case "$(DATABASE_BRANCH_CASE)")

test-database-double-dry-run: build oracle-verify
	$(PYTHON) tools/database_double_dry_run_time.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --report artifacts/database-double-dry-run/development.json $(if $(DATABASE_DOUBLE_DRY_RUN_CASE),--case "$(DATABASE_DOUBLE_DRY_RUN_CASE)")

test-database-dry-run-time: build oracle-verify
	$(PYTHON) tools/database_dry_run_time.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --report artifacts/database-dry-run/development.json $(if $(DATABASE_DRY_RUN_CASE),--case "$(DATABASE_DRY_RUN_CASE)")

test-database-order-roles: build oracle-verify
	$(PYTHON) tools/database_order_roles.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --report artifacts/database-order-roles/development.json $(if $(DATABASE_ORDER_ROLE_CASE),--case "$(DATABASE_ORDER_ROLE_CASE)")

test-database-lifecycle: build oracle-verify
	$(PYTHON) tools/database_lifecycle.py --group "$(DATABASE_LIFECYCLE_GROUP)" --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --report artifacts/database-lifecycle/development.json $(if $(DATABASE_LIFECYCLE_CASE),--case "$(DATABASE_LIFECYCLE_CASE)")

test-database-output: build oracle-verify
	$(PYTHON) tools/database_output.py --suite all --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --report artifacts/database-output/development.json $(if $(DATABASE_CASE),--case "$(DATABASE_CASE)")
bootstrap-locale-data:
	$(PYTHON) tools/locale_data.py --root "$(LOCALE_DATA_ROOT)"

test-catalog-coverage: build oracle-verify catalog-check
	$(PYTHON) tools/catalog_coverage.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --locale-root "$(LOCALE_DATA_ROOT)" --report artifacts/catalog-coverage/development.json $(if $(CATALOG_COVERAGE_CASE),--case "$(CATALOG_COVERAGE_CASE)")

test-database-nls: build oracle-verify catalog-check
	$(PYTHON) tools/database_nls.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --locale-root "$(LOCALE_DATA_ROOT)" --report artifacts/database-output/nls-development.json

test-database-nls-release: release oracle-verify catalog-check
	$(PYTHON) tools/database_nls.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --locale-root "$(LOCALE_DATA_ROOT)" --report artifacts/database-output/nls-optimized.json

test-nls: build oracle-verify catalog-check
	$(PYTHON) tools/nls_diagnostics.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --locale-root "$(LOCALE_DATA_ROOT)" --report artifacts/nls/development.json $(if $(NLS_CASE),--case "$(NLS_CASE)")

test-nls-release: release oracle-verify catalog-check
	$(PYTHON) tools/nls_diagnostics.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --locale-root "$(LOCALE_DATA_ROOT)" --report artifacts/nls/optimized.json $(if $(NLS_CASE),--case "$(NLS_CASE)")

test-nls-directory: build oracle-verify catalog-check
	$(PYTHON) tools/nls_directory.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --locale-root "$(LOCALE_DATA_ROOT)" --report artifacts/nls/directory-development.json $(if $(NLS_DIRECTORY_CASE),--case "$(NLS_DIRECTORY_CASE)")

test-nls-directory-release: release oracle-verify catalog-check
	$(PYTHON) tools/nls_directory.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --locale-root "$(LOCALE_DATA_ROOT)" --report artifacts/nls/directory-optimized.json $(if $(NLS_DIRECTORY_CASE),--case "$(NLS_DIRECTORY_CASE)")

test-locale-collation: build oracle-verify
	$(PYTHON) tools/locale_collation.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --locale-root "$(LOCALE_DATA_ROOT)" --report artifacts/locale-collation/development.json $(if $(COLLATION_CASE),--case "$(COLLATION_CASE)")

test-stdin-sources: build oracle-verify
	$(PYTHON) tools/stdin_sources.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --report artifacts/stdin-sources/development.json $(if $(STDIN_CASE),--case "$(STDIN_CASE)")

test-resource-file-size: build oracle-verify
	$(PYTHON) tools/resource_file_size.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --report artifacts/resource-file-size/development.json $(if $(RESOURCE_FILE_SIZE_CASE),--case "$(RESOURCE_FILE_SIZE_CASE)")

test-stdout-file-size: build oracle-verify
	$(PYTHON) tools/stdout_file_size.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --locale-root "$(LOCALE_DATA_ROOT)" --report artifacts/stdout-file-size/development.json $(if $(STDOUT_FILE_SIZE_CASE),--case "$(STDOUT_FILE_SIZE_CASE)")

test-process-restarts: build oracle-verify
	$(PYTHON) tools/process_restarts.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --report artifacts/process-restarts/development.json $(if $(PROCESS_RESTART_CASE),--case "$(PROCESS_RESTART_CASE)")

test-restart-environment: build oracle-verify
	$(PYTHON) tools/restart_environment.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --report artifacts/restart-environment/development.json $(if $(RESTART_ENVIRONMENT_CASE),--case "$(RESTART_ENVIRONMENT_CASE)")

test-restart-pipe: build oracle-verify
	$(PYTHON) tools/restart_pipe.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --report artifacts/restart-pipe/development.json

test-resource-fd: build oracle-verify
	$(PYTHON) tools/resource_fd.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --report artifacts/resource-fd/development.json $(if $(RESOURCE_FD_CASE),--case "$(RESOURCE_FD_CASE)")

test-jobserver-release: build oracle-verify
	$(PYTHON) tools/jobserver_release.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --report-dir artifacts/jobserver-release/development $(if $(JOBSERVER_RELEASE_CASE),--case "$(JOBSERVER_RELEASE_CASE)")

test-resource-fd-pipe: build oracle-verify
	$(PYTHON) tools/resource_fd_pipe.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --report artifacts/resource-fd-pipe/development.json $(if $(RESOURCE_FD_PIPE_CASE),--case "$(RESOURCE_FD_PIPE_CASE)")

test-resource-fd-fifo-failure: build oracle-verify
	$(PYTHON) tools/resource_fd_fifo_failure.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --locale-root "$(LOCALE_DATA_ROOT)" --report artifacts/resource-fd-fifo/failure-development.json

test-resource-fd-fifo-failure-release: release oracle-verify
	$(PYTHON) tools/resource_fd_fifo_failure.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --locale-root "$(LOCALE_DATA_ROOT)" --report artifacts/resource-fd-fifo/failure-optimized.json

test-resource-fd-fifo: build oracle-verify
	$(PYTHON) tools/resource_fd_fifo.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --report artifacts/resource-fd-fifo/development.json $(if $(RESOURCE_FD_FIFO_CASE),--case "$(RESOURCE_FD_FIFO_CASE)")

test-resource-fd-parallel: build oracle-verify
	$(PYTHON) tools/resource_fd_parallel.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --report "$(RESOURCE_FD_PARALLEL_REPORT)" $(if $(RESOURCE_FD_PARALLEL_CASE),--case "$(RESOURCE_FD_PARALLEL_CASE)")

test-resource-fd-spool: build oracle-verify
	$(PYTHON) tools/resource_fd_spool.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --report artifacts/resource-fd-spool/development.json $(if $(RESOURCE_FD_SPOOL_CASE),--case "$(RESOURCE_FD_SPOOL_CASE)")

test-resource-nproc: build oracle-verify
	$(PYTHON) tools/resource_nproc.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --report "$(RESOURCE_NPROC_REPORT)" $(if $(RESOURCE_NPROC_CASE),--case "$(RESOURCE_NPROC_CASE)")

test-resource-nproc-release: release oracle-verify
	$(PYTHON) tools/resource_nproc.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --report artifacts/resource-nproc/optimized.json $(if $(RESOURCE_NPROC_CASE),--case "$(RESOURCE_NPROC_CASE)")

test-resource-nproc-selfhost: release
	+"$(M4_RELEASE)" -rR --no-print-directory "CANDIDATE=$(M4_RELEASE)" "ORACLE=$(ORACLE)" RESOURCE_NPROC_CASE=path-last-non-missing-error RESOURCE_NPROC_REPORT=artifacts/resource-nproc/selfhost.json test-resource-nproc

test-signal-mask: build oracle-verify
	$(PYTHON) tools/signal_mask.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --report artifacts/signal-mask/development.json $(if $(SIGNAL_MASK_CASE),--case "$(SIGNAL_MASK_CASE)")

test-resource-fd-recipes: build oracle-verify
	$(PYTHON) tools/resource_fd_recipes.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --report artifacts/resource-fd-recipes/development.json $(if $(RESOURCE_FD_RECIPE_CASE),--case "$(RESOURCE_FD_RECIPE_CASE)")

test-native-fenv: build oracle-verify
	$(PYTHON) tools/native_fenv.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --report artifacts/native-fenv/development.json $(if $(NATIVE_FENV_CASE),--case "$(NATIVE_FENV_CASE)")

test-locales: build oracle-verify
	$(PYTHON) tools/locale_profiles.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --report artifacts/locales/development.json $(if $(LOCALE_CASE),--case "$(LOCALE_CASE)")

test-native-modules: build oracle-verify
	$(PYTHON) tools/native_modules.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --report artifacts/native/development.json $(if $(NATIVE_CASE),--case "$(NATIVE_CASE)")

test-directory-reporting: build oracle-verify
	$(PYTHON) tools/directory_reporting.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --report artifacts/directory-reporting/development.json $(if $(DIRECTORY_REPORT_CASE),--case "$(DIRECTORY_REPORT_CASE)")

test-debug-output: build oracle-verify
	$(PYTHON) tools/debug_output.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --report artifacts/debug-output/development.json $(if $(DEBUG_CASE),--case "$(DEBUG_CASE)")

test-debug-traversal: build oracle-verify
	$(PYTHON) tools/debug_traversal.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --report artifacts/debug-traversal/development.json $(if $(DEBUG_TRAVERSAL_CASE),--case "$(DEBUG_TRAVERSAL_CASE)")

test-debug-traversal-nls: build oracle-verify
	$(PYTHON) tools/debug_traversal.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --locale-root "$(LOCALE_DATA_ROOT)" --report artifacts/debug-traversal/french-development.json $(if $(DEBUG_TRAVERSAL_CASE),--case "$(DEBUG_TRAVERSAL_CASE)")

test-debug-verbose: build oracle-verify
	$(PYTHON) tools/debug_verbose.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --locale-root "$(LOCALE_DATA_ROOT)" --report-dir artifacts/debug-verbose/development $(if $(DEBUG_VERBOSE_CASE),--case "$(DEBUG_VERBOSE_CASE)")

test-trace-nls: build oracle-verify catalog-check
	$(PYTHON) tools/trace_nls.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --locale-root "$(LOCALE_DATA_ROOT)" --report artifacts/trace/nls-development.json

test-trace-nls-release: release oracle-verify catalog-check
	$(PYTHON) tools/trace_nls.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --locale-root "$(LOCALE_DATA_ROOT)" --report artifacts/trace/nls-optimized.json

test-trace-job-languages: build oracle-verify catalog-check
	$(PYTHON) tools/trace_job_languages.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --locale-root "$(LOCALE_DATA_ROOT)" --report artifacts/trace-job-languages/development.json

test-trace-job-languages-release: release oracle-verify catalog-check
	$(PYTHON) tools/trace_job_languages.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --locale-root "$(LOCALE_DATA_ROOT)" --report artifacts/trace-job-languages/optimized.json

test-job-debug: build oracle-verify
	$(PYTHON) tools/job_debug.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --report artifacts/job-debug/development.json $(if $(JOB_DEBUG_CASE),--case "$(JOB_DEBUG_CASE)")

test-job-debug-nls: build oracle-verify catalog-check
	$(PYTHON) tools/job_debug.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --locale-root "$(LOCALE_DATA_ROOT)" --report artifacts/job-debug/nls-development.json $(if $(JOB_DEBUG_CASE),--case "$(JOB_DEBUG_CASE)")

test-job-debug-nls-release: release oracle-verify catalog-check
	$(PYTHON) tools/job_debug.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --locale-root "$(LOCALE_DATA_ROOT)" --report artifacts/job-debug/nls-optimized.json $(if $(JOB_DEBUG_CASE),--case "$(JOB_DEBUG_CASE)")

test-job-debug-load: build oracle-verify catalog-check
	$(PYTHON) tools/job_debug_load.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" --locale-root "$(LOCALE_DATA_ROOT)" --report artifacts/job-debug/load-development.json

test-job-debug-load-release: release oracle-verify catalog-check
	$(PYTHON) tools/job_debug_load.py --oracle "$(ORACLE)" --candidate "$(M4_RELEASE)" --locale-root "$(LOCALE_DATA_ROOT)" --report artifacts/job-debug/load-optimized.json

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

test-imported-trace: build oracle-verify
	$(PYTHON) tools/imported_trace.py --oracle "$(ORACLE)" --self-test $(if $(IMPORTED_TRACE_CASE),--case "$(IMPORTED_TRACE_CASE)")
	$(PYTHON) tools/imported_trace.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" $(if $(IMPORTED_TRACE_CASE),--case "$(IMPORTED_TRACE_CASE)")

test-source-streams: build oracle-verify
	$(PYTHON) tools/source_streams.py --oracle "$(ORACLE)" --self-test $(if $(SOURCE_STREAM_CASE),--case "$(SOURCE_STREAM_CASE)")
	$(PYTHON) tools/source_streams.py --oracle "$(ORACLE)" --candidate "$(CANDIDATE)" $(if $(SOURCE_STREAM_CASE),--case "$(SOURCE_STREAM_CASE)")

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

test: test-unit test-tools test-oracle test-builtins test-diff test-safety test-native-modules test-directory-reporting test-debug-output test-debug-traversal test-debug-traversal-nls test-debug-verbose test-job-debug test-job-debug-nls test-job-debug-load test-shuffle test-nested-short-proposal test-generated test-graphs test-scopes test-pattern-scopes test-secondary test-filesystem test-include-stat test-source-streams test-imported-trace test-symlink-times test-archive-times test-directory-search test-whitespace test-word-index test-wildcards test-modes test-intcmp test-diagnostics test-stateful test-jobserver test-load test-runtime-signals test-output-sync test-runtime-effects test-startup-exits test-graph-progress

check: check-core test-oracle test-builtins test-diff test-safety test-native-modules test-directory-reporting test-debug-output test-debug-traversal test-debug-traversal-nls test-debug-verbose test-job-debug test-job-debug-nls test-job-debug-load test-shuffle test-nested-short-proposal test-generated test-graphs test-scopes test-pattern-scopes test-secondary test-filesystem test-include-stat test-source-streams test-imported-trace test-symlink-times test-archive-times test-directory-search test-whitespace test-word-index test-wildcards test-modes test-intcmp test-diagnostics test-stateful test-jobserver test-load test-runtime-signals test-output-sync test-runtime-effects test-startup-exits test-graph-progress

test: test-integration test-database-output test-database-lifecycle test-database-numeric test-database-order-roles test-database-dry-run-time test-native-fenv test-locales test-locale-collation test-stdin-sources test-resource-file-size test-stdout-file-size test-process-restarts test-restart-environment test-restart-pipe test-resource-fd test-resource-fd-recipes test-signal-mask test-resource-fd-spool test-resource-fd-pipe test-resource-fd-fifo test-resource-fd-parallel test-jobserver-release test-nls test-nls-directory test-database-nls

check: test-integration test-database-output test-database-lifecycle test-database-numeric test-database-order-roles test-database-dry-run-time test-native-fenv test-locales test-locale-collation test-stdin-sources test-resource-file-size test-stdout-file-size test-process-restarts test-restart-environment test-restart-pipe test-resource-fd test-resource-fd-recipes test-signal-mask test-resource-fd-spool test-resource-fd-pipe test-resource-fd-fifo test-resource-fd-parallel test-jobserver-release test-nls test-nls-directory test-database-nls

test: test-trace-nls
check: test-trace-nls
test-m4-release: test-trace-nls-release

test check: test-trace-job-languages
test-m4-release: test-trace-job-languages-release

test: test-shell-capture-lifetime
check: test-shell-capture-lifetime
test-m4-release: test-shell-capture-lifetime-release

test check: test-resource-fd-fifo-failure
test-m4-release: test-resource-fd-fifo-failure-release

ci: check test-selfhost

bench: oracle-verify
	$(PYTHON) tools/benchmark.py --oracle "$(ORACLE)" --suite "$(BENCH_SUITE)" --repeat "$(BENCH_REPEAT)" --locale "$(BENCH_LOCALE)" $(if $(BENCH_LOCPATH),--locale-path "$(BENCH_LOCPATH)") $(if $(filter 1,$(BENCH_SILENT)),--silent)$(if $(filter 1,$(BENCH_VALIDATE)), --validate-only)

profile-allocations:
	+CARGO_TARGET_DIR="$(CURDIR)/artifacts/profile/target" RUSTFLAGS="-C debuginfo=1 -C force-frame-pointers=yes" $(CARGO) build --workspace --release --locked
	$(PYTHON) tools/allocations.py --candidate "$(CURDIR)/artifacts/profile/target/release/rmake" --workload "$(PROFILE_WORKLOAD)" --output "$(PROFILE_OUTPUT)"

profile-allocations-frozen:
	$(PYTHON) tools/allocations.py --candidate "$(PROFILE_CANDIDATE)" --workload "$(PROFILE_WORKLOAD)" --output "$(PROFILE_OUTPUT)"


bench-compare: release oracle-verify
	$(PYTHON) tools/benchmark.py --oracle "$(ORACLE)" --candidate "$(BENCH_CANDIDATE)" $(if $(BENCH_BASELINE),--baseline "$(BENCH_BASELINE)") --suite "$(BENCH_SUITE)" --repeat "$(BENCH_REPEAT)" --locale "$(BENCH_LOCALE)" $(if $(BENCH_LOCPATH),--locale-path "$(BENCH_LOCPATH)") $(if $(filter 1,$(BENCH_SILENT)),--silent)$(if $(filter 1,$(BENCH_VALIDATE)), --validate-only)

clean:
	+$(CARGO) clean

test check: test-catalog-coverage

test check: test-database-branch-metadata test-database-double-dry-run

test check: test-database-grouped test-database-implicit-grouped test-database-grouped-double

test check: test-developer-flows test-flag-terminator test-startup-usage test-runtime-option-values
