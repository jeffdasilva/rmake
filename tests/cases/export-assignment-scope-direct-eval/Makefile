unexport X = old
child: unexport X = local
parent: child
child:
	@$(eval export X := $(X)-new)$(info scoped=[$(X)]/[$(shell printf '%s' "$$X")])printf 'scoped=[%s]\n' "$$X" >> effects
after:
	@$(info global=[$(X)]/[$(shell printf '%s' "$$X")])printf 'global=[%s]\n' "$$X" >> effects
