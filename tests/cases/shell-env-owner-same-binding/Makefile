K = $(shell printf '%s' "$$CHAIN")
unexport K
export CHAIN = $(eval owner: F := $$(K))outer
$(info read=[$(K)])
all:;@:
owner:;@$(info owner=[$(F)]):
