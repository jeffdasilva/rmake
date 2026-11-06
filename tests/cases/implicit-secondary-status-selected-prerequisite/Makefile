.SECONDEXPANSION:
SET = $(file >>seen,search)$(shell /bin/sh -c 'exit 7')
all: item.out
all:;@:
%.out: %.in $$(SET)
	@$(info recipe=[$@|$(.SHELLSTATUS)])$(file >>seen,recipe):
.PHONY: item.in
item.in:;@$(info input=[$@|$(.SHELLSTATUS)])$(file >>seen,input):
