BASE := $(shell /bin/sh -c 'exit 7')
$(info $(let x,word,$(x):$(.SHELLSTATUS)/$(shell /bin/sh -c 'exit 3')$(.SHELLSTATUS)))
$(info outer=[$(.SHELLSTATUS)] $(origin .SHELLSTATUS) $(flavor .SHELLSTATUS))
all:; @:
