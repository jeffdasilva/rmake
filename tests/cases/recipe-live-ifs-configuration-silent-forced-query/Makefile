$(file >events,read)
SHELL = $(file >>events,shell)/bin/sh
.SHELLFLAGS = $(file >>events,flags)-c
IFS = $(file >>events,ifs)$(eval MAKEFLAGS += -s)
all: target
	@echo after
target:
	+false
	@echo line
