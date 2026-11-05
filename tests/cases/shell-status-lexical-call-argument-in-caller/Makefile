BASE := $(shell /bin/sh -c 'exit 7')
M = before=$(.SHELLSTATUS); value=$(shell /bin/sh -c 'printf text; exit 3'); after=$(.SHELLSTATUS)
$(info $(call M,$(shell /usr/bin/true)))
$(info outer=[$(.SHELLSTATUS)] $(origin .SHELLSTATUS) $(flavor .SHELLSTATUS))
all:; @:
