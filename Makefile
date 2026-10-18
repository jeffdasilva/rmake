.DEFAULT_GOAL := help

.PHONY: help

help:
	@printf '%s\n' \
		'rmake - a planned Rust-based implementation of GNU Make' \
		'' \
		'Available targets:' \
		'  help  Show this help message (default)' \
		'' \
		'Scaffolding only; Rust build and test commands will follow project planning.'
