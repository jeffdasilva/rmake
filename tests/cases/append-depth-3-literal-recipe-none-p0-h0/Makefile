V=base
.PHONY: all n0 n1 n2 n3
all: n0
n0: V += e0
n0: n1
n1: V += e1
n1: n2
n2: V += e2
n2: n3
n3:;@$(info VALUE=[$(V)]):
