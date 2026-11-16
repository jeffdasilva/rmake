.ONESHELL:
$(file >events,read)
SHELL = $(file >>events,shell)/bin/sh
.SHELLFLAGS = $(file >>events,flags)-c
IFS = $(file >>events,ifs)$(eval MAKEFLAGS += -i)
all: target
	@echo after
target:
	echo line
	false
