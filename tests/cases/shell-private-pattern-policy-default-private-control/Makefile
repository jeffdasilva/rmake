export X = global
%.t: private X = pattern
parent: a.t
a.t:
	@$(info make=[$(X)] shell=[$(shell printf '%s' "$$X")])printf 'recipe=[%s]\n' "$$X" >> effect
