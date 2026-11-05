BASE := $(shell /bin/sh -c 'exit 7')
$(info $(foreach i,a b,[$(i):$(.SHELLSTATUS)/$(if $(filter a,$(i)),$(shell /usr/bin/true))$(.SHELLSTATUS)]))
$(info outer=[$(.SHELLSTATUS)] $(origin .SHELLSTATUS) $(flavor .SHELLSTATUS))
all:; @:
