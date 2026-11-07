unexport X = global
all: unexport X = target
all:
	@$(foreach X,loop,$(eval export X ?= $$(file >>rhs,unexpected)new)$(info inside=[$(X)]/[$(shell printf "%s" "$$X")]))$(info target=[$(X)]/[$(shell printf "%s" "$$X")]):
after:
	@$(info outside=[$(X)]/[$(shell printf "%s" "$$X")]):
