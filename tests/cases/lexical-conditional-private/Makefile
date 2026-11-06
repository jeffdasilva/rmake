export X = global
$(foreach X,loop,$(eval private unexport X ?= $$(file >>rhs,unexpected)new)$(info inside=[$(X)]/[$(shell printf "%s" "$$X")])$(foreach N,n,$(info nested=[$(X)]/[$(shell printf "%s" "$$X")])))
$(info outside=[$(X)]/[$(shell printf "%s" "$$X")])
all:;@:
