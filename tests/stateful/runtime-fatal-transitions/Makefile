.DEFAULT_GOAL=all
include config.mk
.SHELLFLAGS=$(if $(and $(FAIL),$(wildcard trigger)),$(error stopped),-c)
all:target
target:force
	+@python3 produce.py
	@printf 'completed\n'
.PHONY:force
force:;
