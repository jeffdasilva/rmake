.ONESHELL:
EMPTY :=
$(file >events,read)
export CHILD = $(file >>events,export)yes
SHELL = $(file >>events,shell)$(eval MAKEFLAGS += -t)/bin/sh
.SHELLFLAGS = $(file >>events,flags)-c
IFS = $(file >>events,ifs)
.PHONY: force
force:
.PHONY: target
target: force
	$(file >>events,body)printf "%s\n" first > child
	@printf "%s\n" second >> child
