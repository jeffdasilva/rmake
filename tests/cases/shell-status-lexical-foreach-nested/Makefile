BASE := $(shell /bin/sh -c 'exit 7')
$(info $(foreach a,x,$(shell /bin/sh -c 'exit 4')parent=$(.SHELLSTATUS) $(foreach b,y,child-before=$(.SHELLSTATUS) $(shell /usr/bin/true)child-after=$(.SHELLSTATUS)) parent-after=$(.SHELLSTATUS)))
$(info outer=[$(.SHELLSTATUS)] $(origin .SHELLSTATUS) $(flavor .SHELLSTATUS))
all:; @:
