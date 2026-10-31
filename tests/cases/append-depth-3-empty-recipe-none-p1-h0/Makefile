V=base
.PHONY: all n0 n1 n2 n3
all: n0
n0: V += 
n0: n1
n1: private V += 
n1: n2
n2: V += 
n2: n3
n3:;@$(info VALUE=[$(V)]):
