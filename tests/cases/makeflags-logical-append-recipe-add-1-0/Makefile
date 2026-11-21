.SECONDEXPANSION:
a.out: MAKEFLAGS += -k
a.out: a.in
	@$(info before|raw=[$(value MAKEFLAGS)]|value=[$(MAKEFLAGS)]|flavor=$(flavor MAKEFLAGS)|origin=$(origin MAKEFLAGS))$(eval MAKEFLAGS += -s)$(info after0|raw=[$(value MAKEFLAGS)]|value=[$(MAKEFLAGS)]|flavor=$(flavor MAKEFLAGS)|origin=$(origin MAKEFLAGS))$(info recipe|raw=[$(value MAKEFLAGS)]|value=[$(MAKEFLAGS)]|flavor=$(flavor MAKEFLAGS)|origin=$(origin MAKEFLAGS))printf 'parent-env=[%s]\n' "$$MAKEFLAGS"
	@$(MAKE) -f child.mk
	@printf done > completed
