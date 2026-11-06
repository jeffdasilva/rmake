export X = global
%.t: private export X = pattern
parent: child sibling
child:
	@$(eval include edits.mk)$(info child=[$(X)]/[$(shell printf '%s' "$$X")])printf 'child=[%s]\n' "$$X" >> effects
sibling:
	@$(info sibling=[$(X)]/[$(shell printf '%s' "$$X")])printf 'sibling=[%s]\n' "$$X" >> effects
later.t:
	@$(info later=[$(X)]/[$(shell printf '%s' "$$X")])printf 'later=[%s]\n' "$$X" >> effects
after:
	@$(info after=[$(X)]/[$(shell printf '%s' "$$X")])printf 'after=[%s]\n' "$$X" >> effects
