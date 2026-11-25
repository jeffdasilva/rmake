ifeq ($(MAKE_RESTARTS),)
$(shell touch -d @1600000000 worker.inc payload.mk payload.other; touch -d @1600000005 trigger; touch -d @1600000000 tail.inc)
endif
.PHONY: all force
all:
	@echo ALL:$(VALUE):$(or $(MAKE_RESTARTS),0)
	@if test -f payload.mk; then cat payload.mk; fi
include worker.inc
-include payload.other
-include payload.mk
include payload.mk
include tail.inc
$(info READ:$(or $(MAKE_RESTARTS),0):$(VALUE):$(MAKEFILE_LIST))
ifeq ($(MAKE_RESTARTS),)
worker.inc: force
	@echo WORKER; :
payload.mk payload.other &: trigger
	@echo REMAKE:"$@":"$<"; printf "VALUE := new\n" > payload.mk; touch -d @1600000007 payload.mk; false
tail.inc: force
	@echo TAIL; touch -d @1600000000 payload.mk
endif
