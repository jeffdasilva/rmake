all:
	@printf 'child-env=[%s]\nchild-value=[%s]\n' "$$MAKEFLAGS" '$(MAKEFLAGS)'
