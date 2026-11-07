$(shell if test ! -e initialized; then touch -d @1600000000 fragment.mk; touch -d @1600000001 trigger; : >initialized; fi)
$(info restart=[$(MAKE_RESTARTS)])
include fragment.mk
ifeq ($(MAKE_RESTARTS),)
fragment.mk: trigger
	@touch -d @1600000002 $@
endif
branch:: ; @$(info evaluated-branch):
branch:: ; +@printf 'forced-failure\n'; exit 7
after: ; +@printf 'visited-after\n'
