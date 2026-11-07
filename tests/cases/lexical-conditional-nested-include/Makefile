unexport X = global
$(foreach X,loop,$(eval include edits.mk)$(info inside=[$(X)]/[$(shell printf "%s" "$$X")]))
$(info outside=[$(X)]/[$(shell printf "%s" "$$X")])
all:;@:
