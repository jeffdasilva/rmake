export X = global
parent: export X = parent
parent: child
child:
	@$(info child=[$(X)]/[$(shell printf '%s' "$$X")])printf 'child=[%s]\n' "$$X" >> effects
driver:
	@$(eval include edits.mk)$(eval parent: CAP := $$(info retained=[$$(X)]/[$$(shell printf '%s' "$$$$X")]))$(info driver=[$(X)]/[$(shell printf '%s' "$$X")])printf 'driver=[%s]\n' "$$X" >> effects
