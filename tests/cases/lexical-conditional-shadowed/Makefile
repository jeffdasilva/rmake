$(foreach X,outer,$(foreach X,inner,$(eval export X ?= $$(file >>rhs,unexpected)new)$(info inner=[$(X)]/[$(shell printf "%s" "$$X")]))$(info outer=[$(X)]/[$(shell printf "%s" "$$X")]))
$(info outside=[$(X)]/[$(shell printf "%s" "$$X")])
all:;@:
