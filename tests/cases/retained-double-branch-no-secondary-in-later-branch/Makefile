.SECONDEXPANSION:
HEAD = $(shell /bin/sh -c 'exit 7')$(file >>seen,head)
NEXT = $(shell /bin/sh -c 'exit 11')$(file >>seen,next)
THIRD = $(shell /bin/sh -c 'exit 13')$(file >>seen,third)
all: split
all:;@:
split:: shared $$(HEAD)
	@$(shell /bin/sh -c 'exit 3')$(file >>seen,head-recipe):
split:: shared
	@$(eval shared: RESULT := $$(info retained=[$$@|$$(.SHELLSTATUS)])$$(file >>seen,retained)):
shared:;@$(info first=[$@|$(.SHELLSTATUS)])$(file >>seen,shared):
