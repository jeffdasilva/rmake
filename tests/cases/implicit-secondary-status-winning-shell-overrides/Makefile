.SECONDEXPANSION:
SET = $(file >>seen,search)$(shell /bin/sh -c 'exit 7')
NEXT = $(file >>seen,winner)$(shell /bin/sh -c 'exit 11')
all: item.out
all:;@:
%.out: $$(SET) %.missing
	@$(error unexpected recipe)
%.out: %.in $$(NEXT)
	@$(info recipe=[$@|$(.SHELLSTATUS)])$(file >>seen,recipe):
