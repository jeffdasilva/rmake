.SECONDEXPANSION:
parent: unexport MAKEFLAGS = $(file >>reads,ancestor)-k
parent:  child.foo sibling.foo
	@$(info parent=[$(MAKEFLAGS)]):
child.foo: ; @$(eval include edit.mk)$(file >>reads,conditional-complete)$(info child=[$(MAKEFLAGS)]):
sibling.foo: ; @$(info sibling=[$(MAKEFLAGS)]):
after: ; @$(info after=[$(MAKEFLAGS)]):
