.SECONDEXPANSION:
HEAD = $(shell /bin/sh -c 'exit 7')$(file >>seen,head)
NEXT = $(shell /bin/sh -c 'exit 11')$(file >>seen,next)
THIRD = $(shell /bin/sh -c 'exit 13')$(file >>seen,third)
all: split
all:;@:
split:: shared $$(HEAD)
	@:
split:: shared $$(NEXT)
	@$(eval shared: FIRST := $$(info retained=[$$@|$$(.SHELLSTATUS)])$$(file >>seen,retained)$$(shell /bin/sh -c 'exit 17'))$(eval shared: SECOND := $$(info retained=[$$@|$$(.SHELLSTATUS)])$$(file >>seen,retained))$(info outer=[$@|$(.SHELLSTATUS)]):
shared:;@$(shell /bin/sh -c 'exit 5')$(info first=[$@|$(.SHELLSTATUS)])$(file >>seen,shared):
