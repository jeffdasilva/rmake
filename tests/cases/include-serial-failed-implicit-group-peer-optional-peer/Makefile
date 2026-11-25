ifeq ($(MAKE_RESTARTS),)
$(shell touch -d @1600000000 payload.mk payload.other worker.inc; touch -d @1600000005 trigger)
endif
.PHONY: all force
all:
	@echo ALL:$(VALUE):$(or $(MAKE_RESTARTS),0)
	@if test -f payload.mk; then cat payload.mk; fi
include worker.inc
-include payload.mk
$(info READ:$(or $(MAKE_RESTARTS),0):$(VALUE):$(MAKEFILE_LIST))
ifeq ($(MAKE_RESTARTS),)
worker.inc: | payload.other
%.mk %.other &: trigger
	@echo REMAKE:"$@":"$<"; printf "VALUE := new\n" > payload.mk; touch -d @1600000007 payload.mk; false
endif
