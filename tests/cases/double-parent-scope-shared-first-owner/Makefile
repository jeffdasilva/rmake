.SECONDEXPANSION:
HEAD = $(shell /bin/sh -c 'exit 7')$(file >>seen,head)
NEXT = $(shell /bin/sh -c 'exit 11')$(file >>seen,next)
all: split
all:;@:
split:: shared $$(HEAD)
	@:
split:: shared leaf $$(NEXT)
	@:
shared:;@$(info shared=[$@|$(.SHELLSTATUS)])$(file >>seen,shared):
leaf:;@$(info leaf=[$@|$(.SHELLSTATUS)])$(file >>seen,leaf):
