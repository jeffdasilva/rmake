.SECONDEXPANSION:
parent:  child.foo sibling.foo
	@$(info parent=[$(MAKEFLAGS)]/[$(shell printenv MAKEFLAGS)])printf 'parent=[%s]\n' "$$(printenv MAKEFLAGS)" >> effects
child.foo:
	@$(eval MAKEFLAGS += -k)$(eval child.foo: export MAKEFLAGS ?= $$(file >>rhs,expanded)-s)$(info child=[$(MAKEFLAGS)]/[$(shell printenv MAKEFLAGS)])printf 'child=[%s]\n' "$$(printenv MAKEFLAGS)" >> effects
sibling.foo:
	@$(info sibling=[$(MAKEFLAGS)]/[$(shell printenv MAKEFLAGS)])printf 'sibling=[%s]\n' "$$(printenv MAKEFLAGS)" >> effects
after:
	@$(info after=[$(MAKEFLAGS)]/[$(shell printenv MAKEFLAGS)])printf 'after=[%s]\n' "$$(printenv MAKEFLAGS)" >> effects
