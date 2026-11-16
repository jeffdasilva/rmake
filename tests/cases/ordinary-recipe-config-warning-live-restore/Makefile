SHELL = $(eval MAKEFLAGS += --warn-undefined-variables)$(CONFIG_MISSING)/bin/sh
.SHELLFLAGS = $(FLAGS_MISSING)-c
IFS = $(IFS_MISSING)
all: first second
first:
	@:
second:
	@$(eval SHELL := /bin/sh)$(eval .SHELLFLAGS := -c)$(eval IFS :=)$(BODY_MISSING)printf '%s\n' last
