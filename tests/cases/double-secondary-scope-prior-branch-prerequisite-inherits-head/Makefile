.SECONDEXPANSION:
HEAD = $(shell /bin/sh -c 'exit 7')$(file >>seen,head)
NEXT = $(shell /bin/sh -c 'exit 11')$(file >>seen,next)
all: split
all:;@:
split:: $$(HEAD)
	@:
split:: second $$(NEXT)
	@:
split:: third
	@$(info last=[$@|$(.SHELLSTATUS)])$(file >>seen,last):
second:;@:
third:;@$(info third=[$@|$(.SHELLSTATUS)])$(file >>seen,third):
