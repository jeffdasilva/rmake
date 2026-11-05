BASE := $(shell /bin/sh -c 'exit 7')
$(info $(foreach i,a b,$(if $(filter a,$(i)),$(eval CREATED != /usr/bin/printf value))[$(i):$(CREATED):$(.SHELLSTATUS)]))
$(info created=[$(CREATED)])
$(info outer=[$(.SHELLSTATUS)] $(origin .SHELLSTATUS) $(flavor .SHELLSTATUS))
all:; @:
