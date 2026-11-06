export X = global
parent: export X = parent
%.t: private unexport X = pattern
parent: a.t
a.t:
	@$(info make=[$(X)] shell=[$(shell printf '%s' "$$X")])printf 'recipe=[%s]\n' "$$X" >> effect
