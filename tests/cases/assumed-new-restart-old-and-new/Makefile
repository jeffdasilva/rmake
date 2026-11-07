$(shell if test ! -e seeded; then touch -d @1600000004 fragment.mk; touch -d @1600000000 trigger; touch -d @1600000000 preserve; touch -d @1600000001 primer; touch -d @1600000005 result; : >seeded; fi)
$(info read=$(MAKE_RESTARTS))
.DEFAULT_GOAL := all
include fragment.mk
fragment.mk: preserve trigger
	@printf "unexpected fragment recipe\n"
	@printf "# refreshed\n" >$@
	@touch -d @1600000007 $@
preserve: primer
	@printf "unexpected old recipe\n"
	@touch -d @1600000009 $@
all: result
result: trigger preserve
	@printf "goal=%s\n" "$(MAKE_RESTARTS)"
