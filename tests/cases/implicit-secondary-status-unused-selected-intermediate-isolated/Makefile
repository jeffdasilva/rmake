.SECONDEXPANSION:
SET = $(file >>seen,search)$(shell /bin/sh -c 'exit 7')
BASE := $(shell /bin/sh -c 'exit 3')
all: item.out after
all:;@:
.PRECIOUS: %.good
%.out: %.mid %.missing
	@$(error unexpected recipe)
%.out: %.good
	@$(info root=[$@|$(.SHELLSTATUS)])$(file >>seen,root):
%.mid: %.in $$(SET)
	@$(error unexpected recipe)
%.good: %.in
	@:
after:;@$(eval item.mid: RESULT := $$(info unused=[$$(.SHELLSTATUS)])$$(file >>seen,unused)):
