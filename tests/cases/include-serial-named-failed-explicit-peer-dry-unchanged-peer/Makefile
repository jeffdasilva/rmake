ifeq ($(MAKE_RESTARTS),)
$(shell touch -d @1600000000 payload.owner payload.mk tail.inc; touch -d @1600000005 trigger)
endif
.PHONY: all force
all:
	@echo ALL:$(VALUE):$(or $(MAKE_RESTARTS),0)
include payload.owner
include payload.mk
include tail.inc
$(info READ:$(or $(MAKE_RESTARTS),0):$(VALUE):$(MAKEFILE_LIST))
ifeq ($(MAKE_RESTARTS),)
payload.owner payload.mk &: trigger
	@echo EXEC:"$@"; printf "%s\n" "$@" >> executed; false
tail.inc: force
	@echo TAIL; touch -d @1600000015 payload.mk
endif
