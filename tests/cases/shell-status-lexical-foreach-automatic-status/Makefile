BASE := $(shell /bin/sh -c 'exit 7')
$(info $(foreach .SHELLSTATUS,a b,[$(shell /usr/bin/false)$(.SHELLSTATUS):$(origin .SHELLSTATUS)]))
$(info outer=[$(.SHELLSTATUS)] $(origin .SHELLSTATUS) $(flavor .SHELLSTATUS))
all:; @:
