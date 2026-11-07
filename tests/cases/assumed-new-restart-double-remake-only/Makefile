$(shell if test ! -e seeded; then touch -d @1600000004 fragment.mk; touch -d @1600000000 cached; touch -d @1600000001 primer; touch -d @1600000005 result; : >seeded; fi)
$(info read=$(MAKE_RESTARTS))
.DEFAULT_GOAL := all
include fragment.mk
fragment.mk: cached
	@printf "unexpected fragment recipe\n"
	@printf "# refreshed\n" >$@
	@touch -d @1600000007 $@
cached:: primer
	@printf "cached=%s\n" "$(MAKE_RESTARTS)"
	@touch -d @1600000002 $@
all: ; @printf "ordinary\n"
result: cached
	@printf "goal=%s\n" "$(MAKE_RESTARTS)"
	@touch -d @1600000008 $@
