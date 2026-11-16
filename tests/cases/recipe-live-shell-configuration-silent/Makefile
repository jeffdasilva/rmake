$(file >events,read)
SHELL = $(file >>events,shell)$(eval MAKEFLAGS += -s)/bin/sh
.SHELLFLAGS = $(file >>events,flags)-c
IFS = $(file >>events,ifs)
all: target
	@echo after
target:
	false
	@echo line
