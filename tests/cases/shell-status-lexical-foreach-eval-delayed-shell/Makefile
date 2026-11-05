BASE := $(shell /bin/sh -c 'exit 7')
$(info $(foreach i,a b,$(if $(filter a,$(i)),$(eval SAVED := $$(shell /bin/sh -c 'exit 4')$$(.SHELLSTATUS)))[$(i):$(SAVED):$(.SHELLSTATUS)]))
$(info saved=[$(SAVED)])
$(info outer=[$(.SHELLSTATUS)] $(origin .SHELLSTATUS) $(flavor .SHELLSTATUS))
all:; @:
