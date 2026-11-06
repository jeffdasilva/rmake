.SECONDEXPANSION:
HEAD = $(shell /bin/sh -c 'exit 7')$(file >>seen,head)
NEXT = $(shell /bin/sh -c 'exit 11')$(file >>seen,next)
all: split
all:;@:
split:: $$(HEAD)
	@:
split:: local sibling $$(NEXT)
	@:
local: SET != /bin/sh -c 'exit 13'
local:;@$(info local=[$@|$(.SHELLSTATUS)])$(file >>seen,local):
sibling:;@$(info sibling=[$@|$(.SHELLSTATUS)])$(file >>seen,sibling):
