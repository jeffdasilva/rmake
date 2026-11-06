.SECONDEXPANSION:
SET = $(file >>seen,search)$(shell /bin/sh -c 'exit 7')
CHECK = $(info phase=[$(.SHELLSTATUS)])$(file >>seen,phase)$(if $(.SHELLSTATUS),,$(shell /bin/sh -c 'exit 7'))
all: item.out
all:;@:
other: missing
.PRECIOUS: %.mid
%.out: %.mid
	@$(info root=[$@|$(.SHELLSTATUS)])$(file >>seen,root):
%.mid: $$(CHECK) missing
	@$(info child=[$@|$(.SHELLSTATUS)])$(file >>seen,child):
.DEFAULT:;@:
