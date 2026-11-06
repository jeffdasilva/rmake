.SECONDEXPANSION:
HEAD = $(shell /bin/sh -c 'exit 7')$(file >>seen,head)
NEXT = $(shell /bin/sh -c 'exit 11')$(file >>seen,next)
THIRD = $(shell /bin/sh -c 'exit 13')$(file >>seen,third)
all: split
all:;@:
split:: link $$(HEAD)
	@:
split:: link $$(NEXT)
	@$(eval leaf: RESULT := $$(info retained=[$$@|$$(.SHELLSTATUS)])$$(file >>seen,retained)):
link: leaf
link:;@:
leaf:;@$(info first=[$@|$(.SHELLSTATUS)])$(file >>seen,leaf):
