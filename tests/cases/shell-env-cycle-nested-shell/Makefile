K = $(shell printf '%s' "$$CHAIN")
unexport K
export CHAIN = $(shell printf 'inner:%s' '$(K)')
$(info read=[$(K)])
all:;@:
