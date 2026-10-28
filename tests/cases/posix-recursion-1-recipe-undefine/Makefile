.POSIX:
$(info reader=[$(value MAKEFLAGS)]/[$(MAKEFLAGS)])
all:
	@$(eval undefine MAKEOVERRIDES)$(info flags=[$(value MAKEFLAGS)]/[$(MAKEFLAGS)])$(info X=[$(X)]/[$(origin X)])true
	@$(MAKE) --no-print-directory -f child.mk child
