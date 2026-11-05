BASE := $(shell /bin/sh -c 'exit 7')
$(info $(foreach i,$(shell /usr/bin/printf 'a b'),[$(i):$(.SHELLSTATUS)]))
$(info outer=[$(.SHELLSTATUS)] $(origin .SHELLSTATUS) $(flavor .SHELLSTATUS))
all:; @:
