.DEFAULT_GOAL=out
.SECONDEXPANSION:
include config.mk
out: $$(DEPS)
	@python3 build.py '$?' '$<'
