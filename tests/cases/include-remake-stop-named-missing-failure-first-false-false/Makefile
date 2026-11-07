ifeq ($(MAKE_RESTARTS),)
$(shell touch -d @1000000100 changed.mk; touch -d @1000000200 trigger)
endif
$(info read [$(MAKE_RESTARTS)])
default: ; @printf "goal\n"
changed.mk: trigger
	@printf "remake changed\n"
	@touch changed.mk
