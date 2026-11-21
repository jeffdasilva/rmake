.SECONDEXPANSION:
a.out: MAKEFLAGS += -k
a.out: $$(info before|raw=[$$(value MAKEFLAGS)]|value=[$$(MAKEFLAGS)]|flavor=$$(flavor MAKEFLAGS)|origin=$$(origin MAKEFLAGS))$$(eval MAKEFLAGS += -s)$$(info after0|raw=[$$(value MAKEFLAGS)]|value=[$$(MAKEFLAGS)]|flavor=$$(flavor MAKEFLAGS)|origin=$$(origin MAKEFLAGS)) a.in
	@$(info recipe|raw=[$(value MAKEFLAGS)]|value=[$(MAKEFLAGS)]|flavor=$(flavor MAKEFLAGS)|origin=$(origin MAKEFLAGS))printf 'parent-env=[%s]\n' "$$MAKEFLAGS"
	@printf done > completed
