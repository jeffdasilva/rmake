export X = global
parent: unexport X = parent
%.t: private export X = pattern
parent: a.t
a.t:
	@$(info make=[$(X)] shell=[$(shell printf '%s' "$$X")])printf 'recipe=[%s]\n' "$$X" >> effect
