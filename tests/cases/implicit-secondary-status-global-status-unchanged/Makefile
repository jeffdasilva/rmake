.SECONDEXPANSION:
SET = $(file >>seen,search)$(shell /bin/sh -c 'exit 7')
BASE := $(shell /bin/sh -c 'exit 3')
all: item.out sibling
all:;@:
%.out: %.in $$(SET)
	@$(info recipe=[$@|$(.SHELLSTATUS)])$(file >>seen,recipe):
sibling:;@$(info sibling=[$@|$(.SHELLSTATUS)])$(file >>seen,sibling):
