$(file >events,read)
SHELL = $(file >>events,shell)/bin/sh
.SHELLFLAGS = $(file >>events,flags)$(eval MAKEFLAGS += -i)-c
IFS = $(file >>events,ifs)
all: target
	@echo after
target:
	@-false
	@echo line
