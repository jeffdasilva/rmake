export X = global
$(foreach X,loop,$(eval unexport X ?= $$(file >>rhs,unexpected)new)$(info inside=[$(X)]/[$(shell printf "%s" "$$X")]))
$(info outside=[$(X)]/[$(shell printf "%s" "$$X")])
all:;@:
