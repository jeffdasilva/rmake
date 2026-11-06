.SECONDEXPANSION:
HEAD = $(shell /bin/sh -c 'exit 7')$(file >>seen,head)
NEXT = $(shell /bin/sh -c 'exit 11')$(file >>seen,next)
all: split
all:;@:
vpath %.out shelf
GPATH := shelf
.PHONY: force
split:: $$(HEAD)
	@:
split:: leaf.out $$(NEXT)
	@:
leaf.out: force
leaf.out:;@$(info leaf=[$@|$(.SHELLSTATUS)])$(file >>seen,leaf):
