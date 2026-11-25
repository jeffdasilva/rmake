.SECONDEXPANSION:
ifeq ($(MAKE_RESTARTS),)
$(shell touch -d @1600000000 Makefile && touch -d @1600000001 seed)
endif
$(info read|$(MAKE_RESTARTS))

all:
	@test "$$(stat -c %Y Makefile)" -gt 1600000001
	@printf '%s\n' 'goal|$(MAKE_RESTARTS)'

Makefile: $$(eval MAKEFLAGS += -t) seed
	@printf '%s\n' '# unexpected recipe' >> $@
