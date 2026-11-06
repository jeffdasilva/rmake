.SECONDEXPANSION:
HEAD = $(shell /bin/sh -c 'exit 7')$(file >>seen,head)
NEXT = $(shell /bin/sh -c 'exit 11')$(file >>seen,next)
all: split
all:;@:
.INTERMEDIATE: leaf
.PRECIOUS: leaf
split:: $$(HEAD)
	@:
split:: leaf $$(NEXT)
	@:
leaf: input $$(info inspect=[$$@|$$(.SHELLSTATUS)])$$(file >>seen,inspect)
leaf:;@$(info leaf=[$@|$(.SHELLSTATUS)])$(file >>seen,leaf):
input:;@:
