K = $(shell printf '%s' "$$CHAIN")
unexport K
export CHAIN = $(eval include extra.mk)$(TMP)
$(info read=[$(K)] tmp=[$(TMP)])
all:;@:
