.SECONDEXPANSION:
HEAD = $(shell /bin/sh -c 'exit 7')$(file >>seen,head)
NEXT = $(shell /bin/sh -c 'exit 11')$(file >>seen,next)
THIRD = $(shell /bin/sh -c 'exit 13')$(file >>seen,third)
all: split outside
all:;@:
split:: shared $$(HEAD)
	@:
shared:;@$(info first=[$@|$(.SHELLSTATUS)])$(file >>seen,shared):
split:: $$(NEXT)
	@:
outside:;@$(eval shared: RESULT := $$(info retained=[$$@|$$(.SHELLSTATUS)])$$(file >>seen,retained)):
