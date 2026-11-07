.SECONDEXPANSION:
child.foo: unexport MAKEFLAGS = $(file >>reads,own)-k
parent:  child.foo sibling.foo
	@$(info parent=[$(MAKEFLAGS)]):
child.foo: ; @$(eval MAKEFLAGS += -k)$(file >>reads,seed-complete)$(eval include edit.mk)$(file >>reads,conditional-complete)$(info child=[$(MAKEFLAGS)]):
sibling.foo: ; @$(info sibling=[$(MAKEFLAGS)]):
after: ; @$(info after=[$(MAKEFLAGS)]):
