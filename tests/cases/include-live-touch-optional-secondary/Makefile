.SECONDEXPANSION:
ifeq ($(MAKE_RESTARTS),)
$(shell touch -d @1600000000 b.mk && touch -d @1600000001 seed)
endif
$(info read|$(MAKE_RESTARTS))
-include b.mk

all:
	@test "$$(stat -c %Y b.mk)" -gt 1600000001
	@grep -qx '# original' b.mk
	@printf '%s\n' 'goal|$(MAKE_RESTARTS)'

b.mk: $$(eval MAKEFLAGS += -t) seed
	@printf '%s\n' '# unexpected recipe' >> $@
