.SECONDEXPANSION:
ifeq ($(MAKE_RESTARTS),)
$(shell touch -d @1600000001 seed)
endif
$(info read|$(MAKE_RESTARTS))
include b.mk

all:
	@test "$$(stat -c %Y b.mk)" -gt 1600000001
	@test -f b.mk && test ! -s b.mk
	@printf '%s\n' 'goal|$(MAKE_RESTARTS)'

b.mk: seed
	$(eval MAKEFLAGS += -t)@printf '%s\n' '# unexpected recipe' >> $@
