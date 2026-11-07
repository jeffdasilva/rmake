unexport X = global
$(foreach X,loop,$(eval export X = new)$(info inside=[$(X)]/[$(shell printf "%s" "$$X")]))
$(info outside=[$(X)]/[$(shell printf "%s" "$$X")])
all:;@:
