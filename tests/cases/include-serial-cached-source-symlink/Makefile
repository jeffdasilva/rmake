ifeq ($(MAKE_RESTARTS),)
$(shell touch -d @1600000000 worker.inc backing; touch -d @1600000005 trigger; ln -s backing payload.mk; touch -h -d @1600000000 payload.mk)
endif
.PHONY: all force
all:
	@echo ALL:$(VALUE):$(or $(MAKE_RESTARTS),0)
	@if test -f payload.mk; then cat payload.mk; fi
include worker.inc
include payload.mk
$(info READ:$(or $(MAKE_RESTARTS),0):$(VALUE):$(MAKEFILE_LIST))
ifeq ($(MAKE_RESTARTS),)
worker.inc: payload.mk
	@echo WORKER:"$<"; :
payload.mk: trigger
	@echo REMAKE:"$@":"$<"; printf "VALUE := new\n" > "$@"; touch -d @1600000007 "$@"
endif
