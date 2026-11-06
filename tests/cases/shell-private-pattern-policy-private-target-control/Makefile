export X = global
a.t: private unexport X = local
parent: a.t
a.t:
	@$(info make=[$(X)] shell=[$(shell printf '%s' "$$X")])printf 'recipe=[%s]\n' "$$X" >> effect
