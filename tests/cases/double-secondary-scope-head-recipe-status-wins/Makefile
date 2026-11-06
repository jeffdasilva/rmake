.SECONDEXPANSION:
HEAD = $(shell /bin/sh -c 'exit 7')$(file >>seen,head)
NEXT = $(shell /bin/sh -c 'exit 11')$(file >>seen,next)
all: split
all:;@:
split:: $$(HEAD)
	@$(shell /bin/sh -c 'exit 3')$(info head=[$@|$(.SHELLSTATUS)])$(file >>seen,head-recipe):
split:: $$(info later=[$$@|$$(.SHELLSTATUS)])$$(file >>seen,later)
	@$(info next=[$@|$(.SHELLSTATUS)])$(file >>seen,next-recipe):
