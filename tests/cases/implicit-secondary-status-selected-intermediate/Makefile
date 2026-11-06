.SECONDEXPANSION:
SET = $(file >>seen,search)$(shell /bin/sh -c 'exit 7')
all: item.out
all:;@:
.PRECIOUS: %.mid
%.out: %.mid
	@$(info root=[$@|$(.SHELLSTATUS)])$(file >>seen,root):
%.mid: %.in $$(SET)
	@$(info child=[$@|$(.SHELLSTATUS)])$(file >>seen,child):
