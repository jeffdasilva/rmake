.SECONDEXPANSION:
HEAD = $(shell /bin/sh -c 'exit 7')$(file >>seen,head)
NEXT = $(shell /bin/sh -c 'exit 11')$(file >>seen,next)
all: prepare split
all:;@:
.PHONY: prepare
prepare:
	@touch -d @946684800 input && touch -d @946684900 split
split:: input $$(HEAD)
	@$(error unexpected up-to-date recipe)
split::
	@$(info next=[$@|$(.SHELLSTATUS)])$(file >>seen,next-recipe):
