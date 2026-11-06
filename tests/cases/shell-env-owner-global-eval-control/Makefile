K = $(shell printf '%s' "$$CHAIN")
unexport K
export CHAIN = $(eval TMP := $$(K))$(TMP)
$(info read=[$(K)] tmp=[$(TMP)])
all:;@:
