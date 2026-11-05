.PHONY: all alpha beta
all: alpha beta
alpha: KEY := A
beta: KEY := B
alpha beta: $(eval OBSERVED := $(KEY))$(file >>seen,name=[$(OBSERVED)])$(KEY) := $(file >>seen,rhs=[$(KEY)])value-$(KEY)
alpha:;$(info alpha=[$(A)] other=[$(B)])@:
beta:;$(info beta=[$(B)] other=[$(A)])@:
