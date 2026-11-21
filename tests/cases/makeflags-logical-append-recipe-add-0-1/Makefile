.SECONDEXPANSION:
a.out: MAKEFLAGS += $(file >>tail.trace,T)-k
a.out: a.in
	@$(info before|raw=[$(value MAKEFLAGS)]|value=[$(MAKEFLAGS)]|flavor=$(flavor MAKEFLAGS)|origin=$(origin MAKEFLAGS))$(eval MAKEFLAGS += -s)$(info after0|raw=[$(value MAKEFLAGS)]|value=[$(MAKEFLAGS)]|flavor=$(flavor MAKEFLAGS)|origin=$(origin MAKEFLAGS))$(info recipe|raw=[$(value MAKEFLAGS)]|value=[$(MAKEFLAGS)]|flavor=$(flavor MAKEFLAGS)|origin=$(origin MAKEFLAGS))printf 'parent-env=[%s]\n' "$$MAKEFLAGS"
	@printf done > completed
