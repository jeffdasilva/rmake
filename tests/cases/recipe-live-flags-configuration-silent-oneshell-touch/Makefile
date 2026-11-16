.ONESHELL:
$(file >events,read)
SHELL = $(file >>events,shell)/bin/sh
.SHELLFLAGS = $(file >>events,flags)$(eval MAKEFLAGS += -s)-c
IFS = $(file >>events,ifs)
all: target
	@echo after
target:
	+echo line
	false
