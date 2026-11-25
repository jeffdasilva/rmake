ifeq ($(MAKE_RESTARTS),)
$(shell touch -d @1600000000 worker.inc payload.mk; touch -d @1600000005 trigger)
endif
.PHONY: all force
all:
	@echo ALL:$(VALUE):$(or $(MAKE_RESTARTS),0)
	@if test -f payload.mk; then cat payload.mk; fi
include payload.mk
include worker.inc
$(info READ:$(or $(MAKE_RESTARTS),0):$(VALUE):$(MAKEFILE_LIST))
ifeq ($(MAKE_RESTARTS),)
worker.inc: payload.mk
	@echo WORKER:"$<"; :
%.mk: trigger
	@echo REMAKE:"$@":"$<"; printf "VALUE := new\n" > "$@"; touch -d @1600000007 "$@"
endif
