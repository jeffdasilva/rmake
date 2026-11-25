ifeq ($(MAKE_RESTARTS),)
$(shell touch -d @1600000000 worker.inc tail.inc payload.mk; touch -d @1600000005 trigger)
endif
.PHONY: all force
all:
	@echo ALL:$(VALUE):$(or $(MAKE_RESTARTS),0)
	@if test -f payload.mk; then cat payload.mk; fi
include worker.inc
include payload.mk
include tail.inc
$(info READ:$(or $(MAKE_RESTARTS),0):$(VALUE):$(MAKEFILE_LIST))
ifeq ($(MAKE_RESTARTS),)
worker.inc: force
	@echo WORKER; touch -d @1600000003 payload.mk
payload.mk:
%.mk: trigger
	@echo REMAKE:"$@":"$<"; false
tail.inc: force
	@echo TAIL; rm -f payload.mk
endif
