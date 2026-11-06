.SECONDEXPANSION:
SET = $(file >>seen,search)$(shell /bin/sh -c 'exit 7')
CHECK = $(info phase=[$(.SHELLSTATUS)|$(V)])$(file >>seen,phase)$(if $(.SHELLSTATUS),,$(shell /bin/sh -c 'exit 7'))
all: item.out
all:;@:
other: missing
.PRECIOUS: %.mid
%.mid: V := base
%.mid: V += $(file >>seen,pattern)ready
%.out: %.mid
	@$(info root=[$@|$(.SHELLSTATUS)])$(file >>seen,root):
%.mid: $$(CHECK) missing
	@$(info child=[$@|$(.SHELLSTATUS)|$(V)])$(file >>seen,child):
.DEFAULT:;@:
