export X = global
parent: export X = parent
child: export X = child
parent: child sibling
child:
	@$(eval include edits.mk)$(info child=[$(X)]/[$(shell printf '%s' "$$X")])printf 'child=[%s]\n' "$$X" >> effects
sibling:
	@$(info sibling=[$(X)]/[$(shell printf '%s' "$$X")])printf 'sibling=[%s]\n' "$$X" >> effects
after:
	@$(info after=[$(X)]/[$(shell printf '%s' "$$X")])printf 'after=[%s]\n' "$$X" >> effects
