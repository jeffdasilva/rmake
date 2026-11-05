BASE := $(shell /bin/sh -c 'exit 7')
$(info $(let .SHELLSTATUS,word,$(shell /usr/bin/false)$(.SHELLSTATUS):$(origin .SHELLSTATUS)))
$(info outer=[$(.SHELLSTATUS)] $(origin .SHELLSTATUS) $(flavor .SHELLSTATUS))
all:; @:
