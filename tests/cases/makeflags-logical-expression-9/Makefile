A = $(B)
B = $(MAKEFLAGS)
ID = $(1)
.SECONDEXPANSION:
a.out: MAKEFLAGS += -k
a.out: a.in
	@$(info before|raw=[$(value MAKEFLAGS)]|value=[$(MAKEFLAGS)]|flavor=$(flavor MAKEFLAGS)|origin=$(origin MAKEFLAGS))$(eval MAKEFLAGS += -s)$(info after0|raw=[$(value MAKEFLAGS)]|value=[$(MAKEFLAGS)]|flavor=$(flavor MAKEFLAGS)|origin=$(origin MAKEFLAGS))$(info recipe|raw=[$(value MAKEFLAGS)]|value=[$(MAKEFLAGS)]|flavor=$(flavor MAKEFLAGS)|origin=$(origin MAKEFLAGS))$(info edge=[$(MAKEFLAGS:s=S)])printf 'parent-env=[%s]\n' "$$MAKEFLAGS"
	@printf done > completed
