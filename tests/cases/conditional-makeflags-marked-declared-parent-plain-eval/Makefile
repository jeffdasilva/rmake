.SECONDEXPANSION:
parent: unexport MAKEFLAGS = $(file >>reads,ancestor)-k
parent: $$(eval MAKEFLAGS += -k)$$(file >>reads,seed-complete) child.foo sibling.foo
	@$(info parent=[$(MAKEFLAGS)]):
child.foo: ; @$(eval MAKEFLAGS ?= $$(file >>rhs,expanded)-s)$(file >>reads,conditional-complete)$(info child=[$(MAKEFLAGS)]):
sibling.foo: ; @$(info sibling=[$(MAKEFLAGS)]):
after: ; @$(info after=[$(MAKEFLAGS)]):
