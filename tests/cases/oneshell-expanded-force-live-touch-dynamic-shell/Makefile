.ONESHELL:
PLUS := +@
MAKE := :
NEW_SHELL := /bin/sh
$(file >events,read)
export CHILD = $(file >>events,export)yes
SHELL = $(file >>events,shell)$(NEW_SHELL)
.SHELLFLAGS = $(file >>events,flags)-c
IFS = $(file >>events,ifs)
.PHONY: force
force:
target: force
	$(PLUS)$(eval MAKEFLAGS += -t)$(eval NEW_SHELL := ./wrapper)$(file >>events,body)printf "%s\n" forced > child
	@printf "%s\n" second >> child
