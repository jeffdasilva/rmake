ifeq ($(MAKE_RESTARTS),)
$(shell touch -d @1600000000 payload.owner payload.mk other.inc; touch -d @1600000005 trigger)
endif
.PHONY: all force
all:
	@echo ALL:$(VALUE):$(SIDE):$(or $(MAKE_RESTARTS),0)
include payload.owner
include payload.mk
-include other.inc
$(info READ:$(or $(MAKE_RESTARTS),0):$(VALUE):$(SIDE):$(MAKEFILE_LIST))
ifeq ($(MAKE_RESTARTS),)
payload.owner payload.mk &: trigger
	@echo EXEC:"$@"; echo "$@" >> executed; printf "VALUE := written\n" > payload.mk; touch -d @1600000007 payload.mk
other.inc: force
	@echo OTHER; false
endif
