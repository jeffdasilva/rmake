.ONESHELL:
PLUS := +@
MAKE := :
NEW_SHELL := ./wrapper
$(file >events,read)
export CHILD = $(file >>events,export)yes
SHELL = $(file >>events,shell)./wrapper
.SHELLFLAGS = $(file >>events,flags)-c
IFS = $(file >>events,ifs)$(eval MAKEFLAGS += -t)
.PHONY: force
force:
target: force
	$(PLUS)$(file >>events,body)printf "%s\n" forced > child
	@printf "%s\n" second >> child
