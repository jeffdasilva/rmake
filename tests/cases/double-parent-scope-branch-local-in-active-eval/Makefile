.SECONDEXPANSION:
HEAD = $(shell /bin/sh -c 'exit 7')$(file >>seen,head)
NEXT = $(shell /bin/sh -c 'exit 11')$(file >>seen,next)
all: split
all:;@:
split:: $$(HEAD)
	@:
split:: leaf $$(NEXT)
	@:
leaf:;@$(eval leaf: RESULT := $$(info active=[$$@|$$(.SHELLSTATUS)])$$(file >>seen,active)):
