.SECONDEXPANSION:
HEAD = $(shell /bin/sh -c 'exit 7')$(file >>seen,head)
NEXT = $(shell /bin/sh -c 'exit 11')$(file >>seen,next)
THIRD = $(shell /bin/sh -c 'exit 13')$(file >>seen,third)
all: split
all:;@:
vpath %.out shelf
GPATH := shelf
.PHONY: force
split:: shared.out $$(HEAD)
	@:
split:: shared.out $$(NEXT)
	@$(eval shelf/shared.out: RESULT := $$(info retained=[$$@|$$(.SHELLSTATUS)])$$(file >>seen,retained)):
shared.out: force
shared.out:;@$(info first=[$@|$(.SHELLSTATUS)])$(file >>seen,shared):
