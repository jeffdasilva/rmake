$(shell if test ! -e seeded; then touch -d @1600000000 cached; touch -d @1600000001 primer; touch -d @1600000004 fragment.mk; touch -d @1600000005 result; touch -d @1600000006 first; : >seeded; fi)
$(info read=[$(MAKE_RESTARTS)])
.DEFAULT_GOAL := gate
include fragment.mk
ifeq ($(MAKE_RESTARTS),)
fragment.mk: first
	@touch -d @1600000007 $@
else
fragment.mk: primer
	@printf 'unexpected fragment refresh\n'
	@touch -d @1600000007 $@
endif
cached:: primer
	@printf 'declared=[%s|%s]\n' '$@' '$<'
	@touch -d @1600000002 $@
gate: ; @printf 'ordinary\n'
result: cached
	@printf 'result=[%s|%s]\n' '$<' '$?'
	@touch -d @1600000008 $@
.DEFAULT:
	@printf 'default=[%s|%s|%s]\n' '$@' '$<' '$*'
	@printf 'default\n' >recipe-marker
