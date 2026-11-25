ifeq ($(MAKE_RESTARTS),)
$(shell touch -d @1600000000 worker.inc payload.mk payload.other; touch -d @1600000005 trigger)
endif
.PHONY: all force
all:
	@echo ALL:$(VALUE):$(AUX):$(or $(MAKE_RESTARTS),0)
	@if test -f payload.mk; then cat payload.mk; fi
include worker.inc
include payload.mk
$(info READ:$(or $(MAKE_RESTARTS),0):$(VALUE):$(AUX):$(MAKEFILE_LIST))
ifeq ($(MAKE_RESTARTS),)
worker.inc: bridge
	@echo WORKER:"$<"; touch -d @1600000000 payload.mk
.PHONY: bridge
bridge: payload.mk
	@echo BRIDGE:"$<"
payload.mk:
payload.other:
%.mk %.other &: trigger
	@echo REMAKE:"$@":"$<"; printf "VALUE := new\n" > payload.mk; touch -d @1600000007 payload.mk
endif
