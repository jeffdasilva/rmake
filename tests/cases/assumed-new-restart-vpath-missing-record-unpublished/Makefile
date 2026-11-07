$(shell if test ! -e seeded; then touch -d @1600000000 remote/anchor; : >seeded; fi)
$(info read=$(MAKE_RESTARTS))
.DEFAULT_GOAL := all
-include fragment.mk
VPATH := remote
fragment.mk: unentered
	@printf "remake=%s\n" "$(MAKE_RESTARTS)"
	@printf "# created\n" >$@
	@touch -d @1600000007 $@
all:
	@printf "goal=%s\n" "$(MAKE_RESTARTS)"
