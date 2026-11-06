.SECONDEXPANSION:
export MAKEFLAGS = -k
parent: child.foo sibling.foo
	@$(info parent=[$(origin MAKEFLAGS)]/[$(MAKEFLAGS)]/[$(shell printenv MAKEFLAGS)])printenv MAKEFLAGS >> effects || :
child.foo: $$(eval private export MAKEFLAGS ?= $$$$(file >>rhs,unexpected)-k)
	@$(info child=[$(origin MAKEFLAGS)]/[$(MAKEFLAGS)]/[$(shell printenv MAKEFLAGS)])printenv MAKEFLAGS >> effects || :
sibling.foo:
	@$(info sibling=[$(origin MAKEFLAGS)]/[$(MAKEFLAGS)]/[$(shell printenv MAKEFLAGS)])printenv MAKEFLAGS >> effects || :
after:
	@$(info after=[$(origin MAKEFLAGS)]/[$(MAKEFLAGS)]/[$(shell printenv MAKEFLAGS)])printenv MAKEFLAGS >> effects || :
