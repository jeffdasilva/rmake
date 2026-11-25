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
worker.inc: | payload.mk
%.mk %.other &: trigger
	@echo REMAKE:"$@":"$<"; false
endif
