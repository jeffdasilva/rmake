ifeq ($(MAKE_RESTARTS),)
$(shell touch -d @1600000000 owner.inc peer.inc restore.inc fail.inc; touch -d @1600000005 trigger)
endif
stage := $(or $(MAKE_RESTARTS),0)
.PHONY: all force
all: ; @echo goal=$(VALUE):$(stage)
-include fail.inc
include owner.inc
include restore.inc
include peer.inc
$(info read=$(stage):$(VALUE):$(shell stat -c %Y peer.inc):$(MAKEFILE_LIST))
ifeq ($(MAKE_RESTARTS),)
owner.inc peer.inc &:: trigger
	@+echo execute=$@; printf "VALUE := new\n" > peer.inc; printf "%s\n" "$@" >> executed; touch -d @1600000007 peer.inc
restore.inc: force
	@echo restore; touch -d @1600000000 peer.inc
fail.inc: force
	@echo failure; false
endif
