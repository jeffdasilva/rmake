BASE := $(shell /bin/sh -c 'exit 7')
define BODY
WRITTEN != /bin/sh -c 'exit 5'
AFTER := $$(.SHELLSTATUS)
endef
M = $(eval $(BODY))[$(AFTER):$(.SHELLSTATUS)]
$(info $(call M))
$(info after=[$(AFTER)])
$(info outer=[$(.SHELLSTATUS)] $(origin .SHELLSTATUS) $(flavor .SHELLSTATUS))
all:; @:
