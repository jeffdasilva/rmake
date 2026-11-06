.SECONDEXPANSION:
SET = $(file >>seen,search)$(shell /bin/sh -c 'exit 7')
all: item.out
all:;@:
%.out: $$(SET) %.missing
	@$(error unexpected recipe)
.DEFAULT:
	@$(info default=[$@|$(.SHELLSTATUS)])$(file >>seen,default):
