.SECONDEXPANSION:
HEAD = $(shell /bin/sh -c 'exit 7')$(file >>seen,head)
NEXT = $(shell /bin/sh -c 'exit 11')$(file >>seen,next)
all: split
all:;@:
split:: first $$(HEAD)
	@$(info head=[$@|$(.SHELLSTATUS)])$(file >>seen,head-recipe):
split::
	@:
first:;@$(info first=[$@|$(.SHELLSTATUS)])$(file >>seen,first):
