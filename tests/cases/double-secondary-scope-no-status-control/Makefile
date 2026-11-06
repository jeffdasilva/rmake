.SECONDEXPANSION:
HEAD = $(shell /bin/sh -c 'exit 7')$(file >>seen,head)
NEXT = $(shell /bin/sh -c 'exit 11')$(file >>seen,next)
BASE := $(shell /bin/sh -c 'exit 3')
all: split
all:;@:
split:: $$(shell )$$(file >>seen,head)
	@$(info head=[$@|$(.SHELLSTATUS)])$(file >>seen,head-recipe):
split:: $$(shell )$$(file >>seen,next)
	@$(info next=[$@|$(.SHELLSTATUS)])$(file >>seen,next-recipe):
