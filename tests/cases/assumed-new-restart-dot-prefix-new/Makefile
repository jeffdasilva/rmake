$(shell if test ! -e seeded; then touch -d @1600000000 trigger; touch -d @1600000004 fragment.mk; touch -d @1600000005 result; : >seeded; fi)
$(info read=$(MAKE_RESTARTS))
.DEFAULT_GOAL := all
include fragment.mk
fragment.mk: trigger
	@printf "remake=%s\n" "$(MAKE_RESTARTS)"
	@printf "# refreshed\n" >$@
	@touch -d @1600000007 $@
all: result
result: trigger
	@printf "goal=%s\n" "$(MAKE_RESTARTS)"
	@touch -d @1600000008 $@
