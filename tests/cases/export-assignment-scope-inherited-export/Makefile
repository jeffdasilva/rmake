unexport X = old
parent: unexport X = local
parent: child
child:
	@$(eval include edit.mk)$(info scoped=[$(X)]/[$(shell printf '%s' "$$X")])printf 'scoped=[%s]\n' "$$X" >> effects
after:
	@$(info global=[$(X)]/[$(shell printf '%s' "$$X")])printf 'global=[%s]\n' "$$X" >> effects
