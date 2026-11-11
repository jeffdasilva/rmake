$(shell if test ! -e seeded; then touch -d @1600000000 cached primer fragment.mk; touch -d @1600000001 newer; touch -d @1600000002 trigger; : >seeded; fi)
$(info read=[$(MAKE_RESTARTS)])
.DEFAULT_GOAL := cached
include fragment.mk
ifeq ($(MAKE_RESTARTS),)
fragment.mk: trigger
	@touch -d @1600000003 $@
else
fragment.mk: cached
endif
cached:: primer
	@$(error current declared branch must not expand)
.SECONDEXPANSION:
ca%: $$(if $$(wildcard chosen),$$(info search-new)newer,$$(info search-old)$$(file >chosen,yes)primer)
	@$(info synthetic-query):
.DEFAULT: ; @$(info synthetic-default):
gate: ; @$(info prior-query):
