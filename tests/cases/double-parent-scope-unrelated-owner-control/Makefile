.SECONDEXPANSION:
HEAD = $(shell /bin/sh -c 'exit 7')$(file >>seen,head)
NEXT = $(shell /bin/sh -c 'exit 11')$(file >>seen,next)
BASE := $(shell /bin/sh -c 'exit 3')
all: split outside
all:;@:
split:: $$(HEAD)
	@:
split:: leaf $$(NEXT)
	@:
leaf:;@$(info leaf=[$@|$(.SHELLSTATUS)])$(file >>seen,leaf):
outside:;@$(info outside=[$@|$(.SHELLSTATUS)])$(file >>seen,outside):
