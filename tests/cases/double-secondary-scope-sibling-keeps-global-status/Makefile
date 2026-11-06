.SECONDEXPANSION:
HEAD = $(shell /bin/sh -c 'exit 7')$(file >>seen,head)
NEXT = $(shell /bin/sh -c 'exit 11')$(file >>seen,next)
BASE := $(shell /bin/sh -c 'exit 3')
all: split sibling
all:;@:
split:: $$(HEAD)
	@:
split:: $$(NEXT)
	@$(info next=[$@|$(.SHELLSTATUS)])$(file >>seen,next-recipe):
sibling:;@$(info sibling=[$@|$(.SHELLSTATUS)])$(file >>seen,sibling):
