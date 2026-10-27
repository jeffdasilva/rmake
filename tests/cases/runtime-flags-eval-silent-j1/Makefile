.SHELLFLAGS = $(warning config-$@)-c
.PHONY: all
all:
	@$(info expansion-before)$(eval MAKEFLAGS += -s)$(info expansion-after)printf 'child\n'
	@printf 'tail\n'
