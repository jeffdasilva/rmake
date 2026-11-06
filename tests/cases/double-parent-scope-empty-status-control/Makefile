.SECONDEXPANSION:
HEAD = $(shell /bin/sh -c 'exit 7')$(file >>seen,head)
NEXT = $(shell /bin/sh -c 'exit 11')$(file >>seen,next)
BASE := $(shell /bin/sh -c 'exit 3')
all: split
all:;@:
split:: $$(shell )$$(file >>seen,head)
	@:
split:: leaf $$(shell )$$(file >>seen,next)
	@:
leaf:;@$(info leaf=[$@|$(.SHELLSTATUS)])$(file >>seen,leaf):
