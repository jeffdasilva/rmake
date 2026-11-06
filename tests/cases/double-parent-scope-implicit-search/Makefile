.SECONDEXPANSION:
HEAD = $(shell /bin/sh -c 'exit 7')$(file >>seen,head)
NEXT = $(shell /bin/sh -c 'exit 11')$(file >>seen,next)
all: split
all:;@:
split:: $$(HEAD)
	@:
split:: item.out $$(NEXT)
	@:
%.out: %.in $$(info search=[$$@|$$(.SHELLSTATUS)])$$(file >>seen,search)
	@$(info leaf=[$@|$(.SHELLSTATUS)])$(file >>seen,leaf):
