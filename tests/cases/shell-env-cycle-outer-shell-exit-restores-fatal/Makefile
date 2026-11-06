K = $(shell printf '%s' "$$CHAIN")
unexport K
export CHAIN = value/$(K)
$(info read=[$(K)])
A = $(B)
B = $(A)
all:;@printf '%s' '$(A)'
