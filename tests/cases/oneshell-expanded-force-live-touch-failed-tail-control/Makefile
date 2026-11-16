.ONESHELL:
PLUS := +@
MAKE := :
NEW_SHELL := ./wrapper
$(file >events,read)
export CHILD = $(file >>events,export)yes
SHELL = $(file >>events,shell)/bin/sh
.SHELLFLAGS = $(file >>events,flags)-c
IFS = $(file >>events,ifs)
.PHONY: force
force:
target: force
	$(PLUS)$(eval MAKEFLAGS += -t)$(file >>events,body)printf '%s\n' 'first
	second' > child
	@printf tail >> child
