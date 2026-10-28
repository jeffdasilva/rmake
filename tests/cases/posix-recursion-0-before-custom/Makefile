MAKEOVERRIDES=Y=other
$(info reader=[$(value MAKEFLAGS)]/[$(MAKEFLAGS)])
all:
	@$(info flags=[$(value MAKEFLAGS)]/[$(MAKEFLAGS)])$(info X=[$(X)]/[$(origin X)])true
	@$(MAKE) --no-print-directory -f child.mk child
