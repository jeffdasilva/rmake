.SECONDEXPANSION:
BASE := $(shell /bin/sh -c 'exit 3')
all: first after
all:;@:
.PHONY: first
first: $$(shell /bin/sh -c 'exit 7')
after:;@$(info sibling=[$@|$(.SHELLSTATUS)])$(file >>seen,sibling):
