export X = global
parent: export X = parent
%.t: private export X = pattern
parent: a.t
a.t:
	@$(eval include change.mk)$(info make=[$(X)] shell=[$(shell printf '%s' "$$X")])printf 'recipe=[%s]\n' "$$X" >> effect
