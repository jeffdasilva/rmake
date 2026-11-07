$(info read=$(MAKE_RESTARTS))
.DEFAULT_GOAL := all
-include fragment.mk
fragment.mk: unentered
	@printf "remake=%s\n" "$(MAKE_RESTARTS)"
	@printf "# created\n" >$@
	@touch -d @1600000007 $@
all: unentered
	@printf "goal=%s\n" "$(MAKE_RESTARTS)"
