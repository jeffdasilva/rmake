.SECONDEXPANSION:
SET = $(file >>seen,search)$(shell /bin/sh -c 'exit 7')
BASE := $(shell /bin/sh -c 'exit 3')
all: item.out item.mid after
all:;@:
.PRECIOUS: %.good
%.out: %.mid %.missing
	@$(error unexpected recipe)
%.out: %.good
	@$(info root=[$@|$(.SHELLSTATUS)])$(file >>seen,root):
%.mid: %.no $$(SET)
	@$(error unexpected recipe)
%.good: %.in
	@:
item.mid:;@$(info explicit=[$@|$(.SHELLSTATUS)])$(file >>seen,explicit):
after:;@:
