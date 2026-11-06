K = global
unexport K
owner: K = distinct
all: K = $(shell printf '%s' "$$CHAIN")
all: export CHAIN = $(eval owner: F := $$(K))outer
all:;@$(info read=[$(K)]):
owner:;@$(info owner=[$(F)]):
