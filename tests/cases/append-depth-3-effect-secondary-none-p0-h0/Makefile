V=base
.SECONDEXPANSION:
.PHONY: all n0 n1 n2 n3
all: n0
n0: V += $(file >>seen,e0)e0
n0: n1
n1: V += $(file >>seen,e1)e1
n1: n2
n2: V += $(file >>seen,e2)e2
n2: n3
n3: $$(info VALUE=[$$(V)])$$(info EVENTS=[$$(file <seen)]);
