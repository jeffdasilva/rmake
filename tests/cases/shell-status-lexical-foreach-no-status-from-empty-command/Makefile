BASE := $(shell /bin/sh -c 'exit 7')
$(info $(foreach i,a b,[$(i):$(shell )$(.SHELLSTATUS)]))
$(info outer=[$(.SHELLSTATUS)] $(origin .SHELLSTATUS) $(flavor .SHELLSTATUS))
all:; @:
